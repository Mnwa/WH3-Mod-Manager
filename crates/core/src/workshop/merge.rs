//! Fold Workshop details into user metadata and the catalog, like the original's
//! `modUserData` refresh: titles, authors, tags and required items.
use super::{Item, State, needs_update};
use crate::{catalog::Catalog, metadata::Metadata};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::{Duration, SystemTime},
};

/// Catalog indices grouped by numeric Workshop id.
pub fn by_id(catalog: &Catalog) -> HashMap<u64, Vec<usize>> {
    let mut ids: HashMap<u64, Vec<usize>> = HashMap::new();
    for (index, item) in catalog.mods.iter().enumerate() {
        if let Ok(id) = item.workshop_id.parse::<u64>() {
            ids.entry(id).or_default().push(index);
        }
    }
    ids
}

/// Update metadata and catalog entries for every item; returns how many mods matched.
/// User categories are preserved; Workshop titles replace imported titles.
pub fn merge(
    catalog: &mut Catalog,
    metadata: &mut BTreeMap<String, Metadata>,
    items: &[Item],
) -> usize {
    let titles: HashMap<u64, &str> = items
        .iter()
        .map(|item| (item.id, item.title.as_str()))
        .collect();
    let ids = by_id(catalog);
    let mut matched = 0;
    for item in items {
        for &index in ids.get(&item.id).into_iter().flatten() {
            let name = catalog.mods[index].name.to_string();
            let meta = metadata.entry(name).or_default();
            if !item.title.is_empty() {
                meta.human_name.clone_from(&item.title);
            }
            if !item.author.is_empty() {
                meta.author.clone_from(&item.author);
            }
            meta.workshop_id = item.id.to_string();
            meta.tags.clone_from(&item.tags);
            meta.req_mod_id_to_name = item
                .required
                .iter()
                .map(|id| {
                    (
                        id.to_string(),
                        titles.get(id).copied().unwrap_or_default().to_owned(),
                    )
                })
                .collect();
            let meta = meta.clone();
            let title = meta.human_name.clone();
            catalog.update_metadata(index, move |target| *target = meta);
            let entry = &mut catalog.mods[index];
            if !title.is_empty() {
                entry.title = title.into();
            }
            // The Workshop revision time is what the original shows as "updated".
            if item.time_updated > 0 {
                entry.modified =
                    Some(SystemTime::UNIX_EPOCH + Duration::from_secs(item.time_updated.into()));
            }
            matched += 1;
        }
    }
    catalog.rebuild_search_all();
    matched
}

/// Catalog indices whose installed copy is older than the Workshop revision.
pub fn outdated(catalog: &Catalog, items: &[Item], states: &[State]) -> HashSet<usize> {
    let states: HashMap<u64, &State> = states.iter().map(|state| (state.id, state)).collect();
    let ids = by_id(catalog);
    items
        .iter()
        .filter(|item| {
            states
                .get(&item.id)
                .is_some_and(|state| needs_update(item, state))
        })
        .flat_map(|item| ids.get(&item.id).into_iter().flatten().copied())
        .collect()
}

/// Required items referenced by `items` that were not part of the query.
pub fn unknown_required(items: &[Item]) -> Vec<u64> {
    let known: HashSet<u64> = items.iter().map(|item| item.id).collect();
    let mut missing: Vec<u64> = items
        .iter()
        .flat_map(|item| item.required.iter().copied())
        .filter(|id| !known.contains(id))
        .collect();
    missing.sort_unstable();
    missing.dedup();
    missing
}
