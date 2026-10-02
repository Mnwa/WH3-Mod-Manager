#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::collections::BTreeMap;
use wh3_core::{
    catalog::{Catalog, Mod, Source},
    metadata::Metadata,
    workshop::{self, Item, STATE_INSTALLED, STATE_NEEDS_UPDATE, State, merge},
};

fn workshop_mod(id: &str, name: &str) -> Mod {
    Mod::new(
        format!("content/{id}/{name}").into(),
        name.trim_end_matches(".pack").into(),
        id.into(),
        Source::Workshop,
        1,
        false,
        vec![],
    )
}

fn item(id: u64, title: &str, updated: u32, required: Vec<u64>) -> Item {
    Item {
        id,
        title: title.into(),
        author: "Author".into(),
        time_updated: updated,
        tags: vec!["mod".into()],
        required,
        ..Item::default()
    }
}

#[test]
fn merge_fills_titles_authors_requirements_and_keeps_user_categories() {
    let mut catalog = Catalog::new(vec![
        workshop_mod("1", "a.pack"),
        workshop_mod("2", "b.pack"),
    ]);
    let mut metadata = BTreeMap::from([(
        "a.pack".to_owned(),
        Metadata {
            categories: vec!["Campaign".into()],
            ..Metadata::default()
        },
    )]);
    let items = [
        item(1, "Alpha", 100, vec![2, 9]),
        item(2, "Beta", 200, vec![]),
    ];
    assert_eq!(merge::merge(&mut catalog, &mut metadata, &items), 2);
    assert_eq!(&*catalog.mods[0].title, "Alpha");
    assert_eq!(metadata["a.pack"].categories, ["Campaign"]);
    assert_eq!(metadata["a.pack"].author, "Author");
    assert_eq!(
        metadata["a.pack"].req_mod_id_to_name,
        [("2".into(), "Beta".into()), ("9".into(), String::new())]
    );
    // Workshop titles and authors become searchable right away.
    assert_eq!(catalog.query("alpha author", &[0, 1]), [0]);
    assert_eq!(merge::unknown_required(&items), [9]);
}

#[test]
fn outdated_means_a_newer_workshop_revision_than_the_install() {
    let catalog = Catalog::new(vec![
        workshop_mod("1", "a.pack"),
        workshop_mod("2", "b.pack"),
    ]);
    let items = [item(1, "A", 200, vec![]), item(2, "B", 100, vec![])];
    let installed = |id, at| State {
        id,
        state: STATE_INSTALLED,
        installed: Some(at),
        ..State::default()
    };
    let states = [installed(1, 150), installed(2, 150)];
    assert_eq!(
        merge::outdated(&catalog, &items, &states),
        std::collections::HashSet::from([0])
    );
    let flagged = State {
        state: STATE_INSTALLED | STATE_NEEDS_UPDATE,
        ..installed(2, 150)
    };
    assert!(workshop::needs_update(&items[1], &flagged));
}

#[test]
fn collection_links_and_ids_are_parsed() {
    assert_eq!(
        workshop::parse_id(
            "https://steamcommunity.com/sharedfiles/filedetails/?id=3390949038&searchtext="
        ),
        Some(3390949038)
    );
    assert_eq!(workshop::parse_id(" 42 "), Some(42));
    assert_eq!(workshop::parse_id("not a link"), None);
    assert_eq!(workshop::parse_id("id=0"), None);
}
