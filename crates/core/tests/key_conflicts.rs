#![allow(clippy::unwrap_used, clippy::expect_used)]
mod support;

use std::{collections::HashSet, path::Path, sync::atomic::AtomicBool};
use support::{Cell, packed, permission, plain, table, write_pack, zstd};
use wh3_core::{
    catalog::{Catalog, Source},
    conflict::{self, KeyConflict},
    scan,
};

const UNITS: &str = "db\\units_custom_battle_permissions_tables\\";
const ADVISORS: &str = "db\\advisors_tables\\";

fn advisors(names: &[&'static str]) -> Vec<u8> {
    let rows: Vec<Vec<Cell>> = names
        .iter()
        .map(|&name| vec![Cell::Text(name), Cell::Wide("icon.png")])
        .collect();
    table(None, Some(2), &rows)
}

fn scan_custom(root: &Path) -> Catalog {
    scan::scan(&[(root.into(), Source::Custom)], &AtomicBool::new(false))
        .unwrap()
        .catalog
}

fn index(catalog: &Catalog, name: &str) -> usize {
    catalog.by_name[name][0]
}

fn check(catalog: &Catalog, order: &[usize]) -> conflict::Report {
    let enabled: HashSet<usize> = order.iter().copied().collect();
    conflict::check(catalog, order, &enabled, &AtomicBool::new(false)).unwrap()
}

#[test]
fn duplicate_keys_name_the_mod_whose_row_is_loaded() {
    let temp = tempfile::tempdir().unwrap();
    let a_file = format!("{ADVISORS}zz_a");
    let b_file = format!("{ADVISORS}aa_b");
    let b_extra = format!("{ADVISORS}bb_b");
    write_pack(
        &temp.path().join("a.pack"),
        &[plain(&a_file, &advisors(&["shared", "only_a", "twice"]))],
    );
    write_pack(
        &temp.path().join("b.pack"),
        &[
            packed(&b_file, zstd(&advisors(&["shared", "only_b"]))),
            plain(&b_extra, &advisors(&["only_b", "twice"])),
        ],
    );
    write_pack(
        &temp.path().join("c.pack"),
        &[plain(&format!("{ADVISORS}c"), &advisors(&["only_c"]))],
    );
    let catalog = scan_custom(temp.path());
    let [a, b, c] = ["a.pack", "b.pack", "c.pack"].map(|name| index(&catalog, name));

    // `a` has the higher pack priority, but `aa_b` sorts first, so `b`'s rows are loaded.
    let report = check(&catalog, &[a, b, c]);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let table = &report.tables[0];
    assert_eq!(table.key_fields, ["advisor_name"]);
    let files: Vec<&str> = table.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(
        files,
        [
            b_file.as_str(),
            b_extra.as_str(),
            "db\\advisors_tables\\c",
            &a_file
        ]
    );
    assert_eq!(
        table.keys,
        [
            KeyConflict {
                key: vec!["shared".into()],
                files: vec![0, 3],
            },
            KeyConflict {
                key: vec!["twice".into()],
                files: vec![1, 3],
            },
        ],
        "`only_b` is defined twice by one mod, which is not a conflict"
    );
    assert_eq!(table.key_winner(&table.keys[0]), Some(b));
    let issues = report.issues_by_mod();
    assert_eq!(
        (issues[&b].keys_overwrites, issues[&b].keys_overwritten),
        (2, 0)
    );
    assert_eq!(
        (issues[&a].keys_overwrites, issues[&a].keys_overwritten),
        (0, 2)
    );
    assert_eq!(
        (issues[&c].keys_overwrites, issues[&c].keys_overwritten),
        (0, 0)
    );
}

#[test]
fn composite_keys_compare_whole_tuples() {
    let temp = tempfile::tempdir().unwrap();
    let rows = |general: bool| table(None, Some(11), &[permission("emp", "swordsmen", general)]);
    write_pack(
        &temp.path().join("a.pack"),
        &[plain(&format!("{UNITS}a"), &rows(true))],
    );
    write_pack(
        &temp.path().join("b.pack"),
        &[plain(&format!("{UNITS}b"), &rows(false))],
    );
    write_pack(
        &temp.path().join("c.pack"),
        &[plain(&format!("{UNITS}c"), &rows(true))],
    );
    let catalog = scan_custom(temp.path());
    let [a, b, c] = ["a.pack", "b.pack", "c.pack"].map(|name| index(&catalog, name));

    let report = check(&catalog, &[a, b]);
    assert_eq!(
        report.tables[0].key_fields,
        ["faction", "general_unit", "unit"]
    );
    assert!(
        report.tables[0].keys.is_empty(),
        "general_unit is part of the key"
    );

    let report = check(&catalog, &[c, a, b]);
    let keys = &report.tables[0].keys;
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].key, ["emp", "true", "swordsmen"]);
    assert_eq!(report.tables[0].key_winner(&keys[0]), Some(a));
}

#[test]
fn unreadable_tables_become_warnings() {
    let temp = tempfile::tempdir().unwrap();
    let mut broken = advisors(&["shared"]);
    broken.truncate(broken.len() - 2);
    write_pack(
        &temp.path().join("a.pack"),
        &[plain(&format!("{ADVISORS}a"), &advisors(&["shared"]))],
    );
    write_pack(
        &temp.path().join("b.pack"),
        &[plain(&format!("{ADVISORS}b"), &broken)],
    );
    write_pack(
        &temp.path().join("c.pack"),
        &[plain("db\\made_up_tables\\c", b"?")],
    );
    write_pack(
        &temp.path().join("d.pack"),
        &[plain("db\\made_up_tables\\d", b"?")],
    );
    let catalog = scan_custom(temp.path());
    let order: Vec<usize> = ["a.pack", "b.pack", "c.pack", "d.pack"]
        .map(|name| index(&catalog, name))
        .into();

    let report = check(&catalog, &order);
    let advisors = &report.tables[0];
    assert_eq!(advisors.table, "advisors_tables");
    assert!(advisors.keys.is_empty());
    assert_eq!(advisors.unchecked, [1]);
    let unknown = &report.tables[1];
    assert!(unknown.key_fields.is_empty());
    assert_eq!(unknown.unchecked, [0, 1]);
    assert_eq!(report.warnings.len(), 2, "{:?}", report.warnings);
    assert!(report.warnings[0].to_string().contains("made_up_tables"));
    assert!(report.warnings[1].to_string().starts_with("b.pack: "));
}
