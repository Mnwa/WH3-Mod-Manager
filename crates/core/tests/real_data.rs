//! Checks against a real Warhammer III installation; run with `--ignored`.
//!
//! The data folder comes from `WH3_DATA` or a few common Steam library locations, and the
//! Workshop folder from `WH3_WORKSHOP` or the same library. Nothing is written there. Without an
//! installation the tests print a notice and pass.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::{
    collections::{BTreeMap, HashSet},
    env,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Instant,
};
use wh3_core::{
    catalog::{Catalog, Source},
    conflict, db, pack, scan,
};

const GAME: &str = "steamapps/common/Total War WARHAMMER III/data";
const WORKSHOP: &str = "steamapps/workshop/content/1142710";

fn libraries() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for drive in ["c", "d", "e", "f"] {
        roots.push(PathBuf::from(format!(
            "/mnt/{drive}/Program Files (x86)/Steam"
        )));
        roots.push(PathBuf::from(format!("/mnt/{drive}/SteamLibrary")));
        roots.push(PathBuf::from(format!(
            "{drive}:\\Program Files (x86)\\Steam"
        )));
        roots.push(PathBuf::from(format!("{drive}:\\SteamLibrary")));
    }
    roots
}

fn locate(variable: &str, relative: &str) -> Option<PathBuf> {
    if let Some(path) = env::var_os(variable) {
        return Some(PathBuf::from(path)).filter(|path| path.is_dir());
    }
    libraries()
        .into_iter()
        .map(|library| library.join(relative))
        .find(|path| path.is_dir())
}

fn game_data() -> Option<PathBuf> {
    let found = locate("WH3_DATA", GAME);
    if found.is_none() {
        eprintln!("no Warhammer III installation found; set WH3_DATA");
    }
    found
}

fn workshop() -> Option<Catalog> {
    let root = locate("WH3_WORKSHOP", WORKSHOP)?;
    let scan = scan::scan(&[(root, Source::Workshop)], &AtomicBool::new(false)).unwrap();
    Some(scan.catalog)
}

/// Reads every DB table of a pack and returns (parsed, failed, rows) with failures by reason.
fn read_all(path: &Path, failures: &mut BTreeMap<String, usize>) -> (usize, usize, usize) {
    let Ok((mut reader, entries)) = pack::Reader::open_with_index(path) else {
        *failures.entry("unreadable pack".into()).or_default() += 1;
        return (0, 1, 0);
    };
    let (mut parsed, mut failed, mut rows) = (0, 0, 0);
    for entry in &entries {
        let Some(table) = db::table_name(&entry.name) else {
            continue;
        };
        if entry.name.ends_with(".rpfm_reserved") {
            continue;
        }
        match reader
            .read(entry)
            .and_then(|bytes| db::read_table(&bytes, &table))
        {
            Ok(table) => {
                parsed += 1;
                rows += table.rows.len();
            }
            Err(e) => {
                failed += 1;
                let text = e.to_string();
                // Group by reason without the varying numbers.
                let reason: String = text.chars().filter(|c| !c.is_ascii_digit()).collect();
                *failures.entry(reason).or_default() += 1;
            }
        }
    }
    (parsed, failed, rows)
}

#[test]
#[ignore = "needs a Warhammer III installation"]
fn vanilla_tables_parse() {
    let Some(data) = game_data() else { return };
    let started = Instant::now();
    db::schema().unwrap();
    eprintln!("schema decoded in {:?}", started.elapsed());
    let started = Instant::now();
    let mut failures = BTreeMap::new();
    let (parsed, failed, rows) = read_all(&data.join("db.pack"), &mut failures);
    eprintln!(
        "db.pack: {parsed} tables, {rows} rows, {failed} failed in {:?}",
        started.elapsed()
    );
    for (reason, count) in &failures {
        eprintln!("  {count:5} {reason}");
    }
    assert!(parsed > 1000, "vanilla db.pack should have >1000 tables");
    assert!(
        failed * 100 <= parsed,
        "more than 1% of vanilla tables failed"
    );
}

#[test]
#[ignore = "needs a Warhammer III installation with Workshop mods"]
fn workshop_tables_parse() {
    let Some(catalog) = workshop() else { return };
    let started = Instant::now();
    let mut failures = BTreeMap::new();
    let (mut parsed, mut failed, mut rows) = (0, 0, 0);
    for item in &catalog.mods {
        let (p, f, r) = read_all(&item.path, &mut failures);
        (parsed, failed, rows) = (parsed + p, failed + f, rows + r);
    }
    eprintln!(
        "{} mods: {parsed} tables, {rows} rows, {failed} failed in {:?}",
        catalog.mods.len(),
        started.elapsed()
    );
    for (reason, count) in &failures {
        eprintln!("  {count:5} {reason}");
    }
    assert!(parsed > 0);
}

#[test]
#[ignore = "needs a Warhammer III installation with Workshop mods"]
fn workshop_key_conflicts_and_generals() {
    let Some(catalog) = workshop() else { return };
    let order: Vec<usize> = (0..catalog.mods.len()).collect();
    let enabled: HashSet<usize> = order.iter().copied().collect();
    let cancelled = AtomicBool::new(false);
    // The index-only share of the check, for comparison with the full check below.
    let started = Instant::now();
    for item in &catalog.mods {
        let _ = pack::index(&item.path);
    }
    eprintln!(
        "indexes of {} mods read in {:?}",
        catalog.mods.len(),
        started.elapsed()
    );
    let started = Instant::now();
    let report = conflict::check(&catalog, &order, &enabled, &cancelled).unwrap();
    let keys: usize = report.tables.iter().map(|table| table.keys.len()).sum();
    let unchecked: usize = report
        .tables
        .iter()
        .map(|table| table.unchecked.len())
        .sum();
    eprintln!(
        "check of {} mods in {:?}: {} file conflicts, {} shared tables, {keys} key conflicts, \
         {unchecked} unchecked files, {} warnings",
        catalog.mods.len(),
        started.elapsed(),
        report.files.len(),
        report.tables.len(),
        report.warnings.len()
    );
    for table in report
        .tables
        .iter()
        .filter(|table| !table.keys.is_empty())
        .take(5)
    {
        let key = &table.keys[0];
        let winner = table.key_winner(key).unwrap();
        eprintln!(
            "  {} {:?} = {:?}: {} files, winner {}",
            table.table,
            table.key_fields,
            key.key,
            key.files.len(),
            catalog.mods[winner].name
        );
    }
    for warning in report.warnings.iter().take(5) {
        eprintln!("  warning: {warning}");
    }

    let Some(data) = game_data() else { return };
    let started = Instant::now();
    let bytes =
        db::make_units_generals_table(&data, &catalog, &order, &enabled, &cancelled).unwrap();
    let table = db::read_table(&bytes, "units_custom_battle_permissions_tables").unwrap();
    eprintln!(
        "generals table: {} rows, {} bytes in {:?}",
        table.rows.len(),
        bytes.len(),
        started.elapsed()
    );
    let general = table.definition.field("general_unit").unwrap();
    assert!(
        table
            .rows
            .iter()
            .any(|row| row[general] == db::Value::Bool(false))
    );
    assert!(
        table
            .rows
            .iter()
            .all(|row| matches!(row[general], db::Value::Bool(_)))
    );
}
