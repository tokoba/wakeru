//! API Configuration Constants

/// Maximum length of input text (in bytes)
///
/// Allows text up to 10MB.
/// Limitation to prevent resource exhaustion due to processing large text.
pub const MAX_TEXT_LENGTH: usize = 10_000_000;

/// Default bind address
///
/// Standard port for localhost, assumed for use in development environment.
pub const DEFAULT_BIND_ADDR: &str = "127.0.0.1:5530";

/// Default dictionary preset name
///
/// Use UniDic (CWJ) as default.
/// Dictionary based on Corpus of Contemporary Written Japanese.
pub const DEFAULT_PRESET_DICT: &str = "unidic-cwj";
