#![allow(clippy::unwrap_used, clippy::expect_used)]
mod support;

use std::{collections::HashSet, path::Path, sync::atomic::AtomicBool};
use support::{Cell, packed, permission, plain, rows_bytes, table, write_pack, zstd};
use wh3_core::{
    Error,
    catalog::{Catalog, Source},
    db::{self, Value},
    scan,
};

const TABLE: &str = "db\\units_custom_battle_permissions_tables\\";

fn with_general<'a>(mut row: Vec<Cell<'a>>, general: bool) -> Vec<Cell<'a>> {
    row[1] = Cell::Bool(general);
    row
}

fn file(name: &str) -> String {
    format!("{TABLE}{name}")
}

struct Setup {
    _temp: tempfile::TempDir,
    data: std::path::PathBuf,
    catalog: Catalog,
    order: Vec<usize>,
    enabled: HashSet<usize>,
}

fn setup() -> Setup {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let mods = temp.path().join("mods");
    let vanilla = table(
        Some("vanilla"),
        Some(11),
        &[
            permission("emp", "general_one", true),
            permission("emp", "unit_one", false),
        ],
    );
    write_pack(
        &data.join("db.pack"),
        &[packed(&file("data__"), zstd(&vanilla))],
    );
    let a = table(None, Some(11), &[permission("dwf", "unit_two", false)]);
    write_pack(&mods.join("a.pack"), &[plain(&file("a"), &a)]);
    let old = table(None, Some(10), &[]);
    let b = table(None, Some(11), &[permission("grn", "general_two", true)]);
    write_pack(
        &mods.join("b.pack"),
        &[plain(&file("b_old"), &old), plain(&file("b"), &b)],
    );
    let c = table(None, Some(11), &[permission("chs", "disabled", false)]);
    write_pack(&mods.join("c.pack"), &[plain(&file("c"), &c)]);
    write_pack(
        &mods.join("d.pack"),
        &[plain(&file("d"), &a[..a.len() - 1])],
    );

    let catalog = scan::scan(&[(mods, Source::Custom)], &AtomicBool::new(false))
        .unwrap()
        .catalog;
    let order: Vec<usize> = ["a.pack", "b.pack", "d.pack", "c.pack"]
        .map(|name| catalog.by_name[name][0])
        .into();
    let enabled = order[..3].iter().copied().collect();
    Setup {
        _temp: temp,
        data,
        catalog,
        order,
        enabled,
    }
}

fn generate(setup: &Setup, data: &Path, cancelled: bool) -> wh3_core::Result<Vec<u8>> {
    let cancelled = AtomicBool::new(cancelled);
    db::make_units_generals_table(
        data,
        &setup.catalog,
        &setup.order,
        &setup.enabled,
        &cancelled,
    )
}

#[test]
fn every_unit_becomes_a_general_and_generals_keep_a_unit_copy() {
    let setup = setup();
    let bytes = generate(&setup, &setup.data, false).unwrap();

    // The header the original manager writes: fixed GUID, version, the `1` byte, row count.
    let mut expected = vec![0xfd, 0xfe, 0xfc, 0xff, 36, 0];
    for unit in "129d32d8-3563-4d4f-8e19-a815e834e456".encode_utf16() {
        expected.extend_from_slice(&unit.to_le_bytes());
    }
    expected.extend_from_slice(&[0xfc, 0xfd, 0xfe, 0xff, 11, 0, 0, 0, 1, 6, 0, 0, 0]);
    assert_eq!(expected.len(), 91, "the original's fixed header size");
    // Enabled mods in priority order, then vanilla. Per file: every row as a general, then the
    // rows that already were generals as non-general copies. The version 10 file, the broken
    // pack and the disabled mod contribute nothing.
    expected.extend(rows_bytes(&[
        with_general(permission("dwf", "unit_two", false), true),
        permission("grn", "general_two", true),
        with_general(permission("grn", "general_two", true), false),
        permission("emp", "general_one", true),
        with_general(permission("emp", "unit_one", false), true),
        with_general(permission("emp", "general_one", true), false),
    ]));
    assert_eq!(bytes, expected);

    let parsed = db::read_table(&bytes, "units_custom_battle_permissions_tables").unwrap();
    assert_eq!(parsed.version, 11);
    let general: Vec<&Value> = parsed.rows.iter().map(|row| &row[1]).collect();
    let flags = [true, true, false, true, true, false].map(Value::Bool);
    assert_eq!(general, flags.iter().collect::<Vec<_>>());
    assert_eq!(db::UNITS_GENERALS_PATH, format!("{TABLE}!!!!whmm_out"));
}

#[test]
fn missing_vanilla_table_and_cancellation_are_errors() {
    let setup = setup();
    let empty = setup.data.join("empty");
    write_pack(&empty.join("db.pack"), &[plain("db\\other_tables\\x", b"")]);
    assert!(matches!(
        generate(&setup, &empty, false),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        generate(&setup, &setup.data.join("missing"), false),
        Err(Error::Io { .. })
    ));
    assert!(matches!(
        generate(&setup, &setup.data, true),
        Err(Error::Cancelled)
    ));
}
