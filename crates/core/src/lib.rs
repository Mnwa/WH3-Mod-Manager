//! Game files, mod catalogs and presets without UI dependencies.
pub mod catalog;
pub mod conflict;
pub mod error;
pub mod launch;
pub mod localization;
pub mod metadata;
pub mod pack;
pub mod preferences;
pub mod preset;
pub mod scan;
pub mod steam;
pub mod storage;
mod storage_format;

pub use error::{Error, Result};
