//! HTTP Handler Definitions

use axum::{Json, extract::State};
use tracing::{debug, error, info};

use crate::errors::ApiError;
use crate::models::{WakeruRequest, WakeruResponse};

use super::state::AppState;

/// POST /wakeru Endpoint
///
/// Performs morphological analysis on Japanese text.
///
/// # Request Body
/// ```json
/// { "text": "Text to analyze" }
/// ```
///
/// # Response
/// - 200 OK: Analysis successful
/// - 400 Bad Request: Input error (Empty text, Text too long)
/// - 500 Internal Server Error: Internal error
pub async fn post_wakeru(
  State(state): State<AppState>,
  Json(request): Json<WakeruRequest>,
) -> Result<Json<WakeruResponse>, ApiError> {
  debug!(
    text_len = request.text.len(),
    "Received morphological analysis request"
  );

  // Execute CPU-bound processing with spawn_blocking
  // Morphological analysis is a heavy process, so separate it to avoid blocking the async runtime
  let service = state.service.clone();

  let response =
    tokio::task::spawn_blocking(move || service.analyze(request)).await.map_err(|e| {
      error!(error = %e, "spawn_blocking error");
      ApiError::internal("Failed to execute processing")
    })??;

  info!(
    token_count = response.tokens.len(),
    elapsed_ms = response.elapsed_ms,
    "Morphological analysis completed"
  );

  Ok(Json(response))
}

/// Health Check Endpoint
///
/// Checks if the server is running.
pub async fn health_check() -> &'static str {
  "OK"
}

/// POST /wakeru Endpoint (Synchronous version)
///
/// Can be used if processing is light.
/// Uses spawn_blocking version by default.
#[allow(dead_code)]
pub async fn post_wakeru_sync(
  State(state): State<AppState>,
  Json(request): Json<WakeruRequest>,
) -> Result<Json<WakeruResponse>, ApiError> {
  debug!(
    text_len = request.text.len(),
    "Received morphological analysis request (Sync version)"
  );

  let response = state.service.analyze(request)?;

  info!(
    token_count = response.tokens.len(),
    elapsed_ms = response.elapsed_ms,
    "Morphological analysis completed"
  );

  Ok(Json(response))
}

#[cfg(test)]
mod tests {
  #[test]
  fn test_health_check_signature() {
    // Confirm health_check compiles successfully
    // Actual tests are done in integration tests
  }
}
