//! Morphological Analysis Service

use std::time::Instant;

use vibrato_rkyv::Tokenizer as VibratoImpl;
use wakeru::dictionary::DictionaryManager;
use wakeru::tokenizer::should_index;

use crate::config::MAX_TEXT_LENGTH;
use crate::config::{Config, Preset};
use crate::errors::{ApiError, Result};
use crate::models::{TokenDto, WakeruRequest, WakeruResponse};

/// Common interface for morphological analysis service
///
/// This trait allows swapping production implementation (`WakeruApiServiceFull`) with
/// test stubs/mocks.
pub trait WakeruApiService: Send + Sync {
  /// Executes morphological analysis
  ///
  /// # Errors
  /// - Input error (empty string, length exceeded, etc.)
  /// - Internal error
  fn analyze(&self, request: WakeruRequest) -> Result<WakeruResponse>;
}

/// Converts Preset to PresetDictionaryKind of vibrato-rkyv
///
/// Conversion is done in the service layer so that the config layer does not depend on vibrato
#[must_use]
fn preset_to_vibrato_kind(preset: &Preset) -> vibrato_rkyv::dictionary::PresetDictionaryKind {
  use vibrato_rkyv::dictionary::PresetDictionaryKind;
  match preset {
    Preset::Ipadic => PresetDictionaryKind::Ipadic,
    Preset::UnidicCwj => PresetDictionaryKind::UnidicCwj,
    Preset::UnidicCsj => PresetDictionaryKind::UnidicCsj,
  }
}

/// Morphological Analysis Service
///
/// By holding Dictionary and VibratoImpl directly,
/// all tokens before filtering can be obtained.
#[derive(Clone)]
pub struct WakeruApiServiceFull {
  /// vibrato tokenizer (internal implementation)
  inner: VibratoImpl,
}

impl WakeruApiServiceFull {
  /// Initializes the service
  ///
  /// # Arguments
  /// * `config` - Configuration (including dictionary preset)
  ///
  /// # Errors
  /// Returns an error if dictionary load fails
  pub fn new(config: &Config) -> Result<Self> {
    let kind = preset_to_vibrato_kind(&config.preset);

    // Create dictionary manager and load dictionary
    let manager = DictionaryManager::with_preset(kind)
      .map_err(|e| ApiError::config(format!("Failed to create dictionary manager: {}", e)))?;

    let dict =
      manager.load().map_err(|e| ApiError::config(format!("Failed to load dictionary: {}", e)))?;

    // Create VibratoImpl directly
    let inner = VibratoImpl::from_shared_dictionary(dict);

    Ok(Self { inner })
  }

  /// Executes morphological analysis (returns all tokens)
  ///
  /// # Arguments
  /// * `request` - Analysis request
  ///
  /// # Returns
  /// Analysis result (all token sequence and processing time)
  ///
  /// # Errors
  /// - If text is empty
  /// - If text exceeds maximum length
  pub fn analyze(&self, request: WakeruRequest) -> Result<WakeruResponse> {
    // Validate text length
    let text_bytes = request.text.len();
    if text_bytes == 0 {
      return Err(ApiError::invalid_input("Text is empty"));
    }

    if text_bytes > MAX_TEXT_LENGTH {
      return Err(ApiError::text_too_long(text_bytes, MAX_TEXT_LENGTH));
    }

    // Start measuring processing time
    let start = Instant::now();

    // Create worker and analyze
    let mut worker = self.inner.new_worker();
    worker.reset_sentence(&request.text);
    worker.tokenize();

    let mut tokens = Vec::with_capacity(worker.num_tokens());

    for token in worker.token_iter() {
      let surface = token.surface();
      let feature = token.feature();
      let start_byte = token.range_byte().start;
      let end_byte = token.range_byte().end;

      // Determine whether to index
      let should_index_flag = should_index(feature);

      let dto = TokenDto::from_feature(surface, feature, start_byte, end_byte, should_index_flag);
      tokens.push(dto);
    }

    // End measuring processing time
    let elapsed_ms = start.elapsed().as_millis() as u64;

    Ok(WakeruResponse { tokens, elapsed_ms })
  }
}

/// Production implementation of trait `WakeruApiService`
impl WakeruApiService for WakeruApiServiceFull {
  fn analyze(&self, request: WakeruRequest) -> Result<WakeruResponse> {
    // Note: Writing `self.analyze(...)` would recursively call the trait method,
    // so explicitly call the inherent method.
    WakeruApiServiceFull::analyze(self, request)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::Preset;

  fn create_test_config() -> Config {
    Config {
      bind_addr: "127.0.0.1:5531".to_string(),
      preset: Preset::UnidicCwj,
    }
  }

  // Dictionary-dependent tests are opt-in with with_dict_tests feature
  #[test]
  #[cfg_attr(not(feature = "with_dict_tests"), ignore)]
  fn test_service_creation() {
    let config = create_test_config();

    // Confirm dictionary can be loaded
    let service = WakeruApiServiceFull::new(&config)
      .expect("Failed to load dictionary: check test environment");
    let response = service.analyze(WakeruRequest {
      text: "東京".to_string(),
    });
    assert!(response.is_ok());
    let response = response.unwrap();
    assert!(!response.tokens.is_empty());
  }

  #[test]
  #[cfg_attr(not(feature = "with_dict_tests"), ignore)]
  fn test_empty_text_error() {
    let config = create_test_config();
    let service = WakeruApiServiceFull::new(&config)
      .expect("Failed to load dictionary: check test environment");
    let result = service.analyze(WakeruRequest {
      text: "".to_string(),
    });
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "invalid_input");
  }

  #[test]
  #[cfg_attr(not(feature = "with_dict_tests"), ignore)]
  fn test_text_too_long_error() {
    let config = create_test_config();
    let service = WakeruApiServiceFull::new(&config)
      .expect("Failed to load dictionary: check test environment");
    let long_text = "a".repeat(MAX_TEXT_LENGTH + 1);
    let result = service.analyze(WakeruRequest { text: long_text });
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "text_too_long");
  }

  // This does not require dictionary download so can always be run
  #[test]
  fn test_preset_to_vibrato_kind() {
    use vibrato_rkyv::dictionary::PresetDictionaryKind;

    assert_eq!(
      preset_to_vibrato_kind(&Preset::Ipadic),
      PresetDictionaryKind::Ipadic
    );
    assert_eq!(
      preset_to_vibrato_kind(&Preset::UnidicCwj),
      PresetDictionaryKind::UnidicCwj
    );
    assert_eq!(
      preset_to_vibrato_kind(&Preset::UnidicCsj),
      PresetDictionaryKind::UnidicCsj
    );
  }
}
