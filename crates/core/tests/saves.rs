#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::{
    fs,
    time::{Duration, SystemTime},
};
use wh3_core::saves;

fn touch(path: &std::path::Path, bytes: &[u8], age: u64) {
    fs::write(path, bytes).unwrap();
    let modified = SystemTime::now() - Duration::from_secs(age);
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(modified)
        .unwrap();
}

#[test]
fn list_returns_only_save_files_newest_first() {
    let temp = tempfile::tempdir().unwrap();
    touch(&temp.path().join("old.save"), b"12", 300);
    touch(&temp.path().join("new.SAVE"), b"1", 10);
    touch(&temp.path().join("middle.save"), b"123", 100);
    touch(&temp.path().join("notes.txt"), b"x", 0);
    touch(&temp.path().join("old.save.bak"), b"x", 0);
    fs::create_dir(temp.path().join("folder.save")).unwrap();

    let list = saves::list(temp.path()).unwrap();
    let names: Vec<_> = list.iter().map(|save| save.name.as_str()).collect();
    assert_eq!(names, ["new.SAVE", "middle.save", "old.save"]);
    assert_eq!(list[1].size, 3);
    assert_eq!(list[1].path, temp.path().join("middle.save"));
    assert!(list[0].modified > list[1].modified);
}

#[test]
fn missing_save_folder_is_empty_rather_than_an_error() {
    let temp = tempfile::tempdir().unwrap();
    assert!(saves::list(&temp.path().join("absent")).unwrap().is_empty());
}

#[test]
fn mods_follow_upstream_pattern_and_deduplicate_in_order() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("campaign.save");
    let mut bytes = b"header.pack without leading nul".to_vec();
    for chunk in [
        &b"\0\x05\0\0sfo_grimhammer.pack"[..],
        b"\0data.pack\x01\x02",
        // No `.pack` before the next NUL: ignored like the original regex.
        b"\0not a pack\0",
        // Only the shortest name up to the first `.pack` is taken.
        b"\0twice.pack.pack",
        b"\0sfo_grimhammer.pack",
        // The regex needs at least one byte before `.pack`.
        b"\0.pack",
        b"\0\xd0\x9a.pack",
    ] {
        bytes.extend_from_slice(chunk);
    }
    fs::write(&path, bytes).unwrap();
    assert_eq!(
        saves::mods(&path).unwrap(),
        ["sfo_grimhammer.pack", "data.pack", "twice.pack", "К.pack"]
    );
}

#[test]
fn mods_refuses_oversized_saves_without_reading_them() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("huge.save");
    let file = fs::File::create(&path).unwrap();
    file.set_len(saves::MAX_SAVE + 1).unwrap();
    assert!(saves::mods(&path).is_err());
    assert!(saves::mods(&temp.path().join("absent.save")).is_err());
}
