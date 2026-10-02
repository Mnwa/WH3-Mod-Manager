#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use common::{fixture, index_of};
use std::{
    fs,
    sync::atomic::AtomicBool,
    time::{Duration, SystemTime},
};
use wh3_core::{
    catalog::{Catalog, Mod, Source},
    scan, steam,
};

#[test]
fn scan_discovers_thumbnails_like_the_original_manager_and_records_mtime() {
    let temp = tempfile::tempdir().unwrap();
    let (data, workshop, custom) = (
        temp.path().join("data"),
        temp.path().join("workshop/1142710"),
        temp.path().join("custom"),
    );
    fixture(&data.join("own.pack"), &[], &[]);
    fs::write(data.join("own.JPG"), b"").unwrap();
    fs::write(data.join("own.png"), b"").unwrap();
    // A Data pack never takes an unrelated image from the shared data folder...
    fixture(&data.join("lonely.pack"), &[], &[]);
    fs::write(data.join("unrelated.png"), b"").unwrap();
    // ...but borrows the image of its Workshop original.
    fixture(&data.join("copied.pack"), &[], &[]);
    fixture(&workshop.join("42/copied.pack"), &[], &[]);
    fs::write(workshop.join("42/zz_preview.png"), b"").unwrap();
    fs::write(workshop.join("42/aa_preview.jpg"), b"").unwrap();
    fixture(&custom.join("named.pack"), &[], &[]);
    fs::write(custom.join("named.jpg"), b"").unwrap();
    fixture(&custom.join("nested/other.pack"), &[], &[]);
    fs::write(custom.join("nested/cover.jpg"), b"").unwrap();
    let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    fs::File::options()
        .write(true)
        .open(custom.join("named.pack"))
        .unwrap()
        .set_modified(stamp)
        .unwrap();

    let result = scan::scan(
        &[
            (data.clone(), Source::Data),
            (workshop.clone(), Source::Workshop),
            (custom.clone(), Source::Custom),
        ],
        &AtomicBool::new(false),
    )
    .unwrap();
    let catalog = &result.catalog;
    let thumbnail = |name: &str, source: Source| {
        catalog.by_name[name]
            .iter()
            .map(|&i| &catalog.mods[i])
            .find(|item| item.source == source)
            .unwrap()
            .thumbnail
            .clone()
    };
    assert_eq!(
        thumbnail("own.pack", Source::Data),
        Some(data.join("own.png"))
    );
    assert_eq!(thumbnail("lonely.pack", Source::Data), None);
    let preview = Some(workshop.join("42/zz_preview.png"));
    assert_eq!(thumbnail("copied.pack", Source::Workshop), preview);
    assert_eq!(thumbnail("copied.pack", Source::Data), preview);
    assert_eq!(
        thumbnail("named.pack", Source::Custom),
        Some(custom.join("named.jpg"))
    );
    assert_eq!(
        thumbnail("other.pack", Source::Custom),
        Some(custom.join("nested/cover.jpg"))
    );
    let named = &catalog.mods[index_of(catalog, "named.pack")];
    assert_eq!(named.modified, Some(stamp));
    assert!(catalog.mods.iter().all(|item| item.modified.is_some()));
}

#[test]
fn app_manifest_last_update_flags_older_mods_as_outdated() {
    let temp = tempfile::tempdir().unwrap();
    let steamapps = temp.path().join("steamapps");
    let game = steamapps.join("common/Total War WARHAMMER III");
    fs::create_dir_all(&game).unwrap();
    assert_eq!(steam::game_updated(&game), None);
    fs::write(
        steamapps.join("appmanifest_1142710.acf"),
        "\"AppState\"\n{\n\t\"appid\"\t\t\"1142710\"\n\t\"LastUpdated\"\t\t\"1700000000\"\n}\n",
    )
    .unwrap();
    let updated = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    assert_eq!(steam::game_updated(&game), Some(updated));
    for broken in [
        "\"AppState\" { \"LastUpdated\" \"soon\" }",
        "\"AppState\" { \"LastUpdated\" \"0\" }",
        "\"AppState\" { }",
        "\"LastUpdated\" \"99999999999999999999\"",
    ] {
        assert_eq!(steam::manifest_updated(broken), None, "{broken}");
    }

    let at = |offset: i64| {
        let delta = Duration::from_secs(offset.unsigned_abs());
        if offset < 0 {
            updated - delta
        } else {
            updated + delta
        }
    };
    let item = |name: &str, source, modified| {
        Mod::new(
            name.into(),
            name.into(),
            "".into(),
            source,
            0,
            false,
            vec![],
        )
        .with_modified(modified)
    };
    let catalog = Catalog::new(vec![
        item("old.pack", Source::Workshop, Some(at(-60))),
        item("new.pack", Source::Workshop, Some(at(60))),
        item("same.pack", Source::Custom, Some(updated)),
        item("data.pack", Source::Data, Some(at(-1))),
        item("unknown.pack", Source::Custom, None),
    ]);
    assert_eq!(catalog.outdated(updated), [true, false, false, true, false]);
}

#[test]
fn regex_search_is_case_insensitive_and_invalid_patterns_fall_back_to_substring() {
    let catalog = Catalog::demo(20);
    let order: Vec<_> = (0..catalog.mods.len()).collect();
    assert_eq!(catalog.query(r"/^MOD_00001[23]\.pack$/", &order), [12, 13]);
    assert_eq!(catalog.query("/кислев.*00001[0-9]/", &order), [13, 18]);
    assert_eq!(catalog.query("  /mod_00000[01]/ ", &[1, 0]), [1, 0]);

    let odd = Mod::new(
        "odd.pack".into(),
        "Bad [Regex] title".into(),
        "".into(),
        Source::Custom,
        0,
        false,
        vec![],
    );
    let catalog = Catalog::new(vec![odd, catalog.mods[0].clone()]);
    assert_eq!(catalog.query("/bad [regex/", &[0, 1]), [0]);
    assert_eq!(catalog.query(r"/^bad \[regex\] title$/", &[0, 1]), [0]);
    assert_eq!(catalog.query("regex bad", &[0, 1]), [0]);
}
