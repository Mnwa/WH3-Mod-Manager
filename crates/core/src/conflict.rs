use crate::{Error, Result, catalog::Catalog, pack};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub struct Report {
    pub collisions: Vec<Collision>,
    pub warnings: Vec<String>,
}
pub struct Collision {
    pub file: String,
    pub mods: Vec<usize>,
}

pub fn check(
    catalog: &Catalog,
    order: &[usize],
    enabled: &HashSet<usize>,
    cancelled: &AtomicBool,
) -> Result<Report> {
    let mut owners: HashMap<String, usize> = HashMap::new();
    let mut collisions: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut report = Report::default();
    let enabled_names: HashSet<_> = enabled
        .iter()
        .filter_map(|&i| catalog.mods.get(i))
        .map(|item| item.name.to_lowercase())
        .collect();
    for &index in order.iter().filter(|i| enabled.contains(i)) {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let item = &catalog.mods[index];
        for dependency in &item.dependencies {
            if !enabled_names.contains(&dependency.to_lowercase()) {
                report.warnings.push(format!(
                    "{}: не включена зависимость {dependency}",
                    item.name
                ));
            }
        }
        match pack::index(&item.path) {
            Ok(files) => {
                for file in files {
                    if cancelled.load(Ordering::Relaxed) {
                        return Err(Error::Cancelled);
                    }
                    let name = file.name.replace('/', "\\").to_lowercase();
                    if let Some(&owner) = owners.get(&name) {
                        if owner != index {
                            let mods = collisions.entry(name).or_insert_with(|| vec![owner]);
                            if mods.last() != Some(&index) {
                                mods.push(index);
                            }
                        }
                    } else {
                        owners.insert(name, index);
                    }
                }
            }
            Err(e) => report.warnings.push(format!("{}: {e}", item.name)),
        }
    }
    report.collisions = collisions
        .into_iter()
        .map(|(file, mods)| Collision { file, mods })
        .collect();
    Ok(report)
}
