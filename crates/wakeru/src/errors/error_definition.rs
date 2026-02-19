//! エラー定義

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use vibrato_rkyv::dictionary::PresetDictionaryKind;

use crate::config::Language;

/// 設定ファイル（WakeruConfig）関連のエラー
#[derive(Debug, Error, Clone)]
#[non_exhaustive]
pub enum ConfigError {
  /// index.languages が空
  #[error("languages に少なくとも1つの言語を指定してください")]
  EmptyLanguages,

  /// index.default_language が index.languages に含まれていない
  #[error("default_language ({default_language}) は languages に含まれている必要があります")]
  DefaultLanguageNotInLanguages {
    /// 指定された default_language
    default_language: Language,
  },

  /// search.default_limit < 1
  #[error("search.default_limit は 1 以上である必要があります: actual={actual}")]
  InvalidSearchDefaultLimit {
    /// 実際に指定された値
    actual: usize,
  },

  /// search.max_limit < search.default_limit
  #[error(
    "search.max_limit は search.default_limit 以上である必要があります: \
     default_limit={default_limit}, max_limit={max_limit}"
  )]
  InvalidSearchMaxLimit {
    /// search.default_limit
    default_limit: usize,
    /// search.max_limit
    max_limit: usize,
  },

  /// index.writer_memory_bytes が許容範囲外
  #[error(
    "index.writer_memory_bytes は {min}〜{max} バイトの範囲で指定してください: actual={actual}"
  )]
  InvalidWriterMemoryBytes {
    /// 許容される最小値（バイト）
    min: u64,
    /// 許容される最大値（バイト）
    max: u64,
    /// 実際に指定された値（バイト）
    actual: u64,
  },

  /// index.batch_commit_size < 1
  #[error("index.batch_commit_size は 1 以上である必要があります: actual={actual}")]
  InvalidBatchCommitSize {
    /// 実際に指定された値
    actual: usize,
  },

  /// dictionary.cache_dir が「存在するディレクトリ」でない（ファイルである等）
  #[error("dictionary.cache_dir がディレクトリではありません: path={path:?}")]
  InvalidDictionaryCacheDir {
    /// 不正なパス
    path: PathBuf,
  },

  /// dictionary.cache_dir の作成に失敗
  #[error("dictionary.cache_dir の作成に失敗しました: path={path:?}, error={source}")]
  DictionaryCacheDirCreationFailed {
    /// 作成しようとしたパス
    path: PathBuf,
    /// 元となった IO エラー
    #[source]
    source: Arc<io::Error>,
  },
}

/// 辞書関連のエラー
/// Vibrato では mecab, ipadic, unidic 等の辞書を使用可能
/// これらのエラーを定義する
#[derive(Debug, Error, Clone)]
#[non_exhaustive]
pub enum DictionaryError {
  /// キャッシュディレクトリーが見つからない
  #[error("辞書キャッシュディレクトリーが見つかりません")]
  CacheDirNotFound,

  /// キャッシュディレクトリーの作成失敗
  #[error("辞書キャッシュディレクトリーの作成に失敗しました: {0}")]
  CacheDirCreationFailed(Arc<io::Error>),

  /// 指定された辞書が見つからない
  #[error("指定された辞書が見つかりません: {0}")]
  DictionaryNotFound(String),

  /// 辞書のダウンロード失敗（URL, IOエラー等）
  #[error("辞書のダウンロードに失敗しました: {0}")]
  DownloadFailed(String),

  /// 辞書の検証に失敗（ハッシュ不一致等）
  #[error("辞書の検証に失敗しました: {0}")]
  ValidationFailed(String),

  /// 辞書パスが不正
  #[error("辞書パスが不正です: {0}")]
  InvalidPath(PathBuf),

  /// 辞書パスが不正または辞書種別が不正
  #[error("辞書パスまたは辞書種別が不正です: path={0}, preset_kind={1:?}")]
  InvalidPathOrInvalidPresetKind(PathBuf, Option<PresetDictionaryKind>),

  /// vibrato-rkyv による辞書のロード失敗
  #[error("vibrato-rkyv 辞書ロードエラー: {0}")]
  VibratoLoad(Arc<dyn std::error::Error + Send + Sync + 'static>),

  /// vibrato-rkyv のプリセット辞書のダウンロード失敗
  #[error("vibrato-rkyv プリセット辞書ダウンロード失敗: {0}")]
  PresetDictDownloadFailed(Arc<dyn std::error::Error + Send + Sync + 'static>),
}

/// トークナイザー関連エラー
#[derive(Debug, Error, Clone)]
#[non_exhaustive]
pub enum TokenizerError {
  /// 辞書起因のエラー
  #[error("辞書エラー: {0}")]
  Dictionary(#[from] DictionaryError),

  /// 入力テキストが不正
  #[error("トークナイズ対象の入力テキストが不正: {reason}")]
  InvalidInput {
    /// 不正の理由
    reason: String,
  },
}

/// インデクサー関連のエラー
#[derive(Debug, Error, Clone)]
#[non_exhaustive]
pub enum IndexerError {
  /// トークナイザー起因のエラー
  #[error("トークナイザーエラー: {0}")]
  Tokenizer(#[from] TokenizerError),

  /// Tantivy のインデックス操作エラー
  #[error("Tantivy インデックスエラー: {0}")]
  Tantivy(#[from] tantivy::TantivyError),

  /// インデックスパスが不正、またはディレクトリ作成に失敗
  #[error("インデックスパスが不正: {path}: {source}")]
  InvalidIndexPath {
    /// 問題が発生したパス
    path: PathBuf,
    /// 発生した I/O エラー
    #[source]
    source: Arc<io::Error>,
  },

  /// インデックスが既に存在する（CreateNew モードで既存インデックスがあった場合）
  #[error("インデックスは既に存在します: {0}")]
  IndexAlreadyExists(PathBuf),

  /// インデックスが見つからない（OpenExisting モードでインデックスが存在しない場合）
  #[error("インデックスが見つかりません: {0}")]
  IndexNotFound(PathBuf),

  /// 日本語トークナイザーが提供されていない
  #[error("日本語インデックスには VibratoTokenizer が必要です")]
  MissingJapaneseTokenizer,

  /// スキーマと言語の不一致
  #[error("スキーマと言語が一致しません: expected={expected}, actual={actual}")]
  LanguageSchemaMismatch {
    /// 期待するトークナイザー名
    expected: String,
    /// 実際のトークナイザー名
    actual: String,
  },

  /// メタデータの JSON シリアライズ失敗
  #[error("メタデータのシリアライズに失敗しました: doc_id={doc_id}, error={source}")]
  MetadataSerialize {
    /// 対象ドキュメントID
    doc_id: String,
    /// 元となった JSON エラー
    #[source]
    source: Arc<serde_json::Error>,
  },
}

/// 検索関連エラー
#[derive(Debug, Error, Clone)]
#[non_exhaustive]
pub enum SearcherError {
  /// Tantivy の検索処理エラー
  #[error("Tantivy 検索エラー: {0}")]
  Tantivy(#[from] tantivy::TantivyError),

  /// クエリの解析に失敗
  #[error("クエリ解析エラー: {reason}")]
  InvalidQuery {
    /// クエリ不正の理由
    reason: String,
  },

  /// インデックスのスキーマ不整合など、検索に利用できない状態
  #[error("インデックスが不正です: field={field}, reason={reason}")]
  InvalidIndex {
    /// 問題が発生したフィールド名
    field: String,
    /// 不整合の理由
    reason: String,
  },

  /// メタデータ JSON のデシリアライズ失敗
  #[error("メタデータのデシリアライズに失敗しました: doc_id={doc_id}, error={source}")]
  MetadataDeserialize {
    /// 対象ドキュメントID
    doc_id: String,
    /// 元となった JSON エラー
    #[source]
    source: Arc<serde_json::Error>,
  },
}

/// 統合エラー
/// 本クレートの外部に公開するエラー用 API はこのエラーを返すこと
/// `WakeruResult<T>` = `Result<T, WakeruError>` として使用する
#[derive(Debug, Error, Clone)]
#[non_exhaustive]
pub enum WakeruError {
  /// 辞書関連エラー
  #[error(transparent)]
  Dictionary(#[from] DictionaryError),

  /// トークナイザー関連エラー
  #[error(transparent)]
  Tokenizer(#[from] TokenizerError),

  /// インデクサー関連エラー
  #[error(transparent)]
  Indexer(#[from] IndexerError),

  /// 検索関連エラー
  #[error(transparent)]
  Searcher(#[from] SearcherError),

  /// サポートされていない言語
  #[error("サポートされていない言語です: {language}")]
  UnsupportedLanguage {
    /// 指定された言語
    language: Language,
  },

  /// 設定エラー
  #[error(transparent)]
  Config(#[from] ConfigError),
}

/// wakeru クレートの標準 Result 型エイリアス
pub type WakeruResult<T> = Result<T, WakeruError>;
