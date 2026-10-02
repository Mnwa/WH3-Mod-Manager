//! Файлы игры, каталог модов и пресеты без зависимости от интерфейса.
pub mod catalog;
pub mod conflict;
pub mod error;
pub mod launch;
pub mod metadata;
pub mod pack;
pub mod preset;
pub mod scan;
pub mod steam;
pub mod storage;
mod storage_format;

pub use error::{Error, Result};
