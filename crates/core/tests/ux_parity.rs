#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Shared lists, guided bisecting, folder watching and game detection.
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::{Duration, Instant},
};
use wh3_core::{
    bisect::{self, Search},
    catalog::{Catalog, Mod, Source},
    game_process, share, watch,
};

fn item(name: &str, id: &str, source: Source, dependencies: &[&str]) -> Mod {
    Mod::new(
        PathBuf::from(format!("mods/{name}")),
        name.into(),
        id.into(),
        source,
        1,
        false,
        dependencies.iter().map(|d| d.to_string()).collect(),
    )
}

#[test]
fn shared_list_round_trips_with_the_original_format() {
    let catalog = Catalog::new(vec![
        item("b.pack", "111", Source::Workshop, &[]),
        item("my mod%.pack", "", Source::Custom, &[]),
        item("copy.pack", "222", Source::Data, &[]),
        item("off.pack", "333", Source::Workshop, &[]),
    ]);
    let order = [2, 0, 1, 3];
    let enabled: HashSet<usize> = [0, 1, 2].into();
    let text = share::serialize(&catalog, &order, &enabled);
    assert_eq!(text, "local:copy.pack:222;0|111;1|local:my%20mod%25.pack;2");

    let parsed = share::parse(&text);
    assert_eq!(parsed[0].name.as_deref(), Some("copy.pack"));
    assert_eq!(parsed[0].workshop_id, "222");
    assert_eq!(parsed[1].workshop_id, "111");
    assert_eq!(parsed[2].name.as_deref(), Some("my mod%.pack"));
    assert_eq!(parsed[2].load_order, Some(2));

    let by_workshop: HashMap<u64, Vec<usize>> = [(111, vec![0]), (222, vec![2])].into();
    let resolved = share::resolve(&parsed, &catalog, &by_workshop, "Shared".into());
    let names: Vec<_> = resolved
        .preset
        .mods
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(names, ["copy.pack", "b.pack", "my mod%.pack"]);
    assert!(resolved.preset.mods.iter().all(|e| e.load_order.is_none()));
    let applied = resolved.preset.apply(&catalog);
    assert_eq!(&applied.order[..3], &[2, 0, 1]);
    assert_eq!(applied.enabled, enabled);
}

#[test]
fn shared_lists_from_the_original_keep_pins_and_report_what_is_missing() {
    let catalog = Catalog::new(vec![item("a.pack", "1", Source::Workshop, &[])]);
    let parsed = share::parse(" 1 | 99;0 | local:gone.pack | local:bad%zz.pack:7 ");
    assert_eq!(parsed.len(), 4);
    assert_eq!(parsed[3].name.as_deref(), Some("bad%zz.pack"));
    let by_workshop: HashMap<u64, Vec<usize>> = [(1, vec![0])].into();
    let resolved = share::resolve(&parsed, &catalog, &by_workshop, "Shared".into());
    assert_eq!(resolved.missing_workshop, [7, 99]);
    assert_eq!(resolved.missing_local, ["gone.pack"]);
    assert_eq!(resolved.preset.version, Some(2));
    assert!(share::parse("  ").is_empty());
}

#[test]
fn bisecting_keeps_requirements_together_and_finds_the_culprit() {
    let mut mods: Vec<Mod> = (0..9)
        .map(|i| {
            item(
                &format!("m{i}.pack"),
                &format!("{}", 100 + i),
                Source::Workshop,
                &[],
            )
        })
        .collect();
    // m1 needs m7 through a pack dependency; m4 needs m5 through the Workshop.
    mods[1] = item("m1.pack", "101", Source::Workshop, &["M7.pack"]);
    mods[4].metadata.req_mod_id_to_name = vec![("105".into(), "m5".into())];
    let catalog = Catalog::new(mods);
    let suspects: Vec<usize> = (0..9).collect();
    let groups = bisect::groups(&catalog, &suspects);
    assert!(groups.contains(&vec![1, 7]));
    assert!(groups.contains(&vec![4, 5]));
    let (first, rest) = bisect::split(&groups).unwrap();
    assert_eq!(first.len() + rest.len(), 9);
    assert!(first.len().abs_diff(rest.len()) <= 1);

    for culprit in 0..9 {
        let mut search = Search::start(&catalog, suspects.clone()).unwrap();
        let mut steps = 1;
        while search.answer(&catalog, search.testing.contains(&culprit)) {
            steps += 1;
            assert!(steps < 9, "the search must converge");
        }
        assert!(search.suspects.contains(&culprit), "culprit {culprit}");
        assert!(search.suspects.len() <= 2);
    }
    assert!(Search::start(&catalog, vec![3]).is_none());
    assert!(bisect::split(&[vec![1, 7]]).is_none());
}

#[test]
fn watcher_events_are_classified_for_packs_folders_and_saves() {
    use notify::event::{CreateKind, EventKind, ModifyKind, RemoveKind};
    let root = PathBuf::from("/lib/content/1142710");
    let saves = PathBuf::from("/saves");
    let roots = [root.clone()];
    let classify = |kind: EventKind, path: &str| {
        watch::classify(&kind, &PathBuf::from(path), &roots, Some(&saves))
    };
    let create = EventKind::Create(CreateKind::Any);
    let modify = EventKind::Modify(ModifyKind::Any);
    assert_eq!(
        classify(modify, "/lib/content/1142710/5/a.PACK"),
        Some(watch::Change::Packs)
    );
    assert_eq!(
        classify(
            EventKind::Remove(RemoveKind::Folder),
            "/lib/content/1142710/5"
        ),
        Some(watch::Change::Packs)
    );
    assert_eq!(classify(create, "/lib/content/1142710/5/preview.png"), None);
    assert_eq!(classify(modify, "/lib/content/1142710/5"), None);
    assert_eq!(
        classify(create, "/saves/campaign.save"),
        Some(watch::Change::Saves)
    );
    assert_eq!(classify(create, "/saves/settings.txt"), None);
    assert_eq!(classify(create, "/elsewhere/a.pack"), None);
}

#[test]
fn watcher_reports_a_new_pack_and_save() {
    let dir = tempfile::tempdir().unwrap();
    let (mods, saves) = (dir.path().join("mods"), dir.path().join("saves"));
    std::fs::create_dir_all(mods.join("item")).unwrap();
    std::fs::create_dir_all(&saves).unwrap();
    let watcher = watch::watch(&[(mods.clone(), Source::Custom)], Some(&saves)).unwrap();
    std::fs::write(mods.join("item").join("new.pack"), b"pack").unwrap();
    std::fs::write(saves.join("turn 1.save"), b"save").unwrap();
    wait_for_changes(&watcher.signals);
}

#[cfg(unix)]
#[test]
fn watcher_reports_created_and_removed_files_through_symlinked_folders() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let (mods, saves) = (root.join("mods"), root.join("saves"));
    std::fs::create_dir_all(mods.join("item")).unwrap();
    std::fs::create_dir_all(&saves).unwrap();
    let alias = root.join("alias");
    std::os::unix::fs::symlink(&root, &alias).unwrap();
    let watcher = watch::watch(
        &[(alias.join("mods"), Source::Custom)],
        Some(&alias.join("saves")),
    )
    .unwrap();
    let pack = mods.join("item/new.pack");
    let save = saves.join("turn 1.save");
    std::fs::write(&pack, b"pack").unwrap();
    std::fs::write(&save, b"save").unwrap();
    wait_for_changes(&watcher.signals);

    // A separate watcher prevents queued creation events from satisfying deletion.
    drop(watcher);
    let watcher = watch::watch(
        &[(alias.join("mods"), Source::Custom)],
        Some(&alias.join("saves")),
    )
    .unwrap();
    std::fs::remove_file(pack).unwrap();
    std::fs::remove_file(save).unwrap();
    wait_for_changes(&watcher.signals);
}

fn wait_for_changes(signals: &watch::Signals) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while (signals.packs() == 0 || signals.saves() == 0) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(signals.packs() > 0, "no pack change event received");
    assert!(signals.saves() > 0, "no save change event received");
}

#[test]
fn tasklist_output_is_matched_regardless_of_locale() {
    let found = "\r\nWarhammer3.exe               4242 Console     1  9 000 000 K\r\n";
    assert!(game_process::lists_game(found));
    assert!(game_process::lists_game("warhammer3.EXE 1 Console"));
    assert!(!game_process::lists_game(
        "Информация: задачи, отвечающие заданным критериям, отсутствуют."
    ));
    assert!(!game_process::lists_game(
        "INFO: No tasks are running which match the specified criteria."
    ));
}
