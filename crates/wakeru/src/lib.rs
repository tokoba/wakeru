//! wakeru 形態素解析ライブラリー
//!
//! vibrato-rkyv を用いた日本語等の形態素解析を行う

/// 設定モジュール - WakeruConfig, Language等の設定構造体を定義
pub mod config;

/// 辞書モジュール - 形態素解析用辞書の管理・ロード機能を提供
pub mod dictionary;

/// エラーモジュール - WakeruError, WakeruResult等のエラー型を定義
pub mod errors;

/// インデックスモジュール - Tantivyによる全文検索インデックスの構築・管理
pub mod indexer;

/// データモデルモジュール - Document, SearchResult等のデータ構造を定義
pub mod models;

/// 検索モジュール - BM25アルゴリズムによる全文検索機能を提供
pub mod searcher;

/// サービスモジュール - WakeruService等の上位レベルAPIを提供
pub mod service;

/// トークナイザーモジュール - vibrato-rkyvを用いた形態素解析トークナイザー
pub mod tokenizer;

/// 再エクスポート
pub use config::{Language, WakeruConfig};
pub use errors::{WakeruError, WakeruResult};
pub use service::WakeruService;
