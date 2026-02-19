//! API統合テスト
//!
//! Router 経由で HTTP エンドポイントの振る舞いを検証する。
//! スタブサービスを使用するため、辞書ロード不要で軽量かつ高速なテスト。

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

/// 統合テスト用の軽量スタブサービス
///
/// - 空文字列: `invalid_input` エラー
/// - 長さ超過: `text_too_long` エラー
/// - それ以外: 空の tokens と 0ms を返す
struct StubWakeruApiService;

impl WakeruApiService for StubWakeruApiService {
  fn analyze(&self, request: WakeruRequest) -> ApiResult<WakeruResponse> {
    let text_bytes = request.text.len();

    if text_bytes == 0 {
      return Err(ApiError::invalid_input("テキストが空です"));
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

/// テスト用の Router を構築する
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
// 正常系テスト
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

  let payload = serde_json::json!({ "text": "テスト" });

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

  // tokens / elapsed_ms フィールドが存在することを確認
  assert!(json.get("tokens").is_some());
  assert!(json.get("elapsed_ms").is_some());
}

// ============================================================================
// 異常系テスト（サービスエラー）
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

  // MAX_TEXT_LENGTH + 1 バイトのテキストを送る
  // 注: Axum のデフォルトリクエストサイズ制限（2MB）が先に適用されるため、
  // 413 PAYLOAD_TOO_LARGE が返る
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

  // Axum のリクエストサイズ制限により 413 が返ることを確認
  // サービス層の text_too_long エラーはユニットテストでカバー済み
  assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

// ============================================================================
// JSON パースエラーテスト（Axum 側）
// ============================================================================

#[tokio::test]
async fn post_wakeru_invalid_json_returns_client_error() {
  let app = test_app();

  // JSON として不正なボディ
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

  // Axum の Json extractor が返すステータス（400 or 422 等）を許容
  assert!(
    response.status().is_client_error(),
    "expected 4xx, got: {}",
    response.status()
  );
}

#[tokio::test]
async fn post_wakeru_missing_text_field_returns_client_error() {
  let app = test_app();

  // text フィールドが欠落した JSON
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

  // Axum の Json extractor が返すステータス（400）
  assert!(
    response.status().is_client_error(),
    "expected 4xx, got: {}",
    response.status()
  );
}
