//! crates/wakeru/tests/integration_test.rs
//!
//! エンドツーエンド統合テスト。
//! 辞書ロード → トークナイザー構築 → インデックス作成 →
//! ドキュメント追加 → 検索 → 結果検証 の全フローを確認する。

use std::sync::Arc;

use tantivy::tokenizer::TextAnalyzer;
use tempfile::TempDir;
use vibrato_rkyv::dictionary::PresetDictionaryKind;

use wakeru::config::Language;
use wakeru::dictionary::DictionaryManager;
use wakeru::indexer::IndexManager;
use wakeru::models::Document;
use wakeru::searcher::SearchEngine;
use wakeru::tokenizer::vibrato_tokenizer::VibratoTokenizer;

/// 辞書キャッシュの存在を前提条件としてチェックする。
/// キャッシュがなければテストをスキップする。
fn setup_tokenizer() -> Option<Arc<TextAnalyzer>> {
  // プリセット辞書 (IPADIC) を使う DictionaryManager を構築
  let manager = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic).ok()?;

  // 既に辞書キャッシュが存在するか確認（なければテストをスキップ）
  let cache_dir = manager.cache_dir();
  if !cache_dir.join(PresetDictionaryKind::Ipadic.name()).exists() {
    eprintln!("辞書キャッシュなし → テストスキップ");
    return None;
  }

  // 辞書ロード（Arc<Dictionary>）
  let dict = manager.load().ok()?;

  // 共有辞書から VibratoTokenizer を構築し、TextAnalyzer に変換
  let tokenizer = VibratoTokenizer::from_shared_dictionary(dict);
  let analyzer = TextAnalyzer::from(tokenizer);

  Some(Arc::new(analyzer))
}

/// サンプルドキュメント群を生成する。
fn sample_documents() -> Vec<Document> {
  vec![
    Document::new(
      "chunk-001",
      "doc-travel-01",
      "東京タワーやスカイツリーは東京を代表する観光名所です。浅草寺も人気があります。",
    )
    .with_tags(["category:tourism", "region:kanto"]),
    Document::new(
      "chunk-002",
      "doc-travel-01",
      "京都には金閣寺、銀閣寺、清水寺など多くの歴史的な寺院があります。",
    )
    .with_tags(["category:tourism", "region:kansai"]),
    Document::new(
      "chunk-003",
      "doc-travel-02",
      "大阪はたこ焼きやお好み焼きなどの粉物文化が有名です。道頓堀には多くの飲食店があります。",
    )
    .with_tags(["category:food", "region:kansai"]),
    Document::new(
      "chunk-004",
      "doc-tech-01",
      "Rust は安全で高速なプログラミング言語です。所有権システムによりメモリ安全を保証します。",
    )
    .with_tags(["category:tech"]),
    Document::new(
      "chunk-005",
      "doc-tech-01",
      "Tantivy は Rust で書かれた全文検索エンジンです。BM25 スコアリングをサポートしています。",
    )
    .with_tags(["category:tech"]),
  ]
}

/// 基本的な検索フローの統合テスト。
#[test]
fn end_to_end_search_flow() {
  // 辞書キャッシュが無ければテストをスキップ
  let analyzer = match setup_tokenizer() {
    Some(t) => t,
    None => return,
  };

  // 一時ディレクトリにインデックスを作成
  let tmp_dir = TempDir::new().expect("一時ディレクトリ作成失敗");

  // IndexManager 作成（存在しなければ新規作成）
  // 日本語インデックスなので Language::Ja + Some(text_analyzer) を渡す
  let index_manager =
    IndexManager::open_or_create(tmp_dir.path(), Language::Ja, Some((*analyzer).clone()))
      .expect("インデックス作成失敗");

  // ドキュメント追加
  index_manager.add_documents(&sample_documents()).expect("ドキュメント追加失敗");

  // SearchEngine 初期化
  let search_engine = SearchEngine::new(
    index_manager.index(),
    *index_manager.fields(), // SchemaFields は Copy を想定
    index_manager.language(),
  )
  .expect("SearchEngine 初期化失敗");

  // ── テスト1: 「東京」で検索 ──
  let results = search_engine.search("東京", 5).expect("検索失敗");
  assert!(!results.is_empty(), "「東京」の検索結果が空");
  assert_eq!(results[0].doc_id, "chunk-001");

  // ── テスト2: 「寺院」で検索 ──
  let results = search_engine.search("寺院", 5).expect("検索失敗");
  assert!(!results.is_empty(), "「寺院」の検索結果が空");

  // ── テスト3: 「Rust プログラミング」で検索 ──
  let results = search_engine.search("Rust プログラミング", 5).expect("検索失敗");
  assert!(!results.is_empty(), "「Rust」の検索結果が空");
  // 技術系ドキュメントがヒットすること
  assert!(
    results.iter().any(|r| r.doc_id == "chunk-004"),
    "Rust のドキュメントがヒットしていません"
  );

  // ── テスト4: 存在しないキーワード ──
  let results = search_engine.search("zzzzxxxx非存在語", 5).expect("検索失敗");
  assert!(
    results.is_empty(),
    "存在しないキーワードで結果が返っています"
  );
}

/// 空ドキュメントセットでの検索テスト。
#[test]
fn search_on_empty_index() {
  // 辞書キャッシュが無ければテストをスキップ
  let analyzer = match setup_tokenizer() {
    Some(t) => t,
    None => return,
  };

  let tmp_dir = TempDir::new().expect("一時ディレクトリ作成失敗");

  // 空インデックスを作成
  let index_manager =
    IndexManager::open_or_create(tmp_dir.path(), Language::Ja, Some((*analyzer).clone()))
      .expect("インデックス作成失敗");

  // 空のドキュメントセットを追加（実質何もしない）
  index_manager.add_documents(&[]).expect("空ドキュメント追加失敗");

  let search_engine = SearchEngine::new(
    index_manager.index(),
    *index_manager.fields(), // SchemaFields は Copy を想定
    index_manager.language(),
  )
  .expect("SearchEngine 初期化失敗");

  let results = search_engine.search("何か", 5).expect("検索失敗");
  assert!(
    results.is_empty(),
    "空インデックスで結果が返っています（0件であるべき）"
  );
}
