//! wakeru-api server entry point

use std::sync::Arc;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use wakeru_api::ApiError;
use wakeru_api::api::AppState;
use wakeru_api::api::run_server;
use wakeru_api::config::Config;
use wakeru_api::service::WakeruApiServiceFull;

#[tokio::main]
async fn main() -> Result<(), ApiError> {
  // Initialize logging
  tracing_subscriber::registry().with(tracing_subscriber::fmt::layer()).init();

  // Load configuration
  let config = Config::from_env()?;
  tracing::info!(preset = ?config.preset, "Config loaded");

  // Initialize service
  let service = Arc::new(WakeruApiServiceFull::new(&config)?);
  tracing::info!("Morphological analysis service initialized");

  // Create application state
  let state = AppState::new(config, service);

  // Start server
  run_server(state).await
}
