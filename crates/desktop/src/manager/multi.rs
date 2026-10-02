//! Multi-selection: Ctrl/Cmd+click toggles, Shift+click extends from the anchor,
//! and actions on a selected row apply to the whole selection.
use super::Manager;
use gpui_kit::*;
use std::collections::HashSet;

impl Manager {
    pub(super) fn click_row(&mut self, index: usize, modifiers: Modifiers, cx: &mut Context<Self>) {
        // Shift ranges stay inside the list (pane) that was clicked.
        let rows = self.pane_of(index);
        let position = |index: usize| rows.iter().position(|&i| i == index);
        if modifiers.shift
            && let (Some(anchor), Some(end)) = (self.anchor.and_then(position), position(index))
        {
            let (from, to) = (anchor.min(end), anchor.max(end));
            let range: Vec<usize> = rows[from..=to].to_vec();
            if !(modifiers.control || modifiers.platform) {
                self.marked.clear();
            }
            self.marked.extend(range);
        } else if modifiers.control || modifiers.platform {
            if !self.marked.remove(&index) {
                self.marked.insert(index);
            }
            self.anchor = Some(index);
        } else {
            self.marked.clear();
            self.marked.insert(index);
            self.anchor = Some(index);
        }
        self.selected = Some(index);
        self.details.clear();
        cx.notify();
    }

    pub(super) fn select_all_visible(&mut self, cx: &mut Context<Self>) {
        self.marked = self.shown().collect();
        cx.notify();
    }

    /// The mods an action on `index` affects: the selection when `index` is part
    /// of a multi-selection, otherwise just `index`. Returned in load order.
    pub(super) fn targets(&self, index: usize) -> Vec<usize> {
        if self.marked.len() > 1 && self.marked.contains(&index) {
            self.order
                .iter()
                .copied()
                .filter(|i| self.marked.contains(i))
                .collect()
        } else {
            vec![index]
        }
    }

    pub(super) fn set_enabled(&mut self, indices: &[usize], enable: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let always = self.always_enabled_indices();
        for &index in indices {
            if enable {
                self.enabled.insert(index);
            } else if !always.contains(&index) {
                self.enabled.remove(&index);
            }
        }
        self.dirty = true;
        self.refresh_if_enabled_matters(cx);
        cx.notify();
    }

    /// Space and the context menu flip the whole selection to the opposite of `index`.
    pub(super) fn toggle_targets(&mut self, index: usize, cx: &mut Context<Self>) {
        let targets = self.targets(index);
        if targets.len() == 1 {
            self.toggle(index, cx);
        } else {
            let enable = !self.enabled.contains(&index);
            self.set_enabled(&targets, enable, cx);
        }
    }

    pub(super) fn hide_targets(&mut self, index: usize, cx: &mut Context<Self>) {
        let hidden: HashSet<usize> = self.hidden_indices();
        for target in self.targets(index) {
            // Toggle as a group: hide all unless they are all hidden already.
            if !hidden.contains(&index) || hidden.contains(&target) {
                self.toggle_hidden(target, cx);
            }
        }
        self.marked.clear();
    }
}
