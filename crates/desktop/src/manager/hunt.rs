//! "Find the problem mod": a guided version of the original's "Bisect Mod List".
//! Each step enables half of the suspects; the player reports whether the problem is
//! still there, and the suspects halve until one mod (or one linked group) is left.
use super::Manager;
use gpui_kit::*;
use std::collections::{HashMap, HashSet};
use wh3_core::{bisect::Search, catalog::Catalog, message};

/// The list before the search, kept as a preset so closing the manager mid-search
/// never loses it.
pub(super) const BEFORE_SEARCH: &str = "Before problem search";

pub(crate) struct State {
    pub search: Search,
    /// Earlier steps, for "Undo last answer".
    pub history: Vec<Search>,
    /// Set once the suspects cannot be split further.
    pub found: bool,
    /// Mods that require the found ones, switched off with them on request.
    pub dependents: Vec<usize>,
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
                "Half of your enabled mods are switched off at a time. After each game, tell the manager whether the problem is still there. It assumes one mod causes it. Your list is saved under “Before problem search” in the sidebar and restored at the end.",
                "Каждый раз отключается половина включённых модов. После каждой игры скажите менеджеру, осталась ли проблема. Предполагается, что виноват один мод. Ваш список сохранится в боковой панели как «До поиска проблемы» и вернётся в конце.",
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
            dependents: vec![],
        });
        self.apply_hunt_step(cx);
    }

    /// Enable the suspects under test, their requirements and always-enabled mods.
    fn apply_hunt_step(&mut self, cx: &mut Context<Self>) {
        let Some(hunt) = &self.hunt else { return };
        let mut enabled: HashSet<usize> = self.always_enabled_indices();
        enabled.extend(hunt.search.enabled.iter().copied());
        self.enabled = enabled;
        self.dirty = true;
        self.refresh_if_enabled_matters(cx);
        self.refresh_query(cx);
        cx.notify();
    }

    pub(super) fn answer_hunt(&mut self, problem_remains: bool, cx: &mut Context<Self>) {
        let Some(hunt) = &mut self.hunt else { return };
        hunt.history.push(hunt.search.clone());
        hunt.found = !hunt.search.answer(problem_remains);
        if hunt.found {
            hunt.dependents = hunt.search.dependents();
        }
        if hunt.found && hunt.search.suspects.is_empty() {
            // Only possible when every mod of the blamed half was removed mid-search.
            self.end_hunt(false, cx);
            self.status = message!(
                "The suspected mods were removed during the search; your list is back",
                "Подозреваемые моды удалены во время поиска; ваш список возвращён"
            );
            return;
        }
        self.apply_hunt_step(cx);
    }

    /// Keep the search on the same mods after a rescan renumbered the catalog.
    pub(super) fn remap_hunt(&mut self, previous: &Catalog) {
        let Some(hunt) = &mut self.hunt else { return };
        let catalog = &self.catalog;
        let by_path: HashMap<&std::path::Path, usize> = catalog
            .mods
            .iter()
            .enumerate()
            .map(|(index, item)| (item.path.as_path(), index))
            .collect();
        let new_index = |index: usize| {
            let item = previous.mods.get(index)?;
            by_path.get(item.path.as_path()).copied().or_else(|| {
                catalog
                    .by_name
                    .get(&item.name.to_lowercase())?
                    .first()
                    .copied()
            })
        };
        hunt.search.remap(new_index);
        for search in &mut hunt.history {
            search.remap(new_index);
        }
        hunt.dependents = hunt
            .dependents
            .iter()
            .filter_map(|&m| new_index(m))
            .collect();
    }

    pub(super) fn undo_hunt_answer(&mut self, cx: &mut Context<Self>) {
        let Some(hunt) = &mut self.hunt else { return };
        if let Some(previous) = hunt.history.pop() {
            hunt.search = previous;
            hunt.found = false;
            hunt.dependents.clear();
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
        let disable_found = disable_found && hunt.found;
        let dependents = &hunt.dependents;
        if disable_found {
            // Mods that need the culprit would only break the game in its place.
            let always = self.always_enabled_indices();
            for index in hunt.search.suspects.iter().chain(dependents) {
                if !always.contains(index) {
                    self.enabled.remove(index);
                }
            }
            self.refresh_if_enabled_matters(cx);
        }
        self.status = match (disable_found, dependents.len()) {
            (false, _) => message!("Your mod list is back", "Ваш список модов возвращён"),
            (true, 0) => message!(
                "Your list is back without the problem mod: {}",
                "Ваш список возвращён без проблемного мода: {}",
                self.hunt_names(&hunt.search.suspects)
            ),
            (true, count) => message!(
                "Your list is back without the problem mod ({}) and {} mods that require it",
                "Ваш список возвращён без проблемного мода ({}); отключены и моды, которым он нужен: {}",
                self.hunt_names(&hunt.search.suspects),
                count
            ),
        };
        cx.notify();
    }

    /// Titles of the found mods for messages, shortened after three.
    pub(super) fn hunt_names(&self, mods: &[usize]) -> String {
        let mut names: Vec<&str> = mods
            .iter()
            .take(3)
            .map(|&index| &*self.catalog.mods[index].title)
            .collect();
        if mods.len() > 3 {
            names.push("…");
        }
        names.join(", ")
    }
}
