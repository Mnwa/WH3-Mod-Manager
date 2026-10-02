#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::{collections::HashSet, fs, sync::atomic::AtomicBool};
use wh3_core::{
    catalog::{Catalog, Mod, Source},
    conflict, launch, pack,
    preset::Preset,
    scan, steam, storage,
};

fn fixture(path: &std::path::Path, kind: u32, files: &[(&str, &[u8])], hashed: bool) {
    let mut index = Vec::new();
    let mut data = Vec::new();
    for (name, content) in files {
        index.extend_from_slice(&(content.len() as u32).to_le_bytes());
        if hashed {
            index.extend_from_slice(&123u32.to_le_bytes());
        }
        index.push(0);
        index.extend_from_slice(name.as_bytes());
        index.push(0);
        data.extend_from_slice(content);
    }
    let mut bytes = b"PFH5".to_vec();
    for word in [
        kind | if hashed { 0x40 } else { 0 },
        0,
        0,
        files.len() as u32,
        index.len() as u32,
        0,
    ] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend(index);
    bytes.extend(data);
    fs::write(path, bytes).unwrap();
}

#[test]
fn pfh5_index_handles_hashed_names_and_payload_offsets() {
    let temp = tempfile::tempdir().unwrap();
    for hashed in [false, true] {
        let path = temp.path().join("sample.pack");
        fixture(
            &path,
            3,
            &[("db\\units\\a", b"123"), ("script\\a.lua", b"abcde")],
            hashed,
        );
        let files = pack::index(&path).unwrap();
        assert_eq!(
            files.iter().map(|f| (&*f.name, f.size)).collect::<Vec<_>>(),
            [("db\\units\\a", 3), ("script\\a.lua", 5)]
        );
        assert_eq!(files[1].offset, files[0].offset + 3);
    }
}

#[test]
fn malformed_pack_never_reads_outside_file_or_allocates_declared_huge_index() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("bad.pack");
    fixture(&path, 3, &[("x", b"payload")], false);
    let original = fs::read(&path).unwrap();
    for length in 0..original.len() {
        fs::write(&path, &original[..length]).unwrap();
        assert!(
            pack::index(&path).is_err(),
            "accepted truncation at {length}"
        );
    }
    let mut huge = original;
    huge[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
    fs::write(&path, huge).unwrap();
    assert!(pack::header(&path).is_err());
}

#[test]
fn scan_skips_vanilla_packs_deduplicates_roots_and_reports_bad_packs() {
    let temp = tempfile::tempdir().unwrap();
    fixture(&temp.path().join("data.pack"), 0, &[], false);
    fixture(&temp.path().join("mod.pack"), 3, &[], false);
    fixture(&temp.path().join("movie.pack"), 4, &[], false);
    fs::write(temp.path().join("bad.pack"), b"broken").unwrap();
    let root = (temp.path().to_owned(), Source::Data);
    let result = scan::scan(&[root.clone(), root], &AtomicBool::new(false)).unwrap();
    assert_eq!(result.catalog.mods.len(), 2);
    assert_eq!(result.warnings.len(), 1);
    assert!(result.catalog.mods.iter().any(|item| item.movie));
}

#[test]
fn cancellation_stops_scan_before_work() {
    let result = scan::scan(&[("unused".into(), Source::Custom)], &AtomicBool::new(true));
    assert!(matches!(result, Err(wh3_core::Error::Cancelled)));
}

#[test]
fn old_full_and_new_compact_presets_import_with_missing_and_disabled_mods() {
    let catalog = Catalog::demo(4);
    let old = br#"{"name":"legacy","mods":[{"name":"mod_000002.pack","isEnabled":true,"path":"C:/old","loadOrder":0},{"name":"mod_000000.pack","isEnabled":false},{"name":"missing.pack"}]}"#;
    let preset = Preset::parse(old).unwrap().remove(0);
    let applied = preset.apply(&catalog);
    assert_eq!(applied.order, [2, 0, 1, 3]);
    assert_eq!(applied.enabled, HashSet::from([2]));
    assert_eq!(applied.missing, ["missing.pack"]);
    let compact = br#"{"presets":[{"name":"new","mods":[{"name":"mod_000001.pack"}]}]}"#;
    let preset = Preset::parse(compact).unwrap().remove(0);
    assert_eq!(preset.apply(&catalog).enabled, HashSet::from([1]));
}

#[test]
fn preset_roundtrip_preserves_order_and_disabled_state() {
    let catalog = Catalog::demo(10);
    let order: Vec<_> = (0..10).rev().collect();
    let enabled = HashSet::from([1, 3, 8]);
    let original = Preset::capture("test".into(), &catalog, &order, &enabled);
    let encoded = serde_json::to_vec(&original).unwrap();
    let restored = Preset::parse(&encoded).unwrap().remove(0).apply(&catalog);
    assert_eq!((restored.order, restored.enabled), (order, enabled));
}

#[test]
fn search_is_unicode_case_insensitive_and_requires_every_term() {
    let catalog = Catalog::demo(100_000);
    let order: Vec<_> = (0..catalog.mods.len()).rev().collect();
    let found = catalog.query("КИСЛЕВ mod_000003.pack", &order);
    assert_eq!(found, [3]);
    assert_eq!(catalog.query("", &order), order);
    assert!(catalog.query("no-such-mod", &order).is_empty());
}

#[test]
fn conflicts_normalize_paths_and_aggregate_multiple_owners() {
    let temp = tempfile::tempdir().unwrap();
    for (name, internal) in [
        ("a.pack", "db/Units/A"),
        ("b.pack", "db\\units\\a"),
        ("c.pack", "db\\units\\a"),
    ] {
        fixture(&temp.path().join(name), 3, &[(internal, b"one")], false);
    }
    let scan = scan::scan(
        &[(temp.path().into(), Source::Custom)],
        &AtomicBool::new(false),
    )
    .unwrap();
    let report = conflict::check(
        &scan.catalog,
        &[2, 0, 1],
        &HashSet::from([0, 1, 2]),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].path, "db\\units\\a");
    assert_eq!(report.files[0].owners, [2, 0, 1]);
}

#[test]
fn launch_script_preserves_order_and_rejects_ambiguous_names_and_injection() {
    let catalog = Catalog::demo(3);
    let script = launch::script(
        &catalog,
        &[2, 0, 1],
        &HashSet::from([0, 2]),
        &launch::Options::default(),
        std::path::Path::new("temp"),
    )
    .unwrap();
    assert!(script.ends_with("mod \"mod_000002.pack\";\nmod \"mod_000000.pack\";"));
    let mut mods = catalog.mods.clone();
    mods.push(mods[0].clone());
    assert!(
        launch::script(
            &Catalog::new(mods),
            &[0, 3],
            &HashSet::from([0, 3]),
            &launch::Options::default(),
            std::path::Path::new("temp"),
        )
        .is_err()
    );
    let injected = Mod::new(
        "bad\";quit.pack".into(),
        "bad".into(),
        "".into(),
        Source::Custom,
        0,
        false,
        vec![],
    );
    assert!(
        launch::script(
            &Catalog::new(vec![injected]),
            &[0],
            &HashSet::from([0]),
            &launch::Options::default(),
            std::path::Path::new("temp"),
        )
        .is_err()
    );
}

#[test]
fn launch_does_not_overwrite_original_manager_script() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("Warhammer3.exe"), []).unwrap();
    fs::write(temp.path().join("used_mods.txt"), b"original").unwrap();
    fixture(&temp.path().join("a.pack"), 3, &[], false);
    let scan = scan::scan(
        &[(temp.path().into(), Source::Custom)],
        &AtomicBool::new(false),
    )
    .unwrap();
    launch::prepare(
        temp.path(),
        &scan.catalog,
        &[0],
        &HashSet::from([0]),
        &launch::Options::default(),
        &temp.path().join("temp_packs"),
    )
    .unwrap();
    assert_eq!(
        fs::read(temp.path().join("used_mods.txt")).unwrap(),
        b"original"
    );
    assert!(temp.path().join(launch::MOD_LIST).is_file());
}

#[test]
fn storage_replaces_existing_file_and_reports_corruption() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config/settings.json");
    storage::save(&path, &storage::Settings::default()).unwrap();
    let settings = storage::Settings {
        game_path: Some("new".into()),
        ..Default::default()
    };
    storage::save(&path, &settings).unwrap();
    assert_eq!(storage::load(&path).unwrap().game_path, settings.game_path);
    fs::write(&path, b"invalid").unwrap();
    assert!(storage::load(&path).is_err());
}

#[test]
fn steam_vdf_supports_escaped_windows_paths_comments_and_multiple_libraries() {
    let input = r#""libraryfolders" { // comment "path" "wrong"
        "0" { "path" "C:\\Program Files (x86)\\Steam" }
        "1" { "path" "D:\\SteamLibrary" }
    }"#;
    assert_eq!(
        steam::quoted_values(input, "path"),
        [r"C:\Program Files (x86)\Steam", r"D:\SteamLibrary"]
    );
}

#[test]
fn update_is_offered_only_for_newer_releases_with_executable_and_checksums() {
    use wh3_core::update::{EXE_ASSET, SUMS_ASSET, newer};
    let release = |version: &str, assets: &[&str]| {
        self_update::Release::builder()
            .version(version)
            .name(version)
            .assets(
                assets
                    .iter()
                    .map(|name| self_update::ReleaseAsset::new(*name, "https://example.invalid")),
            )
            .build()
            .unwrap()
    };
    let complete = [EXE_ASSET, SUMS_ASSET];
    assert_eq!(
        newer("0.2.0", &release("0.3.0", &complete)).map(|a| a.version),
        Some("0.3.0".into())
    );
    assert!(newer("0.2.0", &release("0.2.0", &complete)).is_none());
    assert!(newer("0.3.0", &release("0.2.9", &complete)).is_none());
    // release.yml uploads assets after publication; a bare release is not installable yet.
    assert!(newer("0.2.0", &release("0.3.0", &[SUMS_ASSET])).is_none());
    assert!(newer("0.2.0", &release("0.3.0", &[EXE_ASSET])).is_none());
}

#[test]
fn original_v2_preset_orders_enabled_mods_by_name_and_pins_like_the_original() {
    // The original launches `sortByNameAndLoadOrder(enabled)`: code-unit name order
    // with pinned mods spliced in at their index; list order does not matter.
    let preset = Preset::parse(
        br#"{"name":"current","version":2,"mods":[
            {"name":"b.pack","loadOrder":0},
            {"name":"a.pack"},
            {"name":"off.pack","isEnabled":false,"loadOrder":0},
            {"name":"@c.pack"},
            {"name":"C.pack","loadOrder":3}
        ]}"#,
    )
    .unwrap()
    .remove(0);
    let names = ["a.pack", "b.pack", "@c.pack", "C.pack", "off.pack"];
    let catalog = Catalog::new(
        names
            .iter()
            .map(|n| {
                Mod::new(
                    format!("x/{n}").into(),
                    String::new(),
                    String::new(),
                    Source::Workshop,
                    0,
                    false,
                    vec![],
                )
            })
            .collect(),
    );
    let applied = preset.apply(&catalog);
    let launched: Vec<_> = applied
        .order
        .iter()
        .filter(|i| applied.enabled.contains(i))
        .map(|&i| &*catalog.mods[i].name)
        .collect();
    assert_eq!(launched, ["b.pack", "@c.pack", "a.pack", "C.pack"]);
}

#[test]
fn compare_mod_names_matches_the_original_including_prefixes() {
    use std::cmp::Ordering::*;
    use wh3_core::preset::compare_mod_names;
    assert_eq!(compare_mod_names("!a", "@a"), Less);
    assert_eq!(compare_mod_names("Z", "a"), Less);
    // A prefix sorts after the longer name in the original.
    assert_eq!(compare_mod_names("mod", "mod_extra"), Greater);
    assert_eq!(compare_mod_names("same", "same"), Equal);
}

#[test]
fn presets_written_by_this_manager_keep_list_order_even_with_pins() {
    let preset = Preset::parse(
        br#"{"name":"mine","version":3,"mods":[{"name":"z.pack","loadOrder":0},{"name":"a.pack"}]}"#,
    )
    .unwrap()
    .remove(0);
    let catalog = Catalog::new(
        ["a.pack", "z.pack"]
            .iter()
            .map(|n| {
                Mod::new(
                    format!("x/{n}").into(),
                    String::new(),
                    String::new(),
                    Source::Workshop,
                    0,
                    false,
                    vec![],
                )
            })
            .collect(),
    );
    assert_eq!(preset.apply(&catalog).order, [1, 0]);
}
