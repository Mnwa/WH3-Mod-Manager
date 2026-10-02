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

/// Runs a search where the problem appears whenever `culprit` is enabled.
fn hunt(catalog: &Catalog, suspects: &[usize], culprit: usize) -> Search {
    let requirements = bisect::requirements(catalog, suspects);
    let mut search = Search::start(catalog, suspects.to_vec()).unwrap();
    let mut steps = 1;
    loop {
        // A test never runs a mod without what it requires.
        let enabled: HashSet<usize> = search.enabled.iter().copied().collect();
        assert_eq!(bisect::closure(&requirements, enabled.clone()), enabled);
        if !search.answer(enabled.contains(&culprit)) {
            return search;
        }
        steps += 1;
        assert!(steps <= suspects.len(), "the search must converge");
    }
}

#[test]
fn bisecting_brings_requirements_along_and_finds_the_culprit() {
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
    let requirements = bisect::requirements(&catalog, &suspects);
    assert_eq!(requirements[&1], [7]);
    assert_eq!(requirements[&4], [5]);
    for culprit in 0..9 {
        let search = hunt(&catalog, &suspects, culprit);
        assert_eq!(search.suspects, [culprit]);
        assert_eq!(
            bisect::closure(&requirements, [culprit]),
            search.enabled.iter().copied().collect::<HashSet<_>>()
        );
    }
    assert!(Search::start(&catalog, vec![3]).is_none());
}

/// Regression: a framework that most mods require used to weld them into one group
/// that could never be split, so "found" named half of the list.
#[test]
fn bisecting_separates_mods_linked_through_a_shared_framework() {
    let mut mods = vec![item("framework.pack", "1", Source::Workshop, &[])];
    for i in 1..40 {
        let mut addon = item(
            &format!("addon{i}.pack"),
            &format!("{}", 100 + i),
            Source::Workshop,
            &[],
        );
        addon.metadata.req_mod_id_to_name = vec![("1".into(), "framework".into())];
        mods.push(addon);
        // A translation needs its addon, chaining it to the framework as well.
        mods.push(item(
            &format!("addon{i}_rus.pack"),
            "",
            Source::Workshop,
            &[&format!("addon{i}.pack")],
        ));
    }
    mods.push(item("loner.pack", "999", Source::Workshop, &[]));
    let catalog = Catalog::new(mods);
    let suspects: Vec<usize> = (0..catalog.mods.len()).collect();
    for culprit in suspects.clone() {
        let search = hunt(&catalog, &suspects, culprit);
        assert_eq!(search.suspects, [culprit], "{}", catalog.mods[culprit].name);
    }
    // Without the framework, every addon and translation stops working.
    let search = hunt(&catalog, &suspects, 0);
    assert_eq!(search.dependents().len(), catalog.mods.len() - 2);
    // Without one addon, only its translation does.
    let search = hunt(&catalog, &suspects, 1);
    assert_eq!(search.dependents(), [2]);
}

#[test]
fn bisecting_keeps_mods_that_require_each_other_together() {
    let catalog = Catalog::new(vec![
        item("a.pack", "", Source::Workshop, &["b.pack"]),
        item("b.pack", "", Source::Workshop, &["a.pack"]),
        item("c.pack", "", Source::Workshop, &[]),
    ]);
    let search = hunt(&catalog, &[0, 1, 2], 1);
    assert_eq!(search.suspects, [0, 1]);
    assert!(Search::start(&catalog, vec![0, 1]).is_none());
}

#[test]
fn bisecting_follows_a_renumbered_catalog() {
    let catalog = Catalog::new(
        (0..6)
            .map(|i| item(&format!("m{i}.pack"), "", Source::Workshop, &[]))
            .collect(),
    );
    let mut search = Search::start(&catalog, (0..6).collect()).unwrap();
    let tested = search.testing.clone();
    // A rescan reversed the indices and m0 was unsubscribed.
    search.remap(|index| (index != 0).then(|| 10 - index));
    let expected: Vec<usize> = tested.iter().filter(|&&m| m != 0).map(|m| 10 - m).collect();
    assert_eq!(search.testing, expected);
    assert!(!search.suspects.contains(&10));
    assert!(search.answer(true) || search.suspects.len() <= 1);
    assert!(search.suspects.iter().all(|m| expected.contains(m)));
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
