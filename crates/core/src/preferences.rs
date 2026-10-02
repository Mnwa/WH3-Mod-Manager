//! UI preferences, stored apart from the WHM library: WHP2 is current, the frozen WHP1
//! (language only) is decoded for migration.
//!
//! WHP2 layout, all little-endian: `WHP2` | language u8 | layout u8 | density u8 |
//! flags u8 | column widths 4 × u16. The length is fixed, so a truncated or padded
//! file is rejected instead of being half-read.
use crate::{Error, Result, error::io, localization::Language, message, storage};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const WHP2: &[u8; 4] = b"WHP2";
const WHP2_LEN: usize = 16;

/// How the mod list is laid out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    /// One table with every mod.
    #[default]
    Single,
    /// Disabled mods on the left, enabled mods in load order on the right.
    Dual,
}

/// Row height of the mod lists, the original's "Row Size".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Density {
    Compact,
    #[default]
    Comfortable,
    Roomy,
}

impl Density {
    pub const ALL: [Self; 3] = [Self::Compact, Self::Comfortable, Self::Roomy];
}

/// Resizable table columns, in the order they are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Pack,
    Author,
    Updated,
    Size,
}

impl Column {
    pub const ALL: [Self; 4] = [Self::Pack, Self::Author, Self::Updated, Self::Size];
    pub fn default_width(self) -> u16 {
        match self {
            Self::Pack => 180,
            Self::Author => 120,
            Self::Updated => 108,
            Self::Size => 84,
        }
    }
    pub fn min_width(self) -> u16 {
        match self {
            Self::Pack | Self::Author => 60,
            Self::Updated | Self::Size => 56,
        }
    }
    pub const MAX_WIDTH: u16 = 600;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    /// `None` until the user picks a language; the system language is used meanwhile.
    pub language: Option<Language>,
    pub layout: Layout,
    pub density: Density,
    /// Group the main list by category.
    pub group_by_category: bool,
    widths: [u16; 4],
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: None,
            layout: Layout::default(),
            density: Density::default(),
            group_by_category: false,
            widths: Column::ALL.map(Column::default_width),
        }
    }
}

impl Preferences {
    pub fn width(&self, column: Column) -> u16 {
        self.widths[column as usize]
    }
    /// Clamp to the column's limits so a drag can never hide a column completely.
    pub fn set_width(&mut self, column: Column, width: f32) {
        let width = width
            .round()
            .clamp(column.min_width() as f32, Column::MAX_WIDTH as f32);
        self.widths[column as usize] = width as u16;
    }
    pub fn reset_widths(&mut self) {
        self.widths = Column::ALL.map(Column::default_width);
    }
    pub fn widths_are_default(&self) -> bool {
        self.widths == Column::ALL.map(Column::default_width)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(WHP2_LEN);
        bytes.extend_from_slice(WHP2);
        bytes.push(match self.language {
            None => 0,
            Some(Language::English) => 1,
            Some(Language::Russian) => 2,
        });
        bytes.push(match self.layout {
            Layout::Single => 0,
            Layout::Dual => 1,
        });
        bytes.push(match self.density {
            Density::Compact => 0,
            Density::Comfortable => 1,
            Density::Roomy => 2,
        });
        bytes.push(u8::from(self.group_by_category));
        for width in self.widths {
            bytes.extend_from_slice(&width.to_le_bytes());
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        match bytes {
            b"WHP1\x00" => return Ok(Self::with_language(Language::English)),
            b"WHP1\x01" => return Ok(Self::with_language(Language::Russian)),
            _ => {}
        }
        if bytes.len() != WHP2_LEN || !bytes.starts_with(WHP2) {
            return Err(invalid());
        }
        let language = match bytes[4] {
            0 => None,
            1 => Some(Language::English),
            2 => Some(Language::Russian),
            _ => return Err(invalid()),
        };
        let layout = match bytes[5] {
            0 => Layout::Single,
            1 => Layout::Dual,
            _ => return Err(invalid()),
        };
        let density = match bytes[6] {
            0 => Density::Compact,
            1 => Density::Comfortable,
            2 => Density::Roomy,
            _ => return Err(invalid()),
        };
        let group_by_category = match bytes[7] {
            0 => false,
            1 => true,
            _ => return Err(invalid()),
        };
        let mut preferences = Self {
            language,
            layout,
            density,
            group_by_category,
            ..Self::default()
        };
        let (widths, _) = bytes[8..].as_chunks::<2>();
        for (column, &width) in Column::ALL.into_iter().zip(widths) {
            preferences.set_width(column, f32::from(u16::from_le_bytes(width)));
        }
        Ok(preferences)
    }

    fn with_language(language: Language) -> Self {
        Self {
            language: Some(language),
            ..Self::default()
        }
    }
}

fn invalid() -> Error {
    Error::Invalid(message!(
        "Invalid interface preferences (WHP)",
        "Повреждены настройки интерфейса (WHP)"
    ))
}

pub fn path() -> Result<PathBuf> {
    Ok(storage::settings_path()?.with_file_name("preferences.whmp"))
}

/// Defaults when the file does not exist yet.
pub fn load(path: &Path) -> Result<Preferences> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Preferences::default());
        }
        Err(error) => return Err(io(path, error)),
    };
    let mut bytes = Vec::new();
    file.take(WHP2_LEN as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io(path, error))?;
    Preferences::decode(&bytes)
}

pub fn save(path: &Path, preferences: &Preferences) -> Result<()> {
    storage::atomic_write(path, &preferences.encode())
}
