#![allow(clippy::unwrap_used)]
use std::{collections::BTreeMap, fs};
use wh3_core::{
    catalog::Catalog,
    metadata::{self, Metadata},
    storage::{self, Settings},
};

#[test]
fn original_v3_export_is_portable_and_preserves_metadata_and_presets() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("config.json");
    let destination = temp.path().join("meta.json");
    let json = br#"{"games":{"wh3":{"currentPreset":{"name":"current","version":2,"mods":[{"name":"mod_000000.pack"}]},"presets":[],"modUserData":{"mod_000000.pack":{"humanName":"A real title","author":"Author","categories":["Campaign"],"tags":["Balance"],"reqModIdToName":[["123","Dependency"]]}},"loadOrderRules":[{"before":"a","after":"b"}]}}}"#;
    fs::write(&source, json).unwrap();
    let bundle = metadata::export(&source, &destination).unwrap();
    assert_eq!(fs::read(&source).unwrap(), json);
    assert!(metadata::export(&source, &source).is_err());
    let mut catalog = Catalog::demo(2);
    assert_eq!(metadata::apply(&mut catalog, &bundle.mods), 1);
    assert_eq!(&*catalog.mods[0].title, "A real title");
    assert_eq!(catalog.query("author balance", &[0, 1]), [0]);
    assert_eq!(
        metadata::read(&destination).unwrap().mods["mod_000000.pack"].req_mod_id_to_name,
        [("123".into(), "Dependency".into())]
    );
    // The original's user rules survive the export and become applied rules.
    assert_eq!(
        metadata::read(&destination).unwrap().user_rules(),
        [wh3_core::load_order::Rule::user("a", "b", "b")]
    );
}

#[test]
fn legacy_per_game_metadata_uses_current_title_and_keeps_snapshot_fields() {
    let json = br#"{"gameToCurrentPreset":{"wh3":{"name":"current","mods":[{"name":"a.pack","humanName":"Fresh"}]}},"gameToPresets":{"wh3":[{"name":"old","mods":[{"name":"a.pack","humanName":"Old","author":"Author","workshopId":"123","tags":["Units"]}]}]}}"#;
    let bundle = metadata::Bundle::parse(json).unwrap();
    assert_eq!(bundle.mods["a.pack"].human_name, "Fresh");
    assert_eq!(bundle.mods["a.pack"].workshop_id, "123");
    assert_eq!(bundle.mods["a.pack"].tags, ["Units"]);
}

#[test]
fn current_metadata_can_explicitly_clear_legacy_categories() {
    let json = br#"{"currentPreset":{"name":"current","mods":[{"name":"a.pack","categories":[]}]},"presets":[{"name":"old","mods":[{"name":"a.pack","categories":["old"]}]}]}"#;
    let bundle = metadata::Bundle::parse(json).unwrap();
    assert!(bundle.mods["a.pack"].categories.is_empty());
}

#[test]
fn binary_library_roundtrips_metadata_and_keeps_previous_generation() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("library.whmm");
    let settings = Settings {
        game_path: Some("C:/Игры/WH3".into()),
        metadata: BTreeMap::from([(
            "a.pack".into(),
            Metadata {
                human_name: "Кислев".into(),
                categories: vec!["Кампания".into()],
                ..Default::default()
            },
        )]),
        ..Default::default()
    };
    storage::save(&path, &settings).unwrap();
    let original = fs::read(&path).unwrap();
    assert!(original.starts_with(b"WHM4"));
    let decoded = storage::load(&path).unwrap();
    assert_eq!(decoded.metadata["a.pack"].human_name, "Кислев");
    assert_eq!(decoded.game_path, settings.game_path);
    storage::save(&path, &Settings::default()).unwrap();
    assert_eq!(fs::read(path.with_extension("whmm.bak")).unwrap(), original);
}

#[test]
fn binary_library_rejects_bitflips_truncation_and_future_version_without_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("library.whmm");
    storage::save(&path, &Settings::default()).unwrap();
    let original = fs::read(&path).unwrap();
    for index in 0..original.len() {
        let mut bytes = original.clone();
        bytes[index] ^= 1;
        fs::write(&path, &bytes).unwrap();
        assert!(storage::load(&path).is_err(), "accepted bitflip {index}");
        assert!(storage::save(&path, &Settings::default()).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    for length in 0..original.len() {
        fs::write(&path, &original[..length]).unwrap();
        assert!(storage::load(&path).is_err());
    }
}

#[test]
fn frozen_v1_fixture_migrates_to_current_without_losing_state() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("library.whmm");
    let fixture = include_bytes!("fixtures/state-v1.whmm");
    fs::write(&path, fixture).unwrap();
    let settings = storage::load(&path).unwrap();
    assert_eq!(
        settings.current.as_ref().unwrap().mods[0].load_order,
        Some(3)
    );
    assert_eq!(settings.game_path, Some("C:/WH3".into()));
    assert!(settings.hidden.is_empty() && settings.always_enabled.is_empty());
    assert_eq!(settings.options, storage::GameOptions::default());
    // Saving migrates in place and keeps the WHM1 generation as the backup.
    storage::save(&path, &settings).unwrap();
    assert!(fs::read(&path).unwrap().starts_with(b"WHM4"));
    assert_eq!(fs::read(path.with_extension("whmm.bak")).unwrap(), fixture);
    let migrated = storage::load(&path).unwrap();
    assert_eq!(migrated.game_path, settings.game_path);
    assert_eq!(migrated.roots.len(), settings.roots.len());
    assert_eq!(migrated.metadata.len(), settings.metadata.len());
}

fn v2_state() -> storage::Settings {
    let fixture = include_bytes!("fixtures/state-v1.whmm");
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("library.whmm");
    fs::write(&path, fixture).unwrap();
    let mut settings = storage::load(&path).unwrap();
    settings.hidden.insert("hidden.pack".into());
    settings.always_enabled.insert("always.pack".into());
    settings.options = storage::GameOptions {
        skip_intro_movies: true,
        script_logging: false,
        auto_start_custom_battle: true,
        close_on_play: true,
        make_units_generals: false,
        ..Default::default()
    };
    settings
}

#[test]
fn frozen_v2_fixture_migrates_to_current_without_losing_state() {
    let temp = tempfile::tempdir().unwrap();
    let fixture = include_bytes!("fixtures/state-v2.whmm");
    let path = temp.path().join("library.whmm");
    fs::write(&path, fixture).unwrap();
    let settings = storage::load(&path).unwrap();
    let expected = v2_state();
    assert_eq!(settings.hidden, expected.hidden);
    assert_eq!(settings.always_enabled, expected.always_enabled);
    assert_eq!(settings.options, expected.options);
    assert!(settings.rules.is_empty() && settings.disabled_rules.is_empty());
    storage::save(&path, &settings).unwrap();
    assert!(fs::read(&path).unwrap().starts_with(b"WHM4"));
    assert_eq!(fs::read(path.with_extension("whmm.bak")).unwrap(), fixture);
    assert_eq!(storage::load(&path).unwrap().hidden, expected.hidden);
}

fn v3_state() -> storage::Settings {
    let mut settings = v2_state();
    settings.rules = vec![wh3_core::load_order::Rule::user(
        "a.pack", "b.pack", "b.pack",
    )];
    settings
        .disabled_rules
        .insert("c.pack\tc.pack\td.pack".into());
    settings.disabled_rule_packs.insert("e.pack".into());
    settings.options.make_units_generals = true;
    settings
}

#[test]
fn frozen_v3_fixture_migrates_to_current_without_losing_state() {
    let temp = tempfile::tempdir().unwrap();
    let fixture = include_bytes!("fixtures/state-v3.whmm");
    let path = temp.path().join("library.whmm");
    fs::write(&path, fixture).unwrap();
    let settings = storage::load(&path).unwrap();
    let expected = v3_state();
    assert_eq!(settings.rules, expected.rules);
    assert_eq!(settings.disabled_rules, expected.disabled_rules);
    assert_eq!(settings.disabled_rule_packs, expected.disabled_rule_packs);
    assert_eq!(settings.hidden, expected.hidden);
    assert_eq!(settings.options, expected.options);
    storage::save(&path, &settings).unwrap();
    assert!(fs::read(&path).unwrap().starts_with(b"WHM4"));
    assert_eq!(fs::read(path.with_extension("whmm.bak")).unwrap(), fixture);
}

fn v4_state() -> storage::Settings {
    let mut settings = v3_state();
    settings.options.raise_priority = true;
    settings.options.clean_up_staging = true;
    settings.options.staging = storage::Staging::Symlink;
    settings
}

#[test]
fn binary_schema_matches_frozen_v4_fixture() {
    let temp = tempfile::tempdir().unwrap();
    let fixture = include_bytes!("fixtures/state-v4.whmm");
    let path = temp.path().join("library.whmm");
    fs::write(&path, fixture).unwrap();
    let settings = storage::load(&path).unwrap();
    let expected = v4_state();
    assert_eq!(settings.options, expected.options);
    assert_eq!(settings.rules, expected.rules);
    let new_path = temp.path().join("roundtrip.whmm");
    storage::save(&new_path, &expected).unwrap();
    assert_eq!(fs::read(new_path).unwrap(), fixture);
}

#[test]
fn legacy_records_reject_option_bits_they_never_had() {
    let mut bytes = include_bytes!("fixtures/state-v4.whmm").to_vec();
    bytes[..4].copy_from_slice(b"WHM3");
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("library.whmm");
    fs::write(&path, &bytes).unwrap();
    // A WHM4 payload relabelled as WHM3 fails validation instead of losing the new options.
    assert!(storage::load(&path).is_err());
}

#[test]
fn v2_rejects_unknown_option_bits() {
    assert!(storage::GameOptions::from_bits(1 << 31).is_none());
    let options = v2_state().options;
    assert_eq!(
        storage::GameOptions::from_bits(options.bits()),
        Some(options)
    );
}

#[test]
fn bytecheck_rejects_invalid_archive_even_with_valid_checksum() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("library.whmm");
    let mut bytes = include_bytes!("fixtures/state-v1.whmm").to_vec();
    let length = bytes.len();
    bytes[length - 8..].fill(255);
    let checksum = crc32fast::hash(&bytes[12..]);
    bytes[8..12].copy_from_slice(&checksum.to_le_bytes());
    fs::write(&path, bytes).unwrap();
    assert!(storage::load(&path).is_err());
}
