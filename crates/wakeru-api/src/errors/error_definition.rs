//! APIエラー定義

use axum::{
  Json,
  http::StatusCode,
  response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

// wakeru クレートのエラー型をインポート
use wakeru::errors::{TokenizerError, WakeruError};

/// エラーの種類
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiErrorKind {
  /// 入力値が無効
  InvalidInput,
  /// テキストが長すぎる
  TextTooLong,
  /// 内部エラー
  Internal,
  /// 設定エラー
  Config,
}

impl ApiErrorKind {
  /// エラーコードを取得
  #[must_use]
  pub fn code(&self) -> &'static str {
    match self {
      Self::InvalidInput => "invalid_input",
      Self::TextTooLong => "text_too_long",
      Self::Internal => "internal_error",
      Self::Config => "config_error",
    }
  }

  /// HTTPステータスコードを取得
  #[must_use]
  pub fn status(&self) -> StatusCode {
    match self {
      Self::InvalidInput | Self::TextTooLong => StatusCode::BAD_REQUEST,
      Self::Internal | Self::Config => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }
}

/// APIエラー
#[derive(Debug, Error)]
pub enum ApiError {
  /// 入力値が無効
  #[error("入力値が無効です: {0}")]
  InvalidInput(String),

  /// テキストが長すぎる
  #[error("テキストが長すぎます: {0} バイト（最大: {1} バイト）")]
  TextTooLong(usize, usize),

  /// 内部エラー
  #[error("内部エラー: {0}")]
  Internal(String),

  /// 設定エラー
  #[error("設定エラー: {0}")]
  Config(String),
}

impl ApiError {
  /// エラーの種類を取得
  #[must_use]
  pub fn kind(&self) -> ApiErrorKind {
    match self {
      Self::InvalidInput(_) => ApiErrorKind::InvalidInput,
      Self::TextTooLong(_, _) => ApiErrorKind::TextTooLong,
      Self::Internal(_) => ApiErrorKind::Internal,
      Self::Config(_) => ApiErrorKind::Config,
    }
  }

  /// エラーコードを取得
  #[must_use]
  pub fn code(&self) -> &'static str {
    self.kind().code()
  }

  /// HTTPステータスコードを取得
  #[must_use]
  pub fn status(&self) -> StatusCode {
    self.kind().status()
  }

  /// 無効な入力エラーを作成
  #[must_use]
  pub fn invalid_input(message: impl Into<String>) -> Self {
    Self::InvalidInput(message.into())
  }

  /// テキスト長超過エラーを作成
  #[must_use]
  pub fn text_too_long(actual: usize, max: usize) -> Self {
    Self::TextTooLong(actual, max)
  }

  /// 内部エラーを作成
  #[must_use]
  pub fn internal(message: impl Into<String>) -> Self {
    Self::Internal(message.into())
  }

  /// 設定エラーを作成
  #[must_use]
  pub fn config(message: impl Into<String>) -> Self {
    Self::Config(message.into())
  }
}

/// エラーレスポンスのJSON構造
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

/// WakeruError から ApiError への変換
///
/// ドメイン層のエラーを API 層のエラーにマッピングする。
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
      // #[non_exhaustive] な enum のため、将来追加されるバリアントに対応
      _ => ApiError::internal(format!("unknown error: {err}")),
    }
  }
}

/// Result 型エイリアス
pub type Result<T> = std::result::Result<T, ApiError>;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn invalid_input_creation() {
    let err = ApiError::invalid_input("テストエラー");
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
    let err = ApiError::internal("内部処理エラー");
    assert_eq!(err.kind(), ApiErrorKind::Internal);
    assert_eq!(err.code(), "internal_error");
    assert_eq!(err.status(), StatusCode::INTERNAL_SERVER_ERROR);
  }

  #[test]
  fn config_creation() {
    let err = ApiError::config("設定ファイルが見つかりません");
    assert_eq!(err.kind(), ApiErrorKind::Config);
    assert_eq!(err.code(), "config_error");
    assert_eq!(err.status(), StatusCode::INTERNAL_SERVER_ERROR);
  }

  #[test]
  fn from_wakeru_error_invalid_input() {
    let wakeru_err = WakeruError::Tokenizer(TokenizerError::InvalidInput {
      reason: "テストエラー".to_string(),
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
