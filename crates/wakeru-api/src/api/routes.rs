//! ルーター定義

use axum::{
  Router,
  routing::{get, post},
};
use tower_http::trace::TraceLayer;

use super::handlers::{health_check, post_wakeru};
use super::state::AppState;
use crate::errors::ApiError;

/// APIルーターを作成する
///
/// # Arguments
/// * `state` - アプリケーション状態
///
/// # Returns
/// 設定済みの Router
pub fn create_router(state: AppState) -> Router {
  Router::new()
    .route("/wakeru", post(post_wakeru))
    .route("/health", get(health_check))
    .layer(TraceLayer::new_for_http())
    .with_state(state)
}

/// サーバーを起動する
///
/// # Arguments
/// * `state` - アプリケーション状態
///
/// # Errors
/// サーバーの起動に失敗した場合にエラーを返す
pub async fn run_server(state: AppState) -> crate::errors::Result<()> {
  let addr = &state.config.bind_addr;
  let listener = tokio::net::TcpListener::bind(addr)
    .await
    .map_err(|e| ApiError::config(format!("バインドに失敗しました: {}", e)))?;

  tracing::info!("サーバーを起動します: http://{}", addr);

  let router = create_router(state);

  axum::serve(listener, router)
    .await
    .map_err(|e| ApiError::internal(format!("サーバーエラー: {}", e)))?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::*;
  use crate::config::{Config, Preset};
  use crate::errors::Result as ApiResult;
  use crate::models::{WakeruRequest, WakeruResponse};
  use crate::service::WakeruApiService;

  /// テスト用のダミー実装（辞書を一切触らない）
  #[derive(Clone)]
  struct DummyService;

  impl WakeruApiService for DummyService {
    fn analyze(&self, _request: WakeruRequest) -> ApiResult<WakeruResponse> {
      Ok(WakeruResponse {
        tokens: Vec::new(),
        elapsed_ms: 0,
      })
    }
  }

  fn create_test_state() -> AppState {
    let config = Config {
      bind_addr: "127.0.0.1:5531".to_string(),
      preset: Preset::UnidicCwj,
    };

    // スタブを注入（辞書ロード不要）
    let service = Arc::new(DummyService) as Arc<dyn WakeruApiService>;
    AppState::new(config, service)
  }

  #[test]
  fn test_router_creation() {
    let state = create_test_state();
    let _router = create_router(state);
    // ルーターが正常に作成できることを確認
  }
}
