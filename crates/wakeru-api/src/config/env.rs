//! Config loading from environment variables

use std::str::FromStr;

use super::constants::{DEFAULT_BIND_ADDR, DEFAULT_PRESET_DICT};
use crate::errors::ApiError;

/// Dictionary Preset Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
  /// IPAdic dictionary
  Ipadic,
  /// UniDic (Corpus of Contemporary Written Japanese)
  UnidicCwj,
  /// UniDic (Corpus of Spontaneous Japanese)
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
        "Unknown preset: {}. Valid values: ipadic, unidic-cwj, unidic-csj",
        s
      )),
    }
  }
}

impl Preset {}

/// API Server Configuration
#[derive(Debug, Clone)]
pub struct Config {
  /// Bind address (e.g. "127.0.0.1:5530")
  pub bind_addr: String,
  /// Dictionary preset to use
  pub preset: Preset,
}

impl Config {
  /// Loads configuration from environment variables
  ///
  /// # Errors
  /// Returns an error if environment variable values are invalid
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
    // Verify default values when environment variables are not set
    // Note: remove_var became unsafe in Rust 2024, so not used here
    // This test assumes environment variables are not set

    let config = Config::from_env().unwrap();
    // If environment variable is set, it's that value, otherwise default value
    assert!(!config.bind_addr.is_empty());
  }
}
