//! Workshop mod staging, the original's `workshopModStagingMode`: before a launch,
//! enabled Workshop mods are copied or linked into `<game>/whmm_copied_mods`, and the
//! mod list loads them from there.
//!
//! Only that folder is written. Unchanged copies (same size and modification time)
//! and links that already point at the right pack are reused; packs that are no longer
//! selected are removed from it. Mods that also exist in `data` stay in `data`.
use crate::{
    Error, Result,
    catalog::{Catalog, Source},
    data_folder::{self, copy_atomic, link_file},
    error::io,
    message,
    storage::Staging,
};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::{Duration, SystemTime},
};

/// Same folder name as the original, so both managers share and clean one place.
pub const FOLDER: &str = "whmm_copied_mods";

pub fn folder(game: &Path) -> PathBuf {
    game.join(FOLDER)
}

/// Progress for the status bar; `total` is set once the plan is known.
#[derive(Debug, Default)]
pub struct Progress {
    pub done: AtomicUsize,
    pub total: AtomicUsize,
}

fn checked_folder(game: &Path) -> Result<PathBuf> {
    let folder = folder(game);
    match fs::symlink_metadata(&folder) {
        Ok(metadata) if !metadata.is_dir() => Err(Error::Invalid(message!(
            "{} is not a plain folder; remove it and try again",
            "{} — не обычная папка; удалите её и повторите",
            folder.display()
        ))),
        _ => Ok(folder),
    }
}

fn same_time(a: SystemTime, b: SystemTime) -> bool {
    a.duration_since(b)
        .or_else(|_| b.duration_since(a))
        .is_ok_and(|difference| difference <= Duration::from_millis(1))
}

fn copy_is_current(source: &Path, destination: &Path) -> bool {
    let (Ok(source), Ok(destination)) = (fs::metadata(source), fs::symlink_metadata(destination))
    else {
        return false;
    };
    destination.is_file()
        && source.len() == destination.len()
        && matches!(
            (source.modified(), destination.modified()),
            (Ok(a), Ok(b)) if same_time(a, b)
        )
}

fn link_is_current(source: &Path, destination: &Path) -> bool {
    fs::read_link(destination).is_ok_and(|target| {
        let (target, source) = (target.to_string_lossy(), source.to_string_lossy());
        if cfg!(windows) {
            target.eq_ignore_ascii_case(&source)
        } else {
            target == source
        }
    })
}

/// Prepare the folder and return a catalog whose staged mods point into it, ready
/// for [`crate::launch::prepare`]. `fresh` (clean up after exit) rebuilds every file.
pub fn stage(
    game: &Path,
    catalog: &Catalog,
    enabled: &HashSet<usize>,
    mode: Staging,
    fresh: bool,
    cancelled: &AtomicBool,
    progress: &Progress,
) -> Result<Catalog> {
    if mode == Staging::Off {
        return Ok(catalog.clone());
    }
    let folder = checked_folder(game)?;
    fs::create_dir_all(&folder).map_err(|error| io(&folder, error))?;
    if mode == Staging::Symlink && !data_folder::can_link(&folder) {
        return Err(Error::Invalid(message!(
            "Symbolic links are unavailable. Run the manager as administrator or turn on Windows Developer Mode, or choose Copy.",
            "Символические ссылки недоступны. Запустите менеджер от имени администратора или включите режим разработчика Windows, либо выберите копирование.",
        )));
    }
    // A pack of the same name in data wins, as in the original.
    let mut in_data: HashSet<String> = catalog
        .mods
        .iter()
        .filter(|item| item.source == Source::Data)
        .map(|item| item.name.to_lowercase())
        .collect();
    if let Ok(entries) = fs::read_dir(data_folder::data_dir(game)) {
        in_data.extend(entries.flatten().filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            name.ends_with(".pack").then_some(name)
        }));
    }
    let mut names = HashSet::new();
    let selected: Vec<usize> = enabled
        .iter()
        .copied()
        .filter(|&index| {
            let item = &catalog.mods[index];
            let name = item.name.to_lowercase();
            item.source == Source::Workshop && !in_data.contains(&name) && names.insert(name)
        })
        .collect();
    progress.total.store(selected.len(), Ordering::Relaxed);
    progress.done.store(0, Ordering::Relaxed);
    // Prune packs this launch does not use, and temporary files of an interrupted run.
    for entry in fs::read_dir(&folder)
        .map_err(|error| io(&folder, error))?
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let stale = (name.ends_with(".pack") && (fresh || !names.contains(&name)))
            || (name.starts_with(".whmm-") && name.ends_with(".tmp"));
        if stale {
            let path = entry.path();
            fs::remove_file(&path).map_err(|error| io(&path, error))?;
        }
    }
    let mut staged = catalog.clone();
    for index in selected {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let source = catalog.mods[index].path.clone();
        let destination = folder.join(data_folder::pack_name(&source)?);
        match mode {
            Staging::Copy if !copy_is_current(&source, &destination) => {
                copy_atomic(&source, &destination)?;
            }
            Staging::Symlink if !link_is_current(&source, &destination) => {
                let temporary = folder.join(format!(".whmm-{index}.tmp"));
                let _ = fs::remove_file(&temporary);
                link_file(&source, &temporary).map_err(|error| io(&temporary, error))?;
                fs::rename(&temporary, &destination).map_err(|error| io(&destination, error))?;
            }
            _ => {}
        }
        staged.mods[index].path = destination;
        progress.done.fetch_add(1, Ordering::Relaxed);
    }
    Ok(staged)
}

/// Bytes the folder holds (links count as their own small size).
pub fn size(game: &Path) -> u64 {
    fs::read_dir(folder(game))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| fs::symlink_metadata(entry.path()).ok())
                .map(|metadata| metadata.len())
                .sum()
        })
        .unwrap_or(0)
}

/// Delete the staging folder. Links inside are removed without touching their targets;
/// a folder that is itself a link is refused.
pub fn clean(game: &Path) -> Result<()> {
    let folder = checked_folder(game)?;
    match fs::remove_dir_all(&folder) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io(&folder, error)),
    }
}
