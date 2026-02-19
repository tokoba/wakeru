//! レスポンスモデル定義

use serde::Serialize;

/// feature配列のインデックス定数
///
/// MeCab/IPA辞書形式の素性配列における各フィールドの位置
const IDX_POS: usize = 0;
const IDX_POS_DETAIL1: usize = 1;
const IDX_POS_DETAIL2: usize = 2;
const IDX_POS_DETAIL3: usize = 3;
const IDX_LEMMA: usize = 6;
const IDX_READING: usize = 7;
const IDX_PRONUNCIATION: usize = 8;

/// 形態素解析レスポンス
#[derive(Debug, Serialize)]
pub struct WakeruResponse {
  /// 解析結果のトークン列
  pub tokens: Vec<TokenDto>,
  /// 処理時間（ミリ秒）
  pub elapsed_ms: u64,
}

/// トークン情報（DTO）
///
/// vibrato-rkyv のトークン情報を API レスポンス用に変換したもの。
#[derive(Debug, Clone, Serialize)]
pub struct TokenDto {
  /// 表層形（元テキストに出現する文字列）
  pub surface: String,
  /// 素性（品詞情報などの完全な文字列）
  pub feature: String,
  /// 品詞（第1要素）
  pub pos: String,
  /// 品詞細分類1（第2要素）
  pub pos_detail1: String,
  /// 品詞細分類2（第3要素）
  pub pos_detail2: String,
  /// 品詞細分類3（第4要素）
  pub pos_detail3: String,
  /// 語彙素読み（辞書形の読み）
  #[serde(skip_serializing_if = "Option::is_none")]
  pub lemma: Option<String>,
  /// 読み
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reading: Option<String>,
  /// 発音
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pronunciation: Option<String>,
  /// 開始バイト位置
  pub start_byte: usize,
  /// 終了バイト位置
  pub end_byte: usize,
  /// インデックス対象かどうか（RAG用途でのフィルタリング用）
  pub should_index: bool,
}

impl TokenDto {
  /// vibrato-rkyv のトークンから変換する
  ///
  /// # Arguments
  /// * `surface` - 表層形
  /// * `feature` - 素性文字列（カンマ区切り）
  /// * `start_byte` - 開始バイト位置
  /// * `end_byte` - 終了バイト位置
  /// * `should_index` - インデックス対象かどうか
  #[must_use]
  pub fn from_feature(
    surface: &str,
    feature: &str,
    start_byte: usize,
    end_byte: usize,
    should_index: bool,
  ) -> Self {
    let parts: Vec<&str> = feature.splitn(13, ',').collect();

    // 各フィールドを抽出（インデックスが範囲内の場合のみ）
    let get_part =
      |idx: usize| -> String { parts.get(idx).map_or(String::new(), |s| (*s).to_string()) };

    // 語彙素読み（辞書形）は辞書によって位置が異なる
    // UniDic: 7番目に基本形がある場合が多い
    let lemma = parts.get(IDX_LEMMA).and_then(|s| {
      if s.is_empty() || *s == "*" {
        None
      } else {
        Some((*s).to_string())
      }
    });

    // 読みと発音の抽出（辞書によって位置が異なるため、柔軟に対応）
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
    // 最小限の素性
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
