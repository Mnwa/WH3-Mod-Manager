//! Duplicate DB keys between the loaded files of shared tables.
//!
//! The original manager compares only single-key tables (`findPackTableCollisions`,
//! `modCompat/packTableCollisions.ts:330`); composite keys are compared here as whole tuples,
//! which is how the game identifies a row.
mod reader;

use super::{KeyConflict, TableConflict};
use crate::{
    Result,
    catalog::Catalog,
    db,
    localization::{Language, Message},
    pack::PackedFile,
};
use reader::{File, Job, Outcome};
use std::{
    collections::{BTreeMap, HashMap},
    sync::atomic::AtomicBool,
};

/// Files that defined a key, as indices into [`TableConflict::files`].
type KeyFiles = HashMap<Vec<String>, Vec<usize>>;

/// Per-mod failures, summarized into one warning each.
#[derive(Default)]
struct Failures {
    by_owner: BTreeMap<usize, (usize, Message)>,
}

impl Failures {
    fn add(&mut self, owner: usize, path: &str, reason: Message) {
        let entry = self
            .by_owner
            .entry(owner)
            .or_insert_with(|| (0, Message::from(format!("{path}: ")).concat(reason)));
        entry.0 += 1;
    }

    fn into_warnings(self, catalog: &Catalog, warnings: &mut Vec<Message>) {
        for (owner, (count, example)) in self.by_owner {
            let name = catalog.mods.get(owner).map_or("", |item| &item.name);
            warnings.push(Message::new(
                format!(
                    "{name}: duplicate keys were not checked in {count} DB file(s), e.g. {}",
                    example.text(Language::English)
                ),
                format!(
                    "{name}: повторяющиеся ключи не проверены в файлах DB ({count}), например {}",
                    example.text(Language::Russian)
                ),
            ));
        }
    }
}

/// Sets `key_fields` from the newest schema version and returns whether rows can be compared.
fn prepare(table: &mut TableConflict, schema: &db::Schema, warnings: &mut Vec<Message>) -> bool {
    let Some(newest) = schema
        .versions(&table.table)
        .and_then(|versions| versions.iter().max_by_key(|definition| definition.version))
    else {
        table.unchecked = (0..table.files.len()).collect();
        warnings.push(crate::message!(
            "{}: the table is not in the bundled schema; duplicate keys were not checked",
            "{}: таблицы нет во встроенной схеме; повторяющиеся ключи не проверены",
            table.table
        ));
        return false;
    };
    table.key_fields = newest
        .key_fields()
        .filter_map(|index| newest.fields.get(index))
        .map(|field| field.name.clone())
        .collect();
    !table.key_fields.is_empty()
}

/// Fills `keys`, `key_fields` and `unchecked` of every table conflict.
///
/// Unreadable files become warnings; only cancellation is an error.
pub(super) fn check(
    tables: &mut [TableConflict],
    entries: &HashMap<String, PackedFile>,
    catalog: &Catalog,
    cancelled: &AtomicBool,
    warnings: &mut Vec<Message>,
) -> Result<()> {
    if tables.is_empty() {
        return Ok(());
    }
    let schema = match db::schema() {
        Ok(schema) => schema,
        Err(e) => {
            warnings.push(e.message());
            return Ok(());
        }
    };
    let checked: Vec<bool> = tables
        .iter_mut()
        .map(|table| prepare(table, schema, warnings))
        .collect();
    // Owned copies of what the readers need, so the report stays free for merging results.
    let names: Vec<String> = tables.iter().map(|table| table.table.clone()).collect();
    let paths: Vec<Vec<String>> = tables
        .iter()
        .map(|table| table.files.iter().map(|file| file.path.clone()).collect())
        .collect();
    // Every pack is opened once, for all of its files.
    let mut by_owner: BTreeMap<usize, Vec<File>> = BTreeMap::new();
    for (table_index, table) in tables.iter().enumerate() {
        if !checked[table_index] {
            continue;
        }
        for (file_index, file) in table.files.iter().enumerate() {
            by_owner.entry(file.owner).or_default().push(File {
                table: table_index,
                file: file_index,
                table_name: &names[table_index],
                entry: entries.get(&file.path),
            });
        }
    }
    let jobs: Vec<Job> = by_owner
        .into_iter()
        .filter_map(|(owner, files)| {
            let path = &catalog.mods.get(owner)?.path;
            Some(Job { owner, path, files })
        })
        .collect();

    let mut found: Vec<KeyFiles> = vec![HashMap::new(); tables.len()];
    let mut failures = Failures::default();
    reader::read_all(&jobs, cancelled, |outcome| match outcome {
        Outcome::Keys { table, file, keys } => {
            for key in keys {
                let files = found[table].entry(key).or_default();
                if files.last() != Some(&file) {
                    files.push(file);
                }
            }
        }
        Outcome::Failed {
            table,
            file,
            owner,
            reason,
        } => {
            failures.add(owner, &paths[table][file], reason);
            tables[table].unchecked.push(file);
        }
    })?;
    failures.into_warnings(catalog, warnings);

    for (table, keys) in tables.iter_mut().zip(found) {
        table.unchecked.sort_unstable();
        table.keys = conflicts(table, keys);
    }
    Ok(())
}

/// Keys whose files belong to more than one mod, files in row priority order.
fn conflicts(table: &TableConflict, keys: KeyFiles) -> Vec<KeyConflict> {
    let mut conflicts: Vec<KeyConflict> = keys
        .into_iter()
        .filter_map(|(key, mut files)| {
            // File indices follow row priority, and packs were not read in that order.
            files.sort_unstable();
            files.dedup();
            let owner = |index: usize| table.files.get(index).map(|file| file.owner);
            let first = owner(*files.first()?);
            files
                .iter()
                .any(|&index| owner(index) != first)
                .then_some(KeyConflict { key, files })
        })
        .collect();
    conflicts.sort_unstable_by(|a, b| a.key.cmp(&b.key));
    conflicts
}
