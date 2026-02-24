//! wakeru Morphological Analysis Library
//!
//! Performs morphological analysis for Japanese and other languages using vibrato-rkyv.

/// Configuration module - Defines configuration structures such as WakeruConfig and Language
pub mod config;

/// Dictionary module - Provides management and loading functionality for morphological analysis dictionaries
pub mod dictionary;

/// Error module - Defines error types such as WakeruError and WakeruResult
pub mod errors;

/// Indexer module - Construction and management of full-text search index using Tantivy
pub mod indexer;

/// Data model module - Defines data structures such as Document and SearchResult
pub mod models;

/// Search module - Provides full-text search functionality using the BM25 algorithm
pub mod searcher;

/// Service module - Provides high-level APIs such as WakeruService
pub mod service;

/// Tokenizer module - Morphological analysis tokenizer using vibrato-rkyv
pub mod tokenizer;

/// Re-exports
pub use config::{Language, WakeruConfig};
pub use errors::{WakeruError, WakeruResult};
pub use service::WakeruService;
