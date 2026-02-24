//! Router Definition

use axum::{
  Router,
  routing::{get, post},
};
use tower_http::trace::TraceLayer;

use super::handlers::{health_check, post_wakeru};
use super::state::AppState;
use crate::errors::ApiError;

/// Create API Router
///
/// # Arguments
/// * `state` - Application state
///
/// # Returns
/// Configured Router
pub fn create_router(state: AppState) -> Router {
  Router::new()
    .route("/wakeru", post(post_wakeru))
    .route("/health", get(health_check))
    .layer(TraceLayer::new_for_http())
    .with_state(state)
}

/// Start the server
///
/// # Arguments
/// * `state` - Application state
///
/// # Errors
/// Returns error if server fails to start
pub async fn run_server(state: AppState) -> crate::errors::Result<()> {
  let addr = &state.config.bind_addr;
  let listener = tokio::net::TcpListener::bind(addr)
    .await
    .map_err(|e| ApiError::config(format!("Failed to bind: {}", e)))?;

  tracing::info!("Starting server: http://{}", addr);

  let router = create_router(state);

  axum::serve(listener, router)
    .await
    .map_err(|e| ApiError::internal(format!("Server error: {}", e)))?;

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

  /// Dummy implementation for testing (Does not touch dictionary)
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

    // Inject stub (No dictionary load needed)
    let service = Arc::new(DummyService) as Arc<dyn WakeruApiService>;
    AppState::new(config, service)
  }

  #[test]
  fn test_router_creation() {
    let state = create_test_state();
    let _router = create_router(state);
    // Confirm router can be created successfully
  }
}
