use super::Manager;
use gpui_kit::*;
use std::{path::PathBuf, sync::Arc};
use wh3_core::{catalog::Catalog, metadata};

impl Manager {
    pub(super) fn import_metadata(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(
                self.language
                    .text(
                        "Metadata file or original config.json",
                        "Метаданные или config.json оригинала",
                    )
                    .into(),
            ),
        });
        self.io_task = Some(cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await
                && let Some(path) = paths.into_iter().next() {
                    let Ok(catalog) = this.update(cx, |this, cx| { this.busy = true; cx.notify(); this.catalog.clone() }) else { return; };
                    let result = cx.background_spawn(async move {
                        let bundle = metadata::read(&path)?;
                        let mut catalog = Catalog::new(catalog.mods.clone());
                        let matched = metadata::apply(&mut catalog, &bundle.mods);
                        Ok::<_, wh3_core::Error>((bundle, catalog, matched))
                    }).await;
                    let _ = this.update(cx, |this, cx| {
                        this.busy = false;
                        match result {
                            Ok((bundle, catalog, matched)) => {
                                this.catalog = Arc::new(catalog);
                                this.settings.metadata.extend(bundle.mods);
                                for preset in bundle.presets {
                                    if let Some(existing) = this.settings.presets.iter_mut().find(|p| p.name == preset.name) { *existing = preset; }
                                    else { this.settings.presets.push(preset); }
                                }
                                if let Some(preset) = bundle.current_preset { this.apply_preset(&preset, cx); }
                                this.diagnostics.extend(bundle.warnings.into_iter().map(metadata::warning_message));
                                this.status = wh3_core::message!("Metadata imported · {matched} matching mods. Save the library.", "Мета импортирована · совпало модов: {matched}. Сохраните библиотеку.", matched = matched);
                                this.dirty = true;
                                this.refresh_query(cx);
                            }
                            Err(error) => this.status = error.message(),
                        }
                        cx.notify();
                    });
                }
        }));
    }

    pub(super) fn export_original_metadata(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(
                self.language
                    .text(
                        "Select the original manager's config.json",
                        "Выберите config.json оригинального менеджера",
                    )
                    .into(),
            ),
        });
        self.io_task = Some(cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await
                && let Some(source) = paths.into_iter().next()
            {
                let Ok(prompt) = this.update(cx, |_, cx| {
                    cx.prompt_for_new_path(&PathBuf::from("."), Some("wh3-metadata.json"))
                }) else {
                    return;
                };
                if let Ok(Ok(Some(destination))) = prompt.await {
                    let result = cx
                        .background_spawn(async move { metadata::export(&source, &destination) })
                        .await;
                    let _ = this.update(cx, |this, cx| {
                        this.status = match result {
                            Ok(bundle) => wh3_core::message!(
                                "Original metadata exported: {} mods, {} presets",
                                "Мета оригинала экспортирована: {} модов, {} пресетов",
                                bundle.mods.len(),
                                bundle.presets.len()
                            ),
                            Err(error) => error.message(),
                        };
                        cx.notify();
                    });
                }
            }
        }));
    }
}
