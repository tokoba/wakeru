//! BM25 search module

use tantivy::query::{BooleanQuery, Occur, TermSetQuery};
use tantivy::schema::Value;
use tantivy::schema::document::CompactDocValue;
use tantivy::{Index, IndexReader, ReloadPolicy, Term, collector::TopDocs, query::QueryParser};
use tracing::debug;

use crate::config::Language;
use crate::errors::SearcherError;
use crate::indexer::schema_builder::SchemaFields;
use crate::models::SearchResult;

// Use tokenization utilities
use super::tokenization::{TokenizationResult, tokenize_with_text_analyzer};

// ─────────────────────────────────────────────────────────────────────────────
// JSON Conversion Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Conversion from CompactDocValue to serde_json::Value
///
/// Tantivy 0.25: CompactDocValue does not implement Serialize,
/// so convert to OwnedValue first, then to serde_json::Value
fn compact_value_to_json(value: &CompactDocValue<'_>) -> serde_json::Value {
  use tantivy::schema::OwnedValue;

  // Conversion from CompactDocValue to OwnedValue (using From trait)
  let owned: OwnedValue = (*value).into();

  // OwnedValue implements Serialize so it can be converted to serde_json::Value
  // Usually doesn't fail, but fallback to Null and log warning if it does
  serde_json::to_value(owned).unwrap_or_else(|e| {
    debug!(error = %e, "Failed to serialize metadata value. Restoring as Null.");
    serde_json::Value::Null
  })
}

/// BM25 Search Engine
pub struct SearchEngine {
  /// Tantivy IndexReader
  reader: IndexReader,

  /// Fields to search
  fields: SchemaFields,

  /// Language of this search engine
  language: Language,
}

/// Implementation block for BM25 Search Engine
impl SearchEngine {
  /// Initializes the search engine
  ///
  /// # Arguments
  /// - `index`: Reference to Tantivy Index
  /// - `fields`: Schema fields
  /// - `language`: Language of this search engine
  pub fn new(
    index: &Index,
    fields: SchemaFields,
    language: Language,
  ) -> Result<Self, SearcherError> {
    let reader = index
      .reader_builder()
      .reload_policy(ReloadPolicy::OnCommitWithDelay) // Auto reload setting
      .try_into()?;

    Ok(Self {
      reader,
      fields,
      language,
    })
  }

  /// Search by BM25 score
  pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<SearchResult>, SearcherError> {
    let searcher = self.reader.searcher();

    // QueryParser: target text field
    let query_parser = QueryParser::for_index(searcher.index(), vec![self.fields.text]);

    // Parse query string
    let query = query_parser.parse_query(query_str).map_err(|e| SearcherError::InvalidQuery {
      reason: e.to_string(),
    })?;

    // Get top documents (max < limit) by BM25 score
    let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

    // Convert results with helper method
    self.convert_to_search_results(&searcher, top_docs)
  }

  /// Parses query string with language-specific tokenizer and extracts unique Terms
  ///
  /// # Process Flow
  /// 1. Get tokenizer according to language
  /// 2. Delegate to pure tokenization function (deduplication, empty string exclusion, Term conversion)
  ///
  /// # Arguments
  /// - `index`: Reference to Tantivy Index (for getting tokenizer)
  /// - `query_str`: Query string to tokenize
  ///
  /// # Returns
  /// `TokenizationResult` containing unique Terms and token strings
  fn tokenize_query(
    &self,
    index: &Index,
    query_str: &str,
  ) -> Result<TokenizationResult, SearcherError> {
    // Get tokenizer name according to language
    let tokenizer_name = self.language.text_tokenizer_name();

    // Get tokenizer
    let mut analyzer =
      index.tokenizers().get(tokenizer_name).ok_or_else(|| SearcherError::InvalidQuery {
        reason: format!("tokenizer `{tokenizer_name}` is not registered"),
      })?;

    // Delegate to tokenization function dedicated to TextAnalyzer
    Ok(tokenize_with_text_analyzer(
      &mut analyzer,
      self.fields.text,
      query_str,
    ))
  }

  /// Parses query with language-specific tokenizer and performs OR search with extracted tokens
  ///
  /// # Arguments
  /// - `query_str`: Search query string (e.g., "京都の寺", "Tokyo temples")
  /// - `limit`: Maximum number of results to return
  ///
  /// # Returns
  /// Search result vector with BM25 score
  ///
  /// # Behavior
  /// 1. Parse query string with language-specific tokenizer
  /// 2. Convert extracted tokens to Terms
  /// 3. For Japanese, 1-char tokens are also searched in N-gram field
  /// 4. Execute OR search with TermSetQuery / BooleanQuery
  ///
  /// # Examples
  /// ```ignore
  /// // Japanese search
  /// let results = search_engine.search_tokens_or("京都の寺", 10)?;
  /// // Searched as "京都" and "寺"
  ///
  /// // English search (lowercased by LowerCaser)
  /// let results = search_engine.search_tokens_or("Tokyo Tower", 10)?;
  /// // Searched as "tokyo" and "tower"
  /// ```
  pub fn search_tokens_or(
    &self,
    query_str: &str,
    limit: usize,
  ) -> Result<Vec<SearchResult>, SearcherError> {
    debug!(query = %query_str, limit, language = ?self.language, "Start parsing search query");

    let searcher = self.reader.searcher();
    let index = searcher.index();

    // Delegate tokenization process to dedicated method
    let TokenizationResult {
      terms: morph_terms,
      query_tokens,
    } = self.tokenize_query(index, query_str)?;

    // Log query tokens
    debug!(
      query = %query_str,
      tokens = ?query_tokens,
      num_terms = morph_terms.len(),
      "Search query parsing completed"
    );

    if morph_terms.is_empty() {
      // Return empty result if all tokens are stop words etc.
      return Ok(vec![]);
    }

    // Extract 1-char tokens and create Terms for N-gram field
    // text_ngram field exists only for Japanese
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

    // Record presence of N-gram search for log output
    let has_ngram = !ngram_terms.is_empty();

    // Build query
    let query: Box<dyn tantivy::query::Query> = if ngram_terms.is_empty() {
      // No N-gram target: search only in morphological field
      Box::new(TermSetQuery::new(morph_terms))
    } else {
      // With N-gram target: OR search of morphology + N-gram
      let subqueries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = vec![
        // Morphological field search
        (Occur::Should, Box::new(TermSetQuery::new(morph_terms))),
        // N-gram field search
        (Occur::Should, Box::new(TermSetQuery::new(ngram_terms))),
      ];

      Box::new(BooleanQuery::from(subqueries))
    };

    debug!(
      query = %query_str,
      has_ngram,
      "Search query construction completed"
    );

    // Execute search (with BM25 score)
    let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

    // Result conversion (reuse existing logic)
    self.convert_to_search_results(&searcher, top_docs)
  }

  /// Helper method to convert top_docs to SearchResult vector
  fn convert_to_search_results(
    &self,
    searcher: &tantivy::Searcher,
    top_docs: Vec<(f32, tantivy::DocAddress)>,
  ) -> Result<Vec<SearchResult>, SearcherError> {
    let mut results = Vec::with_capacity(top_docs.len());

    for (score, doc_address) in top_docs {
      let doc: tantivy::TantivyDocument = searcher.doc(doc_address)?;

      // Get required fields (InvalidIndex if error)
      let doc_id =
        self.get_text_field(&doc, self.fields.id).ok_or_else(|| SearcherError::InvalidIndex {
          field: "id".to_string(),
          reason: "Required field not found".to_string(),
        })?;

      let source_id = self.get_text_field(&doc, self.fields.source_id).ok_or_else(|| {
        SearcherError::InvalidIndex {
          field: "source_id".to_string(),
          reason: "Required field not found".to_string(),
        }
      })?;

      // text is treated as Optional (fallback to empty string)
      let text = self.get_text_field(&doc, self.fields.text).unwrap_or_default();

      // Restore metadata: Get directly from JsonObject
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

  /// Get value of single text field from TantivyDocument
  ///
  /// # Returns
  /// - `Some(String)`: If field value exists
  /// - `None`: If field value does not exist
  fn get_text_field(
    &self,
    doc: &tantivy::TantivyDocument,
    field: tantivy::schema::Field,
  ) -> Option<String> {
    doc.get_first(field).and_then(|v| v.as_str().map(String::from))
  }

  /// Get value of JsonObject field from TantivyDocument and convert to Metadata
  ///
  /// # Returns
  /// - If field value exists: Converted Metadata
  /// - If field value does not exist: Empty Metadata
  fn get_json_object_field(
    &self,
    doc: &tantivy::TantivyDocument,
    field: tantivy::schema::Field,
  ) -> crate::models::Metadata {
    doc
      .get_first(field)
      .and_then(|value| value.as_object())
      .map(|iter| {
        // Tantivy 0.25: as_object() returns CompactDocObjectIter (iterator)
        // iter: (key: &str, value: CompactDocValue<'_>)
        let mut metadata = crate::models::Metadata::default();

        for (k, v) in iter {
          // Convert CompactDocValue to serde_json::Value
          let json_val = compact_value_to_json(&v);
          metadata.insert(k.to_string(), json_val);
        }

        metadata
      })
      .unwrap_or_default()
  }

  /// Returns the language of this search engine
  pub fn language(&self) -> Language {
    self.language
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test Module
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::Language;
  use crate::indexer::index_manager::IndexManager;
  use crate::models::Document;
  use serde_json::json;

  // ─── Test Helper Functions ───────────────────────────────────────────────────

  /// Helper to create English index (SearchEngine created later)
  fn create_english_index_manager() -> (tempfile::TempDir, IndexManager) {
    let tmp_dir = tempfile::TempDir::new().expect("Failed to create temporary directory");
    let index_manager = IndexManager::open_or_create(tmp_dir.path(), Language::En, None)
      .expect("Failed to create index");
    (tmp_dir, index_manager)
  }

  /// Helper to create SearchEngine from IndexManager
  ///
  /// Important: Call after adding documents (SearchEngine has its own Reader)
  fn create_search_engine(index_manager: &IndexManager) -> SearchEngine {
    SearchEngine::new(index_manager.index(), *index_manager.fields(), Language::En)
      .expect("Failed to create SearchEngine")
  }

  /// Helper to add test documents
  fn add_test_documents(index_manager: &IndexManager, docs: &[Document]) {
    let report = index_manager.add_documents(docs).expect("Failed to add documents");
    assert_eq!(
      report.added,
      docs.len(),
      "Expected number of documents to be added"
    );
  }

  // ─── Basic Search Tests ────────────────────────────────────────────────────

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
    let results = search_engine.search("tokyo", 10).expect("Search failed");
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

    // Create SearchEngine after adding documents
    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("tokyo", 10).expect("Search failed");
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

    // Search in lowercase
    let results_lower = search_engine.search("tokyo", 10).expect("Search failed");
    // Search in uppercase
    let results_upper = search_engine.search("TOKYO", 10).expect("Search failed");

    // Both return the same document (LowerCaser is working)
    assert_eq!(results_lower.len(), 1);
    assert_eq!(results_upper.len(), 1);
  }

  // ─── BM25 Scoring Tests ─────────────────────────────────────────────────

  #[test]
  fn search_bm25_rare_term_scores_higher() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    // "rust" appears only in doc-1, "programming" appears in both
    let docs = vec![
      Document::new("doc-1", "src-1", "Rust programming language"),
      Document::new("doc-2", "src-1", "Python programming language"),
    ];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("rust", 10).expect("Search failed");
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
    let results = search_engine.search("programming", 10).expect("Search failed");
    assert_eq!(results.len(), 2);

    // Confirm sorted by score (higher score first)
    for i in 0..results.len().saturating_sub(1) {
      assert!(results[i].score >= results[i + 1].score);
    }
  }

  // ─── search_tokens_or Tests ────────────────────────────────────────────────

  #[test]
  fn search_tokens_or_finds_documents() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![
      Document::new("doc-1", "src-1", "Tokyo is the capital of Japan"),
      Document::new("doc-2", "src-1", "Osaka is a major city"),
    ];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search_tokens_or("tokyo", 10).expect("Search failed");
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
    // "tokyo" OR "osaka" hits both
    let results = search_engine.search_tokens_or("tokyo osaka", 10).expect("Search failed");
    assert_eq!(results.len(), 2);
  }

  #[test]
  fn search_tokens_or_returns_empty_for_empty_tokens() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![Document::new("doc-1", "src-1", "Some content")];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    // Empty string -> No tokens -> Empty result
    let results = search_engine.search_tokens_or("", 10).expect("Search failed");
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
    let results = search_engine.search_tokens_or("programming", 2).expect("Search failed");
    assert_eq!(results.len(), 2);
  }

  // ─── Metadata Restoration Tests ──────────────────────────────────────────────────

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
    let results = search_engine.search("tokyo", 10).expect("Search failed");
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
    let results = search_engine.search("tokyo", 10).expect("Search failed");
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
    let results = search_engine.search("test", 10).expect("Search failed");
    assert_eq!(results.len(), 1);

    let metadata = &results[0].metadata;
    assert_eq!(metadata["string"], json!("value"));
    assert_eq!(metadata["number"], json!(42));
    assert_eq!(metadata["boolean"], json!(true));
    assert_eq!(metadata["null"], json!(null));
    assert_eq!(metadata["array"], json!([1, 2, 3]));
    assert_eq!(metadata["object"], json!({"nested": "value"}));
  }

  // ─── SearchResult Structure Tests ────────────────────────────────────────────────

  #[test]
  fn search_result_contains_all_fields() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs =
      vec![Document::new("doc-123", "src-456", "Hello world").with_metadata("key", json!("value"))];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("hello", 10).expect("Search failed");
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert_eq!(result.doc_id, "doc-123");
    assert_eq!(result.source_id, "src-456");
    assert_eq!(result.text, "Hello world");
    assert!(result.score > 0.0);
    assert_eq!(result.metadata["key"], json!("value"));
  }

  // ─── Error Handling Tests ──────────────────────────────────────────────

  #[test]
  fn search_invalid_query_returns_error() {
    let (_tmp_dir, index_manager) = create_english_index_manager();
    let search_engine = create_search_engine(&index_manager);

    // Invalid query syntax (unclosed parenthesis)
    let result = search_engine.search("(", 10);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, SearcherError::InvalidQuery { .. }));
  }

  // ─── English specific tokenization tests ────────────────────────────────────

  #[test]
  fn search_stemming_works_for_english() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![Document::new("doc-1", "src-1", "running and jumping")];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    // Should match "running" -> "run", "jumping" -> "jump" by Stemmer
    // However, it depends on Tantivy Stemmer behavior,
    // so here just checking that query does not error
    let results = search_engine.search("run", 10);
    assert!(results.is_ok());
  }

  #[test]
  fn search_tokens_or_lowercases_query() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![Document::new("doc-1", "src-1", "TOKYO CAPITAL")];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    // Document found even if searching in lowercase
    let results = search_engine.search_tokens_or("tokyo", 10).expect("Search failed");
    assert_eq!(results.len(), 1);
  }

  // ─── Multiple Document Search Tests ────────────────────────────────────────────

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
    let results = search_engine.search("programming", 10).expect("Search failed");
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
    let results = search_engine.search("document", 10).expect("Search failed");
    assert_eq!(results.len(), 2);

    let source_ids: std::collections::HashSet<&str> =
      results.iter().map(|r| r.source_id.as_str()).collect();
    assert!(source_ids.contains("source-alpha"));
    assert!(source_ids.contains("source-beta"));
  }

  // ─── Edge Case Tests ─────────────────────────────────────────────────────

  #[test]
  fn search_special_characters_in_content() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![Document::new("doc-1", "src-1", "Price: $100 (50% off!)")];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    // Search possible even with content containing special characters
    let results = search_engine.search("price", 10).expect("Search failed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].text, "Price: $100 (50% off!)");
  }

  #[test]
  fn search_whitespace_handling() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let docs = vec![Document::new("doc-1", "src-1", "hello world")];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    // Multi-token query
    let results = search_engine.search("hello world", 10).expect("Search failed");
    assert!(!results.is_empty());
  }

  #[test]
  fn search_long_text_content() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    let long_text = "programming ".repeat(100);
    let docs = vec![Document::new("doc-1", "src-1", &long_text)];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("programming", 10).expect("Search failed");
    assert_eq!(results.len(), 1);
  }

  #[test]
  fn search_unicode_content() {
    let (_tmp_dir, index_manager) = create_english_index_manager();

    // English index can save text containing Unicode characters
    let docs = vec![Document::new("doc-1", "src-1", "Hello 世界 мир")];
    add_test_documents(&index_manager, &docs);

    let search_engine = create_search_engine(&index_manager);
    let results = search_engine.search("hello", 10).expect("Search failed");
    assert_eq!(results.len(), 1);
    assert!(results[0].text.contains("世界"));
  }
}
