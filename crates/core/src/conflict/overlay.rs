//! Builds the virtual file system the game sees from enabled pack indexes, in priority order.
use super::{FileConflict, TableConflict, TableFile, names};
use crate::{Error, Result, pack::PackedFile};
use std::{
    collections::{BTreeMap, HashMap},
    sync::atomic::{AtomicBool, Ordering},
};

/// The game reads DB tables only from this folder; copies elsewhere are inert.
const DB_ROOT: &str = "db\\";
/// RPFM placeholder entries are never loaded by the game.
const RESERVED_SUFFIX: &str = ".rpfm_reserved";

struct Slot {
    winner: usize,
    size: u32,
    conflict: Option<usize>,
}

/// Packs must be added from the highest priority to the lowest, so the first owner of a path
/// is the pack the game actually loads it from.
#[derive(Default)]
pub(super) struct Overlay {
    slots: HashMap<String, Slot>,
    files: Vec<FileConflict>,
    tables: BTreeMap<String, Vec<TableFile>>,
    /// Index entries of the loaded DB files by normalized path, for reading their rows later.
    db_entries: HashMap<String, PackedFile>,
}

/// What the overlay found, before rows are compared.
pub(super) struct Finished {
    pub(super) files: Vec<FileConflict>,
    pub(super) tables: Vec<TableConflict>,
    pub(super) db_entries: HashMap<String, PackedFile>,
}

impl Overlay {
    /// Adds one pack's index and returns whether it contains a start position file.
    pub(super) fn add(
        &mut self,
        index: usize,
        files: &[PackedFile],
        cancelled: &AtomicBool,
    ) -> Result<bool> {
        let mut startpos = false;
        for file in files {
            if cancelled.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let path = names::normalize_path(&file.name);
            if path.ends_with(RESERVED_SUFFIX) {
                continue;
            }
            startpos |= names::is_startpos(&path);
            if let Some(slot) = self.slots.get_mut(&path) {
                if slot.winner == index {
                    continue;
                }
                let conflict = *slot.conflict.get_or_insert_with(|| {
                    self.files.push(FileConflict {
                        path: path.clone(),
                        owners: vec![slot.winner],
                        same_size: true,
                    });
                    self.files.len() - 1
                });
                let entry = &mut self.files[conflict];
                if entry.owners.last() != Some(&index) {
                    entry.owners.push(index);
                    entry.same_size &= file.size == slot.size;
                }
                continue;
            }
            // Only the winning copy of a DB file contributes rows, so shadowed copies are left to
            // the file report and never counted as table conflicts.
            if let Some((table, _)) = path
                .strip_prefix(DB_ROOT)
                .and_then(|rest| rest.split_once('\\'))
                .filter(|(table, file)| !table.is_empty() && !file.is_empty())
            {
                self.tables
                    .entry(table.to_owned())
                    .or_default()
                    .push(TableFile {
                        owner: index,
                        path: path.clone(),
                    });
                self.db_entries.insert(path.clone(), file.clone());
            }
            self.slots.insert(
                path,
                Slot {
                    winner: index,
                    size: file.size,
                    conflict: None,
                },
            );
        }
        Ok(startpos)
    }

    pub(super) fn finish(mut self) -> Finished {
        self.files.sort_by(|a, b| a.path.cmp(&b.path));
        let tables = self
            .tables
            .into_iter()
            .filter_map(|(table, mut files)| {
                let first = files.first()?.owner;
                if files.iter().all(|file| file.owner == first) {
                    return None;
                }
                // Stable sort: equal names keep load-order priority as the tie-breaker.
                files.sort_by(|a, b| {
                    names::compare_names(names::base_name(&a.path), names::base_name(&b.path))
                });
                Some(TableConflict {
                    table,
                    files,
                    key_fields: Vec::new(),
                    keys: Vec::new(),
                    unchecked: Vec::new(),
                })
            })
            .collect();
        Finished {
            files: self.files,
            tables,
            db_entries: self.db_entries,
        }
    }
}
