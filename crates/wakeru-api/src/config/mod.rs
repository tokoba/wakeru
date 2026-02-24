//! Config module

mod constants;
mod env;

pub use constants::{DEFAULT_BIND_ADDR, DEFAULT_PRESET_DICT, MAX_TEXT_LENGTH};
pub use env::{Config, Preset};
