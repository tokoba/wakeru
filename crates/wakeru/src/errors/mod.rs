//! errors module
pub mod error_definition;

/// Re-export major error types
pub use error_definition::{
  ConfigError, DictionaryError, IndexerError, SearcherError, TokenizerError, WakeruError,
  WakeruResult,
};
