//! Per-mod flags kept in the library: hidden and always enabled (WHM2).
use super::{Manager, query::indices_of};
use gpui_kit::*;
use std::collections::HashSet;

impl Manager {
    pub(super) fn always_enabled_indices(&self) -> HashSet<usize> {
        indices_of(&self.catalog, &self.settings.always_enabled)
    }

    /// Presets and bulk actions never leave an always-enabled mod disabled.
    pub(super) fn enforce_always_enabled(&mut self) {
        let always = self.always_enabled_indices();
        self.enabled.extend(always);
    }

    pub(super) fn toggle_always_enabled(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let name = self.catalog.mods[index].name.to_lowercase();
        if !self.settings.always_enabled.remove(&name) {
            self.settings.always_enabled.insert(name);
            self.enabled.insert(index);
            self.refresh_if_enabled_matters(cx);
        }
        self.dirty = true;
        cx.notify();
    }

    /// Hiding also disables the mod unless it is always enabled, as in the original.
    pub(super) fn toggle_hidden(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let name = self.catalog.mods[index].name.to_lowercase();
        if !self.settings.hidden.remove(&name) {
            if !self.settings.always_enabled.contains(&name) {
                self.enabled.remove(&index);
            }
            self.settings.hidden.insert(name);
            if self.selected == Some(index) {
                self.selected = None;
            }
        }
        self.dirty = true;
        self.refresh_query(cx);
        cx.notify();
    }
}
