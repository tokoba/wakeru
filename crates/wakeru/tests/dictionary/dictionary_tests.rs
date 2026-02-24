//! tests for dictionary
//! Integration tests for dictionary management

use vibrato_rkyv::dictionary::PresetDictionaryKind;
use wakeru::dictionary::DictionaryManager;
use wakeru::errors::DictionaryError;

/// Verify that the constructor of DictionaryManager works correctly.
#[test]
fn create_dictionary_manager_with_preset() {
  let result = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic);

  // The constructor itself should succeed as it does not require network access
  assert!(
    result.is_ok(),
    "Failed to build DictionaryManager: {:?}",
    result.err()
  );
}

/// Verify that an error returns when a non-existent path is specified.
#[test]
fn from_local_path_with_nonexistent_file() {
  let result = DictionaryManager::from_local_path("/nonexistent/path/to/system.dic");

  assert!(result.is_err());
  let err = result.unwrap_err();
  // Confirm it is DictionaryError::DictionaryNotFound
  assert!(
    matches!(err, DictionaryError::DictionaryNotFound(_)),
    "Unexpected error type: {:?}",
    err
  );
}

/// Preset dictionary download & load test.
///
/// Added `#[ignore]` because it requires network access and handling large files.
///
/// How to run:
/// ```bash
/// cargo test -- --ignored download_and_load_ipadic
/// ```
#[test]
#[ignore = "Excluded from normal tests as dictionary download takes time"]
fn download_and_load_ipadic() {
  let manager = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic)
    .expect("Failed to build DictionaryManager");

  // Load dictionary (Download occurs on the first time)
  let dict = manager.load();
  assert!(dict.is_ok(), "Failed to load dictionary: {:?}", dict.err());

  // Second load is retrieved from cache
  let dict2 = manager.load();
  assert!(dict2.is_ok(), "Failed to load for the second time");
}

/// Test loading cached dictionary.
///
/// Valid only when `download_and_load_ipadic` has been executed beforehand
/// and the dictionary is cached.
/// Skips automatically if cache does not exist.
#[test]
fn load_cached_dictionary() {
  let manager = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic)
    .expect("Failed to build DictionaryManager");

  // Check if cache exists
  let cache_dir = manager.cache_dir();
  let dict_subdir = cache_dir.join(PresetDictionaryKind::Ipadic.name());

  if !dict_subdir.exists() {
    eprintln!(
      "Skipping as dictionary cache does not exist: {}",
      dict_subdir.display()
    );
    return;
  }

  // Load from cache
  let dict = manager.load();
  assert!(
    dict.is_ok(),
    "Failed to load cached dictionary: {:?}",
    dict.err()
  );
}

/// Verify that basic morphological analysis is possible with the loaded dictionary.
///
/// Requires dictionary cache beforehand.
#[test]
fn basic_tokenize_with_cached_dictionary() {
  let manager = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic)
    .expect("Failed to build DictionaryManager");

  let cache_dir = manager.cache_dir();
  let dict_subdir = cache_dir.join(PresetDictionaryKind::Ipadic.name());

  if !dict_subdir.exists() {
    eprintln!("Skipping as dictionary cache does not exist");
    return;
  }

  // Load dictionary
  let dict = manager.load().expect("Failed to load dictionary");

  // Generate Tokenizer and Worker, and perform morphological analysis
  // Pass Arc<Dictionary> directly using from_shared_dictionary
  let tokenizer = vibrato_rkyv::Tokenizer::from_shared_dictionary(dict);
  let mut worker = tokenizer.new_worker();

  worker.reset_sentence("東京は日本の首都です");
  worker.tokenize();

  // Confirm token count is not zero
  assert!(
    worker.num_tokens() > 0,
    "Morphological analysis result is empty"
  );

  // Output surface form and part-of-speech info for each token (for debugging)
  for token in worker.token_iter() {
    println!(
      "  surface: {:8} | range_byte: {:?} | feature: {}",
      token.surface(),
      token.range_byte(),
      token.feature()
    );
  }
}
