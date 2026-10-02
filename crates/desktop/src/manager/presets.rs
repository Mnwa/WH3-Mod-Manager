use super::Manager;
use gpui_kit::*;
use std::path::PathBuf;
use wh3_core::{preset::Preset, storage};

impl Manager {
    pub(super) fn import(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(
                self.language
                    .text("Preset / configuration JSON", "JSON пресета / конфигурации")
                    .into(),
            ),
        });
        self.io_task = Some(cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await
                && let Some(path) = paths.into_iter().next()
            {
                let result = cx
                    .background_spawn(async move {
                        let metadata = std::fs::metadata(&path).map_err(|e| {
                            wh3_core::message!("File error: {}", "Ошибка файла: {}", e)
                        })?;
                        if metadata.len() > 32 * 1024 * 1024 {
                            return Err(wh3_core::message!(
                                "Preset file exceeds 32 MiB",
                                "Файл пресетов больше 32 МБ"
                            ));
                        }
                        let bytes = std::fs::read(path).map_err(|e| {
                            wh3_core::message!("File error: {}", "Ошибка файла: {}", e)
                        })?;
                        Preset::parse(&bytes).map_err(|e| e.message())
                    })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    match result {
                        Ok(presets) => {
                            let count = presets.len();
                            for preset in presets {
                                this.store_preset(preset);
                            }
                            this.status = wh3_core::message!(
                                "Imported {count} presets. Select a preset to apply it.",
                                "Импортировано пресетов: {count}. Выберите пресет для применения.",
                                count = count
                            );
                            this.dirty = true;
                        }
                        Err(error) => this.status = error,
                    }
                    cx.notify();
                });
            }
        }));
    }

    pub(super) fn export(&mut self, cx: &mut Context<Self>) {
        let preset = self.capture("Export".into());
        let prompt = cx.prompt_for_new_path(&PathBuf::from("."), Some("preset.json"));
        self.io_task = Some(cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = prompt.await {
                let result = cx
                    .background_spawn(async move {
                        storage::atomic_write(&path, &serde_json::to_vec_pretty(&preset)?)
                    })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    this.status = match result {
                        Ok(()) => wh3_core::message!("Preset exported", "Пресет экспортирован"),
                        Err(e) => e.message(),
                    };
                    cx.notify();
                });
            }
        }));
    }

    pub(super) fn save_preset(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let name = self.preset_name.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.status = wh3_core::message!("Enter a preset name", "Введите название пресета");
            cx.notify();
            return;
        }
        let preset = self.capture(name);
        self.store_preset(preset);
        self.dirty = true;
        self.save(cx);
        cx.notify();
    }
}

/// How a stored preset combines with the current list (the original's click,
/// Shift+click and Ctrl+click on a preset).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PresetAction {
    Apply,
    Merge,
    Subtract,
    Replace,
    Delete,
}

/// Snapshot taken on every launch, as the original's "On Last Game Launch".
pub(super) const LAST_LAUNCH: &str = "On Last Game Launch";

impl Manager {
    pub(super) fn preset_action(
        &mut self,
        index: usize,
        action: PresetAction,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        let Some(preset) = self.settings.presets.get(index).cloned() else {
            return;
        };
        match action {
            PresetAction::Apply => {
                self.apply_preset(&preset, cx);
                return;
            }
            PresetAction::Merge | PresetAction::Subtract => {
                let enable = action == PresetAction::Merge;
                let always = self.always_enabled_indices();
                for entry in preset.mods.iter().filter(|entry| entry.is_enabled) {
                    let Some(&index) = self
                        .catalog
                        .by_name
                        .get(&entry.name.to_lowercase())
                        .and_then(|indices| indices.first())
                    else {
                        continue;
                    };
                    if enable {
                        self.enabled.insert(index);
                    } else if !always.contains(&index) {
                        self.enabled.remove(&index);
                    }
                }
                self.status = if enable {
                    wh3_core::message!(
                        "Enabled mods from “{}”",
                        "Включены моды из «{}»",
                        preset.name
                    )
                } else {
                    wh3_core::message!(
                        "Disabled mods from “{}”",
                        "Отключены моды из «{}»",
                        preset.name
                    )
                };
                self.refresh_if_enabled_matters(cx);
            }
            PresetAction::Replace => {
                self.settings.presets[index] = self.capture(preset.name.clone());
                self.status =
                    wh3_core::message!("Preset “{}” updated", "Пресет «{}» обновлён", preset.name);
            }
            PresetAction::Delete => {
                self.settings.presets.remove(index);
                self.status =
                    wh3_core::message!("Preset “{}” deleted", "Пресет «{}» удалён", preset.name);
            }
        }
        self.dirty = true;
        cx.notify();
    }

    pub(super) fn store_preset(&mut self, preset: Preset) {
        if let Some(existing) = self
            .settings
            .presets
            .iter_mut()
            .find(|p| p.name == preset.name)
        {
            *existing = preset;
        } else {
            self.settings.presets.push(preset);
        }
    }
}
