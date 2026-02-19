//! HTTPハンドラー定義

use axum::{Json, extract::State};
use tracing::{debug, error, info};

use crate::errors::ApiError;
use crate::models::{WakeruRequest, WakeruResponse};

use super::state::AppState;

/// POST /wakeru エンドポイント
///
/// 日本語テキストの形態素解析を実行する。
///
/// # Request Body
/// ```json
/// { "text": "解析対象のテキスト" }
/// ```
///
/// # Response
/// - 200 OK: 解析成功
/// - 400 Bad Request: 入力エラー（空テキスト、テキスト長超過）
/// - 500 Internal Server Error: 内部エラー
pub async fn post_wakeru(
  State(state): State<AppState>,
  Json(request): Json<WakeruRequest>,
) -> Result<Json<WakeruResponse>, ApiError> {
  debug!(text_len = request.text.len(), "形態素解析リクエストを受信");

  // CPUバウンドな処理を spawn_blocking で実行
  // 形態素解析は重い処理のため、非同期ランタイムをブロックしないよう分離
  let service = state.service.clone();

  let response =
    tokio::task::spawn_blocking(move || service.analyze(request)).await.map_err(|e| {
      error!(error = %e, "spawn_blocking エラー");
      ApiError::internal("処理の実行に失敗しました")
    })??;

  info!(
    token_count = response.tokens.len(),
    elapsed_ms = response.elapsed_ms,
    "形態素解析完了"
  );

  Ok(Json(response))
}

/// ヘルスチェックエンドポイント
///
/// サーバーが稼働しているかを確認する。
pub async fn health_check() -> &'static str {
  "OK"
}

/// POST /wakeru エンドポイント（同期的バージョン）
///
/// 処理が軽い場合はこちらを使用可能。
/// デフォルトでは spawn_blocking 版を使用する。
#[allow(dead_code)]
pub async fn post_wakeru_sync(
  State(state): State<AppState>,
  Json(request): Json<WakeruRequest>,
) -> Result<Json<WakeruResponse>, ApiError> {
  debug!(
    text_len = request.text.len(),
    "形態素解析リクエストを受信（同期版）"
  );

  let response = state.service.analyze(request)?;

  info!(
    token_count = response.tokens.len(),
    elapsed_ms = response.elapsed_ms,
    "形態素解析完了"
  );

  Ok(Json(response))
}

#[cfg(test)]
mod tests {
  #[test]
  fn test_health_check_signature() {
    // health_check が正常にコンパイルできることを確認
    // 実際のテストは統合テストで行う
  }
}
