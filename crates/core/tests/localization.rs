#![allow(clippy::unwrap_used, clippy::expect_used)]
use wh3_core::{Error, localization::Language, metadata, preferences, preset::Preset};

#[test]
fn language_survives_restart_without_touching_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.whmp");
    let library = dir.path().join("library.whmm");
    std::fs::write(&library, b"unrelated library").unwrap();
    assert_eq!(preferences::load(&path).unwrap(), Language::English);
    for language in [Language::Russian, Language::English] {
        preferences::save(&path, language).unwrap();
        assert_eq!(preferences::load(&path).unwrap(), language);
        assert_eq!(std::fs::read(&library).unwrap(), b"unrelated library");
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 5);
    }
}

#[test]
fn invalid_preferences_are_reported_in_both_languages() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.whmp");
    for bytes in [b"WHP1\x02".as_slice(), b"WHP2\x00", b"WHP1", b"WHP1\x00x"] {
        std::fs::write(&path, bytes).unwrap();
        let message = preferences::load(&path).unwrap_err().message();
        assert!(message.text(Language::English).contains("Invalid language"));
        assert!(message.text(Language::Russian).contains("настройки языка"));
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
        br#"{"currentPreset":{"name":"x","mods":[]},"loadOrderRules":[{}]}"#,
    )
    .unwrap();
    let notices: Vec<_> = bundle
        .warnings
        .into_iter()
        .map(metadata::warning_message)
        .collect();
    assert!(
        notices
            .iter()
            .any(|m| m.text(Language::English).contains("rules"))
    );
    assert!(
        notices
            .iter()
            .any(|m| m.text(Language::Russian).contains("правила"))
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
