//! Small graph helpers for `apply`: transitive reachability and which items an order change moved.

/// Everything each node must precede, following rules through as many hops as they chain. The
/// closure matters because a rule can be implied: with "p before u" and "u before s", p must also
/// precede s, or a placement that only knew stated pairs could leave no legal slot for u.
pub(super) struct Reach {
    words: usize,
    bits: Vec<u64>,
}

impl Reach {
    pub(super) fn new(nodes: usize, successors: &[Vec<usize>]) -> Self {
        let words = nodes.div_ceil(64);
        let mut bits = vec![0; nodes * words];
        let mut stack = Vec::new();
        for start in 0..nodes {
            let row = &mut bits[start * words..(start + 1) * words];
            stack.extend_from_slice(&successors[start]);
            // The visited check is also what stops a hand-built cyclic input from looping forever.
            while let Some(node) = stack.pop() {
                let (word, mask) = (node / 64, 1u64 << (node % 64));
                if row[word] & mask == 0 {
                    row[word] |= mask;
                    stack.extend_from_slice(&successors[node]);
                }
            }
        }
        Self { words, bits }
    }

    /// Whether `from` must be placed before `to`.
    pub(super) fn reaches(&self, from: usize, to: usize) -> bool {
        self.bits[from * self.words + to / 64] & (1u64 << (to % 64)) != 0
    }
}

/// Positions (into the original order) that are not part of the heaviest subsequence keeping its
/// original relative order, in result order. Anchors weigh more than all movers together, so the
/// answer names the movers that really changed place, not ones a neighbour merely stepped over.
pub(super) fn moved_items(result: &[usize], is_mover: impl Fn(usize) -> bool) -> Vec<usize> {
    let len = result.len();
    let anchor_weight = len as u64 + 1;
    // Fenwick tree of (best chain weight, result index ending it) over original positions.
    let mut tree: Vec<(u64, usize)> = vec![(0, usize::MAX); len + 1];
    let mut previous = vec![usize::MAX; len];
    let mut best = (0, usize::MAX);
    for (index, &item) in result.iter().enumerate() {
        let mut found = (0, usize::MAX);
        let mut cursor = item;
        while cursor > 0 {
            found = found.max(tree[cursor]);
            cursor &= cursor - 1;
        }
        let weight = if is_mover(item) { 1 } else { anchor_weight };
        let entry = (found.0 + weight, index);
        previous[index] = found.1;
        best = best.max(entry);
        let mut cursor = item + 1;
        while cursor <= len {
            tree[cursor] = tree[cursor].max(entry);
            cursor += cursor.isolate_lowest_one();
        }
    }
    let mut kept = vec![false; len];
    let mut cursor = best.1;
    while cursor != usize::MAX {
        kept[cursor] = true;
        cursor = previous[cursor];
    }
    result
        .iter()
        .zip(kept)
        .filter_map(|(&item, kept)| (!kept).then_some(item))
        .collect()
}

/// Whether `to` can already be reached from `from`, i.e. whether adding `to -> from` would loop.
pub(super) fn reaches(successors: &[Vec<usize>], from: usize, to: usize) -> bool {
    let mut seen = vec![false; successors.len()];
    let mut stack = vec![from];
    while let Some(node) = stack.pop() {
        if node == to {
            return true;
        }
        if !std::mem::replace(&mut seen[node], true) {
            stack.extend_from_slice(&successors[node]);
        }
    }
    false
}
