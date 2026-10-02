use crate::{Error, Result, catalog::Source, error::io, preset::Preset};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub metadata: std::collections::BTreeMap<String, crate::metadata::Metadata>,
    pub game_path: Option<PathBuf>,
    pub roots: Vec<(PathBuf, Source)>,
    pub presets: Vec<Preset>,
    pub current: Option<Preset>,
}

pub fn settings_path() -> Result<PathBuf> {
    dirs::config_dir()
        .map(|path| path.join("wh3-mod-manager-rust/library.whmm"))
        .ok_or_else(|| {
            Error::Invalid(crate::message!(
                "Configuration folder not found",
                "Не найдена папка настроек"
            ))
        })
}

pub fn load(path: &Path) -> Result<Settings> {
    if let Ok(metadata) = fs::metadata(path)
        && metadata.len() > crate::storage_format::MAX_FILE
    {
        return Err(Error::Invalid(crate::message!(
            "Library exceeds 128 MiB",
            "Хранилище больше 128 МБ"
        )));
    }
    match fs::read(path) {
        Ok(bytes) => crate::storage_format::decode(&bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(io(path, e)),
    }
}

pub fn save(path: &Path, settings: &Settings) -> Result<()> {
    let bytes = crate::storage_format::encode(settings)?;
    if path.exists() {
        // Never replace a corrupt library with an empty state after a failed load.
        load(path)?;
        let previous = fs::read(path).map_err(|e| io(path, e))?;
        atomic_write(&path.with_extension("whmm.bak"), &previous)?;
    }
    atomic_write(path, &bytes)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| io(parent, e))?;
    temporary.write_all(bytes).map_err(|e| io(path, e))?;
    temporary.as_file().sync_all().map_err(|e| io(path, e))?;
    temporary.persist(path).map_err(|e| io(path, e.error))?;
    Ok(())
}
