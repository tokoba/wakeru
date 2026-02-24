//! Tantivy Schema Builder
//!
//! Defines Tantivy index schema for RAG pipeline.
//! Automatically selects appropriate tokenizer for each language.

use tantivy::schema::{
  Field, IndexRecordOption, JsonObjectOptions, STORED, STRING, Schema, TextFieldIndexing,
  TextOptions,
};

use crate::config::Language;

/// Structure holding references to schema fields.
///
/// Since `Schema::get_field()` in Tantivy is string-based search,
/// there is a risk of typo in field names. This structure provides
/// type-safe field references.
#[derive(Clone, Copy, Debug)]
pub struct SchemaFields {
  /// Chunk ID (STRING + STORED) - For exact match
  pub id: Field,
  /// Source Document ID (STRING + STORED)
  pub source_id: Field,
  /// Body field (TEXT + STORED, language-specific tokenizer)
  pub text: Field,
  /// Structured metadata (JsonObject, STORED + INDEXED, raw tokenizer)
  /// Tag filtering etc. is possible
  pub metadata: Field,
  /// Field for 1-char N-gram (TEXT, ja_ngram tokenizer)
  /// For partial match search with 1-char query
  /// Used only in Japanese, None in English
  /// Option because it may not exist in existing indices
  pub text_ngram: Option<Field>,
}

impl SchemaFields {
  /// Reconstructs SchemaFields from an existing schema.
  ///
  /// Used when opening an existing index. Retrieves fields from the schema on disk
  /// and constructs SchemaFields structure.
  ///
  /// # Arguments
  /// - `schema`: Tantivy schema
  ///
  /// # Returns
  /// - `Ok(SchemaFields)`: Field retrieval successful
  /// - `Err(tantivy::TantivyError)`: Required field not found
  ///
  /// # Error conditions
  /// - One of `id`, `source_id`, `text`, `metadata` is not found
  pub fn from_schema(schema: &Schema) -> Result<Self, tantivy::TantivyError> {
    let id = schema.get_field("id").map_err(|e| {
      tantivy::TantivyError::InvalidArgument(format!("Field 'id' not found: {e}"))
    })?;
    let source_id = schema.get_field("source_id").map_err(|e| {
      tantivy::TantivyError::InvalidArgument(format!("Field 'source_id' not found: {e}"))
    })?;
    let text = schema.get_field("text").map_err(|e| {
      tantivy::TantivyError::InvalidArgument(format!("Field 'text' not found: {e}"))
    })?;
    let metadata = schema.get_field("metadata").map_err(|e| {
      tantivy::TantivyError::InvalidArgument(format!("Field 'metadata' not found: {e}"))
    })?;

    // N-gram field is only for Japanese index, or may not exist in old index
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

/// Builds Tantivy schema.
///
/// # Field Configuration
///
/// - `id`: Chunk ID (STRING + STORED) For exact match
/// - `source_id`: Source Document ID (STRING + STORED)
/// - `text`: Body (TEXT + STORED, language-specific tokenizer)
/// - `metadata`: Structured metadata (JsonObject, STORED + INDEXED, raw tokenizer)
/// - `text_ngram`: For 1-char N-gram (TEXT, ja_ngram tokenizer) - Japanese only
///
/// # Tokenizer Settings (Language dependent)
///
/// - Japanese (`Language::Ja`):
///   - `lang_ja` tokenizer for `text` field
///   - `ja_ngram` tokenizer for `text_ngram` field
/// - English (`Language::En`):
///   - `lang_en` tokenizer for `text` field (SimpleTokenizer + LowerCaser)
///   - `text_ngram` field is not created
///
/// Tokenizers must be registered when creating `IndexManager`.
///
/// # Reason for selecting IndexRecordOption
///
/// `WithFreqsAndPositions` is selected:
/// - Term frequency (Freqs) is required for BM25 score calculation
/// - Position information (Positions) is required for phrase search
/// - Position information is also used for highlighting
///
/// # Metadata field design
///
/// `metadata` is JsonObject type and has the following characteristics:
/// - STORED: Restorable in search results
/// - INDEXED (raw tokenizer): Filtering search is possible in `metadata.tags:value` format
/// - raw tokenizer does not tokenize, so it fits exact match search
///
/// # Examples
///
/// ```no_run
/// use wakeru::indexer::schema_builder::build_schema;
/// use wakeru::Language;
///
/// let (schema, fields) = build_schema(Language::Ja);
/// // Pass schema to Index::create_in_dir
/// // Use fields in IndexManager or SearchEngine
/// ```
pub fn build_schema(language: Language) -> (Schema, SchemaFields) {
  let mut builder = Schema::builder();

  // ID field: Exact match search + Stored
  let id = builder.add_text_field("id", STRING | STORED);

  // Source document ID
  let source_id = builder.add_text_field("source_id", STRING | STORED);

  // Body field: Language-specific tokenizer + Record frequency and position
  let text_indexing = TextFieldIndexing::default()
    .set_tokenizer(language.text_tokenizer_name())
    .set_index_option(IndexRecordOption::WithFreqsAndPositions);
  let text_options = TextOptions::default().set_indexing_options(text_indexing).set_stored();
  let text = builder.add_text_field("text", text_options);

  // Metadata field: JsonObject (Filterable search possible)
  // Enable exact match search with raw tokenizer
  // Tantivy 0.25: JsonObjectOptions::set_indexing_options accepts TextFieldIndexing
  let json_indexing =
    TextFieldIndexing::default().set_tokenizer("raw").set_index_option(IndexRecordOption::Basic);
  let metadata_options =
    JsonObjectOptions::default().set_stored().set_indexing_options(json_indexing);
  let metadata = builder.add_json_field("metadata", metadata_options);

  // 1-char N-gram field: Created only for Japanese
  // None for English
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
