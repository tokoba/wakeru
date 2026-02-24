//! API Error Definitions

use axum::{
  Json,
  http::StatusCode,
  response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

// Import wakeru crate error types
use wakeru::errors::{TokenizerError, WakeruError};

/// Error Kinds
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiErrorKind {
  /// Input value is invalid
  InvalidInput,
  /// Text is too long
  TextTooLong,
  /// Internal error
  Internal,
  /// Configuration error
  Config,
}

impl ApiErrorKind {
  /// Get error code
  #[must_use]
  pub fn code(&self) -> &'static str {
    match self {
      Self::InvalidInput => "invalid_input",
      Self::TextTooLong => "text_too_long",
      Self::Internal => "internal_error",
      Self::Config => "config_error",
    }
  }

  /// Get HTTP status code
  #[must_use]
  pub fn status(&self) -> StatusCode {
    match self {
      Self::InvalidInput | Self::TextTooLong => StatusCode::BAD_REQUEST,
      Self::Internal | Self::Config => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }
}

/// API Error
#[derive(Debug, Error)]
pub enum ApiError {
  /// Input value is invalid
  #[error("Invalid input: {0}")]
  InvalidInput(String),

  /// Text is too long
  #[error("Text too long: {0} bytes (max: {1} bytes)")]
  TextTooLong(usize, usize),

  /// Internal error
  #[error("Internal error: {0}")]
  Internal(String),

  /// Configuration error
  #[error("Config error: {0}")]
  Config(String),
}

impl ApiError {
  /// Get error kind
  #[must_use]
  pub fn kind(&self) -> ApiErrorKind {
    match self {
      Self::InvalidInput(_) => ApiErrorKind::InvalidInput,
      Self::TextTooLong(_, _) => ApiErrorKind::TextTooLong,
      Self::Internal(_) => ApiErrorKind::Internal,
      Self::Config(_) => ApiErrorKind::Config,
    }
  }

  /// Get error code
  #[must_use]
  pub fn code(&self) -> &'static str {
    self.kind().code()
  }

  /// Get HTTP status code
  #[must_use]
  pub fn status(&self) -> StatusCode {
    self.kind().status()
  }

  /// Create invalid input error
  #[must_use]
  pub fn invalid_input(message: impl Into<String>) -> Self {
    Self::InvalidInput(message.into())
  }

  /// Create text too long error
  #[must_use]
  pub fn text_too_long(actual: usize, max: usize) -> Self {
    Self::TextTooLong(actual, max)
  }

  /// Create internal error
  #[must_use]
  pub fn internal(message: impl Into<String>) -> Self {
    Self::Internal(message.into())
  }

  /// Create configuration error
  #[must_use]
  pub fn config(message: impl Into<String>) -> Self {
    Self::Config(message.into())
  }
}

/// JSON structure for error response
#[derive(Serialize)]
struct ErrorResponse {
  error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
  code: &'static str,
  message: String,
}

impl IntoResponse for ApiError {
  fn into_response(self) -> Response {
    let status = self.status();
    let body = ErrorResponse {
      error: ErrorBody {
        code: self.code(),
        message: self.to_string(),
      },
    };

    (status, Json(body)).into_response()
  }
}

/// Conversion from WakeruError to ApiError
///
/// Maps domain layer errors to API layer errors.
impl From<WakeruError> for ApiError {
  fn from(err: WakeruError) -> Self {
    match err {
      WakeruError::Tokenizer(TokenizerError::InvalidInput { reason }) => {
        ApiError::invalid_input(reason)
      }
      WakeruError::Dictionary(_) | WakeruError::Tokenizer(TokenizerError::Dictionary(_)) => {
        ApiError::config(format!("dictionary error: {err}"))
      }
      WakeruError::UnsupportedLanguage { language } => {
        ApiError::config(format!("unsupported language: {language:?}"))
      }
      WakeruError::Config(err) => ApiError::config(err.to_string()),
      WakeruError::Indexer(_) | WakeruError::Searcher(_) => {
        ApiError::internal(format!("internal error: {err}"))
      }
      // Since it's a #[non_exhaustive] enum, support variants added in the future
      _ => ApiError::internal(format!("unknown error: {err}")),
    }
  }
}

/// Result Type Alias
pub type Result<T> = std::result::Result<T, ApiError>;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn invalid_input_creation() {
    let err = ApiError::invalid_input("Test Error");
    assert_eq!(err.kind(), ApiErrorKind::InvalidInput);
    assert_eq!(err.code(), "invalid_input");
    assert_eq!(err.status(), StatusCode::BAD_REQUEST);
  }

  #[test]
  fn text_too_long_creation() {
    let err = ApiError::text_too_long(100, 50);
    assert_eq!(err.kind(), ApiErrorKind::TextTooLong);
    assert_eq!(err.code(), "text_too_long");
    assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    assert!(err.to_string().contains("100"));
    assert!(err.to_string().contains("50"));
  }

  #[test]
  fn internal_creation() {
    let err = ApiError::internal("Internal processing error");
    assert_eq!(err.kind(), ApiErrorKind::Internal);
    assert_eq!(err.code(), "internal_error");
    assert_eq!(err.status(), StatusCode::INTERNAL_SERVER_ERROR);
  }

  #[test]
  fn config_creation() {
    let err = ApiError::config("Config file not found");
    assert_eq!(err.kind(), ApiErrorKind::Config);
    assert_eq!(err.code(), "config_error");
    assert_eq!(err.status(), StatusCode::INTERNAL_SERVER_ERROR);
  }

  #[test]
  fn from_wakeru_error_invalid_input() {
    let wakeru_err = WakeruError::Tokenizer(TokenizerError::InvalidInput {
      reason: "Test Error".to_string(),
    });
    let api_err: ApiError = wakeru_err.into();
    assert_eq!(api_err.kind(), ApiErrorKind::InvalidInput);
    assert_eq!(api_err.code(), "invalid_input");
    assert_eq!(api_err.status(), StatusCode::BAD_REQUEST);
  }

  #[test]
  fn from_wakeru_error_config() {
    use wakeru::errors::ConfigError;
    let wakeru_err = WakeruError::Config(ConfigError::EmptyLanguages);
    let api_err: ApiError = wakeru_err.into();
    assert_eq!(api_err.kind(), ApiErrorKind::Config);
    assert_eq!(api_err.code(), "config_error");
    assert_eq!(api_err.status(), StatusCode::INTERNAL_SERVER_ERROR);
  }

  #[test]
  fn from_wakeru_error_internal() {
    use wakeru::errors::IndexerError;
    let wakeru_err = WakeruError::Indexer(IndexerError::MissingJapaneseTokenizer);
    let api_err: ApiError = wakeru_err.into();
    assert_eq!(api_err.kind(), ApiErrorKind::Internal);
    assert_eq!(api_err.code(), "internal_error");
    assert_eq!(api_err.status(), StatusCode::INTERNAL_SERVER_ERROR);
  }
}
