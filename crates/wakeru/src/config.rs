// crates/wakeru/src/config.rs

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use vibrato_rkyv::dictionary::PresetDictionaryKind;

use crate::errors::ConfigError;

/// Supported language types.
///
/// In the multi-language index strategy (Plan B), an independent index is created for each language.
/// A tokenizer suitable for each language is automatically selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
  /// Japanese (Morphological Analysis: VibratoTokenizer)
  Ja,
  /// English (Space separated: SimpleTokenizer + LowerCaser)
  En,
}

impl Language {
  /// Returns the language code (used for index directory names).
  ///
  /// # Examples
  /// - `Language::Ja` → `"ja"`
  /// - `Language::En` → `"en"`
  pub fn code(&self) -> &'static str {
    match self {
      Language::Ja => "ja",
      Language::En => "en",
    }
  }

  /// Returns the tokenizer name to be used for text fields.
  ///
  /// - Japanese: `"lang_ja"` (VibratoTokenizer)
  /// - English: `"lang_en"` (SimpleTokenizer + LowerCaser)
  pub fn text_tokenizer_name(&self) -> &'static str {
    match self {
      Language::Ja => "lang_ja",
      Language::En => "lang_en",
    }
  }

  /// Returns the N-gram tokenizer name (Japanese only).
  ///
  /// - Japanese: `Some("ja_ngram")` (For single character search)
  /// - English: `None` (No N-gram field)
  pub fn ngram_tokenizer_name(&self) -> Option<&'static str> {
    match self {
      Language::Ja => Some("ja_ngram"),
      Language::En => None,
    }
  }
}

impl std::fmt::Display for Language {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.code())
  }
}

/// Top-level configuration for wakeru.
#[derive(Debug, Clone, Deserialize)]
pub struct WakeruConfig {
  /// [dictionary] section
  pub dictionary: DictionaryConfig,
  /// [index] section
  pub index: IndexConfig,
  /// [search] section
  pub search: SearchConfig,
  /// [logging] section
  pub logging: LoggingConfig,
}

/// [dictionary] section configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct DictionaryConfig {
  /// Preset dictionary type: "ipadic" | "unidic-cwj" | "unidic-csj"
  pub preset: DictionaryPreset,
  /// Dictionary cache directory.
  ///
  /// If omitted in TOML, it becomes `None`, and the actual default is assumed to be determined by `DictionaryManager`.
  #[serde(default)]
  pub cache_dir: Option<PathBuf>,
}

/// Preset dictionary type.
///
/// ## Design Background
///
/// This project uses the `PresetDictionaryKind` type provided by the morphological analysis engine [vibrato-rkyv]
/// to specify dictionaries. However, since this external type does not implement the `serde::Deserialize` trait,
/// it cannot be directly deserialized from a TOML configuration file.
///
/// ## Reason for this type's existence
///
/// `DictionaryPreset` is a newly defined enum for the purpose of improving convenience to realize loading from TOML configuration files.
/// Since it has `#[derive(Deserialize)]`, it can be used directly as the `[dictionary].preset` field in the configuration file.
///
/// ## Reason why integration with PresetDictionaryKind is not possible
///
/// Since `PresetDictionaryKind` is a type of an external crate (vibrato-rkyv),
/// we cannot add a `Deserialize` implementation on this project's side
/// (forbidden by Rust's orphan rule).
///
/// Therefore, we adopt a design where `DictionaryPreset` is defined as a type for configuration files,
/// and it is converted to `PresetDictionaryKind` in internal processing.
///
/// ## Conversion method
///
/// Interoperability is possible with the `.into()` method via the `From<DictionaryPreset> for PresetDictionaryKind` trait implementation.
///
/// [vibrato-rkyv]: https://crates.io/crates/vibrato-rkyv
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DictionaryPreset {
  /// IpaDic: The smallest
  Ipadic,
  /// Unidic for written language
  UnidicCwj,
  /// Unidic for spoken language
  UnidicCsj,
}

/// [index] section configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct IndexConfig {
  /// Index storage directory (e.g., "/opt/wakeru/data/index")
  pub data_dir: PathBuf,
  /// Memory buffer size for IndexWriter (bytes)
  pub writer_memory_bytes: usize,
  /// Batch commit size
  pub batch_commit_size: usize,
  /// List of supported languages (e.g., ["ja", "en"])
  #[serde(default = "default_languages")]
  pub languages: Vec<Language>,
  /// Default language (must be included in `languages`)
  #[serde(default = "default_language")]
  pub default_language: Language,
}

/// Default language list (Japanese only)
fn default_languages() -> Vec<Language> {
  vec![Language::Ja]
}

/// Default language (Japanese)
fn default_language() -> Language {
  Language::Ja
}

/// [search] section configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchConfig {
  /// Default search result limit
  pub default_limit: usize,
  /// Maximum search result limit
  pub max_limit: usize,
}

/// [logging] section configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
  /// Log level: "trace" | "debug" | "info" | "warn" | "error"
  pub level: LogLevel,
}

/// Log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
  /// trace
  Trace,

  /// debug
  Debug,

  /// info
  Info,

  /// warn
  Warn,

  ///error
  Error,
}

// ===== Accessor Methods =====

impl WakeruConfig {
  /// Returns the preset dictionary type to pass to DictionaryManager.
  ///
  /// Corresponds to:
  /// ```rust,ignore
  /// let dictionary_manager = DictionaryManager::with_preset(
  ///     config.dictionary_preset(),
  /// )?;
  /// ```
  /// in the design document.
  pub fn dictionary_preset(&self) -> PresetDictionaryKind {
    self.dictionary.preset.into()
  }

  /// Returns the configured dictionary cache directory.
  ///
  /// `None` if unspecified in TOML.
  /// The actual path determination is assumed to be done on the DictionaryManager side.
  pub fn dictionary_cache_dir(&self) -> Option<&Path> {
    self.dictionary.cache_dir.as_deref()
  }

  /// Returns the base directory of the index.
  ///
  /// e.g., "/opt/wakeru/data/index"
  pub fn index_base_dir(&self) -> &Path {
    &self.index.data_dir
  }

  /// Returns the index directory for the specified language.
  ///
  /// Directory structure:
  /// ```text
  /// data/index/
  ///   ├── ja/
  ///   │   ├── meta.json
  ///   │   └── ...
  ///   └── en/
  ///       ├── meta.json
  ///       └── ...
  /// ```
  ///
  /// # Examples
  /// ```ignore
  /// let ja_path = config.index_path_for_language(Language::Ja);
  /// // → "/opt/wakeru/data/index/ja"
  /// ```
  pub fn index_path_for_language(&self, language: Language) -> PathBuf {
    self.index.data_dir.join(language.code())
  }

  /// Returns the index directory for the default collection.
  ///
  /// Based on the design document's directory structure:
  ///   data/index/
  ///     ├── default/
  ///     └── {collection_id}/
  ///
  /// and `WakeruService::init`:
  /// ```rust,ignore
  /// let index_manager = if config.index_path().exists() {
  ///     IndexManager::open(config.index_path(), tokenizer)?
  /// } else {
  ///     IndexManager::create(config.index_path(), tokenizer)?
  /// };
  /// ```
  /// Returns `<data_dir>/default`.
  #[deprecated(note = "Use index_path_for_language() for multi-language support")]
  pub fn index_path(&self) -> PathBuf {
    self.index.data_dir.join("default")
  }

  /// Returns the memory buffer size (bytes) for IndexWriter.
  pub fn writer_memory_bytes(&self) -> usize {
    self.index.writer_memory_bytes
  }

  /// Returns the batch commit size.
  pub fn batch_commit_size(&self) -> usize {
    self.index.batch_commit_size
  }

  /// Returns the list of supported languages.
  pub fn supported_languages(&self) -> &[Language] {
    &self.index.languages
  }

  /// Returns the default language.
  pub fn default_language(&self) -> Language {
    self.index.default_language
  }

  /// Validates the configuration.
  ///
  /// # Validation Items
  /// - `languages` is not empty
  /// - `default_language` is included in `languages`
  /// - `search.default_limit` >= 1
  /// - `search.max_limit` >= `search.default_limit`
  /// - `index.writer_memory_bytes` is within allowable range (1MB - 1GB)
  /// - `index.batch_commit_size` >= 1
  /// - `dictionary.cache_dir` exists or can be created
  ///
  /// # Errors
  /// Returns the corresponding `ConfigError` if validation fails.
  pub fn validate(&self) -> Result<(), ConfigError> {
    // languages is not empty
    if self.index.languages.is_empty() {
      return Err(ConfigError::EmptyLanguages);
    }

    // default_language is included in languages
    if !self.index.languages.contains(&self.index.default_language) {
      return Err(ConfigError::DefaultLanguageNotInLanguages {
        default_language: self.index.default_language,
      });
    }

    // search.default_limit >= 1
    if self.search.default_limit < 1 {
      return Err(ConfigError::InvalidSearchDefaultLimit {
        actual: self.search.default_limit,
      });
    }

    // search.max_limit >= search.default_limit
    if self.search.max_limit < self.search.default_limit {
      return Err(ConfigError::InvalidSearchMaxLimit {
        default_limit: self.search.default_limit,
        max_limit: self.search.max_limit,
      });
    }

    // index.writer_memory_bytes is within allowable range (1MB - 1GB)
    const MIN_WRITER_MEMORY: u64 = 1_000_000; // 1MB
    const MAX_WRITER_MEMORY: u64 = 1_000_000_000; // 1GB
    let writer_memory = self.index.writer_memory_bytes as u64;
    if !(MIN_WRITER_MEMORY..=MAX_WRITER_MEMORY).contains(&writer_memory) {
      return Err(ConfigError::InvalidWriterMemoryBytes {
        min: MIN_WRITER_MEMORY,
        max: MAX_WRITER_MEMORY,
        actual: writer_memory,
      });
    }

    // index.batch_commit_size >= 1
    if self.index.batch_commit_size < 1 {
      return Err(ConfigError::InvalidBatchCommitSize {
        actual: self.index.batch_commit_size,
      });
    }

    // dictionary.cache_dir exists or can be created
    if let Some(cache_dir) = &self.dictionary.cache_dir {
      if cache_dir.exists() {
        // If it exists, check that it is a directory
        if !cache_dir.is_dir() {
          return Err(ConfigError::InvalidDictionaryCacheDir {
            path: cache_dir.clone(),
          });
        }
      } else {
        // If it does not exist, check if it can be created
        if let Err(e) = std::fs::create_dir_all(cache_dir) {
          return Err(ConfigError::DictionaryCacheDirCreationFailed {
            path: cache_dir.clone(),
            source: Arc::new(e),
          });
        }
      }
    }

    Ok(())
  }

  /// Returns the default search result limit.
  pub fn default_search_limit(&self) -> usize {
    self.search.default_limit
  }

  /// Returns the maximum search result limit.
  pub fn max_search_limit(&self) -> usize {
    self.search.max_limit
  }

  /// Returns the log level.
  pub fn log_level(&self) -> LogLevel {
    self.logging.level
  }
}

// ===== Convert library types to types usable in this crate (with some traits added) =====
//
// Implements conversion from DictionaryPreset (for configuration file) -> PresetDictionaryKind (for vibrato-rkyv).
//
// See `DictionaryPreset` doc comments for why this conversion is necessary.

impl From<DictionaryPreset> for PresetDictionaryKind {
  fn from(preset: DictionaryPreset) -> Self {
    match preset {
      DictionaryPreset::Ipadic => PresetDictionaryKind::Ipadic,
      DictionaryPreset::UnidicCwj => PresetDictionaryKind::UnidicCwj,
      DictionaryPreset::UnidicCsj => PresetDictionaryKind::UnidicCsj,
    }
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test Module
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;
  use tempfile::TempDir;

  // ─── Test Helpers ─────────────────────────────────────────────────────

  /// Creates a base valid configuration (uses a temporary directory for each test)
  fn create_valid_config(temp_dir: &TempDir) -> WakeruConfig {
    WakeruConfig {
      dictionary: DictionaryConfig {
        preset: DictionaryPreset::Ipadic,
        cache_dir: Some(temp_dir.path().join("dict")),
      },
      index: IndexConfig {
        data_dir: temp_dir.path().join("index"),
        writer_memory_bytes: 50_000_000,
        batch_commit_size: 1_000,
        languages: vec![Language::Ja, Language::En],
        default_language: Language::Ja,
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

  // ─── Language Tests ────────────────────────────────────────────────────

  #[test]
  fn language_code_returns_correct_value() {
    assert_eq!(Language::Ja.code(), "ja");
    assert_eq!(Language::En.code(), "en");
  }

  #[test]
  fn language_text_tokenizer_name() {
    assert_eq!(Language::Ja.text_tokenizer_name(), "lang_ja");
    assert_eq!(Language::En.text_tokenizer_name(), "lang_en");
  }

  #[test]
  fn language_ngram_tokenizer_name() {
    assert_eq!(Language::Ja.ngram_tokenizer_name(), Some("ja_ngram"));
    assert_eq!(Language::En.ngram_tokenizer_name(), None);
  }

  #[test]
  fn language_display() {
    assert_eq!(format!("{}", Language::Ja), "ja");
    assert_eq!(format!("{}", Language::En), "en");
  }

  // ─── validate() Normal Case Tests ────────────────────────────────────────────

  #[test]
  fn validate_accepts_valid_config() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_valid_config(&temp_dir);

    let result = config.validate();
    assert!(result.is_ok(), "valid config should pass validation");
  }

  #[test]
  fn validate_accepts_min_writer_memory() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.index.writer_memory_bytes = 1_000_000; // 1MB (minimum)

    let result = config.validate();
    assert!(result.is_ok());
  }

  #[test]
  fn validate_accepts_max_writer_memory() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.index.writer_memory_bytes = 1_000_000_000; // 1GB (maximum)

    let result = config.validate();
    assert!(result.is_ok());
  }

  #[test]
  fn validate_accepts_min_batch_commit_size() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.index.batch_commit_size = 1;

    let result = config.validate();
    assert!(result.is_ok());
  }

  #[test]
  fn validate_accepts_default_limit_equals_max_limit() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.search.default_limit = 50;
    config.search.max_limit = 50; // equal is ok

    let result = config.validate();
    assert!(result.is_ok());
  }

  #[test]
  fn validate_accepts_single_language() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.index.languages = vec![Language::En];
    config.index.default_language = Language::En;

    let result = config.validate();
    assert!(result.is_ok());
  }

  // ─── validate() languages Abnormal Cases ───────────────────────────────────────────

  #[test]
  fn validate_rejects_empty_languages() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.index.languages.clear();

    let err = config.validate().unwrap_err();
    assert!(matches!(err, ConfigError::EmptyLanguages));
  }

  #[test]
  fn validate_rejects_default_language_not_in_languages() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.index.languages = vec![Language::En];
    config.index.default_language = Language::Ja; // not in languages

    let err = config.validate().unwrap_err();
    match err {
      ConfigError::DefaultLanguageNotInLanguages { default_language } => {
        assert_eq!(default_language, Language::Ja);
      }
      _ => panic!("expected DefaultLanguageNotInLanguages error"),
    }
  }

  // ─── validate() search Abnormal Cases ──────────────────────────────────────────────

  #[test]
  fn validate_rejects_default_limit_zero() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.search.default_limit = 0;

    let err = config.validate().unwrap_err();
    match err {
      ConfigError::InvalidSearchDefaultLimit { actual } => {
        assert_eq!(actual, 0);
      }
      _ => panic!("expected InvalidSearchDefaultLimit error"),
    }
  }

  #[test]
  fn validate_rejects_max_limit_less_than_default() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.search.default_limit = 50;
    config.search.max_limit = 10; // less than default

    let err = config.validate().unwrap_err();
    match err {
      ConfigError::InvalidSearchMaxLimit {
        default_limit,
        max_limit,
      } => {
        assert_eq!(default_limit, 50);
        assert_eq!(max_limit, 10);
      }
      _ => panic!("expected InvalidSearchMaxLimit error"),
    }
  }

  // ─── validate() index Abnormal Cases ───────────────────────────────────────────────

  #[test]
  fn validate_rejects_writer_memory_too_small() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.index.writer_memory_bytes = 999_999; // less than 1MB

    let err = config.validate().unwrap_err();
    match err {
      ConfigError::InvalidWriterMemoryBytes { min, actual, .. } => {
        assert_eq!(min, 1_000_000);
        assert_eq!(actual, 999_999);
      }
      _ => panic!("expected InvalidWriterMemoryBytes error"),
    }
  }

  #[test]
  fn validate_rejects_writer_memory_too_large() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.index.writer_memory_bytes = 1_000_000_001; // more than 1GB

    let err = config.validate().unwrap_err();
    match err {
      ConfigError::InvalidWriterMemoryBytes { max, actual, .. } => {
        assert_eq!(max, 1_000_000_000);
        assert_eq!(actual, 1_000_000_001);
      }
      _ => panic!("expected InvalidWriterMemoryBytes error"),
    }
  }

  #[test]
  fn validate_rejects_batch_commit_size_zero() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.index.batch_commit_size = 0;

    let err = config.validate().unwrap_err();
    match err {
      ConfigError::InvalidBatchCommitSize { actual } => {
        assert_eq!(actual, 0);
      }
      _ => panic!("expected InvalidBatchCommitSize error"),
    }
  }

  // ─── validate() dictionary.cache_dir Tests ───────────────────────────────

  #[test]
  fn validate_accepts_none_cache_dir() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.dictionary.cache_dir = None;

    let result = config.validate();
    assert!(result.is_ok());
  }

  #[test]
  fn validate_accepts_existing_directory() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("existing-cache");
    fs::create_dir(&cache_dir).unwrap();

    let mut config = create_valid_config(&temp_dir);
    config.dictionary.cache_dir = Some(cache_dir);

    let result = config.validate();
    assert!(result.is_ok());
  }

  #[test]
  fn validate_creates_missing_cache_dir() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("new-cache-dir");

    // Ensure it doesn't exist
    assert!(!cache_dir.exists());

    let mut config = create_valid_config(&temp_dir);
    config.dictionary.cache_dir = Some(cache_dir.clone());

    let result = config.validate();
    assert!(result.is_ok());

    // Check that directory was created
    assert!(cache_dir.exists() && cache_dir.is_dir());
  }

  #[test]
  fn validate_rejects_cache_dir_is_file() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("not-a-dir");
    fs::write(&file_path, b"dummy").unwrap();

    let mut config = create_valid_config(&temp_dir);
    config.dictionary.cache_dir = Some(file_path.clone());

    let err = config.validate().unwrap_err();
    match err {
      ConfigError::InvalidDictionaryCacheDir { path } => {
        assert_eq!(path, file_path);
      }
      _ => panic!("expected InvalidDictionaryCacheDir error"),
    }
  }

  #[test]
  fn validate_rejects_cache_dir_creation_fails() {
    let temp_dir = TempDir::new().unwrap();
    // make parent a file
    let parent_file = temp_dir.path().join("parent_file");
    fs::write(&parent_file, b"dummy").unwrap();

    // trying to create a dir under a file should fail
    let invalid_cache_dir = parent_file.join("child_dir");

    let mut config = create_valid_config(&temp_dir);
    config.dictionary.cache_dir = Some(invalid_cache_dir.clone());

    let err = config.validate().unwrap_err();
    match err {
      ConfigError::DictionaryCacheDirCreationFailed { path, .. } => {
        assert_eq!(path, invalid_cache_dir);
      }
      _ => panic!("expected DictionaryCacheDirCreationFailed error"),
    }
  }

  // ─── Error Priority Tests ────────────────────────────────────────────────

  #[test]
  fn validate_reports_empty_languages_first() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.index.languages.clear(); // First error
    config.search.default_limit = 0; // Second error candidate

    let err = config.validate().unwrap_err();
    // EmptyLanguages reported first
    assert!(matches!(err, ConfigError::EmptyLanguages));
  }

  #[test]
  fn validate_reports_default_language_before_search() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.index.languages = vec![Language::En];
    config.index.default_language = Language::Ja; // First error
    config.search.default_limit = 0; // Second error candidate

    let err = config.validate().unwrap_err();
    assert!(matches!(
      err,
      ConfigError::DefaultLanguageNotInLanguages { .. }
    ));
  }

  // ─── Accessor Method Tests ───────────────────────────────────────────────

  #[test]
  fn index_path_for_language_returns_correct_path() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_valid_config(&temp_dir);

    let ja_path = config.index_path_for_language(Language::Ja);
    let en_path = config.index_path_for_language(Language::En);

    assert!(ja_path.ends_with("ja"));
    assert!(en_path.ends_with("en"));
  }

  #[test]
  fn supported_languages_returns_languages() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_valid_config(&temp_dir);

    let langs = config.supported_languages();
    assert_eq!(langs, &[Language::Ja, Language::En]);
  }

  #[test]
  fn default_language_returns_default() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_valid_config(&temp_dir);

    assert_eq!(config.default_language(), Language::Ja);
  }

  #[test]
  fn dictionary_preset_returns_correct_kind() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_valid_config(&temp_dir);

    let kind: PresetDictionaryKind = config.dictionary_preset();
    assert_eq!(kind, PresetDictionaryKind::Ipadic);
  }

  #[test]
  fn writer_memory_bytes_returns_value() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_valid_config(&temp_dir);

    assert_eq!(config.writer_memory_bytes(), 50_000_000);
  }

  #[test]
  fn batch_commit_size_returns_value() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_valid_config(&temp_dir);

    assert_eq!(config.batch_commit_size(), 1_000);
  }

  #[test]
  fn default_search_limit_returns_value() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_valid_config(&temp_dir);

    assert_eq!(config.default_search_limit(), 10);
  }

  #[test]
  fn max_search_limit_returns_value() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_valid_config(&temp_dir);

    assert_eq!(config.max_search_limit(), 100);
  }

  #[test]
  fn log_level_returns_value() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_valid_config(&temp_dir);

    assert_eq!(config.log_level(), LogLevel::Info);
  }

  // ─── DictionaryPreset Tests ─────────────────────────────────────────────

  #[test]
  fn dictionary_preset_converts_to_preset_kind() {
    assert_eq!(
      PresetDictionaryKind::from(DictionaryPreset::Ipadic),
      PresetDictionaryKind::Ipadic
    );
    assert_eq!(
      PresetDictionaryKind::from(DictionaryPreset::UnidicCwj),
      PresetDictionaryKind::UnidicCwj
    );
    assert_eq!(
      PresetDictionaryKind::from(DictionaryPreset::UnidicCsj),
      PresetDictionaryKind::UnidicCsj
    );
  }

  // ─── Multiple Error Combination Tests ──────────────────────────────────────────

  #[test]
  fn validate_with_multiple_errors_reports_first() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);

    // Set multiple error conditions
    config.index.languages.clear(); // EmptyLanguages
    config.search.default_limit = 0; // InvalidSearchDefaultLimit
    config.search.max_limit = 0; // InvalidSearchMaxLimit
    config.index.writer_memory_bytes = 0; // InvalidWriterMemoryBytes

    let err = config.validate().unwrap_err();
    // Fails at the first check
    assert!(matches!(err, ConfigError::EmptyLanguages));
  }
}
