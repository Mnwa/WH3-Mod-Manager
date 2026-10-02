use crate::{Error, Result, catalog::Source, error::io, preset::Preset};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
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
    /// Lowercase pack names hidden from the list; hiding also disables them.
    pub hidden: BTreeSet<String>,
    /// Lowercase pack names that presets and bulk actions never disable.
    pub always_enabled: BTreeSet<String>,
    pub options: GameOptions,
    /// User load-order rules (WHM3); pack-supplied rules are read from packs instead.
    pub rules: Vec<crate::load_order::Rule>,
    /// Raw `RuleKey`s of individually switched-off pack rules.
    pub disabled_rules: BTreeSet<String>,
    /// Lowercase names of packs whose shipped rules are switched off.
    pub disabled_rule_packs: BTreeSet<String>,
}

/// Game start options persisted with the library (`options` bit set since WHM2,
/// `staging` since WHM4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameOptions {
    pub skip_intro_movies: bool,
    pub script_logging: bool,
    pub auto_start_custom_battle: bool,
    pub close_on_play: bool,
    pub make_units_generals: bool,
    /// Set the game's process priority to High once it starts (WHM4).
    pub raise_priority: bool,
    /// Delete the staged Workshop copies or links when the game exits (WHM4).
    pub clean_up_staging: bool,
    /// How enabled Workshop mods are prepared in the game folder at launch (WHM4).
    pub staging: Staging,
}

/// Workshop mod staging, the original's `workshopModStagingMode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Staging {
    /// Load Workshop mods from their Steam folders.
    #[default]
    Off,
    /// Copy enabled Workshop mods into `<game>/whmm_copied_mods` before launch.
    Copy,
    /// Link enabled Workshop mods into `<game>/whmm_copied_mods` before launch.
    Symlink,
}

impl Staging {
    pub fn code(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Copy => 1,
            Self::Symlink => 2,
        }
    }
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Off),
            1 => Some(Self::Copy),
            2 => Some(Self::Symlink),
            _ => None,
        }
    }
}

impl GameOptions {
    const FLAGS: [u32; 7] = [1, 2, 4, 8, 16, 32, 64];
    /// WHM2 and WHM3 knew only the first five flags.
    const LEGACY_MASK: u32 = 31;

    fn fields(&self) -> [bool; 7] {
        [
            self.skip_intro_movies,
            self.script_logging,
            self.auto_start_custom_battle,
            self.close_on_play,
            self.make_units_generals,
            self.raise_priority,
            self.clean_up_staging,
        ]
    }

    pub fn bits(&self) -> u32 {
        Self::FLAGS
            .iter()
            .zip(self.fields())
            .filter(|(_, on)| *on)
            .fold(0, |bits, (flag, _)| bits | flag)
    }

    /// Reject unknown bits so a newer library is not silently reinterpreted.
    pub fn from_bits(bits: u32) -> Option<Self> {
        if bits & !Self::FLAGS.iter().fold(0, |all, flag| all | flag) != 0 {
            return None;
        }
        let on = |flag: u32| bits & flag != 0;
        Some(Self {
            skip_intro_movies: on(1),
            script_logging: on(2),
            auto_start_custom_battle: on(4),
            close_on_play: on(8),
            make_units_generals: on(16),
            raise_priority: on(32),
            clean_up_staging: on(64),
            staging: Staging::Off,
        })
    }

    /// The option set of a frozen WHM2/WHM3 record.
    pub(crate) fn from_legacy_bits(bits: u32) -> Option<Self> {
        (bits & !Self::LEGACY_MASK == 0)
            .then(|| Self::from_bits(bits))
            .flatten()
    }
}

/// Overrides the data folder (library, preferences, generated packs), e.g. for a
/// portable copy or for testing against a fake game install without touching
/// the real library.
pub const HOME_ENV: &str = "WH3MM_HOME";

pub fn settings_path() -> Result<PathBuf> {
    if let Some(home) = std::env::var_os(HOME_ENV).filter(|home| !home.is_empty()) {
        return Ok(PathBuf::from(home).join("library.whmm"));
    }
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
