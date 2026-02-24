//! Tantivy Index Management Module
//!
//! Responsible for index creation, management, and document addition.
//! Supports Language argument and language-specific tokenizer registration for multi-language support.

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

/// Meta file name used to determine index existence
const META_JSON: &str = "meta.json";

// ─────────────────────────────────────────────────────────────────────────────
// JSON Conversion Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Recursive conversion from serde_json::Value to OwnedValue
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
      // OwnedValue::Object expects Vec<(String, OwnedValue)>
      let obj: Vec<(String, OwnedValue)> =
        map.iter().map(|(k, v)| (k.clone(), serde_json_to_owned(v))).collect();
      O::Object(obj)
    }
  }
}

/// Conversion from Metadata (HashMap) to Tantivy JsonObject (Vec)
///
/// Tantivy 0.25: add_object expects BTreeMap<String, OwnedValue>
fn metadata_to_tantivy_object(metadata: &crate::models::Metadata) -> BTreeMap<String, OwnedValue> {
  metadata.iter().map(|(k, v)| (k.clone(), serde_json_to_owned(v))).collect()
}

/// Structure for Tantivy index creation and management.
///
/// # Responsibilities
///
/// - Index directory creation
/// - Schema definition and tokenizer registration
/// - Document addition (skips duplicates)
/// - IndexWriter commit management
///
/// # Multi-language support
///
/// - Japanese (`Language::Ja`): VibratoTokenizer + N-gram Tokenizer
/// - English (`Language::En`): SimpleTokenizer + LowerCaser
pub struct IndexManager {
  /// Tantivy Index handle
  index: Index,

  /// IndexReader (for searching)
  reader: IndexReader,

  /// Schema fields reference
  fields: SchemaFields,

  /// Language of this index
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
  /// Opens an index. Creates a new one if it does not exist.
  ///
  /// # Arguments
  /// - `index_path`: Directory to save the index
  /// - `language`: Language of the index
  /// - `tokenizer_ja`: Japanese tokenizer (Required for Japanese index)
  ///
  /// # Errors
  /// - Directory creation failure
  /// - Tantivy index creation/open error
  /// - Tokenizer not provided for Japanese index
  /// - Mismatch between existing index and language
  ///
  /// # Design Notes
  ///
  /// - **New creation**: Build schema with `build_schema(language)`
  /// - **Opening existing index**: Reconstruct with `SchemaFields::from_schema(&schema)`
  /// - **Loose coupling**: `tokenizer_ja` is `Option<TextAnalyzer>` and does not depend on VibratoTokenizer
  pub fn open_or_create<P: AsRef<Path>>(
    index_path: P,
    language: Language,
    tokenizer_ja: Option<TextAnalyzer>,
  ) -> Result<Self, IndexerError> {
    let index_path = index_path.as_ref();

    // Determine index existence by meta.json existence
    let meta_json_exists = index_path.join(META_JSON).exists();

    let (index, fields) = if meta_json_exists {
      // Open existing index
      let index = Index::open_in_dir(index_path)?;
      let schema = index.schema();

      // Reconstruct SchemaFields from existing schema
      let fields = SchemaFields::from_schema(&schema)?;

      // Check consistency between schema and language
      Self::assert_schema_matches_language(&schema, language)?;

      (index, fields)
    } else {
      // Create directory (if not exists)
      if !index_path.exists() {
        std::fs::create_dir_all(index_path).map_err(|e| IndexerError::InvalidIndexPath {
          path: index_path.to_path_buf(),
          source: Arc::new(e),
        })?;
      }
      // Use build_schema only when creating new index
      let (schema, fields) = build_schema(language);
      let index = Index::create_in_dir(index_path, schema)?;
      (index, fields)
    };

    // Register tokenizer according to language
    match language {
      Language::Ja => {
        // Japanese tokenizer is required
        let tokenizer = tokenizer_ja.ok_or(IndexerError::MissingJapaneseTokenizer)?;
        index.tokenizers().register(language.text_tokenizer_name(), tokenizer);

        // Register 1-char N-gram tokenizer (for partial match search)
        // Tantivy 0.25.0: NgramTokenizer::new() returns Result
        let ja_ngram_tokenizer = NgramTokenizer::new(1, 1, false)?;
        let ja_ngram = TextAnalyzer::builder(ja_ngram_tokenizer).build();
        index.tokenizers().register("ja_ngram", ja_ngram);
      }
      Language::En => {
        // English: SimpleTokenizer + LowerCaser
        // Tantivy 0.25.0: Use builder pattern
        let en_analyzer = TextAnalyzer::builder(SimpleTokenizer::default())
          .filter(LowerCaser)
          .filter(Stemmer::new(tantivy::tokenizer::Language::English))
          .build();
        index.tokenizers().register(language.text_tokenizer_name(), en_analyzer);
      }
    }

    // Create Reader
    let reader = index.reader()?;

    Ok(Self {
      index,
      reader,
      fields,
      language,
    })
  }

  /// Checks consistency between schema and language.
  ///
  /// Verifies if the tokenizer name of the text field in the existing index
  /// matches the tokenizer name expected for the specified language.
  fn assert_schema_matches_language(
    schema: &tantivy::schema::Schema,
    language: Language,
  ) -> Result<(), IndexerError> {
    let text_field = schema
      .get_field("text")
      .map_err(|e| tantivy::TantivyError::InvalidArgument(e.to_string()))?;

    let field_entry = schema.get_field_entry(text_field);

    // Tantivy 0.25.0: Pattern match FieldType to get TextOptions
    let text_options = match field_entry.field_type() {
      FieldType::Str(options) => options,
      _ => {
        return Err(IndexerError::Tantivy(
          tantivy::TantivyError::InvalidArgument("text field is not a text field".to_string()),
        ));
      }
    };

    // Get tokenizer name from index settings
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

  /// Adds documents to the index.
  ///
  /// - Skips duplicate documents (same ID)
  /// - Continues processing until the end (does not fail-fast)
  /// - Returns result as `AddDocumentsReport`
  ///
  /// # Arguments
  /// - `documents`: Slice of documents to add
  ///
  /// # Returns
  /// - `Ok(AddDocumentsReport)`: Processing statistics (success/skipped count)
  /// - `Err(IndexerError)`: Tantivy level fatal error
  pub fn add_documents(&self, documents: &[Document]) -> Result<AddDocumentsReport, IndexerError> {
    let mut report = AddDocumentsReport::default();
    let mut seen_ids: HashSet<String> = HashSet::with_capacity(documents.len());

    // Create IndexWriter (50MB buffer)
    let mut writer: IndexWriter = self.index.writer(50_000_000)?;

    // Searcher for searching
    let searcher = self.reader.searcher();

    for doc in documents {
      report.record_total();
      let id = doc.id.clone();

      // Duplicate in batch
      let in_batch = !seen_ids.insert(id.clone());

      // Duplicate in index (fast check with doc_freq)
      let term = Term::from_field_text(self.fields.id, &id);
      let in_index = searcher.doc_freq(&term)? > 0;

      if in_batch || in_index {
        // Skip duplicates
        report.record_skipped();
        continue;
      }

      // No duplicate -> Add
      let tantivy_doc = self.to_tantivy_document(doc)?;
      writer.add_document(tantivy_doc)?;
      report.record_added();
    }

    // Commit: Persist to disk
    writer.commit()?;

    // Reload Reader (make new documents visible for subsequent searches)
    self.reader.reload()?;

    Ok(report)
  }

  /// Document -> TantivyDocument conversion (internal method)
  ///
  /// # Returns
  /// - `Ok(TantivyDocument)`: Conversion successful
  fn to_tantivy_document(&self, doc: &Document) -> Result<tantivy::TantivyDocument, IndexerError> {
    let mut tantivy_doc = tantivy::TantivyDocument::default();

    tantivy_doc.add_text(self.fields.id, &doc.id);
    tantivy_doc.add_text(self.fields.source_id, &doc.source_id);
    tantivy_doc.add_text(self.fields.text, &doc.text);

    // Add same text to N-gram field (for partial match search)
    // Only for Japanese index (text_ngram is None for English)
    if let Some(text_ngram_field) = self.fields.text_ngram {
      tantivy_doc.add_text(text_ngram_field, &doc.text);
    }

    // Insert entire metadata as JsonObject
    // tags is also included in metadata["tags"], so double holding is unnecessary
    // Tantivy 0.25: add_object expects BTreeMap<String, OwnedValue>, so conversion is needed
    if !doc.metadata.is_empty() {
      let json_obj = metadata_to_tantivy_object(&doc.metadata);
      tantivy_doc.add_object(self.fields.metadata, json_obj);
    }

    Ok(tantivy_doc)
  }

  /// Returns reference to Tantivy Index (used in SearchEngine)
  pub fn index(&self) -> &Index {
    &self.index
  }

  /// Returns reference to IndexReader
  pub fn reader(&self) -> &IndexReader {
    &self.reader
  }

  /// Returns reference to SchemaFields
  pub fn fields(&self) -> &SchemaFields {
    &self.fields
  }

  /// Returns the language of this index
  pub fn language(&self) -> Language {
    self.language
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tantivy::tokenizer::TextAnalyzer;
  use vibrato_rkyv::dictionary::PresetDictionaryKind;

  /// Confirm that creating a Japanese index and adding documents works correctly.
  #[test]
  fn open_or_create_japanese_and_add_documents() {
    // Build tokenizer from dictionary manager
    let manager = crate::dictionary::DictionaryManager::with_preset(PresetDictionaryKind::Ipadic)
      .expect("Failed to build DictionaryManager");

    let cache_dir = manager.cache_dir();
    if !cache_dir.join(PresetDictionaryKind::Ipadic.name()).exists() {
      eprintln!("No dictionary cache -> Skip");
      return;
    }

    let dict = manager.load().expect("Failed to load dictionary");
    let tokenizer =
      crate::tokenizer::vibrato_tokenizer::VibratoTokenizer::from_shared_dictionary(dict);
    let text_analyzer = TextAnalyzer::from(tokenizer);

    // Create index in temporary directory
    let tmp_dir = tempfile::TempDir::new().expect("Failed to create temporary directory");
    let index_manager =
      IndexManager::open_or_create(tmp_dir.path(), Language::Ja, Some(text_analyzer))
        .expect("Failed to create index");

    // Confirm it is Japanese
    assert_eq!(index_manager.language(), Language::Ja);

    // Confirm text_ngram field exists
    assert!(index_manager.fields().text_ngram.is_some());

    // Add documents
    let docs = vec![
      Document::new("1", "src-1", "東京は日本の首都です").with_tag("category:geo"),
      Document::new("2", "src-1", "大阪は西日本の中心都市です")
        .with_tag("category:geo")
        .with_tag("region:kansai"),
    ];

    let report = index_manager.add_documents(&docs).expect("Failed to add documents");
    assert_eq!(report.added, 2);
    assert_eq!(report.skipped_duplicates, 0);
  }

  /// Confirm that creating an English index and adding documents works correctly.
  #[test]
  fn open_or_create_english_and_add_documents() {
    // Create index in temporary directory
    let tmp_dir = tempfile::TempDir::new().expect("Failed to create temporary directory");
    let index_manager = IndexManager::open_or_create(tmp_dir.path(), Language::En, None)
      .expect("Failed to create index");

    // Confirm it is English
    assert_eq!(index_manager.language(), Language::En);

    // Confirm text_ngram field does not exist
    assert!(index_manager.fields().text_ngram.is_none());

    // Add documents
    let docs = vec![
      Document::new("1", "src-1", "Tokyo is the capital of Japan").with_tag("category:geo"),
      Document::new("2", "src-1", "Osaka is a major city in western Japan")
        .with_tag("category:geo")
        .with_tag("region:kansai"),
    ];

    let report = index_manager.add_documents(&docs).expect("Failed to add documents");
    assert_eq!(report.added, 2);
    assert_eq!(report.skipped_duplicates, 0);
  }

  /// Error test when tokenizer is not provided for Japanese index
  #[test]
  fn missing_japanese_tokenizer_error() {
    let tmp_dir = tempfile::TempDir::new().expect("Failed to create temporary directory");
    let result = IndexManager::open_or_create(tmp_dir.path(), Language::Ja, None);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, IndexerError::MissingJapaneseTokenizer));
  }

  /// Test duplicate skip (Japanese)
  #[test]
  fn duplicate_documents_are_skipped_japanese() {
    let manager = crate::dictionary::DictionaryManager::with_preset(PresetDictionaryKind::Ipadic)
      .expect("Failed to build DictionaryManager");

    let cache_dir = manager.cache_dir();
    if !cache_dir.join(PresetDictionaryKind::Ipadic.name()).exists() {
      eprintln!("No dictionary cache -> Skip");
      return;
    }

    let dict = manager.load().expect("Failed to load dictionary");
    let tokenizer =
      crate::tokenizer::vibrato_tokenizer::VibratoTokenizer::from_shared_dictionary(dict);
    let text_analyzer = TextAnalyzer::from(tokenizer);

    let tmp_dir = tempfile::TempDir::new().expect("Failed to create temporary directory");
    let index_manager =
      IndexManager::open_or_create(tmp_dir.path(), Language::Ja, Some(text_analyzer))
        .expect("Failed to create index");

    // First document
    let docs1 = vec![Document::new("1", "src-1", "東京は日本の首都です")];
    let report1 = index_manager.add_documents(&docs1).expect("Failed to add");
    assert_eq!(report1.added, 1);
    assert_eq!(report1.skipped_duplicates, 0);

    // Add document with same ID -> Skipped
    let docs2 = vec![Document::new("1", "src-1", "大阪は西日本の中心都市です")];
    let report2 = index_manager.add_documents(&docs2).expect("Failed to add");
    assert_eq!(report2.added, 0);
    assert_eq!(report2.skipped_duplicates, 1);
  }

  /// Test duplicate skip (English)
  #[test]
  fn duplicate_documents_are_skipped_english() {
    let tmp_dir = tempfile::TempDir::new().expect("Failed to create temporary directory");
    let index_manager = IndexManager::open_or_create(tmp_dir.path(), Language::En, None)
      .expect("Failed to create index");

    // First document
    let docs1 = vec![Document::new("1", "src-1", "Tokyo is the capital of Japan")];
    let report1 = index_manager.add_documents(&docs1).expect("Failed to add");
    assert_eq!(report1.added, 1);
    assert_eq!(report1.skipped_duplicates, 0);

    // Add document with same ID -> Skipped
    let docs2 = vec![Document::new("1", "src-1", "Osaka is a major city")];
    let report2 = index_manager.add_documents(&docs2).expect("Failed to add");
    assert_eq!(report2.added, 0);
    assert_eq!(report2.skipped_duplicates, 1);
  }
}
