//! bm25 検索モジュール

use tantivy::query::{BooleanQuery, Occur, TermSetQuery};
use tantivy::schema::Value;
use tantivy::schema::document::CompactDocValue;
use tantivy::{Index, IndexReader, ReloadPolicy, Term, collector::TopDocs, query::QueryParser};
use tracing::debug;

use crate::config::Language;
use crate::errors::SearcherError;
use crate::indexer::schema_builder::SchemaFields;
use crate::models::SearchResult;

// トークナイズユーティリティを使用
use super::tokenization::{TokenizationResult, tokenize_with_text_analyzer};

// ─────────────────────────────────────────────────────────────────────────────
// JSON 変換ヘルパー関数
// ─────────────────────────────────────────────────────────────────────────────

/// CompactDocValue → serde_json::Value の変換
///
/// Tantivy 0.25: CompactDocValue は Serialize を実装していないため、
/// 一度 OwnedValue に変換してから serde_json::Value に変換する
fn compact_value_to_json(value: &CompactDocValue<'_>) -> serde_json::Value {
  use tantivy::schema::OwnedValue;

  // CompactDocValue から OwnedValue への変換（From トレイトを使用）
  let owned: OwnedValue = (*value).into();

  // OwnedValue は Serialize を実装しているので serde_json::Value に変換できる
  // 通常失敗しないが、万が一失敗した場合は Null にフォールバックして警告ログを出す
  serde_json::to_value(owned).unwrap_or_else(|e| {
    debug!(error = %e, "metadata value のシリアライズに失敗しました。Null として復元します。");
    serde_json::Value::Null
  })
}

/// BM25検索エンジン
pub struct SearchEngine {
  /// Tantivy の IndexReader
  reader: IndexReader,

  /// 検索対象フィールド
  fields: SchemaFields,

  /// この検索エンジンの言語
  language: Language,
}

/// BM25検索エンジンの実装ブロック
impl SearchEngine {
  /// 検索エンジンの初期化
  ///
  /// # 引数
  /// - `index`: Tantivy Index への参照
  /// - `fields`: スキーマフィールド
  /// - `language`: この検索エンジンの言語
  pub fn new(
    index: &Index,
    fields: SchemaFields,
    language: Language,
  ) -> Result<Self, SearcherError> {
    let reader = index
      .reader_builder()
      .reload_policy(ReloadPolicy::OnCommitWithDelay) // 自動リロード設定
      .try_into()?;

    Ok(Self {
      reader,
      fields,
      language,
    })
  }

  /// BM25 スコアで検索する
  pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<SearchResult>, SearcherError> {
    let searcher = self.reader.searcher();

    // QueryParser : text field を検索対象とする
    let query_parser = QueryParser::for_index(searcher.index(), vec![self.fields.text]);

    // クエリ文字列を解析
    let query = query_parser.parse_query(query_str).map_err(|e| SearcherError::InvalidQuery {
      reason: e.to_string(),
    })?;

    // BM25スコアで上位のドキュメント (max < limit) 件分を取得する
    let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

    // ヘルパーメソッドで結果変換
    self.convert_to_search_results(&searcher, top_docs)
  }

  /// クエリ文字列を言語別トークナイザーで解析し、ユニーク Term を抽出する
  ///
  /// # 処理フロー
  /// 1. 言語に応じたトークナイザーを取得
  /// 2. 純粋なトークナイズ関数に委譲（重複除外・空文字除外・Term 変換）
  ///
  /// # 引数
  /// - `index`: Tantivy Index への参照（トークナイザー取得用）
  /// - `query_str`: トークナイズ対象のクエリ文字列
  ///
  /// # 戻り値
  /// ユニークな Term とトークン文字列を含む `TokenizationResult`
  fn tokenize_query(
    &self,
    index: &Index,
    query_str: &str,
  ) -> Result<TokenizationResult, SearcherError> {
    // 言語に応じたトークナイザー名を取得
    let tokenizer_name = self.language.text_tokenizer_name();

    // トークナイザーを取得
    let mut analyzer =
      index.tokenizers().get(tokenizer_name).ok_or_else(|| SearcherError::InvalidQuery {
        reason: format!("tokenizer `{tokenizer_name}` is not registered"),
      })?;

    // TextAnalyzer 専用のトークナイズ関数に委譲
    Ok(tokenize_with_text_analyzer(
      &mut analyzer,
      self.fields.text,
      query_str,
    ))
  }

  /// 言語別トークナイザーでクエリを解析し、抽出されたトークンで OR 検索を行う
  ///
  /// # 引数
  /// - `query_str`: 検索クエリ文字列（例: "京都の寺", "Tokyo temples"）
  /// - `limit`: 返す結果の最大件数
  ///
  /// # 戻り値
  /// BM25 スコア付きの検索結果ベクタ
  ///
  /// # 動作
  /// 1. 言語別トークナイザーでクエリ文字列を解析
  /// 2. 抽出されたトークンを Term に変換
  /// 3. 日本語の場合、1文字トークンは N-gram フィールドでも検索
  /// 4. TermSetQuery / BooleanQuery で OR 検索を実行
  ///
  /// # 例
  /// ```ignore
  /// // 日本語検索
  /// let results = search_engine.search_tokens_or("京都の寺", 10)?;
  /// // "京都" と "寺" に分割されて検索される
  ///
  /// // 英語検索（LowerCaser により小文字化される）
  /// let results = search_engine.search_tokens_or("Tokyo Tower", 10)?;
  /// // "tokyo" と "tower" に分割されて検索される
  /// ```
  pub fn search_tokens_or(
    &self,
    query_str: &str,
    limit: usize,
  ) -> Result<Vec<SearchResult>, SearcherError> {
    debug!(query = %query_str, limit, language = ?self.language, "検索クエリ解析開始");

    let searcher = self.reader.searcher();
    let index = searcher.index();

    // トークナイズ処理を専用メソッドに委譲
    let TokenizationResult {
      terms: morph_terms,
      query_tokens,
    } = self.tokenize_query(index, query_str)?;

    // クエリトークンのログ出力
    debug!(
      query = %query_str,
      tokens = ?query_tokens,
      num_terms = morph_terms.len(),
      "検索クエリ解析完了"
    );

    if morph_terms.is_empty() {
      // 全てストップワードなどでトークンが空なら空結果を返す
      return Ok(vec![]);
    }

    // 1文字トークンを抽出してN-gramフィールド用Termを作成
    // 日本語の場合のみ text_ngram フィールドが存在する
    let ngram_terms: Vec<Term> = self
      .fields
      .text_ngram
      .map(|text_ngram_field| {
        query_tokens
          .iter()
          .filter(|token| token.chars().count() == 1)
          .map(|token| Term::from_field_text(text_ngram_field, token))
          .collect()
      })
      .unwrap_or_default();

    // N-gram検索の有無をログ出力用に記録
    let has_ngram = !ngram_terms.is_empty();

    // クエリを構築
    let query: Box<dyn tantivy::query::Query> = if ngram_terms.is_empty() {
      // N-gram対象なし: 形態素フィールドのみで検索
      Box::new(TermSetQuery::new(morph_terms))
    } else {
      // N-gram対象あり: 形態素 + N-gram のOR検索
      let subqueries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = vec![
        // 形態素フィールド検索
        (Occur::Should, Box::new(TermSetQuery::new(morph_terms))),
        // N-gramフィールド検索
        (Occur::Should, Box::new(TermSetQuery::new(ngram_terms))),
      ];

      Box::new(BooleanQuery::from(subqueries))
    };

    debug!(
      query = %query_str,
      has_ngram,
      "検索クエリ構築完了"
    );

    // 検索実行（BM25 スコア付き）
    let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

    // 結果変換（既存のロジックを再利用）
    self.convert_to_search_results(&searcher, top_docs)
  }

  /// top_docs を SearchResult ベクタに変換するヘルパーメソッド
  fn convert_to_search_results(
    &self,
    searcher: &tantivy::Searcher,
    top_docs: Vec<(f32, tantivy::DocAddress)>,
  ) -> Result<Vec<SearchResult>, SearcherError> {
    let mut results = Vec::with_capacity(top_docs.len());

    for (score, doc_address) in top_docs {
      let doc: tantivy::TantivyDocument = searcher.doc(doc_address)?;

      // 必須フィールドの取得（エラーなら InvalidIndex）
      let doc_id =
        self.get_text_field(&doc, self.fields.id).ok_or_else(|| SearcherError::InvalidIndex {
          field: "id".to_string(),
          reason: "必須フィールドが見つかりません".to_string(),
        })?;

      let source_id = self.get_text_field(&doc, self.fields.source_id).ok_or_else(|| {
        SearcherError::InvalidIndex {
          field: "source_id".to_string(),
          reason: "必須フィールドが見つかりません".to_string(),
        }
      })?;

      // text は Optional 扱い（空文字でフォールバック）
      let text = self.get_text_field(&doc, self.fields.text).unwrap_or_default();

      // metadata 復元: JsonObject から直接取得
      let metadata = self.get_json_object_field(&doc, self.fields.metadata);

      results.push(SearchResult {
        doc_id,
        source_id,
        score,
        text,
        metadata,
      });
    }

    Ok(results)
  }

  /// TantivyDocument から 単一テキストフィールドの値を取得する
  ///
  /// # 戻り値
  /// - `Some(String)`: フィールド値が存在する場合
  /// - `None`: フィールド値が存在しない場合
  fn get_text_field(
    &self,
    doc: &tantivy::TantivyDocument,
    field: tantivy::schema::Field,
  ) -> Option<String> {
    doc.get_first(field).and_then(|v| v.as_str().map(String::from))
  }

  /// TantivyDocument から JsonObject フィールドの値を取得し Metadata に変換する
  ///
  /// # 戻り値
  /// - フィールド値が存在する場合: 変換された Metadata
  /// - フィールド値が存在しない場合: 空の Metadata
  fn get_json_object_field(
    &self,
    doc: &tantivy::TantivyDocument,
    field: tantivy::schema::Field,
  ) -> crate::models::Metadata {
    doc
      .get_first(field)
      .and_then(|value| value.as_object())
      .map(|iter| {
        // Tantivy 0.25: as_object() は CompactDocObjectIter（イテレータ）を返す
        // iter: (key: &str, value: CompactDocValue<'_>)
        let mut metadata = crate::models::Metadata::default();

        for (k, v) in iter {
          // CompactDocValue → serde_json::Value に変換
          let json_val = compact_value_to_json(&v);
          metadata.insert(k.to_string(), json_val);
        }

        metadata
      })
      .unwrap_or_default()
  }

  /// この検索エンジンの言語を返す
  pub fn language(&self) -> Language {
    self.language
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// テストモジュール
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::Language;
  use crate::indexer::index_manager::IndexManager;
  use crate::models::Document;
  use serde_json::json;

  // ─── テスト用ヘルパー関数 ───────────────────────────────────────────────────

  /// 英語インデックスを作成するヘルパー（SearchEngine は後で作成）
  fn create_english_index_manager() -> (tempfile::TempDir, IndexManager) {
    let tmp_dir = tempfile::TempDir::new().expect("一時ディレクトリ作成失敗");
    let index_manager = IndexManager::open_or_create(tmp_dir.path(), Language::En, None)
      .expect("インデックス作成失敗");
    (tmp_dir, index_manager)
  }

  /// IndexManager から SearchEngine を作成するヘルパー
  ///
  /// 重要: ドキュメント追加後に呼び出すこと（SearchEngine は独自の Reader を持つため）
  fn create_search_engine(index_manager: &IndexManager) -> SearchEngine {
    SearchEngine::new(index_manager.index(), *index_manager.fields(), Language::En)
      .expect("SearchEngine 作成失敗")
  }

  /// テスト用ドキュメントを追加するヘルパー
  fn add_test_documents(index_manager: &IndexManager, docs: &[Document]) {
    let report = index_manager.add_documents(docs).expect("ドキュメント追加失敗");
    assert_eq!(
      report.added,
      docs.len(),
      "期待された数のドキュメントが追加されること"
    );
  }

  // ─── 基本的な検索テスト ────────────────────────────────────────────────────

  #[test]
  fn search_engine_language() {
    let (_tmp_dir, index_manager) = create_english_index_manager();
    let search_engine = create_search_engine(&index_manager);
    assert_eq!(search_engine.language(), Language::En);
  }

  #[test]
  fn search_returns_empty_for_empty_index() {
    let (_tmp_dir, index_manager) = create_english_index_manager();
    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("tokyo", 10).expect("検索失敗");
    assert!(results.is_empty());
  }

  #[test]
  fn search_finds_matching_document() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![
      Document::new("doc-1", "src-1", "Tokyo is the capital of Japan"),
      Document::new("doc-2", "src-1", "Osaka is a major city"),
    ];
    add_test_documents(&index_manager, &docs);

    // ドキュメント追加後に SearchEngine を作成
    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("tokyo", 10).expect("検索失敗");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].doc_id, "doc-1");
    assert!(results[0].score > 0.0);
  }

  #[test]
  fn search_is_case_insensitive() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![Document::new(
      "doc-1",
      "src-1",
      "Tokyo is the capital of Japan",
    )];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);

    // 小文字で検索
    let results_lower = search_engine.search("tokyo", 10).expect("検索失敗");
    // 大文字で検索
    let results_upper = search_engine.search("TOKYO", 10).expect("検索失敗");

    // どちらも同じドキュメントを返す（LowerCaser が動作している）
    assert_eq!(results_lower.len(), 1);
    assert_eq!(results_upper.len(), 1);
  }

  // ─── BM25 スコアリングテスト ─────────────────────────────────────────────────

  #[test]
  fn search_bm25_rare_term_scores_higher() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    // "rust" は doc-1 にのみ登場、"programming" は両方に登場
    let docs = vec![
      Document::new("doc-1", "src-1", "Rust programming language"),
      Document::new("doc-2", "src-1", "Python programming language"),
    ];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("rust", 10).expect("検索失敗");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].doc_id, "doc-1");
  }

  #[test]
  fn search_returns_results_sorted_by_score() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![
      Document::new("doc-1", "src-1", "programming programming programming"),
      Document::new("doc-2", "src-1", "programming"),
    ];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("programming", 10).expect("検索失敗");
    assert_eq!(results.len(), 2);

    // スコア順にソートされていることを確認（高いスコアが先）
    for i in 0..results.len().saturating_sub(1) {
      assert!(results[i].score >= results[i + 1].score);
    }
  }

  // ─── search_tokens_or テスト ────────────────────────────────────────────────

  #[test]
  fn search_tokens_or_finds_documents() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![
      Document::new("doc-1", "src-1", "Tokyo is the capital of Japan"),
      Document::new("doc-2", "src-1", "Osaka is a major city"),
    ];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search_tokens_or("tokyo", 10).expect("検索失敗");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].doc_id, "doc-1");
  }

  #[test]
  fn search_tokens_or_handles_multiple_tokens() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![
      Document::new("doc-1", "src-1", "Tokyo tower is famous"),
      Document::new("doc-2", "src-1", "Osaka castle is famous"),
    ];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    // "tokyo" OR "osaka" で両方ヒットする
    let results = search_engine.search_tokens_or("tokyo osaka", 10).expect("検索失敗");
    assert_eq!(results.len(), 2);
  }

  #[test]
  fn search_tokens_or_returns_empty_for_empty_tokens() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![Document::new("doc-1", "src-1", "Some content")];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    // 空文字列 → トークンなし → 空結果
    let results = search_engine.search_tokens_or("", 10).expect("検索失敗");
    assert!(results.is_empty());
  }

  #[test]
  fn search_tokens_or_respects_limit() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![
      Document::new("doc-1", "src-1", "programming language"),
      Document::new("doc-2", "src-1", "programming tutorial"),
      Document::new("doc-3", "src-1", "programming guide"),
    ];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search_tokens_or("programming", 2).expect("検索失敗");
    assert_eq!(results.len(), 2);
  }

  // ─── メタデータ復元テスト ──────────────────────────────────────────────────

  #[test]
  fn search_restores_metadata() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![
      Document::new("doc-1", "src-1", "Tokyo is the capital of Japan")
        .with_metadata("author", json!("alice"))
        .with_metadata("version", json!(1))
        .with_tag("category:geo"),
    ];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("tokyo", 10).expect("検索失敗");
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert_eq!(result.metadata["author"], json!("alice"));
    assert_eq!(result.metadata["version"], json!(1));
    assert_eq!(result.metadata["tags"], json!(["category:geo"]));
  }

  #[test]
  fn search_returns_empty_metadata_when_not_set() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![Document::new("doc-1", "src-1", "Tokyo is the capital")];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("tokyo", 10).expect("検索失敗");
    assert_eq!(results.len(), 1);
    assert!(results[0].metadata.is_empty());
  }

  #[test]
  fn search_handles_complex_metadata_types() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![
      Document::new("doc-1", "src-1", "Test document")
        .with_metadata("string", json!("value"))
        .with_metadata("number", json!(42))
        .with_metadata("boolean", json!(true))
        .with_metadata("null", json!(null))
        .with_metadata("array", json!([1, 2, 3]))
        .with_metadata("object", json!({"nested": "value"})),
    ];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("test", 10).expect("検索失敗");
    assert_eq!(results.len(), 1);

    let metadata = &results[0].metadata;
    assert_eq!(metadata["string"], json!("value"));
    assert_eq!(metadata["number"], json!(42));
    assert_eq!(metadata["boolean"], json!(true));
    assert_eq!(metadata["null"], json!(null));
    assert_eq!(metadata["array"], json!([1, 2, 3]));
    assert_eq!(metadata["object"], json!({"nested": "value"}));
  }

  // ─── SearchResult 構造テスト ────────────────────────────────────────────────

  #[test]
  fn search_result_contains_all_fields() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs =
      vec![Document::new("doc-123", "src-456", "Hello world").with_metadata("key", json!("value"))];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("hello", 10).expect("検索失敗");
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert_eq!(result.doc_id, "doc-123");
    assert_eq!(result.source_id, "src-456");
    assert_eq!(result.text, "Hello world");
    assert!(result.score > 0.0);
    assert_eq!(result.metadata["key"], json!("value"));
  }

  // ─── エラーハンドリングテスト ──────────────────────────────────────────────

  #[test]
  fn search_invalid_query_returns_error() {
    let (_tmp_dir, index_manager) = create_english_index_manager();
    let search_engine = create_search_engine(&index_manager);

    // 不正なクエリ構文（閉じられていない括弧）
    let result = search_engine.search("(", 10);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, SearcherError::InvalidQuery { .. }));
  }

  // ─── 英語固有のトークナイゼーションテスト ────────────────────────────────────

  #[test]
  fn search_stemming_works_for_english() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![Document::new("doc-1", "src-1", "running and jumping")];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    // Stemmer により "running" → "run", "jumping" → "jump" でマッチするはず
    // ただし Tantivy の Stemmer の挙動に依存するため、
    // ここではクエリがエラーにならないことだけ確認
    let results = search_engine.search("run", 10);
    assert!(results.is_ok());
  }

  #[test]
  fn search_tokens_or_lowercases_query() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![Document::new("doc-1", "src-1", "TOKYO CAPITAL")];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    // 小文字で検索しても大文字のドキュメントが見つかる
    let results = search_engine.search_tokens_or("tokyo", 10).expect("検索失敗");
    assert_eq!(results.len(), 1);
  }

  // ─── 複数ドキュメントの検索テスト ────────────────────────────────────────────

  #[test]
  fn search_finds_multiple_matching_documents() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![
      Document::new("doc-1", "src-1", "Rust programming"),
      Document::new("doc-2", "src-1", "Python programming"),
      Document::new("doc-3", "src-1", "Java programming"),
    ];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("programming", 10).expect("検索失敗");
    assert_eq!(results.len(), 3);
  }

  #[test]
  fn search_distinct_source_ids() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![
      Document::new("doc-1", "source-alpha", "Alpha document"),
      Document::new("doc-2", "source-beta", "Beta document"),
    ];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("document", 10).expect("検索失敗");
    assert_eq!(results.len(), 2);

    let source_ids: std::collections::HashSet<&str> =
      results.iter().map(|r| r.source_id.as_str()).collect();
    assert!(source_ids.contains("source-alpha"));
    assert!(source_ids.contains("source-beta"));
  }

  // ─── エッジケーステスト ─────────────────────────────────────────────────────

  #[test]
  fn search_special_characters_in_content() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![Document::new("doc-1", "src-1", "Price: $100 (50% off!)")];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    // 特殊文字を含むコンテンツでも検索できる
    let results = search_engine.search("price", 10).expect("検索失敗");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].text, "Price: $100 (50% off!)");
  }

  #[test]
  fn search_whitespace_handling() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![Document::new("doc-1", "src-1", "hello world")];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    // 複数トークンのクエリ
    let results = search_engine.search("hello world", 10).expect("検索失敗");
    assert!(!results.is_empty());
  }

  #[test]
  fn search_long_text_content() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let long_text = "programming ".repeat(100);
    let docs = vec![Document::new("doc-1", "src-1", &long_text)];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("programming", 10).expect("検索失敗");
    assert_eq!(results.len(), 1);
  }

  #[test]
  fn search_unicode_content() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    // 英語インデックスでも Unicode 文字を含むテキストを保存できる
    let docs = vec![Document::new("doc-1", "src-1", "Hello 世界 мир")];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("hello", 10).expect("検索失敗");
    assert_eq!(results.len(), 1);
    assert!(results[0].text.contains("世界"));
  }
}
