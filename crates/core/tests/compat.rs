#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use common::{fixture, index_of};
use std::{collections::HashSet, fs, path::Path, sync::atomic::AtomicBool};
use wh3_core::{
    catalog::{Catalog, Source},
    conflict::{self, Availability, ModIssues},
    scan,
};

fn scan_custom(root: &Path) -> Catalog {
    scan::scan(&[(root.into(), Source::Custom)], &AtomicBool::new(false))
        .unwrap()
        .catalog
}

fn check_all(catalog: &Catalog, order: &[usize]) -> conflict::Report {
    let enabled = order.iter().copied().collect();
    conflict::check(catalog, order, &enabled, &AtomicBool::new(false)).unwrap()
}

#[test]
fn file_conflicts_name_the_highest_priority_owner_first() {
    let temp = tempfile::tempdir().unwrap();
    fixture(&temp.path().join("a.pack"), &[], &[("ui/Shared.png", 4)]);
    fixture(&temp.path().join("b.pack"), &[], &[("ui\\shared.png", 4)]);
    fixture(
        &temp.path().join("c.pack"),
        &[],
        &[("ui\\shared.png", 9), ("x.rpfm_reserved", 0)],
    );
    fixture(&temp.path().join("d.pack"), &[], &[("x.rpfm_reserved", 0)]);
    let catalog = scan_custom(temp.path());
    let [a, b, c, d] = ["a.pack", "b.pack", "c.pack", "d.pack"].map(|n| index_of(&catalog, n));

    let report = check_all(&catalog, &[b, a, d]);
    assert_eq!(report.files.len(), 1, "RPFM placeholders are not loaded");
    assert_eq!(report.files[0].owners, [b, a]);
    assert!(report.files[0].same_size);

    let report = check_all(&catalog, &[c, b, a]);
    assert_eq!(report.files[0].path, "ui\\shared.png");
    assert_eq!(report.files[0].owners, [c, b, a]);
    assert!(!report.files[0].same_size);
    let issues = report.issues_by_mod();
    assert_eq!(issues[&c].overwrites, 1);
    assert_eq!((issues[&b].overwritten, issues[&a].overwritten), (1, 1));
}

#[test]
fn table_conflicts_use_loaded_db_files_in_row_priority_order() {
    let temp = tempfile::tempdir().unwrap();
    fixture(
        &temp.path().join("a.pack"),
        &[],
        &[("db\\units_tables\\data", 1)],
    );
    fixture(
        &temp.path().join("b.pack"),
        &[],
        &[
            ("db\\units_tables\\data_extra", 1),
            ("db\\units_tables\\data", 1),
        ],
    );
    fixture(
        &temp.path().join("c.pack"),
        &[],
        &[
            ("unusedtables\\units_tables\\spare", 1),
            ("db\\other_tables\\x", 1),
        ],
    );
    fixture(
        &temp.path().join("d.pack"),
        &[],
        &[("db\\other_tables\\y", 1)],
    );
    let catalog = scan_custom(temp.path());
    let [a, b, c, d] = ["a.pack", "b.pack", "c.pack", "d.pack"].map(|n| index_of(&catalog, n));

    let report = check_all(&catalog, &[a, b, c]);
    assert_eq!(report.tables.len(), 1, "a spare table copy is inert");
    let table = &report.tables[0];
    assert_eq!(table.table, "units_tables");
    // b's `data` is shadowed by a, so only a's copy contributes rows. A prefix sorts last.
    let files: Vec<_> = table.files.iter().map(|f| (f.owner, &*f.path)).collect();
    assert_eq!(
        files,
        [
            (b, "db\\units_tables\\data_extra"),
            (a, "db\\units_tables\\data")
        ]
    );
    assert_eq!(report.files.len(), 1);

    let report = check_all(&catalog, &[d, c]);
    assert_eq!(report.tables[0].table, "other_tables");
    assert_eq!(report.issues_by_mod()[&c].tables, 1);
}

#[test]
fn missing_dependencies_and_requirements_distinguish_disabled_from_absent() {
    let temp = tempfile::tempdir().unwrap();
    let (custom, workshop) = (temp.path().join("custom"), temp.path().join("workshop"));
    fixture(
        &custom.join("main.pack"),
        &["base.pack", "Folder/Disabled.pack", "absent.pack"],
        &[],
    );
    fixture(&custom.join("base.pack"), &[], &[]);
    fixture(&custom.join("disabled.pack"), &[], &[]);
    fixture(&custom.join("copy.pack"), &[], &[]);
    fixture(&workshop.join("100/copy.pack"), &[], &[]);
    fixture(&workshop.join("200/library.pack"), &[], &[]);
    let mut catalog = scan::scan(
        &[(custom, Source::Custom), (workshop, Source::Workshop)],
        &AtomicBool::new(false),
    )
    .unwrap()
    .catalog;
    let main = index_of(&catalog, "main.pack");
    let base = index_of(&catalog, "base.pack");
    let disabled = index_of(&catalog, "disabled.pack");
    let library = index_of(&catalog, "library.pack");
    let local_copy = catalog.by_name["copy.pack"]
        .iter()
        .copied()
        .find(|&i| catalog.mods[i].source == Source::Custom)
        .unwrap();
    catalog.mods[main].metadata.req_mod_id_to_name = vec![
        ("100".into(), "Copied".into()),
        ("200".into(), "Library".into()),
        ("300".into(), "Gone".into()),
    ];

    let report = check_all(&catalog, &[main, base, local_copy]);
    let dependencies: Vec<_> = report
        .missing_dependencies
        .iter()
        .map(|m| (m.owner, &*m.dependency, m.availability))
        .collect();
    assert_eq!(
        dependencies,
        [
            (
                main,
                "Folder/Disabled.pack",
                Availability::Disabled(disabled)
            ),
            (main, "absent.pack", Availability::NotInstalled),
        ]
    );
    // Requirement 100 is met by the enabled local copy of the Workshop pack.
    let required: Vec<_> = report
        .missing_required
        .iter()
        .map(|m| (&*m.workshop_id, &*m.name, m.availability))
        .collect();
    assert_eq!(
        required,
        [
            ("200", "Library", Availability::Disabled(library)),
            ("300", "Gone", Availability::NotInstalled),
        ]
    );
    assert_eq!(
        report.issues_by_mod()[&main],
        ModIssues {
            missing_dependencies: 2,
            missing_required: 2,
            ..Default::default()
        }
    );
}

#[test]
fn startpos_conflicts_ignore_packs_ordered_by_dependencies() {
    let temp = tempfile::tempdir().unwrap();
    let startpos = [("campaigns\\main_warhammer\\StartPos.esf", 1)];
    fixture(&temp.path().join("base.pack"), &[], &startpos);
    fixture(&temp.path().join("addon.pack"), &["middle.pack"], &startpos);
    fixture(&temp.path().join("middle.pack"), &["base.pack"], &[]);
    fixture(&temp.path().join("other.pack"), &[], &startpos);
    fixture(
        &temp.path().join("fake.pack"),
        &[],
        &[("notstartpos.esf", 1)],
    );
    let catalog = scan_custom(temp.path());
    let [base, addon, middle, other, fake] = [
        "base.pack",
        "addon.pack",
        "middle.pack",
        "other.pack",
        "fake.pack",
    ]
    .map(|n| index_of(&catalog, n));

    assert!(
        check_all(&catalog, &[addon, middle, base, fake])
            .startpos
            .is_empty()
    );
    // Without the middle pack the transitive link is gone.
    assert_eq!(check_all(&catalog, &[addon, base]).startpos, [addon, base]);
    assert_eq!(
        check_all(&catalog, &[other, addon, middle, base]).startpos,
        [other, addon, base]
    );
}

#[test]
fn unreadable_duplicate_and_cancelled_checks_are_reported() {
    let temp = tempfile::tempdir().unwrap();
    fixture(&temp.path().join("a/same.pack"), &[], &[("x", 1)]);
    fixture(&temp.path().join("b/same.pack"), &[], &[("x", 1)]);
    fixture(&temp.path().join("broken.pack"), &[], &[("x", 1)]);
    let catalog = scan_custom(temp.path());
    let broken = index_of(&catalog, "broken.pack");
    fs::write(&catalog.mods[broken].path, b"PFH5").unwrap();
    let same = &catalog.by_name["same.pack"];
    let report = check_all(&catalog, &[same[0], same[1], broken]);
    assert!(report.files.is_empty(), "one pack name is loaded once");
    assert_eq!(report.unreadable, [broken]);
    assert_eq!(report.warnings.len(), 2);
    assert!(report.issues_by_mod()[&broken].unreadable);

    let enabled = HashSet::from([0]);
    let result = conflict::check(&catalog, &[0], &enabled, &AtomicBool::new(true));
    assert!(matches!(result, Err(wh3_core::Error::Cancelled)));
}
