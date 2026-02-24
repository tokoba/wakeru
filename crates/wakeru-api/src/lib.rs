//! wakeru-api crate
//!
//! Web server providing morphological analysis functionality as HTTP API.
//!
//! ## Endpoints
//! - `POST /wakeru` - Morphological Analysis
//! - `GET /health` - Health Check
//!
//! ## Usage Example
//! ```bash
//! curl -X POST http://127.0.0.1:5530/wakeru \
//!   -H "Content-Type: application/json" \
//!   -d '{"text": "東京タワーは東京の観光名所です"}'
//! ```

pub mod api;
pub mod config;
pub mod errors;
pub mod models;
pub mod service;

pub use api::AppState;
pub use config::Config;
pub use errors::{ApiError, ApiErrorKind};
pub use models::{TokenDto, WakeruRequest, WakeruResponse};
pub use service::WakeruApiServiceFull;
