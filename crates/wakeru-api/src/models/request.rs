//! Request Model Definition

use serde::Deserialize;

/// Morphological Analysis Request
#[derive(Debug, Deserialize)]
pub struct WakeruRequest {
  /// Text to analyze
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
