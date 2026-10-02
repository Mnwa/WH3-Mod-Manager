//! Finding the mod behind a problem by halving the suspects, like the original's
//! "Bisect Mod List", but as a guided search instead of a pile of presets.
//!
//! Each test enables part of the suspects together with every mod they require, so a
//! test never fails only because a requirement was switched off. Requirements travel
//! with the mods that need them instead of welding linked mods into one group: in a
//! large list most mods are linked through a few frameworks or overhauls, and such a
//! group could never be split, so the search used to "find" half of the list at once.
use crate::catalog::Catalog;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// Requirements of each mod in `pool`, limited to `pool`: Workshop requirements and pack
/// dependencies. Mods outside the pool were not enabled before the search, so the
/// search must not switch them on.
pub fn requirements(catalog: &Catalog, pool: &[usize]) -> HashMap<usize, Vec<usize>> {
    let mut by_workshop: HashMap<&str, usize> = HashMap::new();
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for &index in pool {
        let item = &catalog.mods[index];
        if !item.workshop_id.is_empty() {
            by_workshop.insert(&item.workshop_id, index);
        }
        by_name.insert(item.name.to_lowercase(), index);
    }
    let mut result = HashMap::new();
    for &index in pool {
        let item = &catalog.mods[index];
        let required = item
            .metadata
            .req_mod_id_to_name
            .iter()
            .filter_map(|(id, _)| by_workshop.get(id.as_str()).copied());
        let packs = item
            .dependencies
            .iter()
            .filter_map(|name| by_name.get(&name.to_lowercase()).copied());
        let mut needs: Vec<usize> = required.chain(packs).filter(|&m| m != index).collect();
        needs.sort_unstable();
        needs.dedup();
        if !needs.is_empty() {
            result.insert(index, needs);
        }
    }
    result
}

/// `mods` plus everything they require, directly or through other mods.
pub fn closure(
    requirements: &HashMap<usize, Vec<usize>>,
    mods: impl IntoIterator<Item = usize>,
) -> HashSet<usize> {
    let mut result = HashSet::new();
    let mut stack: Vec<usize> = mods.into_iter().collect();
    while let Some(index) = stack.pop() {
        if result.insert(index) {
            stack.extend(requirements.get(&index).into_iter().flatten().copied());
        }
    }
    result
}

/// The state of one search.
#[derive(Clone, Debug)]
pub struct Search {
    /// Mods that may still cause the problem, in load order.
    pub suspects: Vec<usize>,
    /// Suspects enabled for the current test, in load order.
    pub testing: Vec<usize>,
    /// Suspects switched off for the current test, in load order.
    pub resting: Vec<usize>,
    /// Every mod to enable for the current test: `testing` plus the cleared mods they
    /// require. Once the search is over, the culprit with its requirements.
    pub enabled: Vec<usize>,
    pub step: usize,
    /// Every mod in the search, in load order, and what each of them requires.
    pool: Arc<Vec<usize>>,
    requirements: Arc<HashMap<usize, Vec<usize>>>,
}

impl Search {
    /// Start with the given suspects (the enabled mods, in load order); `None` when
    /// there is nothing to narrow down.
    pub fn start(catalog: &Catalog, suspects: Vec<usize>) -> Option<Self> {
        let requirements = Arc::new(requirements(catalog, &suspects));
        let mut search = Self {
            testing: vec![],
            resting: vec![],
            enabled: vec![],
            step: 1,
            pool: Arc::new(suspects.clone()),
            suspects,
            requirements,
        };
        search.split().then_some(search)
    }

    /// Record the test result. Returns `false` once the culprit is narrowed down to
    /// `suspects`, which can no longer be split.
    pub fn answer(&mut self, problem_remains: bool) -> bool {
        self.suspects = if problem_remains {
            std::mem::take(&mut self.testing)
        } else {
            std::mem::take(&mut self.resting)
        };
        if self.split() {
            self.step += 1;
            return true;
        }
        self.testing.clear();
        self.resting.clear();
        let enabled = closure(&self.requirements, self.suspects.iter().copied());
        self.enabled = self.in_load_order(&enabled);
        false
    }

    /// Choose the suspects for the next test: about half of them, each brought in with
    /// its requirements. `false` when no test can tell the suspects apart, which only
    /// happens for a single mod or mods that require each other.
    fn split(&mut self) -> bool {
        let members: HashSet<usize> = self.suspects.iter().copied().collect();
        let suspects_in = |set: &HashSet<usize>| set.iter().filter(|m| members.contains(m)).count();
        let total = self.suspects.len();
        let target = total.div_ceil(2);
        let mut enabled = HashSet::new();
        let mut count = 0;
        for &index in &self.suspects {
            if enabled.contains(&index) {
                continue;
            }
            let mut added = closure(&self.requirements, [index]);
            added.retain(|m| !enabled.contains(m));
            let new = suspects_in(&added);
            if count + new <= target && count + new < total {
                enabled.extend(added);
                count += new;
            }
        }
        if count == 0 {
            // Every suspect needs more than half of the others: test the smallest
            // requirement chain that still leaves someone out.
            let Some(smallest) = self
                .suspects
                .iter()
                .map(|&index| closure(&self.requirements, [index]))
                .filter(|set| suspects_in(set) < total)
                .min_by_key(|set| suspects_in(set))
            else {
                return false;
            };
            enabled = smallest;
        }
        (self.testing, self.resting) = self.suspects.iter().partition(|m| enabled.contains(m));
        self.enabled = self.in_load_order(&enabled);
        true
    }

    fn in_load_order(&self, set: &HashSet<usize>) -> Vec<usize> {
        self.pool
            .iter()
            .copied()
            .filter(|m| set.contains(m))
            .collect()
    }

    /// Mods in the search that need a suspect, directly or through other mods; they
    /// cannot work once the suspects are switched off.
    pub fn dependents(&self) -> Vec<usize> {
        let suspects: HashSet<usize> = self.suspects.iter().copied().collect();
        self.pool
            .iter()
            .copied()
            .filter(|index| !suspects.contains(index))
            .filter(|&index| {
                closure(&self.requirements, [index])
                    .iter()
                    .any(|m| suspects.contains(m))
            })
            .collect()
    }

    /// Follow a rescan that renumbered the catalog. The current test stays as it is so
    /// the next answer still describes it; mods that are gone are dropped.
    pub fn remap(&mut self, new_index: impl Fn(usize) -> Option<usize>) {
        let map = |list: &[usize]| {
            list.iter()
                .filter_map(|&m| new_index(m))
                .collect::<Vec<_>>()
        };
        self.suspects = map(&self.suspects);
        self.testing = map(&self.testing);
        self.resting = map(&self.resting);
        self.enabled = map(&self.enabled);
        self.pool = Arc::new(map(&self.pool));
        self.requirements = Arc::new(
            self.requirements
                .iter()
                .filter_map(|(&m, needs)| Some((new_index(m)?, map(needs))))
                .collect(),
        );
    }

    /// Tests still needed in the worst case after the current one.
    pub fn remaining_steps(&self) -> usize {
        let larger = self.testing.len().max(self.resting.len());
        usize::BITS as usize - larger.saturating_sub(1).leading_zeros() as usize
    }
}
