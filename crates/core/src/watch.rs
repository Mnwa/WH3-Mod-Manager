//! Change signals for mod folders and campaign saves, so the library follows Steam
//! downloads and new saves without a manual rescan.
//!
//! The watcher only counts relevant events; callers poll the counters and decide when
//! the folders have been quiet long enough to rescan. That keeps bursts of events from
//! a Workshop download from turning into one rescan per file.
use crate::{Error, Result, catalog::Source, message};
use notify::{Event, EventKind, RecursiveMode, Watcher as _};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

/// Event counters; a changed value means "something changed since you last looked".
#[derive(Debug, Default)]
pub struct Signals {
    packs: AtomicU64,
    saves: AtomicU64,
}

impl Signals {
    pub fn packs(&self) -> u64 {
        self.packs.load(Ordering::Relaxed)
    }
    pub fn saves(&self) -> u64 {
        self.saves.load(Ordering::Relaxed)
    }
}

/// Keeps the operating system watches alive; dropping it stops watching.
pub struct Watcher {
    _inner: notify::RecommendedWatcher,
    pub signals: Arc<Signals>,
}

/// What an event means for the manager.
#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    Packs,
    Saves,
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .is_some_and(|value| value.eq_ignore_ascii_case(extension))
}

/// Classify one event path. Packs count anywhere under a mod folder; folders appearing
/// or disappearing directly in a root count too, because Steam moves whole Workshop
/// item folders into place and deletes them on unsubscribe.
pub fn classify(
    kind: &EventKind,
    path: &Path,
    roots: &[PathBuf],
    saves: Option<&Path>,
) -> Option<Change> {
    if matches!(kind, EventKind::Access(_)) {
        return None;
    }
    if let Some(saves) = saves
        && path.starts_with(saves)
    {
        return has_extension(path, "save").then_some(Change::Saves);
    }
    let root = roots.iter().find(|root| path.starts_with(root))?;
    if has_extension(path, "pack") {
        return Some(Change::Packs);
    }
    let structural = matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(notify::event::ModifyKind::Name(_))
    );
    (structural && path.parent() == Some(root.as_path()) && path.extension().is_none())
        .then_some(Change::Packs)
}

/// Watch every mod folder recursively and the save folder for `*.save` files.
/// Folders that do not exist are skipped; the next scan reports them.
pub fn watch(roots: &[(PathBuf, Source)], saves: Option<&Path>) -> Result<Watcher> {
    let signals = Arc::new(Signals::default());
    // FSEvents reports canonical paths, including /private/var for /var on macOS.
    // Resolve roots once for both registration and classification; event paths may
    // already have been deleted and cannot safely be canonicalized in the callback.
    let folders: Vec<PathBuf> = roots
        .iter()
        .map(|(root, _)| root.as_path())
        .filter(|folder| folder.is_dir())
        .map(canonical_folder)
        .collect();
    let saves_folder = saves.filter(|folder| folder.is_dir()).map(canonical_folder);
    let sink = signals.clone();
    let (watched, saves_watched) = (folders.clone(), saves_folder.clone());
    let mut inner = notify::recommended_watcher(move |event: notify::Result<Event>| {
        let Ok(event) = event else { return };
        for path in &event.paths {
            match classify(&event.kind, path, &watched, saves_watched.as_deref()) {
                Some(Change::Packs) => sink.packs.fetch_add(1, Ordering::Relaxed),
                Some(Change::Saves) => sink.saves.fetch_add(1, Ordering::Relaxed),
                None => continue,
            };
        }
    })
    .map_err(watch_error)?;
    for folder in &folders {
        inner
            .watch(folder, RecursiveMode::Recursive)
            .map_err(watch_error)?;
    }
    if let Some(saves) = saves_folder.as_deref() {
        inner
            .watch(saves, RecursiveMode::NonRecursive)
            .map_err(watch_error)?;
    }
    Ok(Watcher {
        _inner: inner,
        signals,
    })
}

/// Windows cannot resolve final paths on some RAM, network and virtual drives. The
/// backends there report events under the registered path, so watching the original
/// path keeps updates working instead of failing every folder.
fn canonical_folder(folder: &Path) -> PathBuf {
    folder
        .canonicalize()
        .unwrap_or_else(|_| folder.to_path_buf())
}

fn watch_error(error: notify::Error) -> Error {
    Error::Invalid(message!(
        "Cannot watch the mod folders for changes: {}. Use Rescan after adding mods.",
        "Не удалось следить за папками модов: {}. После добавления модов нажмите «Пересканировать».",
        error
    ))
}
