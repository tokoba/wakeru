//! Tantivy インデックス管理モジュール
//!
//! インデックスの作成・管理・ドキュメント追加を担当します。
//! 多言語対応のため、Language 引数と言語別トークナイザー登録をサポートします。

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use tantivy::schema::{FieldType, OwnedValue};
use tantivy::tokenizer::{LowerCaser, NgramTokenizer, SimpleTokenizer, Stemmer, TextAnalyzer};
use tantivy::{Index, IndexReader, IndexWriter, Term};

use crate::config::Language;
use crate::errors::IndexerError;
use crate::indexer::report::AddDocumentsReport;
use crate::indexer::schema_builder::{SchemaFields, build_schema};
use crate::models::Document;

/// インデックスの存在判定に使用するメタファイル名
const META_JSON: &str = "meta.json";

// ─────────────────────────────────────────────────────────────────────────────
// JSON 変換ヘルパー関数
// ─────────────────────────────────────────────────────────────────────────────

/// serde_json::Value → OwnedValue の再帰的変換
fn serde_json_to_owned(v: &serde_json::Value) -> OwnedValue {
  use serde_json::Value as J;
  use tantivy::schema::OwnedValue as O;

  match v {
    J::Null => O::Null,
    J::Bool(b) => O::Bool(*b),
    J::Number(n) => {
      if let Some(i) = n.as_i64() {
        O::I64(i)
      } else if let Some(u) = n.as_u64() {
        O::U64(u)
      } else if let Some(f) = n.as_f64() {
        O::F64(f)
      } else {
        O::Null
      }
    }
    J::String(s) => O::Str(s.clone()),
    J::Array(arr) => {
      let vals = arr.iter().map(serde_json_to_owned).collect();
      O::Array(vals)
    }
    J::Object(map) => {
      // OwnedValue::Object は Vec<(String, OwnedValue)> を期待する
      let obj: Vec<(String, OwnedValue)> =
        map.iter().map(|(k, v)| (k.clone(), serde_json_to_owned(v))).collect();
      O::Object(obj)
    }
  }
}

/// Metadata(HashMap) → Tantivy JsonObject(Vec) への変換
///
/// Tantivy 0.25: add_object は BTreeMap<String, OwnedValue> を期待する
fn metadata_to_tantivy_object(metadata: &crate::models::Metadata) -> BTreeMap<String, OwnedValue> {
  metadata.iter().map(|(k, v)| (k.clone(), serde_json_to_owned(v))).collect()
}

/// Tantivy インデックスの作成・管理を行う構造体。
///
/// # 責務
///
/// - インデックスディレクトリの作成
/// - スキーマ定義とトークナイザー登録
/// - ドキュメントの追加（重複時はスキップ）
/// - IndexWriter のコミット管理
///
/// # 多言語対応
///
/// - 日本語 (`Language::Ja`): VibratoTokenizer + N-gram トークナイザー
/// - 英語 (`Language::En`): SimpleTokenizer + LowerCaser
pub struct IndexManager {
  /// Tantivy Index ハンドル
  index: Index,

  /// IndexReader（検索用）
  reader: IndexReader,

  /// スキーマフィールド参照
  fields: SchemaFields,

  /// このインデックスの言語
  language: Language,
}

impl std::fmt::Debug for IndexManager {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("IndexManager")
      .field("language", &self.language)
      .field("fields", &self.fields)
      .finish_non_exhaustive()
  }
}

impl IndexManager {
  /// インデックスを開く。存在しなければ新規作成する。
  ///
  /// # 引数
  /// - `index_path`: インデックス保存先ディレクトリ
  /// - `language`: インデックスの言語
  /// - `tokenizer_ja`: 日本語トークナイザー（日本語インデックスの場合は必須）
  ///
  /// # エラー
  /// - ディレクトリ作成失敗
  /// - Tantivy のインデックス作成/オープンエラー
  /// - 日本語インデックスでトークナイザーが提供されていない
  /// - 既存インデックスと言語の不一致
  ///
  /// # 設計上の注意点
  ///
  /// - **新規作成時**: `build_schema(language)` でスキーマを構築
  /// - **既存インデックスを開く時**: `SchemaFields::from_schema(&schema)` で再構築
  /// - **疎結合**: `tokenizer_ja` は `Option<TextAnalyzer>` で、VibratoTokenizer に依存しない
  pub fn open_or_create<P: AsRef<Path>>(
    index_path: P,
    language: Language,
    tokenizer_ja: Option<TextAnalyzer>,
  ) -> Result<Self, IndexerError> {
    let index_path = index_path.as_ref();

    // meta.json の存在でインデックスの有無を判定
    let meta_json_exists = index_path.join(META_JSON).exists();

    let (index, fields) = if meta_json_exists {
      // 既存インデックスを開く
      let index = Index::open_in_dir(index_path)?;
      let schema = index.schema();

      // 既存スキーマから SchemaFields を再構築
      let fields = SchemaFields::from_schema(&schema)?;

      // スキーマと言語の整合性チェック
      Self::assert_schema_matches_language(&schema, language)?;

      (index, fields)
    } else {
      // ディレクトリを作成（存在しない場合）
      if !index_path.exists() {
        std::fs::create_dir_all(index_path).map_err(|e| IndexerError::InvalidIndexPath {
          path: index_path.to_path_buf(),
          source: Arc::new(e),
        })?;
      }
      // 新規作成時のみ build_schema を使用
      let (schema, fields) = build_schema(language);
      let index = Index::create_in_dir(index_path, schema)?;
      (index, fields)
    };

    // 言語に応じたトークナイザーを登録
    match language {
      Language::Ja => {
        // 日本語トークナイザーは必須
        let tokenizer = tokenizer_ja.ok_or(IndexerError::MissingJapaneseTokenizer)?;
        index.tokenizers().register(language.text_tokenizer_name(), tokenizer);

        // 1文字N-gramトークナイザーを登録（部分一致検索用）
        // Tantivy 0.25.0: NgramTokenizer::new() は Result を返す
        let ja_ngram_tokenizer = NgramTokenizer::new(1, 1, false)?;
        let ja_ngram = TextAnalyzer::builder(ja_ngram_tokenizer).build();
        index.tokenizers().register("ja_ngram", ja_ngram);
      }
      Language::En => {
        // 英語: SimpleTokenizer + LowerCaser
        // Tantivy 0.25.0: ビルダーパターンを使用
        let en_analyzer = TextAnalyzer::builder(SimpleTokenizer::default())
          .filter(LowerCaser)
          .filter(Stemmer::new(tantivy::tokenizer::Language::English))
          .build();
        index.tokenizers().register(language.text_tokenizer_name(), en_analyzer);
      }
    }

    // Reader を作成
    let reader = index.reader()?;

    Ok(Self {
      index,
      reader,
      fields,
      language,
    })
  }

  /// スキーマと言語の整合性をチェックする。
  ///
  /// 既存インデックスの text フィールドのトークナイザー名が、
  /// 指定された言語に期待されるトークナイザー名と一致するか検証する。
  fn assert_schema_matches_language(
    schema: &tantivy::schema::Schema,
    language: Language,
  ) -> Result<(), IndexerError> {
    let text_field = schema
      .get_field("text")
      .map_err(|e| tantivy::TantivyError::InvalidArgument(e.to_string()))?;

    let field_entry = schema.get_field_entry(text_field);

    // Tantivy 0.25.0: FieldType をパターンマッチして TextOptions を取得
    let text_options = match field_entry.field_type() {
      FieldType::Str(options) => options,
      _ => {
        return Err(IndexerError::Tantivy(
          tantivy::TantivyError::InvalidArgument("text field is not a text field".to_string()),
        ));
      }
    };

    // インデックス設定からトークナイザー名を取得
    let indexing_options = text_options.get_indexing_options().ok_or_else(|| {
      IndexerError::Tantivy(tantivy::TantivyError::InvalidArgument(
        "text field is not indexed".to_string(),
      ))
    })?;

    let actual_tokenizer = indexing_options.tokenizer();
    let expected_tokenizer = language.text_tokenizer_name();

    if actual_tokenizer != expected_tokenizer {
      return Err(IndexerError::LanguageSchemaMismatch {
        expected: expected_tokenizer.to_string(),
        actual: actual_tokenizer.to_string(),
      });
    }

    Ok(())
  }

  /// ドキュメントをインデックスに追加する。
  ///
  /// - 重複ドキュメント（同じID）はスキップする
  /// - 処理は最後まで続く（fail-fast しない）
  /// - 結果は `AddDocumentsReport` で返す
  ///
  /// # 引数
  /// - `documents`: 追加するドキュメントのスライス
  ///
  /// # 戻り値
  /// - `Ok(AddDocumentsReport)`: 処理統計（成功/スキップ件数）
  /// - `Err(IndexerError)`: Tantivy レベルの致命的エラー
  pub fn add_documents(&self, documents: &[Document]) -> Result<AddDocumentsReport, IndexerError> {
    let mut report = AddDocumentsReport::default();
    let mut seen_ids: HashSet<String> = HashSet::with_capacity(documents.len());

    // IndexWriter を作成（50MB バッファ）
    let mut writer: IndexWriter = self.index.writer(50_000_000)?;

    // 検索用 Searcher
    let searcher = self.reader.searcher();

    for doc in documents {
      report.record_total();
      let id = doc.id.clone();

      // バッチ内重複
      let in_batch = !seen_ids.insert(id.clone());

      // インデックス内重複（doc_freq で高速チェック）
      let term = Term::from_field_text(self.fields.id, &id);
      let in_index = searcher.doc_freq(&term)? > 0;

      if in_batch || in_index {
        // 重複はスキップ
        report.record_skipped();
        continue;
      }

      // 重複なし → 追加
      let tantivy_doc = self.to_tantivy_document(doc)?;
      writer.add_document(tantivy_doc)?;
      report.record_added();
    }

    // コミット: ディスクに永続化
    writer.commit()?;

    // Reader をリロード（以降の検索で新ドキュメントを見えるようにする）
    self.reader.reload()?;

    Ok(report)
  }

  /// Document → TantivyDocument 変換（内部メソッド）
  ///
  /// # 戻り値
  /// - `Ok(TantivyDocument)`: 変換成功
  fn to_tantivy_document(&self, doc: &Document) -> Result<tantivy::TantivyDocument, IndexerError> {
    let mut tantivy_doc = tantivy::TantivyDocument::default();

    tantivy_doc.add_text(self.fields.id, &doc.id);
    tantivy_doc.add_text(self.fields.source_id, &doc.source_id);
    tantivy_doc.add_text(self.fields.text, &doc.text);

    // N-gramフィールドにも同じ本文を追加（部分一致検索用）
    // 日本語インデックスのみ（英語の場合は text_ngram が None）
    if let Some(text_ngram_field) = self.fields.text_ngram {
      tantivy_doc.add_text(text_ngram_field, &doc.text);
    }

    // メタデータ全体を JsonObject として投入
    // tags も metadata["tags"] に含まれるため、二重保持不要
    // Tantivy 0.25: add_object は BTreeMap<String, OwnedValue> を期待するため変換が必要
    if !doc.metadata.is_empty() {
      let json_obj = metadata_to_tantivy_object(&doc.metadata);
      tantivy_doc.add_object(self.fields.metadata, json_obj);
    }

    Ok(tantivy_doc)
  }

  /// Tantivy Index への参照を返す（SearchEngine で使用）
  pub fn index(&self) -> &Index {
    &self.index
  }

  /// IndexReader への参照を返す
  pub fn reader(&self) -> &IndexReader {
    &self.reader
  }

  /// SchemaFields への参照を返す
  pub fn fields(&self) -> &SchemaFields {
    &self.fields
  }

  /// このインデックスの言語を返す
  pub fn language(&self) -> Language {
    self.language
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tantivy::tokenizer::TextAnalyzer;
  use vibrato_rkyv::dictionary::PresetDictionaryKind;

  /// 日本語インデックスの作成とドキュメント追加が正常に動作することを確認。
  #[test]
  fn open_or_create_japanese_and_add_documents() {
    // 辞書マネージャーからトークナイザーを構築
    let manager = crate::dictionary::DictionaryManager::with_preset(PresetDictionaryKind::Ipadic)
      .expect("DictionaryManager 構築失敗");

    let cache_dir = manager.cache_dir();
    if !cache_dir.join(PresetDictionaryKind::Ipadic.name()).exists() {
      eprintln!("辞書キャッシュなし → スキップ");
      return;
    }

    let dict = manager.load().expect("辞書ロード失敗");
    let tokenizer =
      crate::tokenizer::vibrato_tokenizer::VibratoTokenizer::from_shared_dictionary(dict);
    let text_analyzer = TextAnalyzer::from(tokenizer);

    // 一時ディレクトリにインデックスを作成
    let tmp_dir = tempfile::TempDir::new().expect("一時ディレクトリ作成失敗");
    let index_manager =
      IndexManager::open_or_create(tmp_dir.path(), Language::Ja, Some(text_analyzer))
        .expect("インデックス作成失敗");

    // 日本語であることを確認
    assert_eq!(index_manager.language(), Language::Ja);

    // text_ngram フィールドが存在することを確認
    assert!(index_manager.fields().text_ngram.is_some());

    // ドキュメント追加
    let docs = vec![
      Document::new("1", "src-1", "東京は日本の首都です").with_tag("category:geo"),
      Document::new("2", "src-1", "大阪は西日本の中心都市です")
        .with_tag("category:geo")
        .with_tag("region:kansai"),
    ];

    let report = index_manager.add_documents(&docs).expect("ドキュメント追加失敗");
    assert_eq!(report.added, 2);
    assert_eq!(report.skipped_duplicates, 0);
  }

  /// 英語インデックスの作成とドキュメント追加が正常に動作することを確認。
  #[test]
  fn open_or_create_english_and_add_documents() {
    // 一時ディレクトリにインデックスを作成
    let tmp_dir = tempfile::TempDir::new().expect("一時ディレクトリ作成失敗");
    let index_manager = IndexManager::open_or_create(tmp_dir.path(), Language::En, None)
      .expect("インデックス作成失敗");

    // 英語であることを確認
    assert_eq!(index_manager.language(), Language::En);

    // text_ngram フィールドが存在しないことを確認
    assert!(index_manager.fields().text_ngram.is_none());

    // ドキュメント追加
    let docs = vec![
      Document::new("1", "src-1", "Tokyo is the capital of Japan").with_tag("category:geo"),
      Document::new("2", "src-1", "Osaka is a major city in western Japan")
        .with_tag("category:geo")
        .with_tag("region:kansai"),
    ];

    let report = index_manager.add_documents(&docs).expect("ドキュメント追加失敗");
    assert_eq!(report.added, 2);
    assert_eq!(report.skipped_duplicates, 0);
  }

  /// 日本語インデックスでトークナイザーが提供されていない場合のエラーテスト
  #[test]
  fn missing_japanese_tokenizer_error() {
    let tmp_dir = tempfile::TempDir::new().expect("一時ディレクトリ作成失敗");
    let result = IndexManager::open_or_create(tmp_dir.path(), Language::Ja, None);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, IndexerError::MissingJapaneseTokenizer));
  }

  /// 重複スキップのテスト（日本語）
  #[test]
  fn duplicate_documents_are_skipped_japanese() {
    let manager = crate::dictionary::DictionaryManager::with_preset(PresetDictionaryKind::Ipadic)
      .expect("DictionaryManager 構築失敗");

    let cache_dir = manager.cache_dir();
    if !cache_dir.join(PresetDictionaryKind::Ipadic.name()).exists() {
      eprintln!("辞書キャッシュなし → スキップ");
      return;
    }

    let dict = manager.load().expect("辞書ロード失敗");
    let tokenizer =
      crate::tokenizer::vibrato_tokenizer::VibratoTokenizer::from_shared_dictionary(dict);
    let text_analyzer = TextAnalyzer::from(tokenizer);

    let tmp_dir = tempfile::TempDir::new().expect("一時ディレクトリ作成失敗");
    let index_manager =
      IndexManager::open_or_create(tmp_dir.path(), Language::Ja, Some(text_analyzer))
        .expect("インデックス作成失敗");

    // 最初のドキュメント
    let docs1 = vec![Document::new("1", "src-1", "東京は日本の首都です")];
    let report1 = index_manager.add_documents(&docs1).expect("追加失敗");
    assert_eq!(report1.added, 1);
    assert_eq!(report1.skipped_duplicates, 0);

    // 同じ ID のドキュメントを追加 → スキップされる
    let docs2 = vec![Document::new("1", "src-1", "大阪は西日本の中心都市です")];
    let report2 = index_manager.add_documents(&docs2).expect("追加失敗");
    assert_eq!(report2.added, 0);
    assert_eq!(report2.skipped_duplicates, 1);
  }

  /// 重複スキップのテスト（英語）
  #[test]
  fn duplicate_documents_are_skipped_english() {
    let tmp_dir = tempfile::TempDir::new().expect("一時ディレクトリ作成失敗");
    let index_manager = IndexManager::open_or_create(tmp_dir.path(), Language::En, None)
      .expect("インデックス作成失敗");

    // 最初のドキュメント
    let docs1 = vec![Document::new("1", "src-1", "Tokyo is the capital of Japan")];
    let report1 = index_manager.add_documents(&docs1).expect("追加失敗");
    assert_eq!(report1.added, 1);
    assert_eq!(report1.skipped_duplicates, 0);

    // 同じ ID のドキュメントを追加 → スキップされる
    let docs2 = vec![Document::new("1", "src-1", "Osaka is a major city")];
    let report2 = index_manager.add_documents(&docs2).expect("追加失敗");
    assert_eq!(report2.added, 0);
    assert_eq!(report2.skipped_duplicates, 1);
  }
}
