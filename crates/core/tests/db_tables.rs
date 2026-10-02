#![allow(clippy::unwrap_used, clippy::expect_used)]
mod support;

use support::{Cell, permission, table};
use wh3_core::{
    Error,
    db::{self, Value},
};

const VICTORY: &str = "battle_types_to_victory_conditions_tables";
const PERMISSIONS: &str = "units_custom_battle_permissions_tables";

fn victory_rows() -> Vec<Vec<Cell<'static>>> {
    vec![
        vec![
            Cell::Wide("land_normal_ŵ"),
            Cell::Optional(Some("kill_all")),
            Cell::Optional(None),
            Cell::Optional(Some("")),
            Cell::Optional(None),
        ],
        vec![
            Cell::Wide("siege"),
            Cell::Optional(None),
            Cell::Optional(Some("hold")),
            Cell::Optional(None),
            Cell::Optional(Some("x")),
        ],
    ]
}

#[test]
fn schema_is_embedded_and_resolves_versions() {
    let schema = db::schema().unwrap();
    assert!(schema.versions("main_units_tables").is_some());
    let permissions = schema.resolve(PERMISSIONS, Some(11)).unwrap();
    assert_eq!(permissions.fields.len(), 11);
    let keys: Vec<&str> = permissions
        .key_fields()
        .map(|index| permissions.fields[index].name.as_str())
        .collect();
    assert_eq!(keys, ["faction", "general_unit", "unit"]);
    assert!(schema.resolve(PERMISSIONS, Some(999)).is_none());
    assert!(schema.resolve("not_a_table", None).is_none());
}

#[test]
fn reads_header_rows_and_values() {
    let bytes = table(Some("guid-ŵ"), Some(3), &victory_rows());
    let header = db::read_header(&bytes).unwrap();
    assert_eq!(header.guid.as_deref(), Some("guid-ŵ"));
    assert_eq!((header.version, header.rows), (Some(3), 2));

    let parsed = db::read_table(&bytes, VICTORY).unwrap();
    assert_eq!(parsed.version, 3);
    assert_eq!(
        parsed.rows[0],
        [
            Value::Text("land_normal_ŵ".into()),
            Value::Text("kill_all".into()),
            Value::Missing,
            Value::Text(String::new()),
            Value::Missing,
        ]
    );
    // Composite keys keep every key column; an absent optional key is empty.
    assert_eq!(parsed.key(1).unwrap(), ["siege", "", "hold"]);

    let numbers = table(
        None,
        Some(2),
        &[vec![
            Cell::Optional(None),
            Cell::Text("key"),
            Cell::Optional(Some("m")),
            Cell::Optional(None),
            Cell::Bool(true),
            Cell::Colour(0x00ff_8000),
            Cell::Colour(1),
            Cell::Colour(2),
        ]],
    );
    let ritual = db::read_table(&numbers, "ritual_targets_tables").unwrap();
    assert_eq!(ritual.rows[0][4], Value::Bool(true));
    assert_eq!(ritual.rows[0][5].to_string(), "FF8000");
}

#[test]
fn unversioned_files_use_version_zero() {
    let rows = [vec![
        Cell::F64(0.25),
        Cell::Optional(None),
        Cell::Text("trigger"),
    ]];
    let parsed = db::read_table(
        &table(None, None, &rows),
        "audio_event_trigger_responses_tables",
    )
    .unwrap();
    assert_eq!(parsed.version, 0);
    assert_eq!(parsed.rows[0][0], Value::F64(0.25));
    assert_eq!(parsed.rows[0][0].to_string(), "0.250");
}

fn invalid(bytes: &[u8], table: &str) {
    match db::read_table(bytes, table) {
        Err(Error::Invalid(_)) => {}
        other => panic!("expected invalid data, got {other:?}"),
    }
}

#[test]
fn truncated_and_inconsistent_tables_are_rejected() {
    let bytes = table(Some("g"), Some(11), &[permission("f", "u", true)]);
    for len in 0..bytes.len() {
        invalid(&bytes[..len], PERMISSIONS);
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    invalid(&trailing, PERMISSIONS);

    // A row count far beyond the data is refused before rows are allocated.
    let mut huge = table(None, Some(11), &[]);
    let count = huge.len() - 4;
    huge[count..].copy_from_slice(&u32::MAX.to_le_bytes());
    invalid(&huge, PERMISSIONS);

    // A boolean other than 0/1 means the definition does not match the data.
    let mut flag = table(None, Some(11), &[permission("f", "u", true)]);
    let general = flag.len() - support::rows_bytes(&[permission("f", "u", true)]).len() + 3;
    flag[general] = 7;
    invalid(&flag, PERMISSIONS);

    // The byte before the row count must be 1, and a GUID may not run past the end.
    let mut marker = table(None, Some(11), &[]);
    marker[8] = 2;
    invalid(&marker, PERMISSIONS);
    invalid(&[0xfd, 0xfe, 0xfc, 0xff, 0xff, 0xff, 0x41, 0], PERMISSIONS);

    // Invalid UTF-8 in a StringU8.
    let text = table(None, Some(11), &[permission("\u{e9}", "u", false)]);
    let mut latin1 = text.clone();
    let position = text.iter().position(|&b| b == 0xc3).unwrap();
    latin1[position] = 0xff;
    invalid(&latin1, PERMISSIONS);

    invalid(&table(None, Some(999), &[]), PERMISSIONS);
    invalid(&table(None, Some(11), &[]), "not_a_table");
}

#[test]
fn mutated_tables_never_panic() {
    let bytes = table(Some("guid"), Some(3), &victory_rows());
    // A fixed linear congruential sequence keeps the test deterministic.
    let mut state = 0x2545_f491_u32;
    let mut next = || {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        state >> 8
    };
    for _ in 0..5_000 {
        let mut mutated = bytes.clone();
        for _ in 0..1 + next() % 4 {
            let position = next() as usize % mutated.len();
            mutated[position] = next() as u8;
        }
        let _ = db::read_table(&mutated, VICTORY);
        let _ = db::read_header(&mutated);
    }
}

#[test]
fn table_names_follow_the_live_db_folder() {
    assert_eq!(
        db::table_name("db\\Units_Tables\\mod").as_deref(),
        Some("units_tables")
    );
    assert_eq!(
        db::table_name("db/units_tables/sub/mod").as_deref(),
        Some("units_tables")
    );
    assert_eq!(db::table_name("db\\units_tables\\"), None);
    assert_eq!(db::table_name("unusedtables\\units_tables\\mod"), None);
}
