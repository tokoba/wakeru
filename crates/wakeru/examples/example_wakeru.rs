//! wakeru crate example (refactored)
//!
//! 日本語・英語の多言語インデックス対応版

use tantivy::tokenizer::TextAnalyzer;
use tracing_subscriber::EnvFilter;
use vibrato_rkyv::dictionary::PresetDictionaryKind;
use wakeru::config::Language;
use wakeru::dictionary::DictionaryManager;
use wakeru::indexer::IndexManager;
use wakeru::models::{Document, SearchResult};
use wakeru::searcher::SearchEngine;
use wakeru::tokenizer::VibratoTokenizer;

/// アプリケーション共通の結果型
type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

/// 辞書の準備・トークナイザー構築・インデックスの open/create までを行う。
///
/// `language` に応じて:
/// - Language::Ja: Vibrato + 日本語インデックス
/// - Language::En: SimpleTokenizer + LowerCaser を IndexManager 側で設定
fn init_index_manager(index_dir: &str, language: Language) -> AppResult<IndexManager> {
  match language {
    Language::Ja => {
      // 日本語辞書の準備
      let manager = DictionaryManager::with_preset(PresetDictionaryKind::UnidicCwj)?;
      let dict = manager.load()?;

      // トークナイザーの構築
      let tokenizer = VibratoTokenizer::from_shared_dictionary(dict);

      // VibratoTokenizer から TextAnalyzer を作成
      let text_analyzer = TextAnalyzer::from(tokenizer);

      // 日本語インデックス: Language::Ja + Some(text_analyzer)
      let index_manager =
        IndexManager::open_or_create(index_dir, Language::Ja, Some(text_analyzer))?;

      Ok(index_manager)
    }
    Language::En => {
      // 英語インデックス: SimpleTokenizer + LowerCaser は
      // IndexManager::open_or_create 内で自動登録されるので辞書不要
      let index_manager = IndexManager::open_or_create(index_dir, Language::En, None)?;
      Ok(index_manager)
    }
  }
}

/// 任意の Document 集合をインデックスに追加する関数。
/// Vec<Document>, &[Document], &Vec<Document> などを柔軟に受け取れるよう AsRef<[Document]> で受ける。
fn add_documents<D>(index_manager: &IndexManager, documents: D) -> AppResult<()>
where
  D: AsRef<[Document]>,
{
  let docs = documents.as_ref();

  if docs.is_empty() {
    // 何も入っていなければ何もしないで成功扱い
    return Ok(());
  }

  let report = index_manager.add_documents(docs)?;
  println!(
    "追加結果: total={}, added={}, skipped={}",
    report.total, report.added, report.skipped_duplicates
  );

  Ok(())
}

/// 任意のクエリ文字列で BM25 検索を行い、結果を返す関数。
/// SearchEngine は IndexManager の index() / fields() / language() から都度生成し、
/// ライフタイムをこの関数内に閉じ込めて借用チェーンをシンプルに保つ。
///
/// 注: search_tokens_or() を使用して、クエリを言語に応じたトークナイズした上で OR 検索を行います。
///      日本語では「京都の寺」→「京都」「寺」に分割され、
///      英語ではスペース区切り + 小文字化（LowerCaser）でトークナイズされます。
fn search(index_manager: &IndexManager, query: &str, limit: usize) -> AppResult<Vec<SearchResult>> {
  // SearchEngine は IndexManager の index() / fields() / language() から生成
  let search_engine = SearchEngine::new(
    index_manager.index(),
    *index_manager.fields(),
    index_manager.language(),
  )?;
  // search_tokens_or() で形態素解析 + OR 検索
  let results = search_engine.search_tokens_or(query, limit)?;
  Ok(results)
}

/// 検索結果を標準出力に表示する関数。
fn print_results(query: &str, results: &[SearchResult]) {
  println!("\n検索結果 (クエリ: \"{}\"):", query);
  for result in results {
    println!(
      "  [{:.4}] {} | {} | {:?}",
      result.score, result.doc_id, result.text, result.metadata
    );
  }
}

fn main() -> AppResult<()> {
  // tracing_subscriber の初期化
  // RUST_LOG 環境変数が設定されていればそれを使用
  // デフォルト: 全体は info、wakeru は debug、tantivy は warn 以上
  let env_filter = EnvFilter::try_from_default_env()
    .unwrap_or_else(|_| EnvFilter::new("info,wakeru=debug,tantivy=warn"));
  tracing_subscriber::fmt().with_env_filter(env_filter).with_target(true).with_level(true).init();

  // 1. 日本語インデックスの初期化
  let index_manager_ja = init_index_manager("./.index/ja", Language::Ja)?;

  // 2. 英語インデックスの初期化
  let index_manager_en = init_index_manager("./.index/en", Language::En)?;

  // 3. 日本語ドキュメントを追加
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

  // 4. 英語ドキュメントを追加
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

  // 5. 日本語インデックスで検索
  println!("\n===== 日本語インデックスでの検索 =====");
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

  // 6. 英語インデックスで検索
  println!("\n===== 英語インデックスでの検索 =====");

  // 大文字で検索しても LowerCaser により小文字化されて検索される
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
