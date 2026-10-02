//! Compatibility report for the enabled mods.
//!
//! File conflicts come from pack headers and indexes only. Rows are read only for DB tables that
//! two or more enabled mods load, to find keys they both define.
//!
//! Priority follows the load order: a lower position in `order` is a higher priority, and the
//! game loads a packed file from the highest-priority pack that contains it (the original
//! manager's `higherPriorityPack`, `compatViewModels.ts`).
mod keys;
mod names;
mod overlay;
mod requirements;

use crate::{
    Error, Result,
    catalog::Catalog,
    localization::{Language, Message},
    pack,
};
use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Default)]
pub struct Report {
    /// Packed paths provided by two or more enabled mods, sorted by path.
    pub files: Vec<FileConflict>,
    /// DB tables whose loaded files come from two or more enabled mods, sorted by table name.
    pub tables: Vec<TableConflict>,
    pub missing_dependencies: Vec<MissingDependency>,
    pub missing_required: Vec<MissingRequired>,
    /// Enabled mods with a `startpos.esf` that are not ordered by a dependency on each other,
    /// in priority order. Only one start position is used by a campaign.
    pub startpos: Vec<usize>,
    /// Enabled mods whose pack could not be read; details are in `warnings`.
    pub unreadable: Vec<usize>,
    pub warnings: Vec<Message>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileConflict {
    /// Lower-case path with `\` separators.
    pub path: String,
    /// Catalog indices in priority order; `owners[0]` is the pack the game loads the file from.
    pub owners: Vec<usize>,
    /// Every copy has the same stored size, which usually means an identical file.
    pub same_size: bool,
}

/// DB files of one table that the game loads from different mods.
///
/// Files with the same path are reported in [`Report::files`] instead: only the winning copy is
/// loaded, so it alone is listed here. Sharing a table is harmless by itself; [`Self::keys`] lists
/// the rows that actually replace each other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableConflict {
    /// Table folder name, e.g. `main_units_tables`.
    pub table: String,
    /// In row priority order: for a duplicated key the game keeps the row from the file whose
    /// name sorts first (the original manager's `higherPriorityDatabaseFile`), regardless of pack
    /// order. Equal names keep load-order priority.
    pub files: Vec<TableFile>,
    /// Key column names of the newest schema version. Empty when the table has no key or is not
    /// in the bundled schema; then no rows were compared.
    pub key_fields: Vec<String>,
    /// Keys defined by files of two or more different mods, sorted by key.
    pub keys: Vec<KeyConflict>,
    /// Indices into `files` whose rows could not be read, e.g. an outdated table version; their
    /// keys are not part of `keys`. Details are in [`Report::warnings`].
    pub unchecked: Vec<usize>,
}

/// One key that rows from several mods define; the game keeps only the first file's row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyConflict {
    /// Key values in key column order (see [`TableConflict::key_fields`]).
    pub key: Vec<String>,
    /// Indices into [`TableConflict::files`] in row priority order; `files[0]` wins.
    pub files: Vec<usize>,
}

impl TableConflict {
    /// Catalog index of the mod whose row the game loads for a key conflict.
    pub fn key_winner(&self, key: &KeyConflict) -> Option<usize> {
        let first = *key.files.first()?;
        self.files.get(first).map(|file| file.owner)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableFile {
    pub owner: usize,
    /// Full normalized path, e.g. `db\main_units_tables\!my_units`.
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// Installed but not enabled; the catalog index of a copy that would satisfy it.
    Disabled(usize),
    NotInstalled,
}

/// A pack header dependency that no enabled mod provides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingDependency {
    pub owner: usize,
    /// As written in the pack header.
    pub dependency: String,
    pub availability: Availability,
}

/// A Steam Workshop requirement (`metadata.req_mod_id_to_name`) that no enabled mod provides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingRequired {
    pub owner: usize,
    pub workshop_id: String,
    pub name: String,
    pub availability: Availability,
}

/// Per-row badge counts derived from a [`Report`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModIssues {
    /// Files of this mod hidden by a higher-priority mod.
    pub overwritten: usize,
    /// Files of this mod that hide a lower-priority copy.
    pub overwrites: usize,
    /// Tables this mod shares with other enabled mods.
    pub tables: usize,
    /// DB keys of this mod whose row is replaced by another mod's row.
    pub keys_overwritten: usize,
    /// DB keys of this mod whose row replaces another mod's row.
    pub keys_overwrites: usize,
    pub missing_dependencies: usize,
    pub missing_required: usize,
    pub startpos: bool,
    pub unreadable: bool,
}

impl Report {
    /// Badge counts for every mod that has at least one issue.
    pub fn issues_by_mod(&self) -> HashMap<usize, ModIssues> {
        let mut issues: HashMap<usize, ModIssues> = HashMap::new();
        for conflict in &self.files {
            if let Some((&winner, rest)) = conflict.owners.split_first() {
                issues.entry(winner).or_default().overwrites += 1;
                for &owner in rest {
                    issues.entry(owner).or_default().overwritten += 1;
                }
            }
        }
        for table in &self.tables {
            let owners: HashSet<_> = table.files.iter().map(|file| file.owner).collect();
            for owner in owners {
                issues.entry(owner).or_default().tables += 1;
            }
            for key in &table.keys {
                let Some(winner) = table.key_winner(key) else {
                    continue;
                };
                issues.entry(winner).or_default().keys_overwrites += 1;
                let losers: HashSet<_> = key
                    .files
                    .iter()
                    .filter_map(|&file| table.files.get(file))
                    .map(|file| file.owner)
                    .filter(|&owner| owner != winner)
                    .collect();
                for owner in losers {
                    issues.entry(owner).or_default().keys_overwritten += 1;
                }
            }
        }
        for missing in &self.missing_dependencies {
            issues
                .entry(missing.owner)
                .or_default()
                .missing_dependencies += 1;
        }
        for missing in &self.missing_required {
            issues.entry(missing.owner).or_default().missing_required += 1;
        }
        for &index in &self.startpos {
            issues.entry(index).or_default().startpos = true;
        }
        for &index in &self.unreadable {
            issues.entry(index).or_default().unreadable = true;
        }
        issues
    }
}

/// Checks the enabled mods of `order`.
///
/// Reads pack headers and indexes, plus the DB files of tables that several enabled mods load.
/// Cancellation is checked between packs and between DB files.
pub fn check(
    catalog: &Catalog,
    order: &[usize],
    enabled: &HashSet<usize>,
    cancelled: &AtomicBool,
) -> Result<Report> {
    let mut report = Report::default();
    let mut active = requirements::Active {
        catalog,
        mods: Vec::new(),
        members: HashSet::new(),
        by_name: HashMap::new(),
    };
    for &index in order.iter().filter(|index| enabled.contains(index)) {
        let Some(item) = catalog.mods.get(index) else {
            continue;
        };
        // The game cannot load two packs with one name; the original manager skips such pairs.
        if active.by_name.contains_key(&item.name.to_lowercase()) {
            report.warnings.push(crate::message!(
                "{}: enabled more than once; only the higher-priority copy was checked",
                "{}: включён несколько раз; проверена только копия с более высоким приоритетом",
                item.name
            ));
            continue;
        }
        active.by_name.insert(item.name.to_lowercase(), index);
        active.members.insert(index);
        active.mods.push(index);
    }
    let mut overlay = overlay::Overlay::default();
    let mut startpos = Vec::new();
    for &index in &active.mods {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let item = &catalog.mods[index];
        match pack::index(&item.path) {
            Ok(files) => {
                if overlay.add(index, &files, cancelled)? {
                    startpos.push(index);
                }
            }
            Err(e) => {
                let detail = e.message();
                report.unreadable.push(index);
                report.warnings.push(Message::new(
                    format!("{}: {}", item.name, detail.text(Language::English)),
                    format!("{}: {}", item.name, detail.text(Language::Russian)),
                ));
            }
        }
    }
    let finished = overlay.finish();
    report.files = finished.files;
    report.tables = finished.tables;
    keys::check(
        &mut report.tables,
        &finished.db_entries,
        catalog,
        cancelled,
        &mut report.warnings,
    )?;
    report.missing_dependencies = active.missing_dependencies();
    report.missing_required = active.missing_required();
    report.startpos = active.startpos_conflicts(&startpos);
    Ok(report)
}
