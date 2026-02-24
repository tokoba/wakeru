//! API module

mod handlers;
mod routes;
mod state;

pub use handlers::{health_check, post_wakeru};
pub use routes::{create_router, run_server};
pub use state::AppState;
