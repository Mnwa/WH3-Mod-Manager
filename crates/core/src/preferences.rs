//! WHP1 stores UI preferences independently of the frozen WHM1 library schema.
use crate::{Error, Result, error::io, localization::Language, message, storage};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub fn path() -> Result<PathBuf> {
    Ok(storage::settings_path()?.with_file_name("preferences.whmp"))
}

pub fn load(path: &Path) -> Result<Language> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Language::default());
        }
        Err(error) => return Err(io(path, error)),
    };
    let mut bytes = Vec::new();
    file.take(6)
        .read_to_end(&mut bytes)
        .map_err(|error| io(path, error))?;
    match bytes.as_slice() {
        b"WHP1\x00" => Ok(Language::English),
        b"WHP1\x01" => Ok(Language::Russian),
        _ => Err(Error::Invalid(message!(
            "Invalid language preferences (WHP1)",
            "Повреждены настройки языка (WHP1)"
        ))),
    }
}

pub fn save(path: &Path, language: Language) -> Result<()> {
    storage::atomic_write(
        path,
        match language {
            Language::English => b"WHP1\x00",
            Language::Russian => b"WHP1\x01",
        },
    )
}
