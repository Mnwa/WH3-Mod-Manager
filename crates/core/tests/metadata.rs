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
    assert!(
        bundle
            .warnings
            .iter()
            .any(|warning| warning.contains("правила"))
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
    assert!(original.starts_with(b"WHM1"));
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
fn binary_schema_matches_frozen_v1_fixture() {
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
    let new_path = temp.path().join("roundtrip.whmm");
    storage::save(&new_path, &settings).unwrap();
    assert_eq!(fs::read(new_path).unwrap(), fixture);
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
