//! indexer module
//!
//! Responsible for Tantivy index creation, management, and document addition.

pub mod index_manager;
pub mod report;
pub mod schema_builder;

/// Re-export major types
pub use index_manager::IndexManager;
pub use report::AddDocumentsReport;
pub use schema_builder::{SchemaFields, build_schema};
