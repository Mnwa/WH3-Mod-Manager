use crate::{
    localization::{Language, Message},
    message,
};
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Invalid pack: {0}")]
    Pack(Message),
    #[error("Invalid data: {0}")]
    Invalid(Message),
    /// Third-party update errors keep their original English text.
    #[error("Update failed: {0}")]
    Update(String),
    /// Steam's own error text, kept verbatim.
    #[error("Steam: {0}")]
    Steam(String),
    #[error("Operation cancelled")]
    Cancelled,
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn io(path: &std::path::Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}

impl Error {
    pub fn message(&self) -> Message {
        match self {
            Self::Pack(detail) => Message::new(
                format!("Invalid pack: {}", detail.text(Language::English)),
                format!("Некорректный pack: {}", detail.text(Language::Russian)),
            ),
            Self::Invalid(detail) => Message::new(
                format!("Invalid data: {}", detail.text(Language::English)),
                format!("Некорректные данные: {}", detail.text(Language::Russian)),
            ),
            Self::Update(detail) => message!(
                "Update failed: {}",
                "Не удалось обновить приложение: {}",
                detail
            ),
            Self::Steam(detail) => message!("Steam error: {}", "Ошибка Steam: {}", detail),
            Self::Cancelled => message!("Operation cancelled", "Операция отменена"),
            Self::Io { path, source } => message!(
                "File error at {}: {}",
                "Ошибка файла {}: {}",
                path.display(),
                source
            ),
            Self::Json(source) => message!("Invalid JSON: {}", "Некорректный JSON: {}", source),
        }
    }
}
