//! Warhammer III campaign saves: listing and the packs a save was made with.
use crate::{Error, Result, error::io};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{BufRead, BufReader, ErrorKind, Read},
    path::{Path, PathBuf},
    time::SystemTime,
};

/// Saves larger than this are rejected instead of being scanned for packs.
pub const MAX_SAVE: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Save {
    /// File name including `.save`; the game expects it in `campaign_load`.
    pub name: String,
    pub path: PathBuf,
    pub modified: SystemTime,
    pub size: u64,
}

/// The game's save folder. Only the native Windows layout is known here; on
/// other systems callers pass an explicit folder to [`list`].
pub fn folder() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir().map(|roaming| {
            roaming
                .join("The Creative Assembly")
                .join("Warhammer3")
                .join("save_games")
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

fn is_save(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("save"))
}

/// Lists `.save` files, newest first. A missing folder means no saves yet.
pub fn list(folder: &Path) -> Result<Vec<Save>> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(folder, e)),
    };
    let mut saves = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| io(folder, e))?;
        let path = entry.path();
        if !is_save(&path) {
            continue;
        }
        // Names that are not valid Unicode cannot be passed back to the game.
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            // The game may delete an autosave while the folder is being listed.
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(e) => return Err(io(&path, e)),
        };
        if !metadata.is_file() {
            continue;
        }
        let modified = metadata.modified().map_err(|e| io(&path, e))?;
        saves.push(Save {
            name,
            path,
            modified,
            size: metadata.len(),
        });
    }
    saves.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(saves)
}

/// Pack names recorded in a save, in first-seen order without duplicates.
///
/// Mirrors the original manager's `/\0[^\0]+?\.pack/g` over the whole file: a
/// name starts after a NUL byte and ends at the first `.pack` before the next
/// NUL. Bytes are decoded as lossy UTF-8 instead of 7-bit ASCII so non-ASCII
/// pack names stay readable.
pub fn mods(path: &Path) -> Result<Vec<String>> {
    let file = File::open(path).map_err(|e| io(path, e))?;
    let length = file.metadata().map_err(|e| io(path, e))?.len();
    if length > MAX_SAVE {
        return Err(too_large(path));
    }
    // The limit also covers a file that grows after the size check.
    let mut reader = BufReader::new(file.take(MAX_SAVE + 1));
    let mut segment = Vec::new();
    let mut total = 0u64;
    let mut seen = HashSet::new();
    let mut names = Vec::new();
    let mut after_nul = false;
    loop {
        segment.clear();
        let read = reader
            .read_until(0, &mut segment)
            .map_err(|e| io(path, e))?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > MAX_SAVE {
            return Err(too_large(path));
        }
        let terminated = segment.last() == Some(&0);
        if terminated {
            segment.pop();
        }
        if after_nul
            && let Some(name) = pack_name(&segment)
            && seen.insert(name.clone())
        {
            names.push(name);
        }
        after_nul = terminated;
    }
    Ok(names)
}

/// The regex needs at least one byte before `.pack`, so the search starts at 1.
fn pack_name(segment: &[u8]) -> Option<String> {
    let end = segment
        .get(1..)?
        .windows(5)
        .position(|window| window == b".pack")?
        + 1;
    Some(String::from_utf8_lossy(&segment[..end + 5]).into_owned())
}

fn too_large(path: &Path) -> Error {
    Error::Invalid(crate::message!(
        "{}: save exceeds 512 MiB",
        "{}: сохранение больше 512 МБ",
        path.display()
    ))
}
