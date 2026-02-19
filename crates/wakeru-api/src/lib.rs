//! wakeru-api クレート
//!
//! 形態素解析機能を HTTP API として提供する Web サーバー。
//!
//! ## エンドポイント
//! - `POST /wakeru` - 形態素解析
//! - `GET /health` - ヘルスチェック
//!
//! ## 使用例
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
