// crates/wakeru/src/service.rs

//! WakeruService: Integrated facade for wakeru crate.
//!
//! - Dictionary Management (DictionaryManager)
//! - Index Management (IndexManager) - Per language
//! - Search Engine (SearchEngine) - Per language
//!
//! From outside such as RAG pipeline, only this structure needs to be considered.
//!
//! # Multi-language support
//!
//! Has independent index and search engine for each language:
//! - Japanese: `data/index/ja/` (VibratoTokenizer + N-gram)
//! - English: `data/index/en/` (SimpleTokenizer + LowerCaser)

use std::collections::HashMap;
use std::sync::Arc;

use tantivy::tokenizer::TextAnalyzer;

use crate::config::{Language, WakeruConfig};
use crate::dictionary::DictionaryManager;
use crate::errors::error_definition::{WakeruError, WakeruResult};
use crate::indexer::IndexManager;
use crate::models::{Document, SearchResult};
use crate::searcher::SearchEngine;
use crate::tokenizer::vibrato_tokenizer::VibratoTokenizer;

/// Structure pairing Index and SearchEngine per language.
///
/// This structurally prevents language mismatch.
struct PerLanguage {
  #[allow(dead_code)] // Planned to be used in accessors in the future
  index_manager: IndexManager,
  search_engine: SearchEngine,
}

/// Integrated facade for wakeru crate.
///
/// RAG pipeline accesses all functions through this structure.
///
/// # Multi-language support
///
/// Manages IndexManager and SearchEngine for each language with `HashMap<Language, PerLanguage>`.
/// Performs index creation and search by specifying language.
pub struct WakeruService {
  /// Default language
  default_language: Language,

  /// IndexManager + SearchEngine per language
  langs: HashMap<Language, PerLanguage>,

  /// Dictionary Manager (for Japanese)
  dictionary_manager: Option<DictionaryManager>,
}

impl WakeruService {
  /// Initialization (Load dictionary + Open/Create index for each language + Build SearchEngine)
  ///
  /// # Process Flow
  /// 1. Validate configuration
  /// 2. Build DictionaryManager only when Japanese is supported
  /// 3. Build IndexManager + SearchEngine for each supported language
  ///
  /// # Errors
  /// - Invalid configuration (empty languages, default_language not included, etc.)
  /// - Dictionary load failure
  /// - Index creation/open failure
  pub fn init(config: &WakeruConfig) -> WakeruResult<Self> {
    // Validate configuration (ConfigError is automatically converted to WakeruError with #[from])
    config.validate()?;

    let default_language = config.default_language();

    // Build dictionary manager only when Japanese is supported
    let (dictionary_manager, ja_analyzer) = if config.supported_languages().contains(&Language::Ja)
    {
      let manager = DictionaryManager::with_preset(config.dictionary_preset())?;
      let dict = manager.load()?;
      let tokenizer = VibratoTokenizer::from_shared_dictionary(dict);
      let analyzer = TextAnalyzer::from(tokenizer);
      (Some(manager), Some(Arc::new(analyzer)))
    } else {
      (None, None)
    };

    let mut langs = HashMap::new();

    // Build IndexManager + SearchEngine for each language
    for &lang in config.supported_languages() {
      let index_path = config.index_path_for_language(lang);

      // Prepare tokenizer according to language
      let lang_analyzer = match lang {
        Language::Ja => ja_analyzer.as_ref().map(|a| (**a).clone()),
        Language::En => None, // English is created inside IndexManager
      };

      let index_manager = IndexManager::open_or_create(&index_path, lang, lang_analyzer)?;
      let search_engine = SearchEngine::new(index_manager.index(), *index_manager.fields(), lang)?;

      langs.insert(
        lang,
        PerLanguage {
          index_manager,
          search_engine,
        },
      );
    }

    Ok(Self {
      default_language,
      langs,
      dictionary_manager,
    })
  }

  /// Adds documents to index in specified language.
  ///
  /// # Arguments
  /// - `language`: Target language
  /// - `documents`: Documents to add
  ///
  /// # Errors
  /// - Unsupported language
  /// - Index write error
  pub fn index_documents_with_language(
    &self,
    language: Language,
    documents: &[Document],
  ) -> WakeruResult<()> {
    let per_lang =
      self.langs.get(&language).ok_or(WakeruError::UnsupportedLanguage { language })?;
    per_lang.index_manager.add_documents(documents).map(|_| ()).map_err(WakeruError::from)
  }

  /// Adds documents to index in default language.
  ///
  /// `AddDocumentsReport` is not returned currently, only error propagates to upper layer.
  pub fn index_documents(&self, documents: &[Document]) -> WakeruResult<()> {
    self.index_documents_with_language(self.default_language, documents)
  }

  /// Executes BM25 search in specified language.
  ///
  /// # Arguments
  /// - `language`: Search target language
  /// - `query`: Search query
  /// - `limit`: Maximum number of results
  ///
  /// # Errors
  /// - Unsupported language
  /// - Query parse error
  pub fn search_with_language(
    &self,
    language: Language,
    query: &str,
    limit: usize,
  ) -> WakeruResult<Vec<SearchResult>> {
    let per_lang =
      self.langs.get(&language).ok_or(WakeruError::UnsupportedLanguage { language })?;
    per_lang.search_engine.search(query, limit).map_err(WakeruError::from)
  }

  /// Executes BM25 search in default language.
  ///
  /// `limit` is passed to `SearchEngine::search` as is.
  /// (Caller should consider `default_limit` / `max_limit` as needed).
  pub fn search(&self, query: &str, limit: usize) -> WakeruResult<Vec<SearchResult>> {
    self.search_with_language(self.default_language, query, limit)
  }

  /// Executes OR search of morphologically analyzed tokens in specified language.
  ///
  /// # Arguments
  /// - `language`: Search target language
  /// - `query`: Search query
  /// - `limit`: Maximum number of results
  ///
  /// # Errors
  /// - Unsupported language
  /// - Query parse error
  pub fn search_tokens_or_with_language(
    &self,
    language: Language,
    query: &str,
    limit: usize,
  ) -> WakeruResult<Vec<SearchResult>> {
    let per_lang =
      self.langs.get(&language).ok_or(WakeruError::UnsupportedLanguage { language })?;
    per_lang.search_engine.search_tokens_or(query, limit).map_err(WakeruError::from)
  }

  /// Helper to execute OR search of morphologically analyzed tokens in default language.
  ///
  /// Wrapper for `search_tokens_or` shown in Design Document 5.1.
  pub fn search_tokens_or(&self, query: &str, limit: usize) -> WakeruResult<Vec<SearchResult>> {
    self.search_tokens_or_with_language(self.default_language, query, limit)
  }

  // ===== Accessors =====

  /// Returns default language.
  pub fn default_language(&self) -> Language {
    self.default_language
  }

  /// Returns list of supported languages.
  pub fn supported_languages(&self) -> Vec<Language> {
    self.langs.keys().copied().collect()
  }

  /// Checks if the specified language is supported.
  pub fn is_language_supported(&self, language: Language) -> bool {
    self.langs.contains_key(&language)
  }

  /// Returns reference to internal DictionaryManager (only when Japanese is supported).
  pub fn dictionary_manager(&self) -> Option<&DictionaryManager> {
    self.dictionary_manager.as_ref()
  }

  /// Returns reference to IndexManager of specified language.
  pub fn index_manager(&self, language: Language) -> Option<&IndexManager> {
    self.langs.get(&language).map(|p| &p.index_manager)
  }

  /// Returns reference to SearchEngine of specified language.
  pub fn search_engine(&self, language: Language) -> Option<&SearchEngine> {
    self.langs.get(&language).map(|p| &p.search_engine)
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test Module
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::{
    DictionaryConfig, DictionaryPreset, IndexConfig, LogLevel, LoggingConfig, SearchConfig,
  };
  use crate::models::Document;
  use serde_json::json;

  // ─── Test Helper Functions ───────────────────────────────────────────────────

  /// Create WakeruConfig for testing with English only
  ///
  /// Dictionary manager is unnecessary because Japanese is not included
  fn create_english_only_config(temp_dir: &tempfile::TempDir) -> WakeruConfig {
    WakeruConfig {
      dictionary: DictionaryConfig {
        preset: DictionaryPreset::Ipadic,
        cache_dir: Some(temp_dir.path().join("dict")),
      },
      index: IndexConfig {
        data_dir: temp_dir.path().join("index"),
        writer_memory_bytes: 50_000_000,
        batch_commit_size: 1000,
        languages: vec![Language::En],
        default_language: Language::En,
      },
      search: SearchConfig {
        default_limit: 10,
        max_limit: 100,
      },
      logging: LoggingConfig {
        level: LogLevel::Info,
      },
    }
  }

  /// Create WakeruService with English only
  fn create_english_service() -> (tempfile::TempDir, WakeruService) {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temporary directory");
    let config = create_english_only_config(&temp_dir);
    let service = WakeruService::init(&config).expect("Failed to initialize WakeruService");
    (temp_dir, service)
  }

  // ─── Initialization Tests ──────────────────────────────────────────────────────────

  #[test]
  fn service_initializes_with_english_only() {
    let (_temp_dir, service) = create_english_service();

    // Confirm default language is English
    assert_eq!(service.default_language(), Language::En);

    // Confirm English is supported
    assert!(service.is_language_supported(Language::En));

    // Japanese is not supported (no dictionary)
    assert!(!service.is_language_supported(Language::Ja));
  }

  #[test]
  fn service_supported_languages() {
    let (_temp_dir, service) = create_english_service();

    let languages = service.supported_languages();
    assert_eq!(languages.len(), 1);
    assert!(languages.contains(&Language::En));
  }

  #[test]
  fn service_dictionary_manager_is_none_for_english_only() {
    let (_temp_dir, service) = create_english_service();

    // Dictionary manager does not exist for English only
    assert!(service.dictionary_manager().is_none());
  }

  // ─── Accessor Tests ────────────────────────────────────────────────────────

  #[test]
  fn service_index_manager_accessor() {
    let (_temp_dir, service) = create_english_service();

    // English IndexManager can be retrieved
    let index_manager = service.index_manager(Language::En);
    assert!(index_manager.is_some());
    assert_eq!(index_manager.unwrap().language(), Language::En);

    // Japanese IndexManager does not exist
    assert!(service.index_manager(Language::Ja).is_none());
  }

  #[test]
  fn service_search_engine_accessor() {
    let (_temp_dir, service) = create_english_service();

    // English SearchEngine can be retrieved
    let search_engine = service.search_engine(Language::En);
    assert!(search_engine.is_some());
    assert_eq!(search_engine.unwrap().language(), Language::En);

    // Japanese SearchEngine does not exist
    assert!(service.search_engine(Language::Ja).is_none());
  }

  #[test]
  fn service_is_language_supported() {
    let (_temp_dir, service) = create_english_service();

    assert!(service.is_language_supported(Language::En));
    assert!(!service.is_language_supported(Language::Ja));
  }

  // ─── Document Addition Tests ────────────────────────────────────────────────

  #[test]
  fn service_index_documents_default_language() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![Document::new("doc-1", "src-1", "Hello world")];

    let result = service.index_documents(&docs);
    assert!(result.is_ok());
  }

  #[test]
  fn service_index_documents_with_language() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![Document::new("doc-1", "src-1", "Hello world")];

    let result = service.index_documents_with_language(Language::En, &docs);
    assert!(result.is_ok());
  }

  #[test]
  fn service_index_documents_unsupported_language() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![Document::new("doc-1", "src-1", "Hello world")];

    let result = service.index_documents_with_language(Language::Ja, &docs);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, WakeruError::UnsupportedLanguage { .. }));
  }

  #[test]
  fn service_index_documents_with_metadata() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![
      Document::new("doc-1", "src-1", "Tokyo is the capital")
        .with_metadata("author", json!("alice"))
        .with_tag("category:geo"),
    ];

    let result = service.index_documents(&docs);
    assert!(result.is_ok());
  }

  // ─── Search Tests ────────────────────────────────────────────────────────────

  #[test]
  fn service_search_default_language() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![Document::new("doc-1", "src-1", "Hello world")];
    service.index_documents(&docs).expect("Indexing failed");

    // SearchEngine is created at indexing time,
    // so documents added afterwards cannot be searched (Reader is not reloaded)
    // Here we just check that no error occurs
    let result = service.search("hello", 10);
    assert!(result.is_ok());
  }

  #[test]
  fn service_search_with_language() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![Document::new("doc-1", "src-1", "Hello world")];
    service.index_documents(&docs).expect("Indexing failed");

    let result = service.search_with_language(Language::En, "hello", 10);
    assert!(result.is_ok());
  }

  #[test]
  fn service_search_unsupported_language() {
    let (_temp_dir, service) = create_english_service();

    let result = service.search_with_language(Language::Ja, "hello", 10);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, WakeruError::UnsupportedLanguage { .. }));
  }

  #[test]
  fn service_search_tokens_or_default_language() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![Document::new("doc-1", "src-1", "Hello world")];
    service.index_documents(&docs).expect("Indexing failed");

    let result = service.search_tokens_or("hello", 10);
    assert!(result.is_ok());
  }

  #[test]
  fn service_search_tokens_or_with_language() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![Document::new("doc-1", "src-1", "Hello world")];
    service.index_documents(&docs).expect("Indexing failed");

    let result = service.search_tokens_or_with_language(Language::En, "hello", 10);
    assert!(result.is_ok());
  }

  #[test]
  fn service_search_tokens_or_unsupported_language() {
    let (_temp_dir, service) = create_english_service();

    let result = service.search_tokens_or_with_language(Language::Ja, "hello", 10);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, WakeruError::UnsupportedLanguage { .. }));
  }

  // ─── Integration Tests (Index -> Search) ──────────────────────────────────────

  #[test]
  fn service_full_workflow_index_and_search() {
    // In this test, create a new WakeruService after indexing
    // to verify that documents are correctly persisted

    let temp_dir = tempfile::TempDir::new().expect("Failed to create temporary directory");
    let config = create_english_only_config(&temp_dir);

    // 1. Add documents with the first service
    {
      let service = WakeruService::init(&config).expect("Initialization failed");
      let docs = vec![
        Document::new("doc-1", "src-1", "Tokyo is the capital of Japan"),
        Document::new("doc-2", "src-1", "Osaka is a major city"),
      ];
      service.index_documents(&docs).expect("Indexing failed");
    }

    // 2. Create a new service and search
    {
      let service = WakeruService::init(&config).expect("Initialization failed");
      let results = service.search("tokyo", 10).expect("Search failed");

      assert_eq!(results.len(), 1);
      assert_eq!(results[0].doc_id, "doc-1");
    }
  }

  #[test]
  fn service_full_workflow_with_metadata() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temporary directory");
    let config = create_english_only_config(&temp_dir);

    // 1. Add documents
    {
      let service = WakeruService::init(&config).expect("Initialization failed");
      let docs = vec![
        Document::new("doc-1", "src-1", "Tokyo is the capital")
          .with_metadata("author", json!("alice"))
          .with_tag("category:geo"),
      ];
      service.index_documents(&docs).expect("Indexing failed");
    }

    // 2. Confirm metadata is restored
    {
      let service = WakeruService::init(&config).expect("Initialization failed");
      let results = service.search("tokyo", 10).expect("Search failed");

      assert_eq!(results.len(), 1);
      assert_eq!(results[0].metadata["author"], json!("alice"));
      assert_eq!(results[0].metadata["tags"], json!(["category:geo"]));
    }
  }

  // ─── Error Handling Tests ────────────────────────────────────────────

  #[test]
  fn service_invalid_query_returns_error() {
    let (_temp_dir, service) = create_english_service();

    let result = service.search("(", 10);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, WakeruError::Searcher(_)));
  }

  #[test]
  fn service_duplicate_documents_are_skipped() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temporary directory");
    let config = create_english_only_config(&temp_dir);

    // 1. Add document with same ID twice
    {
      let service = WakeruService::init(&config).expect("Initialization failed");
      let docs1 = vec![Document::new("doc-1", "src-1", "First content")];
      service.index_documents(&docs1).expect("Indexing failed");

      let docs2 = vec![Document::new("doc-1", "src-1", "Second content")];
      service.index_documents(&docs2).expect("Indexing failed"); // Duplicates are skipped
    }

    // 2. Confirm first content is retained
    {
      let service = WakeruService::init(&config).expect("Initialization failed");
      let results = service.search("first", 10).expect("Search failed");

      assert_eq!(results.len(), 1);
      assert_eq!(results[0].text, "First content");
    }
  }

  // ─── Config Validation Tests ──────────────────────────────────────────────

  #[test]
  fn service_init_validates_config() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temporary directory");

    // Invalid config: languages is empty
    let invalid_config = WakeruConfig {
      dictionary: DictionaryConfig {
        preset: DictionaryPreset::Ipadic,
        cache_dir: Some(temp_dir.path().join("dict")),
      },
      index: IndexConfig {
        data_dir: temp_dir.path().join("index"),
        writer_memory_bytes: 50_000_000,
        batch_commit_size: 1000,
        languages: vec![], // Invalid: Empty language list
        default_language: Language::En,
      },
      search: SearchConfig {
        default_limit: 10,
        max_limit: 100,
      },
      logging: LoggingConfig {
        level: LogLevel::Info,
      },
    };

    let result = WakeruService::init(&invalid_config);
    assert!(result.is_err());
  }
}
