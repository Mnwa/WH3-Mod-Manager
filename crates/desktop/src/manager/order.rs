//! Load-order changes. Only these methods mutate `order`; table sorting never does.
use super::Manager;
use gpui_kit::*;
use std::sync::Arc;

/// Payload of a row drag; the preview shows the mod title next to the cursor.
#[derive(Clone)]
pub(super) struct DraggedMod {
    pub index: usize,
    pub title: SharedString,
}

impl Render for DraggedMod {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_1()
            .rounded_md()
            .bg(crate::theme::selection())
            .border_1()
            .border_color(crate::theme::accent())
            .text_sm()
            .text_color(crate::theme::text())
            .child(self.title.clone())
    }
}

impl Manager {
    fn reorder(&mut self, change: impl FnOnce(&mut Vec<usize>) -> bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if !change(Arc::make_mut(&mut self.order)) {
            return;
        }
        self.rebuild_ranks();
        self.dirty = true;
        self.refresh_query(cx);
        self.apply_rules(cx);
        cx.notify();
    }

    pub(super) fn move_selected(&mut self, direction: isize, cx: &mut Context<Self>) {
        if let Some(index) = self.selected {
            self.move_by(index, direction, cx);
        }
    }

    /// Shift `index` (or the selection containing it) one step, keeping the
    /// group's internal order; the move stops at the list edge.
    pub(super) fn move_by(&mut self, index: usize, direction: isize, cx: &mut Context<Self>) {
        let group = self.targets(index);
        group.iter().for_each(|&i| self.pin(i));
        self.reorder(
            |order| {
                let members: std::collections::HashSet<usize> = group.iter().copied().collect();
                let mut positions: Vec<usize> = (0..order.len())
                    .filter(|&p| members.contains(&order[p]))
                    .collect();
                if direction > 0 {
                    positions.reverse();
                }
                let mut changed = false;
                for position in positions {
                    let next = position as isize + direction.signum();
                    if next < 0
                        || next as usize >= order.len()
                        || members.contains(&order[next as usize])
                    {
                        continue;
                    }
                    order.swap(position, next as usize);
                    changed = true;
                }
                changed
            },
            cx,
        );
    }

    pub(super) fn move_to_edge(&mut self, index: usize, top: bool, cx: &mut Context<Self>) {
        let group = self.targets(index);
        group.iter().for_each(|&i| self.pin(i));
        self.reorder(
            |order| {
                let members: std::collections::HashSet<usize> = group.iter().copied().collect();
                order.retain(|i| !members.contains(i));
                if top {
                    order.splice(0..0, group.iter().copied());
                } else {
                    order.extend(group.iter().copied());
                }
                true
            },
            cx,
        );
    }

    /// Drop `index` (or its selection) onto `target`: the group takes the
    /// target's place, landing after it when moved down and before it when moved up.
    pub(super) fn move_onto(&mut self, index: usize, target: usize, cx: &mut Context<Self>) {
        let group = self.targets(index);
        if group.contains(&target) {
            return;
        }
        group.iter().for_each(|&i| self.pin(i));
        self.reorder(
            |order| {
                let (Some(first), Some(to)) = (
                    order.iter().position(|i| group.contains(i)),
                    order.iter().position(|&i| i == target),
                ) else {
                    return false;
                };
                order.retain(|i| !group.contains(i));
                let Some(target_position) = order.iter().position(|&i| i == target) else {
                    return false;
                };
                let at = if first < to {
                    target_position + 1
                } else {
                    target_position
                };
                order.splice(at..at, group.iter().copied());
                true
            },
            cx,
        );
        self.selected = Some(index);
    }
}
