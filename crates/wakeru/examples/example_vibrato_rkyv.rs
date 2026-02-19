//! vibrato-rkyv sample code

use dirs::cache_dir;
use std::error::Error;
use std::fs::create_dir_all;
use std::path::PathBuf;
use vibrato_rkyv::dictionary::PresetDictionaryKind;
use vibrato_rkyv::{Dictionary, Tokenizer}; // Safe modeなど設定する場合に使用

const TOKENIZE_NBEST_NUM: usize = 3;

fn main() -> Result<(), Box<dyn Error>> {
  // cache_dir はOSごとに以下のように定義されている
  // |Platform | Value                               | Example                      |
  // | ------- | ----------------------------------- | ---------------------------- |
  // | Linux   | `$XDG_CACHE_HOME` or `$HOME`/.cache | /home/alice/.cache           |
  // | macOS   | `$HOME`/Library/Caches              | /Users/Alice/Library/Caches  |
  // | Windows | `{FOLDERID_LocalAppData}`           | C:\Users\Alice\AppData\Local |
  // 以下の例では Windows の場合は `C:\Users\{user_name}\AppData\Local\.cache\vibrato-rkyv-assets
  //
  let cache_dir =
    cache_dir().unwrap_or_else(|| PathBuf::from(".cache")).join("vibrato-rkyv-assets");
  // cache_dir を必要なら作成する
  create_dir_all(&cache_dir)?;
  println!("cache_dir: {}", cache_dir.display());

  // 初回はPreset dictionaries をすべてダウンロード
  // 2回目以降はキャッシュからロードする
  // Presetの選択肢として UnidicCwj/Ipadic/UnidicCsj などを選択可能
  // https://clrd.ninjal.ac.jp/unidic/ 国立国語研究所
  // cwj: 現代書き言葉UniDic (W = Write)
  // csj: 現代話し言葉UniDic (S = Speak)
  // ipadic: 情報処理推進機構（IPA）が作成したコーパスをベースに開発された、日本語形態素解析エンジンMeCabの標準的な辞書
  let preset_dict = PresetDictionaryKind::UnidicCwj;
  let dict_dir_name = cache_dir.join(preset_dict.name());
  let dict = Dictionary::from_preset_with_download(preset_dict, &dict_dir_name)?;

  println!("dict files successfully donloaded & loaded");
  println!("selected preset dictionary: {}", preset_dict.name());

  // tokenizer の設定
  let tokenizer = Tokenizer::new(dict);
  let mut worker = tokenizer.new_worker();

  // サンプルテキスト1
  let input_text = "憲法記念日とは、みんなが住んでいる国日本で、みんなが守る約束ごとをはじめた日。1946年の11月3日の「文化の日」に約束ごとができて、1947年の5月3日「憲法記念日」にその約束ごとをみんなで守ることになりました。Official SDK For document parsing tasks, we strongly recommend using our official SDK. Compared with model-only inference, the SDK integrates PP-DocLayoutV3 and provides a complete, easy-to-use pipeline for document parsing, including layout analysis and structured output generation. This significantly reduces the engineering overhead required to build end-to-end document intelligence systems. Note that the SDK is currently designed for document parsing tasks only. For information extraction tasks, please refer to the following section and run inference directly with the model.";
  println!("Target text: {}", input_text);
  worker.reset_sentence(input_text);
  worker.tokenize();
  println!("Tokenization result: ");
  for token in worker.token_iter() {
    println!("{}\t{}", token.surface(), token.feature());
  }

  // サンプルテキスト2 (N-Best)
  let input_text = "憲法記念日とは、みんなが住んでいる国日本で、みんなが守る約束ごとをはじめた日。1946年の11月3日の「文化の日」に約束ごとができて、1947年の5月3日「憲法記念日」にその約束ごとをみんなで守ることになりました。";
  println!("Target text: {}", input_text);
  worker.reset_sentence(input_text);
  worker.tokenize_nbest(TOKENIZE_NBEST_NUM);
  println!("===== normal tokenizer =====");
  println!(
    "Tokenization result(N-best): requested N={}",
    TOKENIZE_NBEST_NUM
  );

  // 見つかったパス数を取得
  let num_paths = worker.num_nbest_paths();
  println!("Found {} paths", num_paths);

  for path_idx in 0..num_paths {
    println!("====== {}-best ======", path_idx + 1);
    if let Some(cost) = worker.path_cost(path_idx) {
      println!("Cost: {}", cost);
    }
    if let Some(token_iter) = worker.nbest_token_iter(path_idx) {
      for token in token_iter {
        println!("{}\t{}", token.surface(), token.feature());
      }
    }
  }

  Ok(())
}
