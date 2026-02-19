//! tokenizer モジュール用統合テスト
//! tests/tokenizer_tests.rs

use tantivy::tokenizer::{TokenStream, Tokenizer};
use vibrato_rkyv::dictionary::PresetDictionaryKind;
use wakeru::dictionary::DictionaryManager;
use wakeru::tokenizer::vibrato_tokenizer::VibratoTokenizer;

/// VibratoTokenizer が正しくトークン列を返すことを確認。
///
/// 辞書キャッシュが必要（事前に `cargo test -- --ignored` で辞書ダウンロード済みであること）
#[test]
fn tokenize_basic_sentence() {
  let manager = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic)
    .expect("DictionaryManager 構築失敗");

  let cache_dir = manager.cache_dir();
  if !cache_dir.join(PresetDictionaryKind::Ipadic.name()).exists() {
    eprintln!("辞書キャッシュが存在しないためスキップ");
    return;
  }

  let dict = manager.load().expect("辞書ロード失敗");

  // VibratoTokenizer を構築（Arc<Dictionary> をそのまま渡す）
  let mut tokenizer = VibratoTokenizer::from_shared_dictionary(dict);

  // トークナイズを実行
  let mut stream = tokenizer.token_stream("東京タワーは東京の観光名所です");

  // トークンを収集
  let mut tokens = Vec::new();
  while stream.advance() {
    tokens.push(stream.token().text.clone());
  }

  // 基本的なアサーション
  assert!(!tokens.is_empty(), "トークンが空です");

  // 内容語が含まれていることを確認
  // （助詞「は」「の」「です」は品詞フィルタで除外される想定）
  println!("トークン: {:?}", tokens);
  assert!(
    tokens.contains(&"東京".to_string()),
    "「東京」が含まれていません"
  );

  // 助詞が除外されていることを確認
  assert!(
    !tokens.contains(&"は".to_string()),
    "助詞「は」が除外されていません"
  );
  assert!(
    !tokens.contains(&"の".to_string()),
    "助詞「の」が除外されていません"
  );
}

/// バイトオフセットが正しいことを確認。
#[test]
fn verify_byte_offsets() {
  let manager = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic)
    .expect("DictionaryManager 構築失敗");

  let cache_dir = manager.cache_dir();
  if !cache_dir.join(PresetDictionaryKind::Ipadic.name()).exists() {
    eprintln!("辞書キャッシュが存在しないためスキップ");
    return;
  }

  let dict = manager.load().expect("辞書ロード失敗");

  // VibratoTokenizer を構築（Arc<Dictionary> をそのまま渡す）
  let mut tokenizer = VibratoTokenizer::from_shared_dictionary(dict);

  let text = "東京タワー";
  let mut stream = tokenizer.token_stream(text);

  while stream.advance() {
    let token = stream.token();

    // オフセットが元テキストのバイト範囲内であることを確認
    assert!(
      token.offset_from <= token.offset_to,
      "offset_from({}) > offset_to({})",
      token.offset_from,
      token.offset_to,
    );
    assert!(
      token.offset_to <= text.len(),
      "offset_to({}) がテキスト長({})を超えています",
      token.offset_to,
      text.len(),
    );

    // オフセットから元テキストのスライスが正しく取得できることを確認
    let slice = &text[token.offset_from..token.offset_to];
    assert_eq!(
      slice, token.text,
      "オフセットスライスがトークンテキストと一致しません"
    );
  }
}
