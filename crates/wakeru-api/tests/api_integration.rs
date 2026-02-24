//! API Integration Tests
//!
//! Verify behavior of HTTP endpoints via Router.
//! Uses stub service, so no dictionary loading required, lightweight and fast.

use std::sync::Arc;

use axum::{
  Router,
  body::Body,
  http::{Request, StatusCode},
  routing::{get, post},
};
use tower::ServiceExt;

use wakeru_api::{
  api::{AppState, health_check, post_wakeru},
  config::{Config, MAX_TEXT_LENGTH, Preset},
  errors::{ApiError, Result as ApiResult},
  models::{WakeruRequest, WakeruResponse},
  service::WakeruApiService,
};

/// Lightweight stub service for integration tests
///
/// - Empty string: `invalid_input` error
/// - Length exceeded: `text_too_long` error
/// - Otherwise: Returns empty tokens and 0ms
struct StubWakeruApiService;

impl WakeruApiService for StubWakeruApiService {
  fn analyze(&self, request: WakeruRequest) -> ApiResult<WakeruResponse> {
    let text_bytes = request.text.len();

    if text_bytes == 0 {
      return Err(ApiError::invalid_input("Text is empty"));
    }

    if text_bytes > MAX_TEXT_LENGTH {
      return Err(ApiError::text_too_long(text_bytes, MAX_TEXT_LENGTH));
    }

    Ok(WakeruResponse {
      tokens: Vec::new(),
      elapsed_ms: 0,
    })
  }
}

/// Build Router for testing
fn test_app() -> Router {
  let config = Config {
    bind_addr: "127.0.0.1:0".to_string(),
    preset: Preset::UnidicCwj,
  };

  let service: Arc<dyn WakeruApiService> = Arc::new(StubWakeruApiService);
  let state = AppState::new(config, service);

  Router::new()
    .route("/health", get(health_check))
    .route("/wakeru", post(post_wakeru))
    .with_state(state)
}

// ============================================================================
// Normal Case Tests
// ============================================================================

#[tokio::test]
async fn health_check_returns_ok() {
  let app = test_app();

  let response = app
    .oneshot(Request::builder().method("GET").uri("/health").body(Body::empty()).unwrap())
    .await
    .expect("request should succeed");

  assert_eq!(response.status(), StatusCode::OK);

  let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("read body");
  assert_eq!(body_bytes.as_ref(), b"OK");
}

#[tokio::test]
async fn post_wakeru_success_returns_200() {
  let app = test_app();

  let payload = serde_json::json!({ "text": "Test" });

  let response = app
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/wakeru")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap(),
    )
    .await
    .expect("request should succeed");

  assert_eq!(response.status(), StatusCode::OK);

  let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("read body");

  let json: serde_json::Value =
    serde_json::from_slice(&body_bytes).expect("body should be valid json");

  // Confirm tokens / elapsed_ms fields exist
  assert!(json.get("tokens").is_some());
  assert!(json.get("elapsed_ms").is_some());
}

// ============================================================================
// Abnormal Case Tests (Service Error)
// ============================================================================

#[tokio::test]
async fn post_wakeru_empty_text_returns_400() {
  let app = test_app();

  let payload = serde_json::json!({ "text": "" });

  let response = app
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/wakeru")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap(),
    )
    .await
    .expect("request should succeed");

  assert_eq!(response.status(), StatusCode::BAD_REQUEST);

  let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("read body");

  let json: serde_json::Value =
    serde_json::from_slice(&body_bytes).expect("body should be valid json");

  assert_eq!(json["error"]["code"], "invalid_input");
}

#[tokio::test]
async fn post_wakeru_too_long_text_returns_413() {
  let app = test_app();

  // Send text of MAX_TEXT_LENGTH + 1 bytes
  // Note: Axum's default request size limit (2MB) applies first,
  // so 413 PAYLOAD_TOO_LARGE returns
  let long_text = "a".repeat(MAX_TEXT_LENGTH + 1);
  let payload = serde_json::json!({ "text": long_text });

  let response = app
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/wakeru")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap(),
    )
    .await
    .expect("request should succeed");

  // Confirm 413 returns due to Axum's request size limit
  // text_too_long error in service layer is covered by unit test
  assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

// ============================================================================
// JSON Parse Error Tests (Axum side)
// ============================================================================

#[tokio::test]
async fn post_wakeru_invalid_json_returns_client_error() {
  let app = test_app();

  // Invalid JSON body
  let invalid_body = "{ invalid json";

  let response = app
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/wakeru")
        .header("content-type", "application/json")
        .body(Body::from(invalid_body))
        .unwrap(),
    )
    .await
    .expect("request should succeed");

  // Accept status returned by Axum's Json extractor (400 or 422 etc.)
  assert!(
    response.status().is_client_error(),
    "expected 4xx, got: {}",
    response.status()
  );
}

#[tokio::test]
async fn post_wakeru_missing_text_field_returns_client_error() {
  let app = test_app();

  // JSON missing text field
  let payload = serde_json::json!({ "foo": "bar" });

  let response = app
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/wakeru")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap(),
    )
    .await
    .expect("request should succeed");

  // Axum's Json extractor returns status (400)
  assert!(
    response.status().is_client_error(),
    "expected 4xx, got: {}",
    response.status()
  );
}
