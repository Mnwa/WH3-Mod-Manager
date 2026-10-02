#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::{
    collections::HashSet,
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
use wh3_core::{
    catalog::{Catalog, Mod, Source},
    launch::{self, Options},
    localization::Language,
    pack,
};

const ALL: Options = Options {
    skip_intro_movies: true,
    script_logging: true,
    auto_start_custom_battle: true,
    // Building the generals table needs a real game install; see real_data.rs.
    make_units_generals: false,
};

fn user_mod(path: PathBuf) -> Mod {
    Mod::new(
        path,
        "title".into(),
        "".into(),
        Source::Custom,
        0,
        false,
        vec![],
    )
}

fn contents(path: &Path) -> Vec<(String, Vec<u8>)> {
    let mut file = fs::File::open(path).unwrap();
    pack::index(path)
        .unwrap()
        .into_iter()
        .map(|entry| {
            assert!(!entry.compressed);
            let mut bytes = vec![0; entry.size as usize];
            file.seek(SeekFrom::Start(entry.offset)).unwrap();
            file.read_exact(&mut bytes).unwrap();
            (entry.name, bytes)
        })
        .collect()
}

#[test]
fn written_pack_round_trips_through_reader_with_upstream_header() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("out.pack");
    let files: [(&str, &[u8]); 3] = [
        ("db\\units\\a", b"123"),
        ("empty", b""),
        ("script\\a.lua", b"abcde"),
    ];
    pack::write(&path, &files).unwrap();

    let header = pack::header(&path).unwrap();
    assert_eq!((header.version, header.flags), (5, 3));
    assert!(header.dependencies.is_empty());
    assert_eq!(header.file_count, 3);
    let raw = fs::read(&path).unwrap();
    assert_eq!(&raw[24..28], &0x7fff_ffffu32.to_le_bytes());
    assert_eq!(
        contents(&path),
        files
            .iter()
            .map(|(name, bytes)| (name.to_string(), bytes.to_vec()))
            .collect::<Vec<_>>()
    );
    assert_eq!(raw.len() as u64, header.pack_size);
}

#[test]
fn pack_writer_rejects_ambiguous_or_unterminable_names() {
    assert!(pack::encode(&[("a\\b", b"1"), ("A\\B", b"2")]).is_err());
    assert!(pack::encode(&[("a\0b", b"1")]).is_err());
    assert!(pack::encode(&[("", b"1")]).is_err());
    assert_eq!(pack::encode(&[]).unwrap().len(), 28);
}

#[test]
fn start_pack_files_match_the_original_manager() {
    assert!(launch::start_pack_files(&Options::default()).is_empty());
    let files = launch::start_pack_files(&ALL);
    let names: Vec<_> = files.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        names,
        [
            "movies\\epilepsy_warning\\epilepsy_warning_en.ca_vp8",
            "movies\\gam_int.ca_vp8",
            "movies\\startup_movie_01.ca_vp8",
            "movies\\startup_movie_02.ca_vp8",
            "movies\\startup_movie_03.ca_vp8",
            "movies\\startup_movie_04.ca_vp8",
            "script\\enable_console_logging",
            "script\\frontend\\mod\\pj_auto_custom_battles.lua",
        ]
    );
    // The game expects an ascending index; the upstream order already is.
    assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
    for (_, movie) in &files[..6] {
        assert_eq!(movie.len(), 595);
        assert!(movie.starts_with(b"CAMV"));
        assert!(movie.ends_with(&[0x21, 0x02, 0x00, 0x00, 0x01]));
    }
    assert_eq!(files[6].1, [0]);
    let lua = std::str::from_utf8(&files[7].1).unwrap();
    assert_eq!(lua.len(), 1218);
    assert!(lua.starts_with(
        "core:remove_listener(\"pj_auto_custom_battles_FrontendScreenTransition\")\n"
    ));
    assert!(lua.ends_with("\t\trun_it()\n\tend\n)\n"));

    let only_logging = Options {
        script_logging: true,
        ..Options::default()
    };
    assert_eq!(launch::start_pack_files(&only_logging).len(), 1);
}

#[test]
fn script_appends_generated_pack_last_and_only_when_needed() {
    let catalog = Catalog::demo(3);
    let enabled = HashSet::from([0, 2]);
    let dir = Path::new("/config/temp_packs");
    let script = launch::script(&catalog, &[2, 0, 1], &enabled, &ALL, dir).unwrap();
    let lines: Vec<_> = script.lines().collect();
    assert_eq!(
        lines,
        [
            "add_working_directory \"demo\";",
            "mod \"mod_000002.pack\";",
            "mod \"mod_000000.pack\";",
            "add_working_directory \"/config/temp_packs\";",
            "mod \"!!!!out.pack\";",
        ]
    );
    let plain = launch::script(&catalog, &[2, 0, 1], &enabled, &Options::default(), dir).unwrap();
    assert!(!plain.contains(launch::TEMP_PACK));
    assert!(!plain.contains("temp_packs"));

    let clash = Catalog::new(vec![user_mod("mods/!!!!OUT.pack".into())]);
    let one = HashSet::from([0]);
    assert!(launch::script(&clash, &[0], &one, &ALL, dir).is_err());
    assert!(launch::script(&clash, &[0], &one, &Options::default(), dir).is_ok());
    assert!(launch::script(&catalog, &[0], &one, &ALL, Path::new("bad;dir")).is_err());
}

#[test]
fn prepare_writes_generated_pack_and_drops_stale_one() {
    let temp = tempfile::tempdir().unwrap();
    let game = temp.path().join("game");
    let mods = temp.path().join("mods");
    let packs = temp.path().join("temp_packs");
    fs::create_dir_all(&game).unwrap();
    fs::write(game.join("Warhammer3.exe"), []).unwrap();
    pack::write(&mods.join("user.pack"), &[]).unwrap();
    let catalog = Catalog::new(vec![user_mod(mods.join("user.pack"))]);
    let enabled = HashSet::from([0]);

    launch::prepare(&game, &catalog, &[0], &enabled, &ALL, &packs).unwrap();
    let generated = packs.join(launch::TEMP_PACK);
    let expected: Vec<_> = launch::start_pack_files(&ALL)
        .into_iter()
        .map(|(name, bytes)| (name.to_string(), bytes))
        .collect();
    assert_eq!(contents(&generated), expected);
    let list = fs::read_to_string(game.join(launch::MOD_LIST)).unwrap();
    assert!(list.ends_with(&format!(
        "add_working_directory \"{}\";\nmod \"!!!!out.pack\";",
        packs.display()
    )));

    launch::prepare(&game, &catalog, &[0], &enabled, &Options::default(), &packs).unwrap();
    assert!(!generated.exists());
    let list = fs::read_to_string(game.join(launch::MOD_LIST)).unwrap();
    assert!(!list.contains(launch::TEMP_PACK));
}

#[test]
fn arguments_match_upstream_direct_launch() {
    assert_eq!(launch::arguments(None), ["wh3_rust_mods.txt;"]);
    assert_eq!(launch::arguments(Some("")), ["wh3_rust_mods.txt;"]);
    assert_eq!(
        launch::arguments(Some("Karl Franz turn 12.save")),
        [
            "game_startup_mode",
            "campaign_load",
            "Karl Franz turn 12.save",
            ";",
            "wh3_rust_mods.txt;",
        ]
    );
}

#[test]
fn start_rejects_save_names_that_would_split_the_command_line() {
    for save in ["a;b.save", "a\"b.save", "..\\other.save", "a\nb.save"] {
        let error = launch::start(Path::new("missing"), Some(save)).unwrap_err();
        assert!(
            error
                .message()
                .text(Language::English)
                .contains("save name")
        );
    }
}
