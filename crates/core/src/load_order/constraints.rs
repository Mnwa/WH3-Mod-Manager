//! The rules of one `apply` call in terms of positions ("items") in the input order.
use super::{Rule, graph, pack_key};
use std::collections::{HashMap, HashSet};

/// One enabled pack some rule names: the first enabled occurrence of its name in the order.
struct Node {
    item: usize,
    pinned: bool,
    subject: bool,
}

/// An edge is (before node, after node, index into the rule slice).
type Edge = (usize, usize, usize);

pub(super) struct Constraints {
    node_of_item: Vec<Option<usize>>,
    nodes: Vec<Node>,
    /// Edges the arrangement must satisfy.
    edges: Vec<Edge>,
    /// Edges that would need a pin to move; reported, never enforced.
    overridden: Vec<Edge>,
    reach: graph::Reach,
}

impl Constraints {
    pub(super) fn new<'a>(
        order: &[usize],
        names: impl Fn(usize) -> &'a str,
        enabled: &HashSet<usize>,
        rules: &[Rule],
        pinned: &HashSet<usize>,
    ) -> Self {
        let mut item_of_key: HashMap<String, usize> = HashMap::new();
        for (item, &mod_index) in order.iter().enumerate() {
            if enabled.contains(&mod_index) {
                item_of_key
                    .entry(pack_key(names(mod_index)))
                    .or_insert(item);
            }
        }
        let mut node_of_item = vec![None; order.len()];
        let mut nodes = Vec::new();
        let mut node = |item: usize| {
            *node_of_item[item].get_or_insert_with(|| {
                nodes.push(Node {
                    item,
                    pinned: pinned.contains(&order[item]),
                    subject: false,
                });
                nodes.len() - 1
            })
        };
        let mut candidates = Vec::new();
        for (index, rule) in rules.iter().enumerate() {
            let ends = (
                item_of_key.get(&pack_key(&rule.before)),
                item_of_key.get(&pack_key(&rule.after)),
            );
            if let (Some(&before), Some(&after)) = ends
                && before != after
            {
                candidates.push((node(before), node(after), index));
            }
        }
        let mut constraints = Self {
            node_of_item,
            nodes,
            edges: Vec::new(),
            overridden: Vec::new(),
            reach: graph::Reach::new(0, &[]),
        };
        let successors = constraints.split_pin_conflicts(candidates);
        constraints.reach = graph::Reach::new(constraints.nodes.len(), &successors);
        constraints.mark_subjects(rules);
        constraints
    }

    /// Pins never move, so their current relative order is a fact the rules must fit around. It
    /// joins the graph as a chain; rules between unpinned mods go in next (they cannot form a cycle
    /// with that chain alone), then each rule touching a pin in rule order, dropping any that
    /// would close a cycle. The result depends only on the rules and the pins' relative order.
    fn split_pin_conflicts(&mut self, candidates: Vec<Edge>) -> Vec<Vec<usize>> {
        let mut successors = vec![Vec::new(); self.nodes.len()];
        let mut pins: Vec<usize> = (0..self.nodes.len())
            .filter(|&node| self.nodes[node].pinned)
            .collect();
        pins.sort_unstable_by_key(|&node| self.nodes[node].item);
        for pair in pins.windows(2) {
            successors[pair[0]].push(pair[1]);
        }
        let (free, pinned): (Vec<Edge>, Vec<Edge>) = candidates
            .into_iter()
            .partition(|&(from, to, _)| !self.nodes[from].pinned && !self.nodes[to].pinned);
        for edge in free {
            successors[edge.0].push(edge.1);
            self.edges.push(edge);
        }
        for edge in pinned {
            if graph::reaches(&successors, edge.1, edge.0) {
                self.overridden.push(edge);
            } else {
                successors[edge.0].push(edge.1);
                self.edges.push(edge);
            }
        }
        successors
    }

    /// The packs an enforced rule is about. A subject naming neither end could never satisfy its
    /// rule by moving, so both ends become movable instead of the rule silently doing nothing.
    fn mark_subjects(&mut self, rules: &[Rule]) {
        for &(from, to, index) in &self.edges {
            let rule = &rules[index];
            let subject = pack_key(&rule.subject);
            let is_before = subject == pack_key(&rule.before);
            let is_after = subject == pack_key(&rule.after);
            self.nodes[from].subject |= is_before || !is_after;
            self.nodes[to].subject |= is_after || !is_before;
        }
    }

    pub(super) fn initial_movers(&self) -> Vec<bool> {
        let movable = |node: &Node| node.subject && !node.pinned;
        self.nodes.iter().map(movable).collect()
    }

    pub(super) fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub(super) fn is_mover(&self, item: usize, movers: &[bool]) -> bool {
        self.node_of_item[item].is_some_and(|node| movers[node])
    }

    /// Marks the given unpinned nodes as movers; reports whether any was new.
    pub(super) fn promote(
        &self,
        movers: &mut [bool],
        nodes: impl IntoIterator<Item = usize>,
    ) -> bool {
        let mut promoted = false;
        for node in nodes {
            if !self.nodes[node].pinned && !movers[node] {
                movers[node] = true;
                promoted = true;
            }
        }
        promoted
    }

    /// One placement pass: movers are lifted out and reinserted in their old order, everything else
    /// keeps its relative place.
    pub(super) fn place(&self, movers: &[bool]) -> Vec<usize> {
        let len = self.node_of_item.len();
        let mut result: Vec<usize> = (0..len)
            .filter(|&item| !self.is_mover(item, movers))
            .collect();
        for base in 0..len {
            if let Some(mover) = self.node_of_item[base].filter(|&node| movers[node]) {
                let slot = self.slot(&result, mover, base);
                result.insert(slot, base);
            }
        }
        result
    }

    /// Where `mover` may sit among the items placed so far, as close to `base` as allowed.
    fn slot(&self, placed: &[usize], mover: usize, base: usize) -> usize {
        let (mut earliest, mut latest) = (0, placed.len());
        for (index, &item) in placed.iter().enumerate() {
            let Some(other) = self.node_of_item[item] else {
                continue;
            };
            if self.reach.reaches(other, mover) {
                earliest = earliest.max(index + 1);
            }
            if self.reach.reaches(mover, other) {
                latest = latest.min(index);
            }
        }
        // An impossible range only occurs while some constrained mod is not yet a mover; giving way
        // to the mods that must come first, like the original, lets the violation promote it.
        // Aiming for the old position means a mod that must precede something far down lands just
        // above it rather than at the very top.
        base.min(placed.len()).clamp(earliest, latest.max(earliest))
    }

    /// Enforced edges the arrangement breaks; `result` lists items in their new order.
    pub(super) fn violated(&self, result: &[usize]) -> Vec<Edge> {
        self.broken(&self.edges, result)
    }

    /// Rule indices of pin-overridden edges the arrangement breaks, in rule order.
    pub(super) fn overridden_violated(&self, result: &[usize]) -> Vec<usize> {
        let mut rules: Vec<usize> = self
            .broken(&self.overridden, result)
            .into_iter()
            .map(|edge| edge.2)
            .collect();
        rules.sort_unstable();
        rules
    }

    fn broken(&self, edges: &[Edge], result: &[usize]) -> Vec<Edge> {
        let mut position = vec![0; result.len()];
        for (index, &item) in result.iter().enumerate() {
            position[item] = index;
        }
        let item = |node: usize| self.nodes[node].item;
        edges
            .iter()
            .copied()
            .filter(|&(from, to, _)| position[item(from)] > position[item(to)])
            .collect()
    }
}
