// crates/wakeru/src/config.rs

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use vibrato_rkyv::dictionary::PresetDictionaryKind;

use crate::errors::ConfigError;

/// サポートする言語種別。
///
/// 多言語インデックス方式（B案）では、言語ごとに独立したインデックスを作成する。
/// 各言語に適したトークナイザーが自動的に選択される。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
  /// 日本語（形態素解析: VibratoTokenizer）
  Ja,
  /// 英語（スペース区切り: SimpleTokenizer + LowerCaser）
  En,
}

impl Language {
  /// 言語コードを返す（インデックスディレクトリ名に使用）。
  ///
  /// # 例
  /// - `Language::Ja` → `"ja"`
  /// - `Language::En` → `"en"`
  pub fn code(&self) -> &'static str {
    match self {
      Language::Ja => "ja",
      Language::En => "en",
    }
  }

  /// テキストフィールドで使用するトークナイザー名を返す。
  ///
  /// - 日本語: `"lang_ja"`（VibratoTokenizer）
  /// - 英語: `"lang_en"`（SimpleTokenizer + LowerCaser）
  pub fn text_tokenizer_name(&self) -> &'static str {
    match self {
      Language::Ja => "lang_ja",
      Language::En => "lang_en",
    }
  }

  /// N-gram トークナイザー名を返す（日本語のみ）。
  ///
  /// - 日本語: `Some("ja_ngram")`（1文字検索用）
  /// - 英語: `None`（N-gram フィールドなし）
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

/// wakeruのトップレベル設定。
#[derive(Debug, Clone, Deserialize)]
pub struct WakeruConfig {
  /// [dictionary] セクション
  pub dictionary: DictionaryConfig,
  /// [index] セクション
  pub index: IndexConfig,
  /// [search] セクション
  pub search: SearchConfig,
  /// [logging] セクション
  pub logging: LoggingConfig,
}

/// [dictionary] セクション設定。
#[derive(Debug, Clone, Deserialize)]
pub struct DictionaryConfig {
  /// プリセット辞書種別: "ipadic" | "unidic-cwj" | "unidic-csj"
  pub preset: DictionaryPreset,
  /// 辞書キャッシュディレクトリ。
  ///
  /// TOML で省略された場合は `None` となり、
  /// 実際のデフォルトは `DictionaryManager` 側で決定する想定。
  #[serde(default)]
  pub cache_dir: Option<PathBuf>,
}

/// プリセット辞書種別。
///
/// ## 設計背景
///
/// 本プロジェクトでは、形態素解析エンジン [vibrato-rkyv] が提供する
/// `PresetDictionaryKind` 型を使用して辞書を指定します。しかし、この外部型は
/// `serde::Deserialize` トレイトを実装していないため、TOML 設定ファイルから
/// 直接デシリアライズすることができません。
///
/// ## この型が存在する理由
///
/// `DictionaryPreset` は TOML 設定ファイルからの読み込みを実現するために
/// 利便性向上を目的として新規定義された enum です。
/// `#[derive(Deserialize)]` を持つため、設定ファイルの `[dictionary].preset`
/// フィールドとして直接使用できます。
///
/// ## PresetDictionaryKind との統合ができない理由
///
/// `PresetDictionaryKind` は外部クレート (vibrato-rkyv) の型であるため、
/// 本プロジェクト側で `Deserialize` 実装を追加することができません
///（Rust の孤児ルール / orphan rule により禁止されています）。
///
/// そのため、設定ファイル用の型として `DictionaryPreset` を定義し、
/// 内部処理で `PresetDictionaryKind` に変換する設計を採用しています。
///
/// ## 変換方法
///
/// `From<DictionaryPreset> for PresetDictionaryKind` トレイト実装により、
/// `.into()` メソッドで相互運用可能です。
///
/// [vibrato-rkyv]: https://crates.io/crates/vibrato-rkyv
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DictionaryPreset {
  /// IpaDic: もっとも小型
  Ipadic,
  /// Unidic の書き言葉用
  UnidicCwj,
  /// Unidic の話し言葉用
  UnidicCsj,
}

/// [index] セクション設定。
#[derive(Debug, Clone, Deserialize)]
pub struct IndexConfig {
  /// インデックス保存ディレクトリ（例: "/opt/wakeru/data/index"）
  pub data_dir: PathBuf,
  /// IndexWriter のメモリバッファサイズ（バイト）
  pub writer_memory_bytes: usize,
  /// バッチコミットサイズ
  pub batch_commit_size: usize,
  /// サポートする言語一覧（例: ["ja", "en"]）
  #[serde(default = "default_languages")]
  pub languages: Vec<Language>,
  /// デフォルト言語（`languages` に含まれる必要がある）
  #[serde(default = "default_language")]
  pub default_language: Language,
}

/// デフォルトの言語一覧（日本語のみ）
fn default_languages() -> Vec<Language> {
  vec![Language::Ja]
}

/// デフォルト言語（日本語）
fn default_language() -> Language {
  Language::Ja
}

/// [search] セクション設定。
#[derive(Debug, Clone, Deserialize)]
pub struct SearchConfig {
  /// デフォルトの検索結果上限
  pub default_limit: usize,
  /// 最大検索結果上限
  pub max_limit: usize,
}

/// [logging] セクション設定。
#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
  /// ログレベル: "trace" | "debug" | "info" | "warn" | "error"
  pub level: LogLevel,
}

/// ログレベル。
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

// ===== アクセサメソッド =====

impl WakeruConfig {
  /// DictionaryManager に渡すためのプリセット辞書種別を返す。
  ///
  /// 設計書中の:
  /// ```rust,ignore
  /// let dictionary_manager = DictionaryManager::with_preset(
  ///     config.dictionary_preset(),
  /// )?;
  /// ```
  /// に対応。
  pub fn dictionary_preset(&self) -> PresetDictionaryKind {
    self.dictionary.preset.into()
  }

  /// 設定された辞書キャッシュディレクトリを返す。
  ///
  /// TOML で未指定の場合は `None`。
  /// 実際のパス決定は DictionaryManager 側で行う想定。
  pub fn dictionary_cache_dir(&self) -> Option<&Path> {
    self.dictionary.cache_dir.as_deref()
  }

  /// インデックスのベースディレクトリを返す。
  ///
  /// 例: "/opt/wakeru/data/index"
  pub fn index_base_dir(&self) -> &Path {
    &self.index.data_dir
  }

  /// 指定言語のインデックスディレクトリを返す。
  ///
  /// ディレクトリ構成:
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
  /// # 例
  /// ```ignore
  /// let ja_path = config.index_path_for_language(Language::Ja);
  /// // → "/opt/wakeru/data/index/ja"
  /// ```
  pub fn index_path_for_language(&self, language: Language) -> PathBuf {
    self.index.data_dir.join(language.code())
  }

  /// デフォルトコレクションのインデックスディレクトリを返す。
  ///
  /// 設計書のディレクトリ構成:
  ///   data/index/
  ///     ├── default/
  ///     └── {collection_id}/
  ///
  /// と、`WakeruService::init` 中の:
  /// ```rust,ignore
  /// let index_manager = if config.index_path().exists() {
  ///     IndexManager::open(config.index_path(), tokenizer)?
  /// } else {
  ///     IndexManager::create(config.index_path(), tokenizer)?
  /// };
  /// ```
  /// を踏まえ、`<data_dir>/default` を返す。
  #[deprecated(note = "Use index_path_for_language() for multi-language support")]
  pub fn index_path(&self) -> PathBuf {
    self.index.data_dir.join("default")
  }

  /// IndexWriter のメモリバッファサイズ（バイト）を返す。
  pub fn writer_memory_bytes(&self) -> usize {
    self.index.writer_memory_bytes
  }

  /// バッチコミットサイズを返す。
  pub fn batch_commit_size(&self) -> usize {
    self.index.batch_commit_size
  }

  /// サポートする言語一覧を返す。
  pub fn supported_languages(&self) -> &[Language] {
    &self.index.languages
  }

  /// デフォルト言語を返す。
  pub fn default_language(&self) -> Language {
    self.index.default_language
  }

  /// 設定の妥当性を検証する。
  ///
  /// # 検証項目
  /// - `languages` が空でない
  /// - `default_language` が `languages` に含まれている
  /// - `search.default_limit` >= 1
  /// - `search.max_limit` >= `search.default_limit`
  /// - `index.writer_memory_bytes` が許容範囲内（1MB〜1GB）
  /// - `index.batch_commit_size` >= 1
  /// - `dictionary.cache_dir` が存在するディレクトリ または 作成可能
  ///
  /// # エラー
  /// 検証に失敗した場合、対応する `ConfigError` を返す。
  pub fn validate(&self) -> Result<(), ConfigError> {
    // languages が空でない
    if self.index.languages.is_empty() {
      return Err(ConfigError::EmptyLanguages);
    }

    // default_language が languages に含まれている
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

    // index.writer_memory_bytes が許容範囲内（1MB〜1GB）
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

    // dictionary.cache_dir が存在するディレクトリ または 作成可能
    if let Some(cache_dir) = &self.dictionary.cache_dir {
      if cache_dir.exists() {
        // 存在する場合、ディレクトリであることを確認
        if !cache_dir.is_dir() {
          return Err(ConfigError::InvalidDictionaryCacheDir {
            path: cache_dir.clone(),
          });
        }
      } else {
        // 存在しない場合、作成可能か確認
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

  /// デフォルトの検索結果上限を返す。
  pub fn default_search_limit(&self) -> usize {
    self.search.default_limit
  }

  /// 最大検索結果上限を返す。
  pub fn max_search_limit(&self) -> usize {
    self.search.max_limit
  }

  /// ログレベルを返す。
  pub fn log_level(&self) -> LogLevel {
    self.logging.level
  }
}

// ===== ライブラリ型を本クレートで使用可能な型(一部trait付与版)に変換 =====
//
// DictionaryPreset（設定ファイル用）→ PresetDictionaryKind（vibrato-rkyv 用）
// の変換を実装します。
//
// この変換が必要な理由は `DictionaryPreset` のドキュメントコメントを参照してください。

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
// テストモジュール
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;
  use tempfile::TempDir;

  // ─── テスト用ヘルパー ─────────────────────────────────────────────────────

  /// 正常な設定のベースを作成（テストごとに一時ディレクトリを使用）
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

  // ─── Language のテスト ────────────────────────────────────────────────────

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

  // ─── validate() 正常系のテスト ────────────────────────────────────────────

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

  // ─── validate() languages 異常系 ───────────────────────────────────────────

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

  // ─── validate() search 異常系 ──────────────────────────────────────────────

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

  // ─── validate() index 異常系 ───────────────────────────────────────────────

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

  // ─── validate() dictionary.cache_dir テスト ───────────────────────────────

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

    // 存在しないことを確認
    assert!(!cache_dir.exists());

    let mut config = create_valid_config(&temp_dir);
    config.dictionary.cache_dir = Some(cache_dir.clone());

    let result = config.validate();
    assert!(result.is_ok());

    // ディレクトリが作成されたことを確認
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
    // 親をファイルにする
    let parent_file = temp_dir.path().join("parent_file");
    fs::write(&parent_file, b"dummy").unwrap();

    // 親がファイルの下にディレクトリを作ろうとすると失敗する
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

  // ─── エラー優先順位のテスト ────────────────────────────────────────────────

  #[test]
  fn validate_reports_empty_languages_first() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.index.languages.clear(); // 最初のエラー
    config.search.default_limit = 0; // 2番目のエラー候補

    let err = config.validate().unwrap_err();
    // EmptyLanguages が最初に報告される
    assert!(matches!(err, ConfigError::EmptyLanguages));
  }

  #[test]
  fn validate_reports_default_language_before_search() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);
    config.index.languages = vec![Language::En];
    config.index.default_language = Language::Ja; // 最初のエラー
    config.search.default_limit = 0; // 2番目のエラー候補

    let err = config.validate().unwrap_err();
    assert!(matches!(
      err,
      ConfigError::DefaultLanguageNotInLanguages { .. }
    ));
  }

  // ─── アクセサメソッドのテスト ───────────────────────────────────────────────

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

  // ─── DictionaryPreset のテスト ─────────────────────────────────────────────

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

  // ─── 複数異常値の組み合わせテスト ──────────────────────────────────────────

  #[test]
  fn validate_with_multiple_errors_reports_first() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_valid_config(&temp_dir);

    // 複数のエラー条件を設定
    config.index.languages.clear(); // EmptyLanguages
    config.search.default_limit = 0; // InvalidSearchDefaultLimit
    config.search.max_limit = 0; // InvalidSearchMaxLimit
    config.index.writer_memory_bytes = 0; // InvalidWriterMemoryBytes

    let err = config.validate().unwrap_err();
    // 最初のチェックでエラーになる
    assert!(matches!(err, ConfigError::EmptyLanguages));
  }
}
