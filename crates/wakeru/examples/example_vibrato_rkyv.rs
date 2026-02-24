//! vibrato-rkyv sample code

use dirs::cache_dir;
use std::error::Error;
use std::fs::create_dir_all;
use std::path::PathBuf;
use vibrato_rkyv::dictionary::PresetDictionaryKind;
use vibrato_rkyv::{Dictionary, Tokenizer}; // Used when configuring Safe mode etc.

const TOKENIZE_NBEST_NUM: usize = 3;

fn main() -> Result<(), Box<dyn Error>> {
  // cache_dir is defined for each OS as follows:
  // |Platform | Value                               | Example                      |
  // | ------- | ----------------------------------- | ---------------------------- |
  // | Linux   | `$XDG_CACHE_HOME` or `$HOME`/.cache | /home/alice/.cache           |
  // | macOS   | `$HOME`/Library/Caches              | /Users/Alice/Library/Caches  |
  // | Windows | `{FOLDERID_LocalAppData}`           | C:\Users\Alice\AppData\Local |
  // In the example below, for Windows: `C:\Users\{user_name}\AppData\Local\.cache\vibrato-rkyv-assets`
  //
  let cache_dir =
    cache_dir().unwrap_or_else(|| PathBuf::from(".cache")).join("vibrato-rkyv-assets");
  // Create cache_dir if necessary
  create_dir_all(&cache_dir)?;
  println!("cache_dir: {}", cache_dir.display());

  // Download all preset dictionaries on the first run
  // Load from cache from the second time onwards
  // Options for Preset: UnidicCwj/Ipadic/UnidicCsj etc.
  // https://clrd.ninjal.ac.jp/unidic/ NINJAL
  // cwj: UniDic for Contemporary Written Japanese (W = Write)
  // csj: UniDic for Spoken Japanese (S = Speak)
  // ipadic: Standard dictionary for Japanese morphological analysis engine MeCab, developed based on corpus created by IPA
  let preset_dict = PresetDictionaryKind::UnidicCwj;
  let dict_dir_name = cache_dir.join(preset_dict.name());
  let dict = Dictionary::from_preset_with_download(preset_dict, &dict_dir_name)?;

  println!("dict files successfully donloaded & loaded");
  println!("selected preset dictionary: {}", preset_dict.name());

  // Tokenizer configuration
  let tokenizer = Tokenizer::new(dict);
  let mut worker = tokenizer.new_worker();

  // Sample text 1
  let input_text = "憲法記念日とは、みんなが住んでいる国日本で、みんなが守る約束ごとをはじめた日。1946年の11月3日の「文化の日」に約束ごとができて、1947年の5月3日「憲法記念日」にその約束ごとをみんなで守ることになりました。Official SDK For document parsing tasks, we strongly recommend using our official SDK. Compared with model-only inference, the SDK integrates PP-DocLayoutV3 and provides a complete, easy-to-use pipeline for document parsing, including layout analysis and structured output generation. This significantly reduces the engineering overhead required to build end-to-end document intelligence systems. Note that the SDK is currently designed for document parsing tasks only. For information extraction tasks, please refer to the following section and run inference directly with the model.";
  println!("Target text: {}", input_text);
  worker.reset_sentence(input_text);
  worker.tokenize();
  println!("Tokenization result: ");
  for token in worker.token_iter() {
    println!("{}\t{}", token.surface(), token.feature());
  }

  // Sample text 2 (N-Best)
  let input_text = "憲法記念日とは、みんなが住んでいる国日本で、みんなが守る約束ごとをはじめた日。1946年の11月3日の「文化の日」に約束ごとができて、1947年の5月3日「憲法記念日」にその約束ごとをみんなで守ることになりました。";
  println!("Target text: {}", input_text);
  worker.reset_sentence(input_text);
  worker.tokenize_nbest(TOKENIZE_NBEST_NUM);
  println!("===== normal tokenizer =====");
  println!(
    "Tokenization result(N-best): requested N={}",
    TOKENIZE_NBEST_NUM
  );

  // Get number of found paths
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
