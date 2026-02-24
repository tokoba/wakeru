//! Tokenizer for Tantivy using vibrato

use std::sync::Arc;
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};
use tracing::debug;
use vibrato_rkyv::Dictionary;
use vibrato_rkyv::Tokenizer as VibratoImpl;

/// Japanese Tokenizer for Tantivy using Vibrato-rkyv
///
/// - Stateless (only holds dictionary reference)
/// - `Clone + Send + Sync`
/// - Implements Tantivy's `Tokenizer` trait
#[derive(Clone)]
pub struct VibratoTokenizer {
  inner: VibratoImpl,
}

/// Implementation of Tantivy's TokenStream trait
///
/// - No lifetime parameters (fully owned type)
/// - Consumes token sequence sequentially with `IntoIter`
/// - Performs `token.position += 1` with `advance`
pub struct VibratoTokenStream {
  /// Iterator of (Surface form, Start byte, End byte)
  tokens: std::vec::IntoIter<(String, usize, usize)>,

  /// Tantivy's Token (overwritten and reused every time)
  token: Token,
}

impl VibratoTokenizer {
  /// Constructs a tokenizer from an already loaded Dictionary
  ///
  /// Constructor corresponding to `vibrato_rkyv::Tokenizer::new(dict)`.
  pub fn from_dictionary(dict: Dictionary) -> Self {
    Self {
      inner: VibratoImpl::new(dict),
    }
  }

  /// Constructs a tokenizer from a shared dictionary (`Arc<Dictionary>`).
  ///
  /// Use this when the dictionary is shared via `Arc`, such as `DictionaryManager::load()`.
  ///
  /// Internally calls `vibrato_rkyv::Tokenizer::from_shared_dictionary(dict)`.
  ///
  /// # Examples
  /// ```rust,no_run
  /// # use wakeru::dictionary::DictionaryManager;
  /// # use wakeru::tokenizer::vibrato_tokenizer::VibratoTokenizer;
  /// # use vibrato_rkyv::dictionary::PresetDictionaryKind;
  /// let manager = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic).unwrap();
  /// let dict = manager.load().unwrap();
  /// let tokenizer = VibratoTokenizer::from_shared_dictionary(dict);
  /// ```
  pub fn from_shared_dictionary(dict: Arc<Dictionary>) -> Self {
    Self {
      inner: VibratoImpl::from_shared_dictionary(dict),
    }
  }
}

impl Tokenizer for VibratoTokenizer {
  // Use owned stream without lifetime parameters
  type TokenStream<'a> = VibratoTokenStream;

  /// Generates TokenStream from `&mut self` (mutable reference)
  fn token_stream<'a>(&'a mut self, input_text: &'a str) -> Self::TokenStream<'a> {
    // worker holds lattice for analysis and calculation area.
    // Created each time
    let mut worker = self.inner.new_worker();

    // Set string and execute analysis with normal tokenizer
    worker.reset_sentence(input_text);
    worker.tokenize();

    // Log input text
    debug!(input_text = %input_text, "Start morphological analysis");

    // Accumulate Vibrato results in Vec once, then convert to IntoIter
    let mut tokens = Vec::with_capacity(worker.num_tokens());
    // Part-of-speech filtering etc. can be added in this code block if needed
    // e.g.) Exclude particles and symbols to reduce index size
    for token in worker.token_iter() {
      let surface = token.surface();
      let feature = token.feature();
      let indexed = should_index(feature);

      // Debug log for each token
      debug!(
        surface = %surface,
        ?feature,
        start = token.range_byte().start,
        end = token.range_byte().end,
        indexed,
        "Token"
      );

      if indexed {
        tokens.push((
          surface.to_string(),
          // Manage offset in bytes instead of characters to match tantivy specification
          // range_char() is prohibited
          token.range_byte().start,
          token.range_byte().end,
        ));
      }
    }

    // Log indexed tokens
    debug!(
      input_text = %input_text,
      total_tokens = worker.num_tokens(),
      indexed_tokens = tokens.len(),
      "Morphological analysis completed"
    );

    VibratoTokenStream {
      tokens: tokens.into_iter(),
      token: Token::default(),
    }
  }
}

/// Part-of-speech filtering
///
/// Detailed version excluding particles, auxiliary verbs, symbols, fillers, interjections, conjunctions, prefixes, adnominals,
/// and excluding pronouns and non-independent nouns among nouns.
///
/// ## Suffix support in UniDic-based dictionaries
/// In UniDic-based dictionaries, "Kinkakuji" is split into "Kinkaku/ji", and "ji" is analyzed as `Suffix,Nominal`.
/// We want to treat "ji", "eki" (station), "onsen" (hot spring), etc. attached to place names as meaningful content words,
/// so `Suffix,Nominal` is included in the index target.
pub fn should_index(feature: &str) -> bool {
  // ─── Highest priority: Parts of speech to exclude ───
  // Particle, Auxiliary verb, Symbol, Filler, Interjection, Conjunction, Prefix, Adnominal
  if feature.starts_with("助詞")
    || feature.starts_with("助動詞")
    || feature.starts_with("記号")
    || feature.starts_with("フィラー")
    || feature.starts_with("感動詞")
    || feature.starts_with("接続詞")
    || feature.starts_with("接頭詞")
    || feature.starts_with("連体詞")
  {
    return false;
  }

  // ─── UniDic: Treat Suffix,Nominal as noun equivalent ───
  // Example: "接尾辞,名詞的,一般,*,*,*,寺,テラ,寺,テラ,*,*,*,*,*,*"
  // Treat "ji", "eki", "onsen" etc. attached to place names as meaningful content words
  if feature.starts_with("接尾辞,名詞的") {
    return true;
  }

  // ─── Detailed classification check for Nouns ───
  if feature.starts_with("名詞") {
    // Exclude: Pronoun, Non-independent
    if feature.starts_with("名詞,代名詞") || feature.starts_with("名詞,非自立") {
      return false;
    }
    // Include other nouns
    return true;
  }

  // ─── Include all Verbs and Adjectives ───
  if feature.starts_with("動詞") || feature.starts_with("形容詞") {
    return true;
  }

  // ─── Include Adjectival Nouns (UniDic) as content words ───
  // Words like "kireida", "shizukada" (adjectival verbs)
  if feature.starts_with("形状詞") {
    return true;
  }

  // ─── Adverbs: Include only General ───
  if feature.starts_with("副詞") {
    return feature.starts_with("副詞,一般");
  }

  // ─── Exclude others ───
  false
}

impl TokenStream for VibratoTokenStream {
  /// Advances to the next token.
  ///
  /// - `next()` 1 item from `tokens` `IntoIter` and overwrite `self.token`
  /// - Increment position with `self.token.position += 1`
  fn advance(&mut self) -> bool {
    if let Some((surface, start, end)) = self.tokens.next() {
      // Update Token content (String is reused by move)
      self.token.text = surface;
      self.token.offset_from = start;
      self.token.offset_to = end;

      // Tantivy's Token::default() is initialized with position = usize::MAX,
      // so normal += 1 causes overflow panic.
      // Using wrapping_add(1) results in usize::MAX + 1 = 0, allowing correct count start from 0.
      self.token.position = self.token.position.wrapping_add(1);
      // Fixed to 1 as it is word unit
      self.token.position_length = 1;

      true
    } else {
      false
    }
  }

  fn token(&self) -> &Token {
    &self.token
  }

  fn token_mut(&mut self) -> &mut Token {
    &mut self.token
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Verify that content words (Noun, General) are indexed
  #[test]
  fn index_common_noun() {
    assert!(should_index("名詞,一般,*,*,*,*,東京,トウキョウ,トーキョー"));
  }

  /// Verify that proper nouns are indexed
  #[test]
  fn index_proper_noun() {
    assert!(should_index(
      "名詞,固有名詞,地域,一般,*,*,東京,トウキョウ,トーキョー"
    ));
  }

  /// Verify that Sa-irregular conjugation nouns are indexed
  #[test]
  fn index_sahen_noun() {
    assert!(should_index("名詞,サ変接続,*,*,*,*,検索,ケンサク,ケンサク"));
  }

  /// Verify that verbs are indexed
  #[test]
  fn index_verb() {
    assert!(should_index("動詞,自立,*,*,一段,連用形,食べる,タベ,タベ"));
  }

  /// Verify that adjectives are indexed
  #[test]
  fn index_adjective() {
    assert!(should_index(
      "形容詞,自立,*,*,形容詞・アウオ段,基本形,高い,タカイ,タカイ"
    ));
  }

  /// Verify that particles are excluded
  #[test]
  fn exclude_particle() {
    assert!(!should_index("助詞,格助詞,一般,*,*,*,が,ガ,ガ"));
  }

  /// Verify that symbols are excluded
  #[test]
  fn exclude_symbol() {
    assert!(!should_index("記号,句点,*,*,*,*,。,。,。"));
  }

  /// Verify that nouns, pronouns are excluded
  #[test]
  fn exclude_pronoun() {
    assert!(!should_index("名詞,代名詞,一般,*,*,*,これ,コレ,コレ"));
  }

  /// Verify that nouns, non-independent are excluded
  #[test]
  fn exclude_dependent_noun() {
    assert!(!should_index("名詞,非自立,一般,*,*,*,こと,コト,コト"));
  }

  /// Verify that conjunctions are excluded
  #[test]
  fn exclude_conjunction() {
    assert!(!should_index("接続詞,*,*,*,*,*,しかし,シカシ,シカシ"));
  }

  /// Verify that auxiliary verbs are excluded
  #[test]
  fn exclude_auxiliary_verb() {
    assert!(!should_index(
      "助動詞,*,*,*,特殊・デス,基本形,です,デス,デス"
    ));
  }

  /// Verify that fillers are excluded
  #[test]
  fn exclude_filler() {
    assert!(!should_index("フィラー,*,*,*,*,*,えー,エー,エー"));
  }

  /// Verify that interjections are excluded
  #[test]
  fn exclude_interjection() {
    assert!(!should_index("感動詞,*,*,*,*,*,はい,ハイ,ハイ"));
  }

  /// Verify that UniDic suffix, nominal are indexed
  /// Assuming "ji" when "Kinkakuji" is split into "Kinkaku/ji"
  #[test]
  fn index_suffix_noun_for_unidic() {
    assert!(should_index(
      "接尾辞,名詞的,一般,*,*,*,寺,テラ,寺,テラ,*,*,*,*,*,*"
    ));
  }

  /// Verify that UniDic suffix, nominal (place name related) are indexed
  #[test]
  fn index_suffix_noun_place_names() {
    // Station
    assert!(should_index(
      "接尾辞,名詞的,一般,*,*,*,駅,エキ,駅,エキ,*,*,*,*,*,*"
    ));
    // Hot spring
    assert!(should_index(
      "接尾辞,名詞的,一般,*,*,*,温泉,オンセン,温泉,オンセン,*,*,*,*,*,*"
    ));
  }

  /// Verify that other types of suffixes (verbal etc.) are excluded
  #[test]
  fn exclude_other_suffix_types() {
    // Suffix, Verbal is excluded
    assert!(!should_index("接尾辞,動詞的,*,*,*,*,れる,レル,れる,レル"));
    // Suffix, Adjectival is excluded
    assert!(!should_index("接尾辞,形容詞的,*,*,*,*,しい,シイ,しい,シイ"));
  }

  /// Verify that Adjectival Nouns (UniDic) are indexed
  /// Words like "kireida", "shizukada" (adjectival verbs)
  #[test]
  fn index_adjectival_noun() {
    assert!(should_index(
      "形状詞,一般,*,*,*,*,きれい,キレイ,キレイ,きれい,キレイ,1,C2,*"
    ));
    assert!(should_index(
      "形状詞,一般,*,*,*,*,静か,シズカ,シズカ,静か,シズカ,0,C2,*"
    ));
  }

  /// Verify that UniDic auxiliary symbols (periods, commas) are excluded
  /// `feature.starts_with("記号")` does not match, but excluded by allow-list method
  #[test]
  fn exclude_unidic_punctuation() {
    // Period
    assert!(!should_index(
      "補助記号,句点,*,*,*,*,*,。,。,*,。,*,記号,*,*,*,*,*,*,補助,*,*,*,*,*,*,*,6880571302400,25"
    ));
    // Comma
    assert!(!should_index(
      "補助記号,読点,*,*,*,*,*,、,、,*,、,*,記号,*,*,*,*,*,*,補助,*,*,*,*,*,*,*,6605693395456,24"
    ));
  }
}
