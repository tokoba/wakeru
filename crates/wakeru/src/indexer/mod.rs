//! indexer モジュール
//!
//! Tantivy インデックスの作成・管理・ドキュメント追加を担当します。

pub mod index_manager;
pub mod report;
pub mod schema_builder;

/// 主要な型を再エクスポート
pub use index_manager::IndexManager;
pub use report::AddDocumentsReport;
pub use schema_builder::{SchemaFields, build_schema};
