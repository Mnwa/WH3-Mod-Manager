use super::Manager;
use gpui_kit::*;
use std::sync::Arc;
use wh3_core::{preset::Preset, storage};

impl Manager {
    /// Pinned (hand-placed) mods keep their position as `loadOrder`, the field
    /// the original uses for pins, so presets and the library remember them.
    pub(super) fn capture(&self, name: String) -> Preset {
        let mut preset = Preset::capture(name, &self.catalog, &self.order, &self.enabled);
        let pinned: std::collections::HashSet<&str> = self
            .rules
            .pinned
            .iter()
            .map(|&i| &*self.catalog.mods[i].name)
            .collect();
        for (position, entry) in preset.mods.iter_mut().enumerate() {
            if pinned.contains(entry.name.as_str()) {
                entry.load_order = Some(position);
            }
        }
        preset
    }

    pub(super) fn apply_preset(&mut self, preset: &Preset, cx: &mut Context<Self>) {
        let applied = preset.apply(&self.catalog);
        self.order = Arc::new(applied.order);
        // Rows index the catalog directly; until the debounced query finishes,
        // show the new order so no stale index from a previous catalog survives.
        self.visible = self.order.to_vec();
        self.visible_enabled.clear();
        self.groups = Default::default();
        self.grouped.clear();
        self.rebuild_ranks();
        self.enabled = applied.enabled;
        self.enforce_always_enabled();
        self.rules.pinned = preset
            .mods
            .iter()
            .filter(|entry| entry.load_order.is_some())
            .filter_map(|entry| {
                self.catalog
                    .by_name
                    .get(&entry.name.to_lowercase())?
                    .first()
                    .copied()
            })
            .collect();
        self.apply_rules(cx);
        let missing = applied.missing.len();
        self.status = super::presets::preset_message(
            &preset.name,
            &format!("Preset “{{}}” · {missing} missing mods"),
            &format!("Пресет «{{}}» · отсутствует модов: {missing}"),
        );
        self.diagnostics = applied
            .missing
            .into_iter()
            .map(|name| {
                wh3_core::message!(
                    "Mod not found: {name}",
                    "Не найден мод: {name}",
                    name = name
                )
            })
            .collect();
        self.dirty = true;
        self.refresh_query(cx);
        cx.notify();
    }

    pub(super) fn rebuild_ranks(&mut self) {
        self.ranks.resize(self.catalog.mods.len(), 0);
        for (rank, &index) in self.order.iter().enumerate() {
            self.ranks[index] = rank + 1;
        }
    }

    pub(super) fn save(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.demo {
            return;
        }
        self.settings.current = Some(self.capture("Current".into()));
        let settings = self.settings.clone();
        self.busy = true;
        self.status = wh3_core::message!("Saving…", "Сохранение…");
        let task = cx
            .background_spawn(async move { storage::save(&storage::settings_path()?, &settings) });
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(()) => {
                        this.dirty = false;
                        this.status = wh3_core::message!(
                            "Settings and mod order saved",
                            "Настройки и порядок модов сохранены"
                        );
                    }
                    Err(error) => this.status = error.message(),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(crate) fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.preferences_busy {
            self.status = wh3_core::message!("Saving language preference…", "Сохранение языка…");
            cx.notify();
            return false;
        }
        if self.demo || (!self.dirty && !self.busy) {
            return true;
        }
        if self.busy {
            self.status = wh3_core::message!(
                "Wait for the current operation to finish before closing",
                "Дождитесь завершения операции перед закрытием"
            );
            cx.notify();
            return false;
        }
        self.settings.current = Some(self.capture("Current".into()));
        let settings = self.settings.clone();
        let handle = window.window_handle();
        let language = self.language;
        self.busy = true;
        let task = cx
            .background_spawn(async move { storage::save(&storage::settings_path()?, &settings) });
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let succeeded = result.is_ok();
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(()) => this.dirty = false,
                    Err(error) => {
                        this.status =
                            error.message()
                    }
                }
                cx.notify();
            });
            if succeeded {
                let _ = cx.update_window(handle, |_, window, _| window.remove_window());
            } else if let Ok(prompt) = cx.update_window(handle, |_, window, cx| {
                window.prompt(PromptLevel::Warning, language.text("Could not save the library", "Не удалось сохранить библиотеку"), Some(language.text("Stay to resolve the error, or close without saving the latest changes.", "Можно остаться и исправить ошибку или закрыть окно без сохранения последних изменений.")), &[language.text("Stay", "Остаться"), language.text("Close without saving", "Закрыть без сохранения")], cx)
            }) && prompt.await == Ok(1) {
                let _ = cx.update_window(handle, |_, window, _| window.remove_window());
            }
        }));
        false
    }
}
