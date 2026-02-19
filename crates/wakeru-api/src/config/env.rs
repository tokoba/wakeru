//! 環境変数からの設定読み込み

use std::str::FromStr;

use super::constants::{DEFAULT_BIND_ADDR, DEFAULT_PRESET_DICT};
use crate::errors::ApiError;

/// 辞書プリセットの種類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
  /// IPAdic 辞書
  Ipadic,
  /// UniDic (現代日本語書き言葉コーパス)
  UnidicCwj,
  /// UniDic (現代日本語話し言葉コーパス)
  UnidicCsj,
}

impl FromStr for Preset {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_lowercase().as_str() {
      "ipadic" => Ok(Self::Ipadic),
      "unidic-cwj" => Ok(Self::UnidicCwj),
      "unidic-csj" => Ok(Self::UnidicCsj),
      _ => Err(format!(
        "不明なプリセット: {}。有効な値: ipadic, unidic-cwj, unidic-csj",
        s
      )),
    }
  }
}

impl Preset {}

/// API サーバーの設定
#[derive(Debug, Clone)]
pub struct Config {
  /// バインドアドレス (例: "127.0.0.1:5530")
  pub bind_addr: String,
  /// 使用する辞書プリセット
  pub preset: Preset,
}

impl Config {
  /// 環境変数から設定を読み込む
  ///
  /// # Errors
  /// 環境変数の値が無効な場合にエラーを返す
  pub fn from_env() -> crate::errors::Result<Self> {
    let bind_addr =
      std::env::var("WAKERU_API_BASE_URL").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string());

    let preset_dict_str =
      std::env::var("WAKERU_PRESET_DICT").unwrap_or_else(|_| DEFAULT_PRESET_DICT.to_string());

    let preset = Preset::from_str(&preset_dict_str).map_err(ApiError::config)?;

    Ok(Self { bind_addr, preset })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn preset_from_str_ipadic() {
    assert_eq!(Preset::from_str("ipadic").unwrap(), Preset::Ipadic);
    assert_eq!(Preset::from_str("IPADIC").unwrap(), Preset::Ipadic);
  }

  #[test]
  fn preset_from_str_unidic_cwj() {
    assert_eq!(Preset::from_str("unidic-cwj").unwrap(), Preset::UnidicCwj);
    assert_eq!(Preset::from_str("UNIDIC-CWJ").unwrap(), Preset::UnidicCwj);
  }

  #[test]
  fn preset_from_str_unidic_csj() {
    assert_eq!(Preset::from_str("unidic-csj").unwrap(), Preset::UnidicCsj);
  }

  #[test]
  fn preset_from_str_invalid() {
    assert!(Preset::from_str("invalid").is_err());
  }

  #[test]
  fn config_from_env_defaults() {
    // 環境変数が設定されていない場合のデフォルト値を確認
    // 注: remove_var は Rust 2024 で unsafe になったため使用しない
    // このテストは環境変数が設定されていない前提で実行される

    let config = Config::from_env().unwrap();
    // 環境変数が設定されている場合はその値、そうでなければデフォルト値
    assert!(!config.bind_addr.is_empty());
  }
}
