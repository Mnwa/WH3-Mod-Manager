//! "Outdated overwriting packs", as the original warns: Workshop mods older than
//! the last game update that replace vanilla DB tables or Lua scripts. Age alone
//! is not a signal: most mods keep working across patches.
use crate::{
    Error, Result,
    catalog::{Catalog, Source},
    pack,
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::SystemTime,
};

/// Packed files whose vanilla copies a mod can silently freeze in an old state.
fn is_candidate(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("db\\") || name.starts_with("db/") || name.ends_with(".lua")
}

fn normalized(name: &str) -> String {
    name.replace('/', "\\").to_ascii_lowercase()
}

/// DB and Lua paths of the game's own packs in `data` (pack type below 3, the
/// same rule the scanner uses to skip vanilla packs). Only paths are read.
pub fn vanilla_files(data: &Path, cancelled: &AtomicBool) -> Result<HashSet<String>> {
    let mut files = HashSet::new();
    let entries = fs::read_dir(data).map_err(|e| crate::error::io(data, e))?;
    for entry in entries.flatten() {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let path = entry.path();
        let is_pack = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pack"));
        if !is_pack
            || pack::header(&path)
                .map(|h| h.flags & 0xf >= 3)
                .unwrap_or(true)
        {
            continue;
        }
        if let Ok(index) = pack::index(&path) {
            files.extend(
                index
                    .iter()
                    .filter(|f| is_candidate(&f.name))
                    .map(|f| normalized(&f.name)),
            );
        }
    }
    Ok(files)
}

/// Workshop mods older than `game_updated` with the vanilla files they overwrite.
pub fn overwriting(
    catalog: &Catalog,
    game_updated: SystemTime,
    vanilla: &HashSet<String>,
    cancelled: &AtomicBool,
) -> Result<HashMap<usize, Vec<String>>> {
    let mut outdated = HashMap::new();
    for (index, item) in catalog.mods.iter().enumerate() {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        if item.source != Source::Workshop || !item.is_outdated(game_updated) {
            continue;
        }
        let Ok(files) = pack::index(&item.path) else {
            continue;
        };
        let overwritten: Vec<String> = files
            .iter()
            .filter(|f| is_candidate(&f.name))
            .map(|f| normalized(&f.name))
            .filter(|name| vanilla.contains(name))
            .collect();
        if !overwritten.is_empty() {
            outdated.insert(index, overwritten);
        }
    }
    Ok(outdated)
}
