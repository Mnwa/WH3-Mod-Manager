//! Dependency, Steam requirement and start position checks over the enabled mods.
use super::{Availability, MissingDependency, MissingRequired, names};
use crate::catalog::Catalog;
use std::collections::{HashMap, HashSet};

/// Enabled mods in priority order, indexed by normalized pack name.
pub(super) struct Active<'a> {
    pub(super) catalog: &'a Catalog,
    pub(super) mods: Vec<usize>,
    pub(super) members: HashSet<usize>,
    pub(super) by_name: HashMap<String, usize>,
}

impl Active<'_> {
    /// A disabled catalog mod that would satisfy the requirement, otherwise "not installed".
    fn availability(&self, candidates: impl IntoIterator<Item = usize>) -> Availability {
        candidates
            .into_iter()
            .find(|candidate| !self.members.contains(candidate))
            .map_or(Availability::NotInstalled, Availability::Disabled)
    }

    pub(super) fn missing_dependencies(&self) -> Vec<MissingDependency> {
        let mut missing = Vec::new();
        for &index in &self.mods {
            for dependency in &self.catalog.mods[index].dependencies {
                let name = names::normalize_pack_name(dependency);
                if self.by_name.contains_key(&name) {
                    continue;
                }
                let candidates = self.catalog.by_name.get(&name).into_iter().flatten();
                missing.push(MissingDependency {
                    owner: index,
                    dependency: dependency.clone(),
                    availability: self.availability(candidates.copied()),
                });
            }
        }
        missing
    }

    /// A requirement is met by an enabled mod with that Workshop id, or by an enabled pack with
    /// the same name as an installed copy of that item (a Workshop mod copied into `data`).
    pub(super) fn missing_required(&self) -> Vec<MissingRequired> {
        let mut by_workshop_id: HashMap<&str, Vec<usize>> = HashMap::new();
        for (index, item) in self.catalog.mods.iter().enumerate() {
            if !item.workshop_id.is_empty() {
                by_workshop_id
                    .entry(&item.workshop_id)
                    .or_default()
                    .push(index);
            }
        }
        let mut missing = Vec::new();
        for &index in &self.mods {
            for (id, name) in &self.catalog.mods[index].metadata.req_mod_id_to_name {
                let installed = by_workshop_id.get(id.as_str()).map(Vec::as_slice);
                let satisfied = installed.unwrap_or_default().iter().any(|&candidate| {
                    self.members.contains(&candidate)
                        || self
                            .by_name
                            .contains_key(&self.catalog.mods[candidate].name.to_lowercase())
                });
                if satisfied {
                    continue;
                }
                missing.push(MissingRequired {
                    owner: index,
                    workshop_id: id.clone(),
                    name: name.clone(),
                    availability: self.availability(installed.unwrap_or_default().iter().copied()),
                });
            }
        }
        missing
    }

    /// Start position packs that conflict without an ordering: two packs connected by a direct
    /// or transitive dependency are intentionally ordered and do not conflict with each other.
    pub(super) fn startpos_conflicts(&self, startpos: &[usize]) -> Vec<usize> {
        let mut conflicting = HashSet::new();
        for (position, &first) in startpos.iter().enumerate() {
            for &second in &startpos[position + 1..] {
                if !self.depends_on(first, second) && !self.depends_on(second, first) {
                    conflicting.extend([first, second]);
                }
            }
        }
        startpos
            .iter()
            .copied()
            .filter(|index| conflicting.contains(index))
            .collect()
    }

    fn depends_on(&self, source: usize, target: usize) -> bool {
        let target = self.catalog.mods[target].name.to_lowercase();
        let mut pending: Vec<&String> = self.catalog.mods[source].dependencies.iter().collect();
        let mut visited = HashSet::new();
        while let Some(dependency) = pending.pop() {
            let name = names::normalize_pack_name(dependency);
            if name == target {
                return true;
            }
            if !visited.insert(name.clone()) {
                continue;
            }
            if let Some(&next) = self.by_name.get(&name) {
                pending.extend(&self.catalog.mods[next].dependencies);
            }
        }
        false
    }
}
