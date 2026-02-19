//! errors モジュール
pub mod error_definition;

/// 主要なエラー型を再エクスポート
pub use error_definition::{
  ConfigError, DictionaryError, IndexerError, SearcherError, TokenizerError, WakeruError,
  WakeruResult,
};
