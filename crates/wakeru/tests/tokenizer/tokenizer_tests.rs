//! integration tests for tokenizer module
//! tests/tokenizer_tests.rs

use tantivy::tokenizer::{TokenStream, Tokenizer};
use vibrato_rkyv::dictionary::PresetDictionaryKind;
use wakeru::dictionary::DictionaryManager;
use wakeru::tokenizer::vibrato_tokenizer::VibratoTokenizer;

/// Verify that VibratoTokenizer returns correct token sequence.
///
/// Requires dictionary cache (Must be downloaded beforehand with `cargo test -- --ignored`)
#[test]
fn tokenize_basic_sentence() {
  let manager = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic)
    .expect("Failed to build DictionaryManager");

  let cache_dir = manager.cache_dir();
  if !cache_dir.join(PresetDictionaryKind::Ipadic.name()).exists() {
    eprintln!("Skipping as dictionary cache does not exist");
    return;
  }

  let dict = manager.load().expect("Failed to load dictionary");

  // Build VibratoTokenizer (Pass Arc<Dictionary> as is)
  let mut tokenizer = VibratoTokenizer::from_shared_dictionary(dict);

  // Execute tokenization
  let mut stream = tokenizer.token_stream("東京タワーは東京の観光名所です");

  // Collect tokens
  let mut tokens = Vec::new();
  while stream.advance() {
    tokens.push(stream.token().text.clone());
  }

  // Basic assertions
  assert!(!tokens.is_empty(), "Tokens are empty");

  // Verify content words are included
  // (Particles "は", "の", "です" are expected to be excluded by part-of-speech filter)
  println!("Tokens: {:?}", tokens);
  assert!(
    tokens.contains(&"東京".to_string()),
    "Does not contain '東京'"
  );

  // Verify particles are excluded
  assert!(
    !tokens.contains(&"は".to_string()),
    "Particle 'は' is not excluded"
  );
  assert!(
    !tokens.contains(&"の".to_string()),
    "Particle 'の' is not excluded"
  );
}

/// Verify that byte offsets are correct.
#[test]
fn verify_byte_offsets() {
  let manager = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic)
    .expect("Failed to build DictionaryManager");

  let cache_dir = manager.cache_dir();
  if !cache_dir.join(PresetDictionaryKind::Ipadic.name()).exists() {
    eprintln!("Skipping as dictionary cache does not exist");
    return;
  }

  let dict = manager.load().expect("Failed to load dictionary");

  // Build VibratoTokenizer (Pass Arc<Dictionary> as is)
  let mut tokenizer = VibratoTokenizer::from_shared_dictionary(dict);

  let text = "東京タワー";
  let mut stream = tokenizer.token_stream(text);

  while stream.advance() {
    let token = stream.token();

    // Verify offset is within the byte range of original text
    assert!(
      token.offset_from <= token.offset_to,
      "offset_from({}) > offset_to({})",
      token.offset_from,
      token.offset_to,
    );
    assert!(
      token.offset_to <= text.len(),
      "offset_to({}) exceeds text length({})",
      token.offset_to,
      text.len(),
    );

    // Verify correct slice of original text can be obtained from offset
    let slice = &text[token.offset_from..token.offset_to];
    assert_eq!(
      slice, token.text,
      "Offset slice does not match token text"
    );
  }
}
