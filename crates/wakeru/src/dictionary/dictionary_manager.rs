//! Dictionary Management Module
//!
//! Manages loading of vibrato-rkyv dictionary data and downloading of preset dictionaries.
//! Automatically downloads on the first run, and loads from the cache directory from the second time onwards.
//! Preset dictionaries include IPADIC, UniDic, etc.
//! It is also possible to load a local dictionary directly.

use crate::errors::error_definition::DictionaryError;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use vibrato_rkyv::Dictionary;
use vibrato_rkyv::dictionary::LoadMode;
use vibrato_rkyv::dictionary::PresetDictionaryKind;

/// Dictionary manager structure for vibrato-rkyv
pub struct DictionaryManager {
  /// Dictionary cache directory
  cache_dir: PathBuf,

  /// Type of preset dictionary `Ipadic`, `UnidicCwj`, `UnidicCsj`, etc.
  /// Should be `None` for local dictionaries
  preset_kind: Option<PresetDictionaryKind>,

  /// Dictionary file path (Required when setting a local dictionary, unnecessary for preset dictionaries `None`)
  dictionary_path: Option<PathBuf>,

  /// Cache of loaded dictionary (Initialized only once at the first load)
  /// Held in Arc for sharing
  /// DictionaryError implements Clone so it can hold Result
  dictionary: OnceLock<Result<Arc<Dictionary>, DictionaryError>>,
}

/// Implementation block for DictionaryManager
impl DictionaryManager {
  /// Returns the path of the cache directory
  pub fn cache_dir(&self) -> &Path {
    &self.cache_dir
  }

  /// Constructor for DictionaryManager using a preset dictionary
  pub fn with_preset(preset_kind: PresetDictionaryKind) -> Result<Self, DictionaryError> {
    let cache_dir = default_cache_dir()?;

    Ok(Self {
      cache_dir,
      preset_kind: Some(preset_kind),
      dictionary_path: None, // Dictionary path is not needed when using a preset dictionary
      dictionary: OnceLock::new(), // New load
    })
  }

  /// Constructor for DictionaryManager using a local dictionary file
  pub fn from_local_path<P: AsRef<Path>>(path: P) -> Result<Self, DictionaryError> {
    let path = path.as_ref().to_path_buf();

    if !path.is_file() {
      // Error if the file does not exist
      let s = path.display().to_string();
      return Err(DictionaryError::DictionaryNotFound(s));
    }

    // Use the parent directory of the current directory as the cache directory
    let cache_dir = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));

    Ok(Self {
      cache_dir,
      preset_kind: None,
      dictionary_path: Some(path),
      dictionary: OnceLock::new(),
    })
  }

  /// Load dictionary
  /// Returns `Arc<Dictionary>` as we want a shared dictionary
  /// - Loads the dictionary file from the specified path on the first call
  /// - Returns a clone of `Arc<Dictionary>` from the second call onwards
  /// - If an error occurs on the first call, caches the error and keeps returning it
  pub fn load(&self) -> Result<Arc<Dictionary>, DictionaryError> {
    self.dictionary.get_or_init(|| self.load_inner().map(Arc::new)).clone()
  }

  /// Internal implementation of dictionary loading
  fn load_inner(&self) -> Result<Dictionary, DictionaryError> {
    match (&self.dictionary_path, self.preset_kind) {
      /* Match with a tuple of dictionary path and preset dictionary type */
      // Case of local dictionary specification: dictionary path exists, no preset dictionary type
      (Some(path), _) => Self::load_from_local_path(path),

      // Case of preset dictionary specification: no dictionary path, preset dictionary type exists
      (None, Some(preset_kind)) => self.load_from_preset(preset_kind),

      // Error if neither dictionary path nor dictionary type matches
      _ => Err(DictionaryError::InvalidPathOrInvalidPresetKind(
        self.cache_dir.clone(),
        self.preset_kind,
      )),
    }
  }

  /// Loads a dictionary from a local dictionary file
  fn load_from_local_path(path: &Path) -> Result<Dictionary, DictionaryError> {
    Dictionary::from_path(path, LoadMode::TrustCache)
      .map_err(|e| DictionaryError::VibratoLoad(Arc::new(e)))
  }

  /// Load processing when preset dictionary is set
  /// Downloads and loads the dictionary file on the first run
  /// Loads from the cache directory from the second time onwards
  fn load_from_preset(
    &self,
    preset_kind: PresetDictionaryKind,
  ) -> Result<Dictionary, DictionaryError> {
    // Create cache directory (for the first time)
    // Create a new one if the directory does not exist
    std::fs::create_dir_all(&self.cache_dir)
      .map_err(|e| DictionaryError::CacheDirCreationFailed(Arc::new(e)))?;

    // Create a subdirectory based on the dictionary name
    let dict_dir = self.cache_dir.join(preset_kind.name());

    // Download for the first time, load from cache from the second time onwards
    Dictionary::from_preset_with_download(preset_kind, &dict_dir)
      .map_err(|e| DictionaryError::PresetDictDownloadFailed(Arc::new(e)))
  }
}

/// Returns the default cache directory path according to the OS
///
/// | OS      | Example Path                              |
/// |---------|-------------------------------------------|
/// | Linux   | `~/.cache/wakeru/dict`                    |
/// | macOS   | `~/Library/Caches/wakeru/dict`             |
/// | Windows | `C:\Users\{user}\AppData\Local\wakeru\dict` |
fn default_cache_dir() -> Result<PathBuf, DictionaryError> {
  let base = dirs::cache_dir().ok_or(DictionaryError::CacheDirNotFound)?;

  Ok(base.join("wakeru").join("dict"))
}

/// Manual `Debug` implementation for `DictionaryManager`
///
/// Since `vibrato_rkyv::Dictionary` does not implement the `Debug` trait,
/// `#[derive(Debug)]` cannot be used. Displays only meta information instead.
impl fmt::Debug for DictionaryManager {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("DictionaryManager")
      .field("cache_dir", &self.cache_dir)
      .field("preset_kind", &self.preset_kind)
      .field("dictionary_path", &self.dictionary_path)
      // The inner Dictionary is defined in vibrato_rkyv,
      // and since the Debug trait is not implemented, show only the initialized flag
      .field("dictionary_initialized", &self.dictionary.get().is_some())
      .finish()
  }
}
