//! Turns user and pack rules into one consistent, cycle-free rule set (`resolveLoadOrderRules`,
//! `loadOrderRules.ts:191-298`).
use super::{Rule, RuleKey, RuleSource, pack_key};
use std::collections::{HashMap, HashSet};

/// Why a rule is not in effect.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DropReason {
    /// The user switched this pack rule off.
    Disabled,
    /// The user ignores every rule its source pack ships, including future ones.
    DisabledPack,
    /// An empty pack name, or a pack ordered against itself.
    Invalid,
    /// This pack is not present, so there is nothing to order.
    MissingPack(String),
    /// A user rule about the same pair (either direction) replaced it. Not a problem.
    OverriddenByUser,
    /// Packs demand opposite orders for the same pair, so neither direction is applied.
    Contradiction,
    /// Honouring it would make the order loop back on itself.
    Cycle,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resolution {
    /// Rules in effect, in the deterministic order they were accepted.
    pub accepted: Vec<Rule>,
    /// Every other input rule with the first reason it fell out, grouped by resolution stage.
    pub dropped: Vec<(Rule, DropReason)>,
}

type PairKey = (String, String);

/// Resolves the rules deterministically, like the original:
///
/// 1. Pack rules from `disabled_packs` or listed in `disabled_rules` are dropped first, so a rule
///    the user muted can never cause a cycle or a contradiction.
/// 2. Rules naming a pack missing from `present` are dropped (the original passes every known
///    pack here, enabled or not: `appSlice.ts:57-59`).
/// 3. One rule per unordered pair: a user rule replaces any pack rule about the pair.
/// 4. Pack rules disagreeing about a pair all cancel out; unanimous ones are all kept.
/// 5. User rules first, then pack rules ordered by source, before and after key, are accepted one
///    at a time; a rule that would close a cycle with already accepted ones is dropped.
///
/// The `source` field decides how a rule is treated, so a `User` rule passed in `pack` acts as a
/// user rule. Exact duplicates (same [`RuleKey`]) are folded into their first occurrence.
pub fn resolve(
    user: &[Rule],
    pack: &[Rule],
    disabled_rules: &HashSet<RuleKey>,
    disabled_packs: &HashSet<String>,
    present: &HashSet<String>,
) -> Resolution {
    let present: HashSet<String> = present.iter().map(|name| pack_key(name)).collect();
    let disabled_packs: HashSet<String> =
        disabled_packs.iter().map(|name| pack_key(name)).collect();
    let mut dropped = Vec::new();

    let mut seen = HashSet::new();
    let mut user_rules = Vec::new();
    let mut live_pack_rules = Vec::new();
    for rule in user.iter().chain(pack) {
        let key = rule.key();
        if !seen.insert(key.clone()) {
            continue;
        }
        match &rule.source {
            RuleSource::User => user_rules.push(rule),
            RuleSource::Pack(source) if disabled_packs.contains(&pack_key(source)) => {
                dropped.push((rule.clone(), DropReason::DisabledPack));
            }
            RuleSource::Pack(_) if disabled_rules.contains(&key) => {
                dropped.push((rule.clone(), DropReason::Disabled));
            }
            RuleSource::Pack(_) => live_pack_rules.push(rule),
        }
    }

    let mut usable = |rule: &Rule| match unusable_reason(rule, &present) {
        Some(reason) => {
            dropped.push((rule.clone(), reason));
            false
        }
        None => true,
    };
    user_rules.retain(|rule| usable(rule));
    live_pack_rules.retain(|rule| usable(rule));

    // A later user rule about the same pair replaces an earlier one, as re-stating a pair does in
    // the original's `addLoadOrderRule`.
    let mut candidates: Vec<&Rule> = Vec::new();
    let mut user_pairs: HashMap<PairKey, usize> = HashMap::new();
    for rule in user_rules {
        match user_pairs.get(&pair_key(rule)) {
            Some(&slot) => {
                dropped.push((candidates[slot].clone(), DropReason::OverriddenByUser));
                candidates[slot] = rule;
            }
            None => {
                user_pairs.insert(pair_key(rule), candidates.len());
                candidates.push(rule);
            }
        }
    }

    let mut groups: Vec<Vec<&Rule>> = Vec::new();
    let mut group_of_pair: HashMap<PairKey, usize> = HashMap::new();
    for rule in live_pack_rules {
        let pair = pair_key(rule);
        if user_pairs.contains_key(&pair) {
            dropped.push((rule.clone(), DropReason::OverriddenByUser));
            continue;
        }
        let group = *group_of_pair.entry(pair).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[group].push(rule);
    }
    for group in groups {
        let direction = pack_key(&group[0].before);
        if group.iter().all(|rule| pack_key(&rule.before) == direction) {
            candidates.extend(group);
        } else {
            dropped.extend(
                group
                    .into_iter()
                    .map(|rule| (rule.clone(), DropReason::Contradiction)),
            );
        }
    }

    // A fixed acceptance order keeps the rule dropped to break a cycle the same between runs.
    candidates.sort_by_cached_key(|rule| {
        let source = match &rule.source {
            RuleSource::User => None,
            RuleSource::Pack(name) => Some(pack_key(name)),
        };
        (source, pack_key(&rule.before), pack_key(&rule.after))
    });

    let mut successors: HashMap<String, Vec<String>> = HashMap::new();
    let mut accepted = Vec::new();
    for rule in candidates {
        let (before, after) = (pack_key(&rule.before), pack_key(&rule.after));
        if can_reach(&successors, &after, &before) {
            dropped.push((rule.clone(), DropReason::Cycle));
            continue;
        }
        let next = successors.entry(before).or_default();
        if !next.contains(&after) {
            next.push(after);
        }
        accepted.push(rule.clone());
    }
    Resolution { accepted, dropped }
}

fn unusable_reason(rule: &Rule, present: &HashSet<String>) -> Option<DropReason> {
    let (before, after) = (pack_key(&rule.before), pack_key(&rule.after));
    if before.is_empty() || after.is_empty() || before == after {
        return Some(DropReason::Invalid);
    }
    [(before, &rule.before), (after, &rule.after)]
        .into_iter()
        .find(|(key, _)| !present.contains(key))
        .map(|(_, name)| DropReason::MissingPack(name.clone()))
}

/// Both directions of a pair share one key, so a rule and its reverse collide deliberately.
fn pair_key(rule: &Rule) -> PairKey {
    let (before, after) = (pack_key(&rule.before), pack_key(&rule.after));
    if before <= after {
        (before, after)
    } else {
        (after, before)
    }
}

fn can_reach(successors: &HashMap<String, Vec<String>>, from: &str, to: &str) -> bool {
    let mut seen = HashSet::from([from]);
    let mut stack = vec![from];
    while let Some(current) = stack.pop() {
        if current == to {
            return true;
        }
        for next in successors.get(current).into_iter().flatten() {
            if seen.insert(next) {
                stack.push(next);
            }
        }
    }
    false
}
