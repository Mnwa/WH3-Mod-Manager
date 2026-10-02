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
            prompt: Some("JSON пресета / конфигурации".into()),
        });
        self.io_task = Some(cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await
                && let Some(path) = paths.into_iter().next()
            {
                let result = cx
                    .background_spawn(async move {
                        let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
                        if metadata.len() > 32 * 1024 * 1024 {
                            return Err("Файл пресетов больше 32 МБ".to_owned());
                        }
                        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                        Preset::parse(&bytes).map_err(|e| e.to_string())
                    })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    match result {
                        Ok(presets) => {
                            let count = presets.len();
                            for preset in presets {
                                if let Some(existing) = this
                                    .settings
                                    .presets
                                    .iter_mut()
                                    .find(|p| p.name == preset.name)
                                {
                                    *existing = preset;
                                } else {
                                    this.settings.presets.push(preset);
                                }
                            }
                            this.status = format!(
                                "Импортировано пресетов: {count}. Выберите пресет для применения."
                            )
                            .into();
                            this.dirty = true;
                        }
                        Err(error) => this.status = error.into(),
                    }
                    cx.notify();
                });
            }
        }));
    }

    pub(super) fn export(&mut self, cx: &mut Context<Self>) {
        let preset = self.capture("Экспорт".into());
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
                        Ok(()) => "Пресет экспортирован".into(),
                        Err(e) => e.to_string().into(),
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
            self.status = "Введите название пресета".into();
            cx.notify();
            return;
        }
        let preset = self.capture(name.clone());
        if let Some(existing) = self.settings.presets.iter_mut().find(|p| p.name == name) {
            *existing = preset;
        } else {
            self.settings.presets.push(preset);
        }
        self.dirty = true;
        self.save(cx);
        cx.notify();
    }
}
