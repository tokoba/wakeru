//! Tantivy スキーマビルダー
//!
//! RAG パイプライン向けの Tantivy インデックススキーマを定義します。
//! 言語ごとに適切なトークナイザーを自動選択します。

use tantivy::schema::{
  Field, IndexRecordOption, JsonObjectOptions, STORED, STRING, Schema, TextFieldIndexing,
  TextOptions,
};

use crate::config::Language;

/// スキーマフィールドへの参照をまとめて保持する構造体。
///
/// Tantivy の `Schema::get_field()` は文字列ベースの検索であるため、
/// フィールド名を typo するリスクがあります。この構造体は型安全な
/// フィールド参照を提供します。
#[derive(Clone, Copy, Debug)]
pub struct SchemaFields {
  /// チャンク ID (STRING + STORED) - 完全一致用
  pub id: Field,
  /// 元ドキュメント ID (STRING + STORED)
  pub source_id: Field,
  /// 本文フィールド (TEXT + STORED, 言語別トークナイザー)
  pub text: Field,
  /// 構造化メタデータ (JsonObject, STORED + INDEXED, raw tokenizer)
  /// タグ等のフィルタリング検索が可能
  pub metadata: Field,
  /// 1文字N-gram用フィールド (TEXT, ja_ngram トークナイザー)
  /// 1文字クエリでの部分一致検索用
  /// 日本語のみで使用、英語では None
  /// 既存インデックスでは存在しない可能性があるため Option
  pub text_ngram: Option<Field>,
}

impl SchemaFields {
  /// 既存スキーマから SchemaFields を再構築する。
  ///
  /// 既存インデックスを開く際に使用する。ディスク上のスキーマから
  /// フィールドを取得し、SchemaFields 構造体を構築する。
  ///
  /// # 引数
  /// - `schema`: Tantivy スキーマ
  ///
  /// # 戻り値
  /// - `Ok(SchemaFields)`: フィールド取得成功
  /// - `Err(tantivy::TantivyError)`: 必須フィールドが見つからない
  ///
  /// # エラー条件
  /// - `id`, `source_id`, `text`, `metadata` のいずれかが見つからない
  pub fn from_schema(schema: &Schema) -> Result<Self, tantivy::TantivyError> {
    let id = schema.get_field("id").map_err(|e| {
      tantivy::TantivyError::InvalidArgument(format!("フィールド 'id' が見つかりません: {e}"))
    })?;
    let source_id = schema.get_field("source_id").map_err(|e| {
      tantivy::TantivyError::InvalidArgument(format!(
        "フィールド 'source_id' が見つかりません: {e}"
      ))
    })?;
    let text = schema.get_field("text").map_err(|e| {
      tantivy::TantivyError::InvalidArgument(format!("フィールド 'text' が見つかりません: {e}"))
    })?;
    let metadata = schema.get_field("metadata").map_err(|e| {
      tantivy::TantivyError::InvalidArgument(format!("フィールド 'metadata' が見つかりません: {e}"))
    })?;

    // N-gram用フィールドは日本語インデックスのみ、または古いインデックスに存在しない可能性
    let text_ngram = schema.get_field("text_ngram").ok();

    Ok(Self {
      id,
      source_id,
      text,
      metadata,
      text_ngram,
    })
  }
}

/// Tantivy スキーマを構築する。
///
/// # フィールド構成
///
/// - `id`: チャンク ID (STRING + STORED) 完全一致用
/// - `source_id`: 元文書 ID (STRING + STORED)
/// - `text`: 本文 (TEXT + STORED, 言語別トークナイザー)
/// - `metadata`: 構造化メタデータ (JsonObject, STORED + INDEXED, raw tokenizer)
/// - `text_ngram`: 1文字N-gram用 (TEXT, ja_ngram トークナイザー) - 日本語のみ
///
/// # トークナイザー設定（言語依存）
///
/// - 日本語 (`Language::Ja`):
///   - `text` フィールドには `lang_ja` トークナイザー
///   - `text_ngram` フィールドには `ja_ngram` トークナイザー
/// - 英語 (`Language::En`):
///   - `text` フィールドには `lang_en` トークナイザー（SimpleTokenizer + LowerCaser）
///   - `text_ngram` フィールドは作成されない
///
/// トークナイザーは `IndexManager` 作成時に登録される必要があります。
///
/// # IndexRecordOption の選択理由
///
/// `WithFreqsAndPositions` を選択しています：
/// - BM25 スコア計算には出現頻度 (Freqs) が必要
/// - フレーズ検索には位置情報 (Positions) が必要
/// - ハイライト表示にも position 情報が活用される
///
/// # metadata フィールドの設計
///
/// `metadata` は JsonObject 型で、以下の特徴を持ちます：
/// - STORED: 検索結果で復元可能
/// - INDEXED (raw tokenizer): `metadata.tags:value` 形式でフィルタリング検索が可能
/// - raw トークナイザーはトークナイズを行わないため、完全一致検索に適する
///
/// # 例
///
/// ```no_run
/// use wakeru::indexer::schema_builder::build_schema;
/// use wakeru::Language;
///
/// let (schema, fields) = build_schema(Language::Ja);
/// // schema を Index::create_in_dir に渡す
/// // fields を IndexManager や SearchEngine で使用
/// ```
pub fn build_schema(language: Language) -> (Schema, SchemaFields) {
  let mut builder = Schema::builder();

  // ID フィールド: 完全一致検索 + 格納
  let id = builder.add_text_field("id", STRING | STORED);

  // 元ドキュメント ID
  let source_id = builder.add_text_field("source_id", STRING | STORED);

  // 本文フィールド: 言語別トークナイザー + 頻度・位置を記録
  let text_indexing = TextFieldIndexing::default()
    .set_tokenizer(language.text_tokenizer_name())
    .set_index_option(IndexRecordOption::WithFreqsAndPositions);
  let text_options = TextOptions::default().set_indexing_options(text_indexing).set_stored();
  let text = builder.add_text_field("text", text_options);

  // メタデータフィールド: JsonObject（フィルタリング検索可能）
  // raw トークナイザーで完全一致検索を可能にする
  // Tantivy 0.25: JsonObjectOptions::set_indexing_options は TextFieldIndexing を受け取る
  let json_indexing =
    TextFieldIndexing::default().set_tokenizer("raw").set_index_option(IndexRecordOption::Basic);
  let metadata_options =
    JsonObjectOptions::default().set_stored().set_indexing_options(json_indexing);
  let metadata = builder.add_json_field("metadata", metadata_options);

  // 1文字N-gram用フィールド: 日本語のみ作成
  // 英語の場合は None
  let text_ngram = language.ngram_tokenizer_name().map(|tokenizer_name| {
    let text_ngram_indexing = TextFieldIndexing::default()
      .set_tokenizer(tokenizer_name)
      .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let text_ngram_options = TextOptions::default().set_indexing_options(text_ngram_indexing);
    builder.add_text_field("text_ngram", text_ngram_options)
  });

  let schema = builder.build();

  (
    schema,
    SchemaFields {
      id,
      source_id,
      text,
      metadata,
      text_ngram,
    },
  )
}
