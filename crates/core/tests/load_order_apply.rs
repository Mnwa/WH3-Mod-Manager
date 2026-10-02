#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::collections::HashSet;
use wh3_core::load_order::{self, Applied, Rule};

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| format!("{name}.pack")).collect()
}

fn resolve_all(user: &[Rule], pack: &[Rule], mods: &[String]) -> Vec<Rule> {
    let present = mods.iter().cloned().collect();
    load_order::resolve(user, pack, &HashSet::new(), &HashSet::new(), &present).accepted
}

/// Applies to the identity order with everything enabled unless stated otherwise.
fn apply_to(
    mods: &[String],
    order: &[usize],
    rules: &[Rule],
    disabled: &[usize],
    pinned: &[usize],
) -> Applied {
    let enabled = order
        .iter()
        .copied()
        .filter(|index| !disabled.contains(index))
        .collect();
    let pinned = pinned.iter().copied().collect();
    load_order::apply(
        order,
        |index| mods[index].as_str(),
        &enabled,
        rules,
        &pinned,
    )
}

fn order_names(mods: &[String], order: &[usize]) -> Vec<String> {
    order
        .iter()
        .map(|&index| mods[index].trim_end_matches(".pack").to_owned())
        .collect()
}
#[test]
fn apply_moves_only_the_subject_to_the_nearest_legal_slot() {
    let mods = names(&["a", "b", "c", "d", "e"]);
    let order: Vec<usize> = (0..5).collect();
    let applied = apply_to(&mods, &order, &[Rule::pack("e", "e", "b")], &[], &[]);
    assert_eq!(
        order_names(&mods, &applied.order),
        ["a", "e", "b", "c", "d"]
    );
    assert_eq!(applied.moved, vec![4]);

    // The same pair stated from b's side moves b instead.
    let applied = apply_to(&mods, &order, &[Rule::pack("b", "e", "b")], &[], &[]);
    assert_eq!(
        order_names(&mods, &applied.order),
        ["a", "c", "d", "e", "b"]
    );
    assert_eq!(applied.moved, vec![1]);

    let again = apply_to(
        &mods,
        &applied.order,
        &[Rule::pack("b", "e", "b")],
        &[],
        &[],
    );
    assert_eq!(again.order, applied.order);
    assert!(again.moved.is_empty());
}

#[test]
fn apply_follows_implied_rules_and_case_insensitive_names() {
    let mods = names(&["S", "x", "U", "y", "P"]);
    let order: Vec<usize> = (0..5).collect();
    let rules = [Rule::pack("p", "p", "u"), Rule::pack("u", "U", "s")];
    let applied = apply_to(&mods, &order, &rules, &[], &[]);
    assert_eq!(
        order_names(&mods, &applied.order),
        ["P", "U", "S", "x", "y"]
    );
    assert!(applied.overridden_by_pin.is_empty());
}

#[test]
fn apply_ignores_disabled_mods_and_never_moves_them() {
    let mods = names(&["a", "b", "c", "d"]);
    let order: Vec<usize> = (0..4).collect();
    let rule = [Rule::pack("d", "d", "b")];
    let applied = apply_to(&mods, &order, &rule, &[1], &[]);
    assert_eq!(applied.order, order, "the rule names a disabled mod");

    let rule = [Rule::pack("d", "d", "a")];
    let applied = apply_to(&mods, &order, &rule, &[1], &[]);
    assert_eq!(order_names(&mods, &applied.order), ["d", "a", "b", "c"]);
    assert_eq!(applied.moved, vec![3]);
}

#[test]
fn apply_uses_the_first_enabled_copy_of_a_duplicated_name() {
    let mods = vec![
        "a.pack".to_owned(),
        "b.pack".to_owned(),
        "A.pack".to_owned(),
    ];
    let applied = apply_to(&mods, &[0, 1, 2], &[Rule::pack("b", "b", "a")], &[], &[]);
    assert_eq!(applied.order, [1, 0, 2]);
    let applied = apply_to(&mods, &[0, 1, 2], &[Rule::pack("b", "b", "a")], &[0], &[]);
    assert_eq!(
        applied.order,
        [0, 1, 2],
        "the enabled copy already loads after b"
    );
}

#[test]
fn pinned_mods_stay_put_and_the_other_end_gives_way() {
    let mods = names(&["a", "b", "c"]);
    // a is the subject but pinned, so b moves above it instead.
    let applied = apply_to(&mods, &[0, 1, 2], &[Rule::pack("a", "b", "a")], &[], &[0]);
    assert_eq!(order_names(&mods, &applied.order), ["b", "a", "c"]);
    assert!(applied.overridden_by_pin.is_empty());
}

#[test]
fn rules_that_would_move_a_pin_are_reported_not_fixed() {
    let mods = names(&["p", "a", "b", "q"]);
    let order: Vec<usize> = (0..4).collect();
    let both_pinned = [Rule::user("q", "p", "q")];
    let applied = apply_to(&mods, &order, &both_pinned, &[], &[0, 3]);
    assert_eq!(applied.order, order);
    assert_eq!(applied.overridden_by_pin, both_pinned.to_vec());

    // q before a before b before p cannot hold with p pinned above q; the unpinned pair still does.
    let chain = [
        Rule::user("q", "a", "a"),
        Rule::user("a", "b", "a"),
        Rule::user("b", "p", "b"),
    ];
    let applied = apply_to(&mods, &order, &chain, &[], &[0, 3]);
    let names = order_names(&mods, &applied.order);
    let at = |name: &str| names.iter().position(|n| n == name).unwrap();
    assert!(at("p") < at("q") && at("a") < at("b"));
    assert!(!applied.overridden_by_pin.is_empty());
    assert!(!applied.overridden_by_pin.contains(&chain[1]));
    let again = apply_to(&mods, &applied.order, &chain, &[], &[0, 3]);
    assert_eq!(again.order, applied.order);
}

#[test]
fn unresolved_cyclic_rules_still_terminate() {
    let mods = names(&["a", "b", "c"]);
    let cyclic = [
        Rule::user("a", "b", "a"),
        Rule::user("b", "c", "b"),
        Rule::user("c", "a", "c"),
    ];
    let applied = apply_to(&mods, &[2, 1, 0], &cyclic, &[], &[]);
    let mut sorted = applied.order.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, [0, 1, 2]);
}

/// xorshift64*: deterministic, dependency-free randomness for the property test.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
}

#[test]
fn random_orders_satisfy_every_rule_between_unpinned_enabled_mods() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..3000 {
        let count = 2 + rng.below(11);
        let mods: Vec<String> = (0..count).map(|i| format!("m{i}.pack")).collect();
        let mut order: Vec<usize> = (0..count).collect();
        for i in (1..count).rev() {
            order.swap(i, rng.below(i + 1));
        }
        let disabled: Vec<usize> = (0..count).filter(|_| rng.chance(15)).collect();
        // Half the cases have no pins, where every accepted rule must hold.
        let pin_percent = if rng.chance(50) { 0 } else { 25 };
        let pinned: Vec<usize> = (0..count).filter(|_| rng.chance(pin_percent)).collect();
        let (mut user, mut pack) = (Vec::new(), Vec::new());
        for _ in 0..rng.below(count * 2) {
            let (a, b) = (&mods[rng.below(count)], &mods[rng.below(count)]);
            if rng.chance(30) {
                user.push(Rule::user(a, b, if rng.chance(50) { a } else { b }));
            } else if rng.chance(50) {
                pack.push(Rule::pack(a, a, b));
            } else {
                pack.push(Rule::pack(b, a, b));
            }
        }
        let rules = resolve_all(&user, &pack, &mods);
        let applied = apply_to(&mods, &order, &rules, &disabled, &pinned);

        let mut sorted = applied.order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..count).collect::<Vec<_>>());
        let position = |index: usize| applied.order.iter().position(|&i| i == index).unwrap();
        let index_of = |name: &str| mods.iter().position(|m| m == name).unwrap();
        let mut broken = Vec::new();
        for rule in &rules {
            let (before, after) = (index_of(&rule.before), index_of(&rule.after));
            if disabled.contains(&before) || disabled.contains(&after) {
                continue;
            }
            if position(before) > position(after) {
                assert!(
                    pinned.contains(&before) || pinned.contains(&after),
                    "a rule between unpinned mods broke: {rule:?} in {:?}",
                    order_names(&mods, &applied.order)
                );
                broken.push(rule.clone());
            }
        }
        assert_eq!(applied.overridden_by_pin, broken);

        let kept_in_place = |sequence: &[usize], filter: &dyn Fn(usize) -> bool| -> Vec<usize> {
            sequence.iter().copied().filter(|&i| filter(i)).collect()
        };
        let is_pinned = |i: usize| pinned.contains(&i);
        assert_eq!(
            kept_in_place(&applied.order, &is_pinned),
            kept_in_place(&order, &is_pinned)
        );
        let unmoved = |i: usize| !applied.moved.contains(&i);
        assert_eq!(
            kept_in_place(&applied.order, &unmoved),
            kept_in_place(&order, &unmoved)
        );

        let again = apply_to(&mods, &applied.order, &rules, &disabled, &pinned);
        assert_eq!(again.order, applied.order, "apply must be idempotent");
    }
}
