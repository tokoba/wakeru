//! リクエストモデル定義

use serde::Deserialize;

/// 形態素解析リクエスト
#[derive(Debug, Deserialize)]
pub struct WakeruRequest {
  /// 解析対象のテキスト
  pub text: String,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn deserialize_valid_request() {
    let json = r#"{"text": "東京"}"#;
    let req: WakeruRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.text, "東京");
  }

  #[test]
  fn deserialize_empty_text() {
    let json = r#"{"text": ""}"#;
    let req: WakeruRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.text, "");
  }
}
