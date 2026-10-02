//! Reorders an explicit load order so it satisfies resolved rules, moving as little as possible
//! (`applyLoadOrderEdges`, `loadOrderRules.ts:397-446`).
use super::{Rule, constraints::Constraints, graph};
use std::collections::HashSet;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// The full input order (enabled and disabled mods) after the rules were applied.
    pub order: Vec<usize>,
    /// Catalog indices that changed place, in their new order. A mod that only shifted because a
    /// moved mod stepped over it is not listed.
    pub moved: Vec<usize>,
    /// Rules between two enabled mods that the order breaks because satisfying them would need a
    /// pinned mod to move. In input order.
    pub overridden_by_pin: Vec<Rule>,
}

/// Applies `rules` (normally [`super::Resolution::accepted`]) to `order`, a list of catalog indices.
///
/// Only enabled mods take part: a rule naming a disabled or absent pack is inert, and disabled mods
/// are never moved. When a name occurs more than once, the first enabled occurrence is the one
/// rules refer to. Like the original, only rule subjects are moved at first; a rule that still
/// breaks promotes its ends to movers and the pass repeats. Each mover is put back as close to its
/// old position as the rules allow, so unconstrained mods keep their relative order.
///
/// Pinned mods never move: their order relative to each other and to every other non-moving mod
/// is kept, while unpinned mods may be placed on either side of them. Rules that cannot hold
/// without moving a pin are reported instead of fixed, because manual placement wins by design.
/// Every rule between two enabled unpinned mods holds afterwards, as long as `rules` is acyclic
/// (cyclic input still terminates, without that guarantee).
///
/// Which rules lose to pins depends only on the rules and the pins' relative order, never on where
/// unpinned mods happen to be, and an order that breaks no other rule is returned unchanged. That
/// is what makes applying twice a no-op.
pub fn apply<'a>(
    order: &[usize],
    names: impl Fn(usize) -> &'a str,
    enabled: &HashSet<usize>,
    rules: &[Rule],
    pinned: &HashSet<usize>,
) -> Applied {
    let constraints = Constraints::new(order, names, enabled, rules, pinned);
    let identity: Vec<usize> = (0..order.len()).collect();
    if constraints.violated(&identity).is_empty() {
        return Applied {
            order: order.to_vec(),
            moved: Vec::new(),
            overridden_by_pin: overridden(&constraints, &identity, rules),
        };
    }

    let mut movers = constraints.initial_movers();
    let mut result = constraints.place(&movers);
    loop {
        let violated = constraints.violated(&result);
        if violated.is_empty() {
            break;
        }
        let ends = violated.iter().flat_map(|&(from, to, _)| [from, to]);
        // Moving only the subjects is not always enough: "a after m2" plus "a before x1" implies
        // m2 before x1, and neither is a subject. Promoting every unpinned constrained mod is the
        // last resort that always works once the pins are consistent with the rules.
        if !constraints.promote(&mut movers, ends)
            && !constraints.promote(&mut movers, 0..constraints.node_count())
        {
            break;
        }
        result = constraints.place(&movers);
    }

    let moved = graph::moved_items(&result, |item| constraints.is_mover(item, &movers));
    Applied {
        order: result.iter().map(|&item| order[item]).collect(),
        moved: moved.into_iter().map(|item| order[item]).collect(),
        overridden_by_pin: overridden(&constraints, &result, rules),
    }
}

/// Rules set aside for the pins that the arrangement actually breaks; one can hold by chance.
fn overridden(constraints: &Constraints, result: &[usize], rules: &[Rule]) -> Vec<Rule> {
    constraints
        .overridden_violated(result)
        .into_iter()
        .map(|rule| rules[rule].clone())
        .collect()
}
