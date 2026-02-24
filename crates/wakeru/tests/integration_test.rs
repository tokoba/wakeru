//! crates/wakeru/tests/integration_test.rs
//!
//! End-to-end integration test.
//! Verifies the entire flow: Load dictionary -> Build tokenizer -> Create index ->
//! Add documents -> Search -> Verify results.

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

/// Check for the existence of dictionary cache as a prerequisite.
/// Skip test if cache does not exist.
fn setup_tokenizer() -> Option<Arc<TextAnalyzer>> {
  // Build DictionaryManager using preset dictionary (IPADIC)
  let manager = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic).ok()?;

  // Check if dictionary cache already exists (Skip test if not)
  let cache_dir = manager.cache_dir();
  if !cache_dir.join(PresetDictionaryKind::Ipadic.name()).exists() {
    eprintln!("No dictionary cache -> Skip test");
    return None;
  }

  // Load dictionary (Arc<Dictionary>)
  let dict = manager.load().ok()?;

  // Build VibratoTokenizer from shared dictionary and convert to TextAnalyzer
  let tokenizer = VibratoTokenizer::from_shared_dictionary(dict);
  let analyzer = TextAnalyzer::from(tokenizer);

  Some(Arc::new(analyzer))
}

/// Generate sample documents.
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

/// Integration test for basic search flow.
#[test]
fn end_to_end_search_flow() {
  // Skip test if no dictionary cache
  let analyzer = match setup_tokenizer() {
    Some(t) => t,
    None => return,
  };

  // Create index in temporary directory
  let tmp_dir = TempDir::new().expect("Failed to create temporary directory");

  // Create IndexManager (Create new if not exists)
  // Pass Language::Ja + Some(text_analyzer) as it is a Japanese index
  let index_manager =
    IndexManager::open_or_create(tmp_dir.path(), Language::Ja, Some((*analyzer).clone()))
      .expect("Failed to create index");

  // Add documents
  index_manager.add_documents(&sample_documents()).expect("Failed to add documents");

  // Initialize SearchEngine
  let search_engine = SearchEngine::new(
    index_manager.index(),
    *index_manager.fields(), // SchemaFields assumes Copy
    index_manager.language(),
  )
  .expect("Failed to initialize SearchEngine");

  // ── Test 1: Search for "東京" ──
  let results = search_engine.search("東京", 5).expect("Search failed");
  assert!(!results.is_empty(), "Search result for '東京' is empty");
  assert_eq!(results[0].doc_id, "chunk-001");

  // ── Test 2: Search for "寺院" ──
  let results = search_engine.search("寺院", 5).expect("Search failed");
  assert!(!results.is_empty(), "Search result for '寺院' is empty");

  // ── Test 3: Search for "Rust プログラミング" ──
  let results = search_engine.search("Rust プログラミング", 5).expect("Search failed");
  assert!(!results.is_empty(), "Search result for 'Rust' is empty");
  // Ensure tech document hits
  assert!(
    results.iter().any(|r| r.doc_id == "chunk-004"),
    "Rust document not found"
  );

  // ── Test 4: Non-existent keyword ──
  let results = search_engine.search("zzzzxxxx非存在語", 5).expect("Search failed");
  assert!(
    results.is_empty(),
    "Result returned for non-existent keyword"
  );
}

/// Search test on empty document set.
#[test]
fn search_on_empty_index() {
  // Skip test if no dictionary cache
  let analyzer = match setup_tokenizer() {
    Some(t) => t,
    None => return,
  };

  let tmp_dir = TempDir::new().expect("Failed to create temporary directory");

  // Create empty index
  let index_manager =
    IndexManager::open_or_create(tmp_dir.path(), Language::Ja, Some((*analyzer).clone()))
      .expect("Failed to create index");

  // Add empty document set (effectively does nothing)
  index_manager.add_documents(&[]).expect("Failed to add empty documents");

  let search_engine = SearchEngine::new(
    index_manager.index(),
    *index_manager.fields(), // SchemaFields assumes Copy
    index_manager.language(),
  )
  .expect("Failed to initialize SearchEngine");

  let results = search_engine.search("何か", 5).expect("Search failed");
  assert!(
    results.is_empty(),
    "Result returned on empty index (should be 0)"
  );
}
