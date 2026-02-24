//! Error Definitions

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use vibrato_rkyv::dictionary::PresetDictionaryKind;

use crate::config::Language;

/// Configuration file (WakeruConfig) related errors
#[derive(Debug, Error, Clone)]
#[non_exhaustive]
pub enum ConfigError {
  /// index.languages is empty
  #[error("Please specify at least one language in languages")]
  EmptyLanguages,

  /// index.default_language is not included in index.languages
  #[error("default_language ({default_language}) must be included in languages")]
  DefaultLanguageNotInLanguages {
    /// Specified default_language
    default_language: Language,
  },

  /// search.default_limit < 1
  #[error("search.default_limit must be 1 or greater: actual={actual}")]
  InvalidSearchDefaultLimit {
    /// Actually specified value
    actual: usize,
  },

  /// search.max_limit < search.default_limit
  #[error(
    "search.max_limit must be greater than or equal to search.default_limit: \
     default_limit={default_limit}, max_limit={max_limit}"
  )]
  InvalidSearchMaxLimit {
    /// search.default_limit
    default_limit: usize,
    /// search.max_limit
    max_limit: usize,
  },

  /// index.writer_memory_bytes is out of range
  #[error(
    "index.writer_memory_bytes must be in the range of {min} to {max} bytes: actual={actual}"
  )]
  InvalidWriterMemoryBytes {
    /// Minimum allowed value (bytes)
    min: u64,
    /// Maximum allowed value (bytes)
    max: u64,
    /// Actually specified value (bytes)
    actual: u64,
  },

  /// index.batch_commit_size < 1
  #[error("index.batch_commit_size must be 1 or greater: actual={actual}")]
  InvalidBatchCommitSize {
    /// Actually specified value
    actual: usize,
  },

  /// dictionary.cache_dir is not an "existing directory" (e.g. it is a file)
  #[error("dictionary.cache_dir is not a directory: path={path:?}")]
  InvalidDictionaryCacheDir {
    /// Invalid path
    path: PathBuf,
  },

  /// Failed to create dictionary.cache_dir
  #[error("Failed to create dictionary.cache_dir: path={path:?}, error={source}")]
  DictionaryCacheDirCreationFailed {
    /// Path attempted to create
    path: PathBuf,
    /// Original IO error
    #[source]
    source: Arc<io::Error>,
  },
}

/// Dictionary related errors
/// Vibrato can use dictionaries such as mecab, ipadic, unidic
/// Define these errors
#[derive(Debug, Error, Clone)]
#[non_exhaustive]
pub enum DictionaryError {
  /// Cache directory not found
  #[error("Dictionary cache directory not found")]
  CacheDirNotFound,

  /// Failed to create cache directory
  #[error("Failed to create dictionary cache directory: {0}")]
  CacheDirCreationFailed(Arc<io::Error>),

  /// Specified dictionary not found
  #[error("Specified dictionary not found: {0}")]
  DictionaryNotFound(String),

  /// Dictionary download failed (URL, IO error, etc.)
  #[error("Failed to download dictionary: {0}")]
  DownloadFailed(String),

  /// Dictionary validation failed (Hash mismatch, etc.)
  #[error("Dictionary validation failed: {0}")]
  ValidationFailed(String),

  /// Invalid dictionary path
  #[error("Invalid dictionary path: {0}")]
  InvalidPath(PathBuf),

  /// Invalid dictionary path or invalid dictionary type
  #[error("Invalid dictionary path or invalid dictionary type: path={0}, preset_kind={1:?}")]
  InvalidPathOrInvalidPresetKind(PathBuf, Option<PresetDictionaryKind>),

  /// Failed to load dictionary by vibrato-rkyv
  #[error("vibrato-rkyv dictionary load error: {0}")]
  VibratoLoad(Arc<dyn std::error::Error + Send + Sync + 'static>),

  /// Failed to download preset dictionary by vibrato-rkyv
  #[error("vibrato-rkyv preset dictionary download failed: {0}")]
  PresetDictDownloadFailed(Arc<dyn std::error::Error + Send + Sync + 'static>),
}

/// Tokenizer related errors
#[derive(Debug, Error, Clone)]
#[non_exhaustive]
pub enum TokenizerError {
  /// Dictionary caused error
  #[error("Dictionary error: {0}")]
  Dictionary(#[from] DictionaryError),

  /// Invalid input text
  #[error("Invalid input text for tokenization: {reason}")]
  InvalidInput {
    /// Reason for invalidity
    reason: String,
  },
}

/// Indexer related errors
#[derive(Debug, Error, Clone)]
#[non_exhaustive]
pub enum IndexerError {
  /// Tokenizer caused error
  #[error("Tokenizer error: {0}")]
  Tokenizer(#[from] TokenizerError),

  /// Tantivy index operation error
  #[error("Tantivy index error: {0}")]
  Tantivy(#[from] tantivy::TantivyError),

  /// Invalid index path, or failed to create directory
  #[error("Invalid index path: {path}: {source}")]
  InvalidIndexPath {
    /// Path where the problem occurred
    path: PathBuf,
    /// IO error occurred
    #[source]
    source: Arc<io::Error>,
  },

  /// Index already exists (when CreateNew mode found existing index)
  #[error("Index already exists: {0}")]
  IndexAlreadyExists(PathBuf),

  /// Index not found (when OpenExisting mode found no index)
  #[error("Index not found: {0}")]
  IndexNotFound(PathBuf),

  /// Japanese tokenizer is not provided
  #[error("VibratoTokenizer is required for Japanese index")]
  MissingJapaneseTokenizer,

  /// Mismatch between schema and language
  #[error("Schema and language mismatch: expected={expected}, actual={actual}")]
  LanguageSchemaMismatch {
    /// Expected tokenizer name
    expected: String,
    /// Actual tokenizer name
    actual: String,
  },

  /// Metadata JSON serialization failed
  #[error("Failed to serialize metadata: doc_id={doc_id}, error={source}")]
  MetadataSerialize {
    /// Target document ID
    doc_id: String,
    /// Original JSON error
    #[source]
    source: Arc<serde_json::Error>,
  },
}

/// Search related errors
#[derive(Debug, Error, Clone)]
#[non_exhaustive]
pub enum SearcherError {
  /// Tantivy search processing error
  #[error("Tantivy search error: {0}")]
  Tantivy(#[from] tantivy::TantivyError),

  /// Failed to parse query
  #[error("Query parse error: {reason}")]
  InvalidQuery {
    /// Reason for query invalidity
    reason: String,
  },

  /// State where index cannot be used for search, such as schema inconsistency
  #[error("Invalid index: field={field}, reason={reason}")]
  InvalidIndex {
    /// Field name where the problem occurred
    field: String,
    /// Reason for inconsistency
    reason: String,
  },

  /// Metadata JSON deserialization failed
  #[error("Failed to deserialize metadata: doc_id={doc_id}, error={source}")]
  MetadataDeserialize {
    /// Target document ID
    doc_id: String,
    /// Original JSON error
    #[source]
    source: Arc<serde_json::Error>,
  },
}

/// Unified error
/// API exposed to the outside of this crate should return this error
/// Use as `WakeruResult<T>` = `Result<T, WakeruError>`
#[derive(Debug, Error, Clone)]
#[non_exhaustive]
pub enum WakeruError {
  /// Dictionary related error
  #[error(transparent)]
  Dictionary(#[from] DictionaryError),

  /// Tokenizer related error
  #[error(transparent)]
  Tokenizer(#[from] TokenizerError),

  /// Indexer related error
  #[error(transparent)]
  Indexer(#[from] IndexerError),

  /// Search related error
  #[error(transparent)]
  Searcher(#[from] SearcherError),

  /// Unsupported language
  #[error("Unsupported language: {language}")]
  UnsupportedLanguage {
    /// Specified language
    language: Language,
  },

  /// Configuration error
  #[error(transparent)]
  Config(#[from] ConfigError),
}

/// Standard Result type alias for wakeru crate
pub type WakeruResult<T> = Result<T, WakeruError>;
