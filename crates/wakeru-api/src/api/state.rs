//! API State Definition

use std::sync::Arc;

use crate::config::Config;
use crate::service::WakeruApiService;

/// Application State
///
/// State shared across the entire server.
/// Contains configuration and service.
#[derive(Clone)]
pub struct AppState {
  /// Configuration
  pub config: Config,
  /// Morphological Analysis Service
  ///
  /// - Production: `Arc::new(WakeruApiServiceFull::new(&config)?)`
  /// - Test: `Arc::new(StubWakeruApiService)`
  pub service: Arc<dyn WakeruApiService>,
}

impl AppState {
  /// Creates a new AppState
  #[must_use]
  pub fn new(config: Config, service: Arc<dyn WakeruApiService>) -> Self {
    Self { config, service }
  }
}
