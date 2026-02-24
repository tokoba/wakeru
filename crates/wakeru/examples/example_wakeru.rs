//! wakeru crate example (refactored)
//!
//! Multi-language index support version (Japanese/English)

use tantivy::tokenizer::TextAnalyzer;
use tracing_subscriber::EnvFilter;
use vibrato_rkyv::dictionary::PresetDictionaryKind;
use wakeru::config::Language;
use wakeru::dictionary::DictionaryManager;
use wakeru::indexer::IndexManager;
use wakeru::models::{Document, SearchResult};
use wakeru::searcher::SearchEngine;
use wakeru::tokenizer::VibratoTokenizer;

/// Application common result type
type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Prepares dictionary, builds tokenizer, and opens/creates index.
///
/// Depending on `language`:
/// - Language::Ja: Vibrato + Japanese index
/// - Language::En: SimpleTokenizer + LowerCaser set on IndexManager side
fn init_index_manager(index_dir: &str, language: Language) -> AppResult<IndexManager> {
  match language {
    Language::Ja => {
      // Prepare Japanese dictionary
      let manager = DictionaryManager::with_preset(PresetDictionaryKind::UnidicCwj)?;
      let dict = manager.load()?;

      // Build tokenizer
      let tokenizer = VibratoTokenizer::from_shared_dictionary(dict);

      // Create TextAnalyzer from VibratoTokenizer
      let text_analyzer = TextAnalyzer::from(tokenizer);

      // Japanese index: Language::Ja + Some(text_analyzer)
      let index_manager =
        IndexManager::open_or_create(index_dir, Language::Ja, Some(text_analyzer))?;

      Ok(index_manager)
    }
    Language::En => {
      // English index: SimpleTokenizer + LowerCaser is
      // automatically registered in IndexManager::open_or_create, so no dictionary needed
      let index_manager = IndexManager::open_or_create(index_dir, Language::En, None)?;
      Ok(index_manager)
    }
  }
}

/// Function to add arbitrary set of Documents to index.
/// Receives as AsRef<[Document]> to flexibly accept Vec<Document>, &[Document], &Vec<Document> etc.
fn add_documents<D>(index_manager: &IndexManager, documents: D) -> AppResult<()>
where
  D: AsRef<[Document]>,
{
  let docs = documents.as_ref();

  if docs.is_empty() {
    // If empty, do nothing and treat as success
    return Ok(());
  }

  let report = index_manager.add_documents(docs)?;
  println!(
    "Addition result: total={}, added={}, skipped={}",
    report.total, report.added, report.skipped_duplicates
  );

  Ok(())
}

/// Function to perform BM25 search with arbitrary query string and return results.
/// SearchEngine is generated from index_manager.index() / fields() / language() each time,
/// keeping lifetime confined within this function to simplify borrow chain.
///
/// Note: Uses search_tokens_or() to tokenize query according to language and then perform OR search.
///       In Japanese, "京都の寺" is split into "京都" and "寺".
///       In English, it is tokenized by space separation + lowercasing (LowerCaser).
fn search(index_manager: &IndexManager, query: &str, limit: usize) -> AppResult<Vec<SearchResult>> {
  // Generate SearchEngine from index_manager's index() / fields() / language()
  let search_engine = SearchEngine::new(
    index_manager.index(),
    *index_manager.fields(),
    index_manager.language(),
  )?;
  // Morphological analysis + OR search with search_tokens_or()
  let results = search_engine.search_tokens_or(query, limit)?;
  Ok(results)
}

/// Function to display search results to standard output.
fn print_results(query: &str, results: &[SearchResult]) {
  println!("\nSearch results (Query: \"{}\"):", query);
  for result in results {
    println!(
      "  [{:.4}] {} | {} | {:?}",
      result.score, result.doc_id, result.text, result.metadata
    );
  }
}

fn main() -> AppResult<()> {
  // Initialize tracing_subscriber
  // Use RUST_LOG environment variable if set
  // Default: info for global, debug for wakeru, warn or above for tantivy
  let env_filter = EnvFilter::try_from_default_env()
    .unwrap_or_else(|_| EnvFilter::new("info,wakeru=debug,tantivy=warn"));
  tracing_subscriber::fmt().with_env_filter(env_filter).with_target(true).with_level(true).init();

  // 1. Initialize Japanese index
  let index_manager_ja = init_index_manager("./.index/ja", Language::Ja)?;

  // 2. Initialize English index
  let index_manager_en = init_index_manager("./.index/en", Language::En)?;

  // 3. Add Japanese documents
  let documents_ja = vec![
    Document::new("1", "doc-A", "東京タワーは東京の観光名所です").with_tag("category:tourism"),
    Document::new("2", "doc-A", "京都には金閣寺や銀閣寺があります").with_tag("category:tourism"),
    Document::new("3", "doc-B", "中国には万里の長城があります。").with_tag("category:tourism"),
    Document::new("4", "doc-C", "大阪の道頓堀は美味しい食べ物で有名です").with_tag("category:food"),
    Document::new("5", "doc-C", "北海道では冬に雪祭りが開催されます").with_tag("category:event"),
    Document::new("6", "doc-D", "浅草の雷門は歴史的な観光スポットです")
      .with_tag("category:tourism"),
    Document::new("7", "doc-E", "箱根は温泉で有名な観光地です").with_tag("category:onsen"),
    Document::new("8", "doc-F", "新しいアルバムが来月リリースされます").with_tag("category:music"),
    Document::new(
      "9",
      "doc-G",
      "ライブコンサートのチケットをオンラインで購入しました",
    )
    .with_tag("category:music"),
    Document::new(
      "10",
      "doc-H",
      "最新のヘッドホンはノイズキャンセリングが優秀です",
    )
    .with_tag("category:shopping"),
    Document::new("11", "doc-I", "セールで冬物コートを2着買いました").with_tag("category:shopping"),
    Document::new(
      "12",
      "doc-J",
      "明日は重要なプレゼンがあるので資料を仕上げます",
    )
    .with_tag("category:work"),
    Document::new(
      "13",
      "doc-K",
      "チームミーティングで新しいプロジェクトのスコープを決定しました",
    )
    .with_tag("category:work"),
    Document::new("14", "doc-L", "昨夜はフットサルで汗を流しました").with_tag("category:sports"),
    Document::new(
      "15",
      "doc-M",
      "マラソン大会に向けて週5回ランニングしています",
    )
    .with_tag("category:sports"),
    Document::new("16", "doc-N", "料理教室でパスタの作り方を学びました")
      .with_tag("category:cooking"),
    Document::new("17", "doc-O", "ベランダでハーブを育て始めました").with_tag("category:gardening"),
    Document::new(
      "18",
      "doc-P",
      "最近は瞑想を習慣にしてストレス管理をしています",
    )
    .with_tag("category:wellness"),
    Document::new("19", "doc-Q", "週末にボードゲーム会を開く予定です")
      .with_tag("category:entertainment"),
    Document::new(
      "20",
      "doc-R",
      "新しい言語を学ぶためにオンラインコースに登録しました",
    )
    .with_tag("category:education"),
    Document::new("21", "doc-S", "写真撮影が趣味で週末は街を散策します").with_tag("category:hobby"),
    Document::new("22", "doc-T", "近所でフリーマーケットが開催されていました")
      .with_tag("category:community"),
    Document::new("23", "doc-U", "ペットの健康診断に動物病院へ行きました").with_tag("category:pet"),
    Document::new("24", "doc-V", "最近はDIYで棚を自作するのに夢中です").with_tag("category:DIY"),
    Document::new("25", "doc-W", "エコバッグを持ち歩いて環境に配慮しています")
      .with_tag("category:environment"),
    Document::new("26", "doc-X", "週末は友人とカフェで語り合いました").with_tag("category:social"),
  ];

  add_documents(&index_manager_ja, &documents_ja)?;

  // 4. Add English documents
  let documents_en = vec![
    Document::new(
      "en-1",
      "doc-en-A",
      "Tokyo Tower is a famous sightseeing spot in Japan",
    )
    .with_tag("category:tourism"),
    Document::new(
      "en-2",
      "doc-en-B",
      "Kyoto has many beautiful temples such as Kinkaku-ji and Ginkaku-ji",
    )
    .with_tag("category:tourism"),
    Document::new(
      "en-3",
      "doc-en-C",
      "Osaka is well known for its delicious street food in Dotonbori",
    )
    .with_tag("category:food"),
    Document::new(
      "en-4",
      "doc-en-D",
      "I bought a new pair of noise cancelling headphones on sale",
    )
    .with_tag("category:shopping"),
    Document::new(
      "en-5",
      "doc-en-E",
      "We have an important team meeting about a new project tomorrow",
    )
    .with_tag("category:work"),
    Document::new(
      "en-6",
      "doc-en-F",
      "Running a marathon is one of my long term personal goals",
    )
    .with_tag("category:sports"),
    Document::new(
      "en-7",
      "doc-en-G",
      "I started taking an online course to learn the Rust programming language",
    )
    .with_tag("category:education"),
  ];

  add_documents(&index_manager_en, &documents_en)?;

  // 5. Search in Japanese index
  println!("\n===== Search in Japanese Index =====");
  let search_limit = 10;

  let query = "東京観光";
  let results = search(&index_manager_ja, query, search_limit)?;
  print_results(query, &results);

  let query = "Rust プログラミング";
  let results = search(&index_manager_ja, query, search_limit)?;
  print_results(query, &results);

  let query = "京都の寺";
  let results = search(&index_manager_ja, query, search_limit)?;
  print_results(query, &results);

  let query = "京都 寺";
  let results = search(&index_manager_ja, query, search_limit)?;
  print_results(query, &results);

  let query = "東京 寺";
  let results = search(&index_manager_ja, query, search_limit)?;
  print_results(query, &results);

  let query = "仕事と健康";
  let results = search(&index_manager_ja, query, search_limit)?;
  print_results(query, &results);

  let query = "寺";
  let results = search(&index_manager_ja, query, search_limit)?;
  print_results(query, &results);

  // 6. Search in English index
  println!("\n===== Search in English Index =====");

  // Document is found even if searched in uppercase because of LowerCaser
  let query = "TOKYO tower temple";
  let results = search(&index_manager_en, query, search_limit)?;
  print_results(query, &results);

  let query = "Kyoto temple";
  let results = search(&index_manager_en, query, search_limit)?;
  print_results(query, &results);

  let query = "street food Osaka";
  let results = search(&index_manager_en, query, search_limit)?;
  print_results(query, &results);

  let query = "noise cancelling headphones";
  let results = search(&index_manager_en, query, search_limit)?;
  print_results(query, &results);

  let query = "Rust programming course";
  let results = search(&index_manager_en, query, search_limit)?;
  print_results(query, &results);

  Ok(())
}
