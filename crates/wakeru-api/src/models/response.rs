//! Response Model Definition

use serde::Serialize;

/// Constants for feature array indices
///
/// Position of each field in the feature array of MeCab/IPAdic dictionary format
const IDX_POS: usize = 0;
const IDX_POS_DETAIL1: usize = 1;
const IDX_POS_DETAIL2: usize = 2;
const IDX_POS_DETAIL3: usize = 3;
const IDX_LEMMA: usize = 6;
const IDX_READING: usize = 7;
const IDX_PRONUNCIATION: usize = 8;

/// Morphological Analysis Response
#[derive(Debug, Serialize)]
pub struct WakeruResponse {
  /// Token sequence of analysis result
  pub tokens: Vec<TokenDto>,
  /// Elapsed time (milliseconds)
  pub elapsed_ms: u64,
}

/// Token Information (DTO)
///
/// Converted from vibrato-rkyv token information for API response.
#[derive(Debug, Clone, Serialize)]
pub struct TokenDto {
  /// Surface form (string appearing in original text)
  pub surface: String,
  /// Feature (complete string including part-of-speech info)
  pub feature: String,
  /// Part of Speech (1st element)
  pub pos: String,
  /// POS detail 1 (2nd element)
  pub pos_detail1: String,
  /// POS detail 2 (3rd element)
  pub pos_detail2: String,
  /// POS detail 3 (4th element)
  pub pos_detail3: String,
  /// Lemma (dictionary form reading)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub lemma: Option<String>,
  /// Reading
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reading: Option<String>,
  /// Pronunciation
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pronunciation: Option<String>,
  /// Start byte position
  pub start_byte: usize,
  /// End byte position
  pub end_byte: usize,
  /// Whether to index (for filtering in RAG usage)
  pub should_index: bool,
}

impl TokenDto {
  /// Convert from vibrato-rkyv token
  ///
  /// # Arguments
  /// * `surface` - Surface form
  /// * `feature` - Feature string (comma separated)
  /// * `start_byte` - Start byte position
  /// * `end_byte` - End byte position
  /// * `should_index` - Whether to index
  #[must_use]
  pub fn from_feature(
    surface: &str,
    feature: &str,
    start_byte: usize,
    end_byte: usize,
    should_index: bool,
  ) -> Self {
    let parts: Vec<&str> = feature.splitn(13, ',').collect();

    // Extract each field (only if index is within range)
    let get_part =
      |idx: usize| -> String { parts.get(idx).map_or(String::new(), |s| (*s).to_string()) };

    // Lemma (dictionary form) position varies by dictionary
    // UniDic: Often at 7th position
    let lemma = parts.get(IDX_LEMMA).and_then(|s| {
      if s.is_empty() || *s == "*" {
        None
      } else {
        Some((*s).to_string())
      }
    });

    // Extract reading and pronunciation (handle flexibly as position varies by dictionary)
    let reading = parts.get(IDX_READING).and_then(|s| {
      if s.is_empty() || *s == "*" {
        None
      } else {
        Some((*s).to_string())
      }
    });

    let pronunciation = parts.get(IDX_PRONUNCIATION).and_then(|s| {
      if s.is_empty() || *s == "*" {
        None
      } else {
        Some((*s).to_string())
      }
    });

    Self {
      surface: surface.to_string(),
      feature: feature.to_string(),
      pos: get_part(IDX_POS),
      pos_detail1: get_part(IDX_POS_DETAIL1),
      pos_detail2: get_part(IDX_POS_DETAIL2),
      pos_detail3: get_part(IDX_POS_DETAIL3),
      lemma,
      reading,
      pronunciation,
      start_byte,
      end_byte,
      should_index,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn token_dto_from_feature_full() {
    let feature = "名詞,一般,*,*,*,*,東京,トウキョウ,トーキョー";
    let dto = TokenDto::from_feature("東京", feature, 0, 6, true);

    assert_eq!(dto.surface, "東京");
    assert_eq!(dto.feature, feature);
    assert_eq!(dto.pos, "名詞");
    assert_eq!(dto.pos_detail1, "一般");
    assert_eq!(dto.pos_detail2, "*");
    assert_eq!(dto.pos_detail3, "*");
    assert_eq!(dto.lemma, Some("東京".to_string()));
    assert_eq!(dto.reading, Some("トウキョウ".to_string()));
    assert_eq!(dto.pronunciation, Some("トーキョー".to_string()));
    assert_eq!(dto.start_byte, 0);
    assert_eq!(dto.end_byte, 6);
    assert!(dto.should_index);
  }

  #[test]
  fn token_dto_from_feature_short() {
    // Minimal feature
    let feature = "名詞";
    let dto = TokenDto::from_feature("test", feature, 0, 4, false);

    assert_eq!(dto.surface, "test");
    assert_eq!(dto.pos, "名詞");
    assert_eq!(dto.pos_detail1, "");
    assert_eq!(dto.lemma, None);
    assert!(!dto.should_index);
  }

  #[test]
  fn wakeru_response_serialization() {
    let response = WakeruResponse {
      tokens: vec![TokenDto::from_feature(
        "東京",
        "名詞,一般,*,*,*,*,東京,トウキョウ",
        0,
        6,
        true,
      )],
      elapsed_ms: 42,
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"tokens\""));
    assert!(json.contains("\"elapsed_ms\":42"));
    assert!(json.contains("\"surface\":\"東京\""));
    assert!(json.contains("\"should_index\":true"));
  }
}
