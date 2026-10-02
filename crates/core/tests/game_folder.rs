#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Staging Workshop mods and copying/linking packs into `data` touch only the files
//! they name, never overwrite and never follow links when deleting.
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use wh3_core::{
    catalog::{Catalog, Mod, Source},
    data_folder::{self, Placement},
    staging::{self, Progress},
    storage::Staging,
};

fn pack(path: &Path, bytes: &[u8]) -> PathBuf {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
    path.to_owned()
}

fn item(path: &Path, source: Source) -> Mod {
    Mod::new(
        path.to_owned(),
        "t".into(),
        String::new(),
        source,
        1,
        false,
        vec![],
    )
}

struct Game {
    _dir: tempfile::TempDir,
    game: PathBuf,
    catalog: Catalog,
}

/// A game with one data pack and three Workshop packs, one shadowed by data.
fn game() -> Game {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    let workshop = dir.path().join("workshop");
    let data = pack(&game.join("data/shared.pack"), b"data copy");
    let mods = vec![
        item(&data, Source::Data),
        item(&pack(&workshop.join("1/a.pack"), b"aaaa"), Source::Workshop),
        item(&pack(&workshop.join("2/b.pack"), b"bb"), Source::Workshop),
        item(
            &pack(&workshop.join("3/shared.pack"), b"workshop"),
            Source::Workshop,
        ),
    ];
    Game {
        _dir: dir,
        game,
        catalog: Catalog::new(mods),
    }
}

fn index(catalog: &Catalog, name: &str, source: Source) -> usize {
    catalog
        .mods
        .iter()
        .position(|m| &*m.name == name && m.source == source)
        .unwrap()
}

#[test]
fn copy_staging_reuses_current_copies_and_prunes_unselected_packs() {
    let g = game();
    let (a, b, shared) = (
        index(&g.catalog, "a.pack", Source::Workshop),
        index(&g.catalog, "b.pack", Source::Workshop),
        index(&g.catalog, "shared.pack", Source::Workshop),
    );
    let folder = staging::folder(&g.game);
    let unrelated = pack(&folder.join("notes.txt"), b"keep me");
    let enabled: HashSet<usize> = [a, b, shared].into();
    let progress = Progress::default();
    let cancel = AtomicBool::new(false);
    let staged = staging::stage(
        &g.game,
        &g.catalog,
        &enabled,
        Staging::Copy,
        false,
        &cancel,
        &progress,
    )
    .unwrap();
    assert_eq!(staged.mods[a].path, folder.join("a.pack"));
    assert_eq!(fs::read(folder.join("a.pack")).unwrap(), b"aaaa");
    // The data copy wins, so the Workshop pack of the same name is not staged.
    assert_eq!(staged.mods[shared].path, g.catalog.mods[shared].path);
    assert!(!folder.join("shared.pack").exists());
    assert_eq!(progress.total.load(std::sync::atomic::Ordering::Relaxed), 2);

    let before = fs::metadata(folder.join("a.pack"))
        .unwrap()
        .modified()
        .unwrap();
    let only_a: HashSet<usize> = [a].into();
    staging::stage(
        &g.game,
        &g.catalog,
        &only_a,
        Staging::Copy,
        false,
        &cancel,
        &progress,
    )
    .unwrap();
    assert_eq!(
        fs::metadata(folder.join("a.pack"))
            .unwrap()
            .modified()
            .unwrap(),
        before
    );
    assert!(
        !folder.join("b.pack").exists(),
        "unselected packs are pruned"
    );
    assert_eq!(
        fs::read(&unrelated).unwrap(),
        b"keep me",
        "only packs are pruned"
    );

    staging::clean(&g.game).unwrap();
    assert!(!folder.exists());
    assert!(g.catalog.mods[a].path.exists(), "sources stay");
}

#[cfg(unix)]
#[test]
fn link_staging_points_at_the_workshop_pack_and_cleaning_keeps_targets() {
    let g = game();
    let a = index(&g.catalog, "a.pack", Source::Workshop);
    let enabled: HashSet<usize> = [a].into();
    let (progress, cancel) = (Progress::default(), AtomicBool::new(false));
    let staged = staging::stage(
        &g.game,
        &g.catalog,
        &enabled,
        Staging::Symlink,
        true,
        &cancel,
        &progress,
    )
    .unwrap();
    let link = staged.mods[a].path.clone();
    assert_eq!(fs::read_link(&link).unwrap(), g.catalog.mods[a].path);
    staging::clean(&g.game).unwrap();
    assert_eq!(fs::read(&g.catalog.mods[a].path).unwrap(), b"aaaa");
}

#[test]
fn cancelled_staging_stops_before_writing() {
    let g = game();
    let enabled: HashSet<usize> = [index(&g.catalog, "a.pack", Source::Workshop)].into();
    let cancel = AtomicBool::new(true);
    let result = staging::stage(
        &g.game,
        &g.catalog,
        &enabled,
        Staging::Copy,
        false,
        &cancel,
        &Progress::default(),
    );
    assert!(matches!(result, Err(wh3_core::Error::Cancelled)));
    assert!(!staging::folder(&g.game).join("a.pack").exists());
}

#[test]
fn copies_into_data_never_overwrite_and_removal_stays_in_data() {
    let g = game();
    let a = g.catalog.mods[index(&g.catalog, "a.pack", Source::Workshop)]
        .path
        .clone();
    let shared = g.catalog.mods[index(&g.catalog, "shared.pack", Source::Workshop)]
        .path
        .clone();
    let cancel = AtomicBool::new(false);
    let outcome =
        data_folder::place(&g.game, &[a.clone(), shared], Placement::Copy, &cancel).unwrap();
    assert_eq!(outcome.done, ["a.pack"]);
    assert_eq!(
        outcome.failed.len(),
        1,
        "the existing data pack is reported"
    );
    let data = data_folder::data_dir(&g.game);
    assert_eq!(fs::read(data.join("shared.pack")).unwrap(), b"data copy");
    assert_eq!(fs::read(data.join("a.pack")).unwrap(), b"aaaa");

    assert!(
        data_folder::remove(&g.game, &a).is_err(),
        "only packs in data"
    );
    data_folder::remove(&g.game, &data.join("a.pack")).unwrap();
    assert!(a.exists());
}

#[cfg(unix)]
#[test]
fn links_in_data_are_listed_for_cleanup_and_removed_without_their_targets() {
    let g = game();
    let a = g.catalog.mods[index(&g.catalog, "a.pack", Source::Workshop)]
        .path
        .clone();
    let cancel = AtomicBool::new(false);
    data_folder::place(&g.game, std::slice::from_ref(&a), Placement::Link, &cancel).unwrap();
    // Test packs are not real PFH files, so the link is added to the catalog by hand.
    let mut catalog_mods = g.catalog.mods.clone();
    let link = data_folder::data_dir(&g.game).join("a.pack");
    catalog_mods.push(item(&link, Source::Data).with_link(true));
    let catalog = Catalog::new(catalog_mods);
    let links = data_folder::removable(&catalog, true);
    assert_eq!(links, std::slice::from_ref(&link));
    let duplicates = data_folder::removable(&catalog, false);
    assert!(duplicates.contains(&link));
    assert!(duplicates.iter().any(|p| p.ends_with("shared.pack")));
    let outcome = data_folder::remove_all(&g.game, &links);
    assert_eq!(outcome.done, ["a.pack"]);
    assert_eq!(fs::read(&a).unwrap(), b"aaaa");
}
