// crates/wakeru/src/service.rs

//! WakeruService: wakeru クレートの統合ファサード。
//!
//! - 辞書管理 (DictionaryManager)
//! - インデックス管理 (IndexManager) - 言語ごと
//! - 検索エンジン (SearchEngine) - 言語ごと
//!
//! RAG パイプラインなどの外部からは、この構造体だけを意識すればよい。
//!
//! # 多言語対応
//!
//! 言語ごとに独立したインデックスと検索エンジンを持ちます：
//! - 日本語: `data/index/ja/` (VibratoTokenizer + N-gram)
//! - 英語: `data/index/en/` (SimpleTokenizer + LowerCaser)

use std::collections::HashMap;
use std::sync::Arc;

use tantivy::tokenizer::TextAnalyzer;

use crate::config::{Language, WakeruConfig};
use crate::dictionary::DictionaryManager;
use crate::errors::error_definition::{WakeruError, WakeruResult};
use crate::indexer::IndexManager;
use crate::models::{Document, SearchResult};
use crate::searcher::SearchEngine;
use crate::tokenizer::vibrato_tokenizer::VibratoTokenizer;

/// 言語ごとの Index と SearchEngine をペアにする構造体。
///
/// これにより言語ミスマッチを構造的に防止する。
struct PerLanguage {
  #[allow(dead_code)] // 将来的にアクセサで使用予定
  index_manager: IndexManager,
  search_engine: SearchEngine,
}

/// wakeru クレートの統合ファサード。
///
/// RAG パイプラインからはこの構造体を通じて全機能にアクセスする。
///
/// # 多言語サポート
///
/// `HashMap<Language, PerLanguage>` で各言語の IndexManager と SearchEngine を管理。
/// 言語を指定してインデックス作成・検索を行う。
pub struct WakeruService {
  /// デフォルト言語
  default_language: Language,

  /// 言語ごとの IndexManager + SearchEngine
  langs: HashMap<Language, PerLanguage>,

  /// 辞書マネージャ（日本語用）
  dictionary_manager: Option<DictionaryManager>,
}

impl WakeruService {
  /// 初期化（辞書ロード + 各言語のインデックス open/create + SearchEngine 構築）
  ///
  /// # 処理フロー
  /// 1. 設定の妥当性を検証
  /// 2. 日本語サポート時のみ DictionaryManager を構築
  /// 3. 各サポート言語の IndexManager + SearchEngine を構築
  ///
  /// # エラー
  /// - 設定が不正（languages が空、default_language が含まれていない等）
  /// - 辞書ロード失敗
  /// - インデックス作成/オープン失敗
  pub fn init(config: &WakeruConfig) -> WakeruResult<Self> {
    // 設定の妥当性を検証（ConfigError は #[from] で WakeruError に自動変換）
    config.validate()?;

    let default_language = config.default_language();

    // 日本語サポート時のみ辞書マネージャを構築
    let (dictionary_manager, ja_analyzer) = if config.supported_languages().contains(&Language::Ja)
    {
      let manager = DictionaryManager::with_preset(config.dictionary_preset())?;
      let dict = manager.load()?;
      let tokenizer = VibratoTokenizer::from_shared_dictionary(dict);
      let analyzer = TextAnalyzer::from(tokenizer);
      (Some(manager), Some(Arc::new(analyzer)))
    } else {
      (None, None)
    };

    let mut langs = HashMap::new();

    // 各言語の IndexManager + SearchEngine を構築
    for &lang in config.supported_languages() {
      let index_path = config.index_path_for_language(lang);

      // 言語に応じたトークナイザーを準備
      let lang_analyzer = match lang {
        Language::Ja => ja_analyzer.as_ref().map(|a| (**a).clone()),
        Language::En => None, // 英語は IndexManager 内部で作成
      };

      let index_manager = IndexManager::open_or_create(&index_path, lang, lang_analyzer)?;
      let search_engine = SearchEngine::new(index_manager.index(), *index_manager.fields(), lang)?;

      langs.insert(
        lang,
        PerLanguage {
          index_manager,
          search_engine,
        },
      );
    }

    Ok(Self {
      default_language,
      langs,
      dictionary_manager,
    })
  }

  /// 指定言語でドキュメントをインデックスに追加する。
  ///
  /// # 引数
  /// - `language`: 対象言語
  /// - `documents`: 追加するドキュメント
  ///
  /// # エラー
  /// - サポートされていない言語
  /// - インデックス書き込みエラー
  pub fn index_documents_with_language(
    &self,
    language: Language,
    documents: &[Document],
  ) -> WakeruResult<()> {
    let per_lang =
      self.langs.get(&language).ok_or(WakeruError::UnsupportedLanguage { language })?;
    per_lang.index_manager.add_documents(documents).map(|_| ()).map_err(WakeruError::from)
  }

  /// デフォルト言語でドキュメントをインデックスに追加する。
  ///
  /// `AddDocumentsReport` は現在は返さず、エラーのみ上位へ伝播します。
  pub fn index_documents(&self, documents: &[Document]) -> WakeruResult<()> {
    self.index_documents_with_language(self.default_language, documents)
  }

  /// 指定言語で BM25 検索を実行する。
  ///
  /// # 引数
  /// - `language`: 検索対象言語
  /// - `query`: 検索クエリ
  /// - `limit`: 結果の最大件数
  ///
  /// # エラー
  /// - サポートされていない言語
  /// - クエリ解析エラー
  pub fn search_with_language(
    &self,
    language: Language,
    query: &str,
    limit: usize,
  ) -> WakeruResult<Vec<SearchResult>> {
    let per_lang =
      self.langs.get(&language).ok_or(WakeruError::UnsupportedLanguage { language })?;
    per_lang.search_engine.search(query, limit).map_err(WakeruError::from)
  }

  /// デフォルト言語で BM25 検索を実行する。
  ///
  /// `limit` はそのまま `SearchEngine::search` に渡しています
  ///（必要に応じて呼び出し側で `default_limit` / `max_limit` を考慮してください）。
  pub fn search(&self, query: &str, limit: usize) -> WakeruResult<Vec<SearchResult>> {
    self.search_with_language(self.default_language, query, limit)
  }

  /// 指定言語で形態素解析済みトークンの OR 検索を実行する。
  ///
  /// # 引数
  /// - `language`: 検索対象言語
  /// - `query`: 検索クエリ
  /// - `limit`: 結果の最大件数
  ///
  /// # エラー
  /// - サポートされていない言語
  /// - クエリ解析エラー
  pub fn search_tokens_or_with_language(
    &self,
    language: Language,
    query: &str,
    limit: usize,
  ) -> WakeruResult<Vec<SearchResult>> {
    let per_lang =
      self.langs.get(&language).ok_or(WakeruError::UnsupportedLanguage { language })?;
    per_lang.search_engine.search_tokens_or(query, limit).map_err(WakeruError::from)
  }

  /// デフォルト言語で形態素解析済みトークンの OR 検索を実行するヘルパー。
  ///
  /// 設計書 5.1 で示されている `search_tokens_or` のラッパです。
  pub fn search_tokens_or(&self, query: &str, limit: usize) -> WakeruResult<Vec<SearchResult>> {
    self.search_tokens_or_with_language(self.default_language, query, limit)
  }

  // ===== アクセサ =====

  /// デフォルト言語を返す。
  pub fn default_language(&self) -> Language {
    self.default_language
  }

  /// サポートされている言語一覧を返す。
  pub fn supported_languages(&self) -> Vec<Language> {
    self.langs.keys().copied().collect()
  }

  /// 指定言語がサポートされているか確認する。
  pub fn is_language_supported(&self, language: Language) -> bool {
    self.langs.contains_key(&language)
  }

  /// 内部の DictionaryManager への参照を返す（日本語サポート時のみ）。
  pub fn dictionary_manager(&self) -> Option<&DictionaryManager> {
    self.dictionary_manager.as_ref()
  }

  /// 指定言語の IndexManager への参照を返す。
  pub fn index_manager(&self, language: Language) -> Option<&IndexManager> {
    self.langs.get(&language).map(|p| &p.index_manager)
  }

  /// 指定言語の SearchEngine への参照を返す。
  pub fn search_engine(&self, language: Language) -> Option<&SearchEngine> {
    self.langs.get(&language).map(|p| &p.search_engine)
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// テストモジュール
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::{
    DictionaryConfig, DictionaryPreset, IndexConfig, LogLevel, LoggingConfig, SearchConfig,
  };
  use crate::models::Document;
  use serde_json::json;

  // ─── テスト用ヘルパー関数 ───────────────────────────────────────────────────

  /// 英語のみのテスト用 WakeruConfig を作成
  ///
  /// 日本語を含まないため辞書マネージャーが不要になる
  fn create_english_only_config(temp_dir: &tempfile::TempDir) -> WakeruConfig {
    WakeruConfig {
      dictionary: DictionaryConfig {
        preset: DictionaryPreset::Ipadic,
        cache_dir: Some(temp_dir.path().join("dict")),
      },
      index: IndexConfig {
        data_dir: temp_dir.path().join("index"),
        writer_memory_bytes: 50_000_000,
        batch_commit_size: 1000,
        languages: vec![Language::En],
        default_language: Language::En,
      },
      search: SearchConfig {
        default_limit: 10,
        max_limit: 100,
      },
      logging: LoggingConfig {
        level: LogLevel::Info,
      },
    }
  }

  /// 英語のみの WakeruService を作成
  fn create_english_service() -> (tempfile::TempDir, WakeruService) {
    let temp_dir = tempfile::TempDir::new().expect("一時ディレクトリ作成失敗");
    let config = create_english_only_config(&temp_dir);
    let service = WakeruService::init(&config).expect("WakeruService 初期化失敗");
    (temp_dir, service)
  }

  // ─── 初期化テスト ──────────────────────────────────────────────────────────

  #[test]
  fn service_initializes_with_english_only() {
    let (_temp_dir, service) = create_english_service();

    // デフォルト言語が英語であることを確認
    assert_eq!(service.default_language(), Language::En);

    // 英語がサポートされていることを確認
    assert!(service.is_language_supported(Language::En));

    // 日本語はサポートされていない（辞書なし）
    assert!(!service.is_language_supported(Language::Ja));
  }

  #[test]
  fn service_supported_languages() {
    let (_temp_dir, service) = create_english_service();

    let languages = service.supported_languages();
    assert_eq!(languages.len(), 1);
    assert!(languages.contains(&Language::En));
  }

  #[test]
  fn service_dictionary_manager_is_none_for_english_only() {
    let (_temp_dir, service) = create_english_service();

    // 英語のみの場合は辞書マネージャーが存在しない
    assert!(service.dictionary_manager().is_none());
  }

  // ─── アクセサテスト ────────────────────────────────────────────────────────

  #[test]
  fn service_index_manager_accessor() {
    let (_temp_dir, service) = create_english_service();

    // 英語の IndexManager が取得できる
    let index_manager = service.index_manager(Language::En);
    assert!(index_manager.is_some());
    assert_eq!(index_manager.unwrap().language(), Language::En);

    // 日本語の IndexManager は存在しない
    assert!(service.index_manager(Language::Ja).is_none());
  }

  #[test]
  fn service_search_engine_accessor() {
    let (_temp_dir, service) = create_english_service();

    // 英語の SearchEngine が取得できる
    let search_engine = service.search_engine(Language::En);
    assert!(search_engine.is_some());
    assert_eq!(search_engine.unwrap().language(), Language::En);

    // 日本語の SearchEngine は存在しない
    assert!(service.search_engine(Language::Ja).is_none());
  }

  #[test]
  fn service_is_language_supported() {
    let (_temp_dir, service) = create_english_service();

    assert!(service.is_language_supported(Language::En));
    assert!(!service.is_language_supported(Language::Ja));
  }

  // ─── ドキュメント追加テスト ────────────────────────────────────────────────

  #[test]
  fn service_index_documents_default_language() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![Document::new("doc-1", "src-1", "Hello world")];

    let result = service.index_documents(&docs);
    assert!(result.is_ok());
  }

  #[test]
  fn service_index_documents_with_language() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![Document::new("doc-1", "src-1", "Hello world")];

    let result = service.index_documents_with_language(Language::En, &docs);
    assert!(result.is_ok());
  }

  #[test]
  fn service_index_documents_unsupported_language() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![Document::new("doc-1", "src-1", "Hello world")];

    let result = service.index_documents_with_language(Language::Ja, &docs);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, WakeruError::UnsupportedLanguage { .. }));
  }

  #[test]
  fn service_index_documents_with_metadata() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![
      Document::new("doc-1", "src-1", "Tokyo is the capital")
        .with_metadata("author", json!("alice"))
        .with_tag("category:geo"),
    ];

    let result = service.index_documents(&docs);
    assert!(result.is_ok());
  }

  // ─── 検索テスト ────────────────────────────────────────────────────────────

  #[test]
  fn service_search_default_language() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![Document::new("doc-1", "src-1", "Hello world")];
    service.index_documents(&docs).expect("インデックス失敗");

    // SearchEngine がインデックス時に作成されるため、
    // 追加後のドキュメントは検索できない（Reader が再ロードされない）
    // ここではエラーにならないことだけ確認
    let result = service.search("hello", 10);
    assert!(result.is_ok());
  }

  #[test]
  fn service_search_with_language() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![Document::new("doc-1", "src-1", "Hello world")];
    service.index_documents(&docs).expect("インデックス失敗");

    let result = service.search_with_language(Language::En, "hello", 10);
    assert!(result.is_ok());
  }

  #[test]
  fn service_search_unsupported_language() {
    let (_temp_dir, service) = create_english_service();

    let result = service.search_with_language(Language::Ja, "hello", 10);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, WakeruError::UnsupportedLanguage { .. }));
  }

  #[test]
  fn service_search_tokens_or_default_language() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![Document::new("doc-1", "src-1", "Hello world")];
    service.index_documents(&docs).expect("インデックス失敗");

    let result = service.search_tokens_or("hello", 10);
    assert!(result.is_ok());
  }

  #[test]
  fn service_search_tokens_or_with_language() {
    let (_temp_dir, service) = create_english_service();

    let docs = vec![Document::new("doc-1", "src-1", "Hello world")];
    service.index_documents(&docs).expect("インデックス失敗");

    let result = service.search_tokens_or_with_language(Language::En, "hello", 10);
    assert!(result.is_ok());
  }

  #[test]
  fn service_search_tokens_or_unsupported_language() {
    let (_temp_dir, service) = create_english_service();

    let result = service.search_tokens_or_with_language(Language::Ja, "hello", 10);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, WakeruError::UnsupportedLanguage { .. }));
  }

  // ─── 統合テスト（インデックス→検索）──────────────────────────────────────

  #[test]
  fn service_full_workflow_index_and_search() {
    // このテストでは、インデックス作成後に新しい WakeruService を作成して
    // ドキュメントが正しく永続化されていることを確認

    let temp_dir = tempfile::TempDir::new().expect("一時ディレクトリ作成失敗");
    let config = create_english_only_config(&temp_dir);

    // 1. 最初のサービスでドキュメントを追加
    {
      let service = WakeruService::init(&config).expect("初期化失敗");
      let docs = vec![
        Document::new("doc-1", "src-1", "Tokyo is the capital of Japan"),
        Document::new("doc-2", "src-1", "Osaka is a major city"),
      ];
      service.index_documents(&docs).expect("インデックス失敗");
    }

    // 2. 新しいサービスを作成して検索
    {
      let service = WakeruService::init(&config).expect("初期化失敗");
      let results = service.search("tokyo", 10).expect("検索失敗");

      assert_eq!(results.len(), 1);
      assert_eq!(results[0].doc_id, "doc-1");
    }
  }

  #[test]
  fn service_full_workflow_with_metadata() {
    let temp_dir = tempfile::TempDir::new().expect("一時ディレクトリ作成失敗");
    let config = create_english_only_config(&temp_dir);

    // 1. ドキュメントを追加
    {
      let service = WakeruService::init(&config).expect("初期化失敗");
      let docs = vec![
        Document::new("doc-1", "src-1", "Tokyo is the capital")
          .with_metadata("author", json!("alice"))
          .with_tag("category:geo"),
      ];
      service.index_documents(&docs).expect("インデックス失敗");
    }

    // 2. メタデータが復元されることを確認
    {
      let service = WakeruService::init(&config).expect("初期化失敗");
      let results = service.search("tokyo", 10).expect("検索失敗");

      assert_eq!(results.len(), 1);
      assert_eq!(results[0].metadata["author"], json!("alice"));
      assert_eq!(results[0].metadata["tags"], json!(["category:geo"]));
    }
  }

  // ─── エラーハンドリングテスト ────────────────────────────────────────────

  #[test]
  fn service_invalid_query_returns_error() {
    let (_temp_dir, service) = create_english_service();

    let result = service.search("(", 10);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, WakeruError::Searcher(_)));
  }

  #[test]
  fn service_duplicate_documents_are_skipped() {
    let temp_dir = tempfile::TempDir::new().expect("一時ディレクトリ作成失敗");
    let config = create_english_only_config(&temp_dir);

    // 1. 同じ ID のドキュメントを2回追加
    {
      let service = WakeruService::init(&config).expect("初期化失敗");
      let docs1 = vec![Document::new("doc-1", "src-1", "First content")];
      service.index_documents(&docs1).expect("インデックス失敗");

      let docs2 = vec![Document::new("doc-1", "src-1", "Second content")];
      service.index_documents(&docs2).expect("インデックス失敗"); // 重複はスキップされる
    }

    // 2. 最初のコンテンツが保持されていることを確認
    {
      let service = WakeruService::init(&config).expect("初期化失敗");
      let results = service.search("first", 10).expect("検索失敗");

      assert_eq!(results.len(), 1);
      assert_eq!(results[0].text, "First content");
    }
  }

  // ─── 設定バリデーションテスト ──────────────────────────────────────────────

  #[test]
  fn service_init_validates_config() {
    let temp_dir = tempfile::TempDir::new().expect("一時ディレクトリ作成失敗");

    // 無効な設定: languages が空
    let invalid_config = WakeruConfig {
      dictionary: DictionaryConfig {
        preset: DictionaryPreset::Ipadic,
        cache_dir: Some(temp_dir.path().join("dict")),
      },
      index: IndexConfig {
        data_dir: temp_dir.path().join("index"),
        writer_memory_bytes: 50_000_000,
        batch_commit_size: 1000,
        languages: vec![], // 無効: 空の言語リスト
        default_language: Language::En,
      },
      search: SearchConfig {
        default_limit: 10,
        max_limit: 100,
      },
      logging: LoggingConfig {
        level: LogLevel::Info,
      },
    };

    let result = WakeruService::init(&invalid_config);
    assert!(result.is_err());
  }
}
