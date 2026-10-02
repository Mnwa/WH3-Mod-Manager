//! "Find the problem mod": a guided version of the original's "Bisect Mod List".
//! Each step enables half of the suspects; the player reports whether the problem is
//! still there, and the suspects halve until one mod (or one linked group) is left.
use super::Manager;
use gpui_kit::*;
use std::collections::HashSet;
use wh3_core::{bisect::Search, message};

/// The list before the search, kept as a preset so closing the manager mid-search
/// never loses it.
pub(super) const BEFORE_SEARCH: &str = "Before problem search";

pub(crate) struct State {
    pub search: Search,
    /// Earlier steps, for "Undo last answer".
    pub history: Vec<Search>,
    /// Set once the suspects cannot be split further.
    pub found: bool,
}

impl Manager {
    pub(super) fn confirm_hunt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let l = self.language;
        self.confirm(
            l.text(
                "Find the mod that causes a problem?",
                "Найти мод, который вызывает проблему?",
            )
            .into(),
            l.text(
                "Half of your enabled mods are switched off at a time. After each game, tell the manager whether the problem is still there. It assumes one mod causes it. Your list is saved as the preset “Before problem search” and restored at the end.",
                "Каждый раз отключается половина включённых модов. После каждой игры скажите менеджеру, осталась ли проблема. Предполагается, что виноват один мод. Ваш список сохранится как пресет «Before problem search» и вернётся в конце.",
            ),
            l.text("Start", "Начать"),
            |this, cx| this.start_hunt(cx),
            window,
            cx,
        );
    }

    fn start_hunt(&mut self, cx: &mut Context<Self>) {
        let always = self.always_enabled_indices();
        let suspects: Vec<usize> = self
            .order
            .iter()
            .copied()
            .filter(|index| self.enabled.contains(index) && !always.contains(index))
            .collect();
        let Some(search) = Search::start(&self.catalog, suspects) else {
            self.status = message!(
                "Enable at least two mods to search among them",
                "Включите хотя бы два мода, чтобы искать среди них"
            );
            cx.notify();
            return;
        };
        let before = self.capture(BEFORE_SEARCH.into());
        self.store_preset(before);
        self.hunt = Some(State {
            search,
            history: vec![],
            found: false,
        });
        self.apply_hunt_step(cx);
    }

    /// Enable the half under test plus always-enabled mods.
    fn apply_hunt_step(&mut self, cx: &mut Context<Self>) {
        let Some(hunt) = &self.hunt else { return };
        let mut enabled: HashSet<usize> = self.always_enabled_indices();
        if hunt.found {
            enabled.extend(hunt.search.suspects.iter().copied());
        } else {
            enabled.extend(hunt.search.testing.iter().copied());
        }
        self.enabled = enabled;
        self.dirty = true;
        self.refresh_if_enabled_matters(cx);
        self.refresh_query(cx);
        cx.notify();
    }

    pub(super) fn answer_hunt(&mut self, problem_remains: bool, cx: &mut Context<Self>) {
        let catalog = self.catalog.clone();
        let Some(hunt) = &mut self.hunt else { return };
        hunt.history.push(hunt.search.clone());
        hunt.found = !hunt.search.answer(&catalog, problem_remains);
        self.apply_hunt_step(cx);
    }

    pub(super) fn undo_hunt_answer(&mut self, cx: &mut Context<Self>) {
        let Some(hunt) = &mut self.hunt else { return };
        if let Some(previous) = hunt.history.pop() {
            hunt.search = previous;
            hunt.found = false;
            self.apply_hunt_step(cx);
        }
    }

    /// Restore the list from before the search; optionally keep the found mods off.
    pub(super) fn end_hunt(&mut self, disable_found: bool, cx: &mut Context<Self>) {
        let Some(hunt) = self.hunt.take() else { return };
        let Some(before) = self
            .settings
            .presets
            .iter()
            .find(|preset| preset.name == BEFORE_SEARCH)
            .cloned()
        else {
            return;
        };
        self.apply_preset(&before, cx);
        if disable_found && hunt.found {
            let always = self.always_enabled_indices();
            for index in &hunt.search.suspects {
                if !always.contains(index) {
                    self.enabled.remove(index);
                }
            }
            self.refresh_if_enabled_matters(cx);
        }
        self.status = if disable_found {
            message!(
                "Your list is back, without the {} mods that caused the problem",
                "Ваш список возвращён без модов, вызвавших проблему: {}",
                hunt.search.suspects.len()
            )
        } else {
            message!("Your mod list is back", "Ваш список модов возвращён")
        };
        cx.notify();
    }
}
