//! Explicit, user-started file operations in the game's `data` folder: copy or link
//! packs into it (the original's "Copy to data" and "Create symbolic links in data")
//! and remove those copies again.
//!
//! Every operation names its files exactly, touches only top-level `.pack` files in
//! `data`, never overwrites an existing file and never follows a link when deleting.
use crate::{
    Error, Result,
    catalog::{Catalog, Source},
    error::io,
    localization::Message,
    message,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Copy,
    Link,
}

/// What a batch did; failures do not stop the other files.
#[derive(Debug, Default)]
pub struct Outcome {
    pub done: Vec<String>,
    pub failed: Vec<Message>,
}

pub fn data_dir(game: &Path) -> PathBuf {
    game.join("data")
}

/// A plain pack file name: no separators, no `.`/`..`, ends in `.pack`.
pub(crate) fn pack_name(path: &Path) -> Result<&str> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.contains(['/', '\\']) && *name != "." && *name != "..")
        .filter(|name| {
            Path::new(name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("pack"))
        });
    name.ok_or_else(|| {
        Error::Invalid(message!(
            "Not a pack file: {}",
            "Это не pack-файл: {}",
            path.display()
        ))
    })
}

/// Whether this process may create symbolic links (administrator or Windows
/// Developer Mode); probed by creating one, which is what actually matters.
pub fn can_link(probe_dir: &Path) -> bool {
    let Ok(dir) = tempfile::tempdir_in(probe_dir).or_else(|_| tempfile::tempdir()) else {
        return false;
    };
    let target = dir.path().join("target");
    fs::write(&target, b"").is_ok() && link_file(&target, &dir.path().join("link")).is_ok()
}

pub(crate) fn link_file(source: &Path, link: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(source, link)
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, link)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (source, link);
        Err(std::io::ErrorKind::Unsupported.into())
    }
}

/// Copy a file through a temporary name and keep the source's modification time, so
/// a half-written pack is never visible under its real name.
pub(crate) fn copy_atomic(source: &Path, destination: &Path) -> Result<()> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let temporary = tempfile::Builder::new()
        .prefix(".whmm-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(|error| io(parent, error))?;
    let mut input = fs::File::open(source).map_err(|error| io(source, error))?;
    std::io::copy(&mut input, &mut temporary.as_file()).map_err(|error| io(destination, error))?;
    if let Ok(modified) = input.metadata().and_then(|m| m.modified()) {
        let _ = temporary.as_file().set_modified(modified);
    }
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| io(destination, error))?;
    temporary
        .persist(destination)
        .map_err(|error| io(destination, error.error))?;
    Ok(())
}

/// Copy or link `packs` into `data`. Existing files are skipped and reported, so a
/// newer pack a modder keeps in `data` is never replaced.
pub fn place(
    game: &Path,
    packs: &[PathBuf],
    placement: Placement,
    cancelled: &AtomicBool,
) -> Result<Outcome> {
    let data = data_dir(game);
    if !data.is_dir() {
        return Err(Error::Invalid(message!(
            "The game's data folder was not found: {}",
            "Не найдена папка data игры: {}",
            data.display()
        )));
    }
    let mut outcome = Outcome::default();
    for source in packs {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let name = pack_name(source)?;
        let destination = data.join(name);
        if fs::symlink_metadata(&destination).is_ok() {
            outcome.failed.push(message!(
                "{} is already in data; remove it first to replace it",
                "{} уже есть в data; сначала удалите его, чтобы заменить",
                name
            ));
            continue;
        }
        let result = match placement {
            Placement::Copy => copy_atomic(source, &destination),
            Placement::Link => link_file(source, &destination).map_err(|e| io(&destination, e)),
        };
        match result {
            Ok(()) => outcome.done.push(name.to_owned()),
            Err(error) => outcome
                .failed
                .push(error.message().prefixed(&format!("{name}: "))),
        }
    }
    Ok(outcome)
}

/// Delete one pack from `data` (a link is removed, never its target).
pub fn remove(game: &Path, pack: &Path) -> Result<()> {
    let data = data_dir(game);
    let name = pack_name(pack)?;
    if pack.parent() != Some(data.as_path()) {
        return Err(Error::Invalid(message!(
            "Only packs directly in the data folder can be deleted: {}",
            "Удалять можно только pack-файлы прямо в папке data: {}",
            pack.display()
        )));
    }
    let path = data.join(name);
    fs::remove_file(&path).map_err(|error| io(&path, error))
}

/// Packs in `data` that the catalog also has outside `data` (the original's "Clean
/// data"), or only the links among them (its "Clean symbolic links in data").
pub fn removable(catalog: &Catalog, links_only: bool) -> Vec<PathBuf> {
    catalog
        .mods
        .iter()
        .filter(|item| item.source == Source::Data)
        .filter(|item| {
            if links_only {
                item.linked
            } else {
                catalog
                    .by_name
                    .get(&item.name.to_lowercase())
                    .is_some_and(|all| all.iter().any(|&i| catalog.mods[i].source != Source::Data))
                    || item.linked
            }
        })
        .map(|item| item.path.clone())
        .collect()
}

pub fn remove_all(game: &Path, packs: &[PathBuf]) -> Outcome {
    let mut outcome = Outcome::default();
    for pack in packs {
        let name = pack
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match remove(game, pack) {
            Ok(()) => outcome.done.push(name),
            Err(error) => outcome
                .failed
                .push(error.message().prefixed(&format!("{name}: "))),
        }
    }
    outcome
}
