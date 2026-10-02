#![allow(clippy::unwrap_used, clippy::expect_used)]
use wh3_core::{
    Error,
    localization::Language,
    metadata,
    preferences::{self, Column, Density, Layout, Preferences},
    preset::Preset,
};

#[test]
fn preferences_survive_restart_without_touching_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.whmp");
    let library = dir.path().join("library.whmm");
    std::fs::write(&library, b"unrelated library").unwrap();
    assert_eq!(preferences::load(&path).unwrap(), Preferences::default());
    let mut prefs = Preferences::default();
    prefs.language = Some(Language::Russian);
    prefs.layout = Layout::Dual;
    prefs.density = Density::Roomy;
    prefs.group_by_category = true;
    prefs.set_width(Column::Author, 9999.);
    prefs.set_width(Column::Size, 1.);
    preferences::save(&path, &prefs).unwrap();
    let loaded = preferences::load(&path).unwrap();
    assert_eq!(loaded, prefs);
    assert_eq!(loaded.width(Column::Author), Column::MAX_WIDTH);
    assert_eq!(loaded.width(Column::Size), Column::Size.min_width());
    assert_eq!(std::fs::read(&library).unwrap(), b"unrelated library");
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 16);
}

#[test]
fn whp1_language_files_migrate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.whmp");
    for (bytes, language) in [
        (b"WHP1\x00", Language::English),
        (b"WHP1\x01", Language::Russian),
    ] {
        std::fs::write(&path, bytes).unwrap();
        let loaded = preferences::load(&path).unwrap();
        assert_eq!(loaded.language, Some(language));
        assert_eq!(loaded.layout, Layout::Single);
        assert!(loaded.widths_are_default());
    }
}

#[test]
fn first_start_follows_the_system_language() {
    for tag in ["ru", "ru-RU", "ru_BY", "RU-kz"] {
        assert_eq!(Language::from_locale(tag), Language::Russian, "{tag}");
    }
    for tag in ["en-US", "uk-UA", "de", "rus", ""] {
        assert_eq!(Language::from_locale(tag), Language::English, "{tag}");
    }
}

#[test]
fn invalid_preferences_are_reported_in_both_languages() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.whmp");
    let mut valid = Preferences::default().encode();
    valid[5] = 7;
    for bytes in [
        b"WHP1\x02".as_slice(),
        b"WHP2\x00",
        b"WHP1",
        b"WHP1\x00x",
        &valid,
        &Preferences::default().encode()[..15],
    ] {
        std::fs::write(&path, bytes).unwrap();
        let message = preferences::load(&path).unwrap_err().message();
        assert!(
            message
                .text(Language::English)
                .contains("Invalid interface preferences")
        );
        assert!(
            message
                .text(Language::Russian)
                .contains("настройки интерфейса")
        );
    }
}

#[test]
fn existing_results_can_change_language_without_repeating_work() {
    let error = Preset::parse(br#"{"name":"","mods":[]}"#).err().unwrap();
    let message = error.message();
    assert!(
        message
            .text(Language::English)
            .contains("Preset name is missing")
    );
    assert!(message.text(Language::Russian).contains("отсутствует имя"));
    let packed = Error::Pack(wh3_core::message!("truncated name", "обрезанное имя")).message();
    assert_eq!(
        packed.text(Language::English),
        "Invalid pack: truncated name"
    );
    assert_eq!(
        packed.text(Language::Russian),
        "Некорректный pack: обрезанное имя"
    );
    let bundle = metadata::Bundle::parse(
        br#"{"currentPreset":{"name":"x","mods":[]},"loadOrderRules":[{"before":"a.pack","after":"b.pack","subjectPackName":"b.pack"},{"before":"c.pack","after":"d.pack","sourcePackName":"c.pack"}],"disabledModLoadOrderRules":["k"],"loadOrderRuleDisabledPacks":["e.pack"],"alwaysEnabledModNames":["always.pack"],"hiddenModNames":["hidden.pack"],"isSkipIntroMoviesEnabled":true,"isMakeUnitsGeneralsEnabled":true}"#,
    )
    .unwrap();
    // User rules are imported and applied; pack-supplied ones come from the packs.
    assert_eq!(
        bundle.user_rules(),
        [wh3_core::load_order::Rule::user(
            "a.pack", "b.pack", "b.pack"
        )]
    );
    assert_eq!(bundle.disabled_load_order_rules, ["k"]);
    assert_eq!(bundle.load_order_rule_disabled_packs, ["e.pack"]);
    assert_eq!(bundle.always_enabled, ["always.pack"]);
    assert_eq!(bundle.hidden, ["hidden.pack"]);
    let options = bundle.options.expect("start options are imported");
    assert!(options.skip_intro_movies && options.make_units_generals && !options.script_logging);
    let notices: Vec<_> = bundle
        .warnings
        .into_iter()
        .map(metadata::warning_message)
        .collect();
    assert!(
        notices
            .iter()
            .any(|m| m.text(Language::English).contains("Workshop fields"))
    );
    assert!(
        notices
            .iter()
            .any(|m| m.text(Language::Russian).contains("поля Workshop"))
    );
}

#[test]
fn cli_exports_identical_metadata_in_both_languages_without_changing_source() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("config.json");
    let original = br#"{"currentPreset":{"name":"my preset","mods":[]}}"#;
    std::fs::write(&source, original).unwrap();
    let mut results = Vec::new();
    for (language, heading) in [("en", "Exported:"), ("ru", "Экспортировано:")] {
        let destination = dir.path().join(format!("{language}.json"));
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_wh3-meta"))
            .arg(format!("--lang={language}"))
            .arg("export")
            .arg(&source)
            .arg(&destination)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .starts_with(heading)
        );
        results.push(std::fs::read(destination).unwrap());
    }
    assert_eq!(results[0], results[1]);
    assert_eq!(std::fs::read(source).unwrap(), original);
}
