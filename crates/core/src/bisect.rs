//! Finding the mod behind a problem by halving the suspects, like the original's
//! "Bisect Mod List", but as a guided search instead of a pile of presets.
//!
//! Mods that require each other stay in the same half, so a test never fails only
//! because a requirement was switched off.
use crate::catalog::Catalog;
use std::collections::{HashMap, HashSet};

/// Suspects grouped so that requirement-linked mods share a group; groups keep the
/// order of `suspects`.
pub fn groups(catalog: &Catalog, suspects: &[usize]) -> Vec<Vec<usize>> {
    let members: HashSet<usize> = suspects.iter().copied().collect();
    let mut by_workshop: HashMap<&str, usize> = HashMap::new();
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for &index in suspects {
        let item = &catalog.mods[index];
        if !item.workshop_id.is_empty() {
            by_workshop.insert(&item.workshop_id, index);
        }
        by_name.insert(item.name.to_lowercase(), index);
    }
    let mut links: HashMap<usize, Vec<usize>> = HashMap::new();
    for &index in suspects {
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
        for other in required.chain(packs).filter(|other| *other != index) {
            links.entry(index).or_default().push(other);
            links.entry(other).or_default().push(index);
        }
    }
    let rank: HashMap<usize, usize> = suspects.iter().enumerate().map(|(i, &m)| (m, i)).collect();
    let mut seen = HashSet::new();
    let mut groups = Vec::new();
    for &start in suspects {
        if !seen.insert(start) {
            continue;
        }
        let mut group = vec![start];
        let mut stack = vec![start];
        while let Some(current) = stack.pop() {
            for &next in links.get(&current).into_iter().flatten() {
                if members.contains(&next) && seen.insert(next) {
                    group.push(next);
                    stack.push(next);
                }
            }
        }
        group.sort_by_key(|member| rank.get(member).copied().unwrap_or(usize::MAX));
        groups.push(group);
    }
    groups
}

/// Split whole groups into a tested half and the rest, as close to half the mods as
/// possible. `None` when the suspects cannot be split any further.
pub fn split(groups: &[Vec<usize>]) -> Option<(Vec<usize>, Vec<usize>)> {
    if groups.len() < 2 {
        return None;
    }
    let total: usize = groups.iter().map(Vec::len).sum();
    let target = total.div_ceil(2);
    // The boundary after which the running count is closest to the target; ties take
    // the larger first half, like the original. Both halves stay non-empty.
    let mut best = (usize::MAX, 0, 1);
    let mut count = 0;
    for (boundary, group) in groups.iter().enumerate().take(groups.len() - 1) {
        count += group.len();
        let distance = count.abs_diff(target);
        if distance < best.0 || (distance == best.0 && count > best.1) {
            best = (distance, count, boundary + 1);
        }
    }
    let (first, rest) = groups.split_at(best.2);
    Some((first.concat(), rest.concat()))
}

/// The state of one search.
#[derive(Clone, Debug)]
pub struct Search {
    /// Mods that may still cause the problem, in load order.
    pub suspects: Vec<usize>,
    /// The half enabled for the current test.
    pub testing: Vec<usize>,
    /// The other half, switched off for the current test.
    pub resting: Vec<usize>,
    pub step: usize,
}

impl Search {
    /// Start with the given suspects; `None` when there is nothing to narrow down.
    pub fn start(catalog: &Catalog, suspects: Vec<usize>) -> Option<Self> {
        let (testing, resting) = split(&groups(catalog, &suspects))?;
        Some(Self {
            suspects,
            testing,
            resting,
            step: 1,
        })
    }

    /// Record the test result. Returns `false` once the culprit is narrowed down to
    /// `suspects`, which can no longer be split.
    pub fn answer(&mut self, catalog: &Catalog, problem_remains: bool) -> bool {
        self.suspects = if problem_remains {
            std::mem::take(&mut self.testing)
        } else {
            std::mem::take(&mut self.resting)
        };
        match split(&groups(catalog, &self.suspects)) {
            Some((testing, resting)) => {
                self.testing = testing;
                self.resting = resting;
                self.step += 1;
                true
            }
            None => false,
        }
    }

    /// Tests still needed in the worst case after the current one.
    pub fn remaining_steps(&self) -> usize {
        let larger = self.testing.len().max(self.resting.len());
        usize::BITS as usize - larger.saturating_sub(1).leading_zeros() as usize
    }
}
