use super::{Filter, Manager};
use gpui_kit::*;
use wh3_core::{catalog::Source, pack, steam};

impl Manager {
    pub(super) fn toggle(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if self.enabled.contains(&index) && self.is_always_enabled(index) {
            self.status = wh3_core::message!(
                "{} is kept always enabled",
                "{} всегда включён",
                self.catalog.mods[index].name
            );
        } else if !self.enabled.remove(&index) {
            self.enabled.insert(index);
        }
        self.dirty = true;
        self.refresh_if_enabled_matters(cx);
        cx.notify();
    }

    /// After the enabled set changes: rules only constrain enabled mods, and the
    /// visible rows change only when enabled state filters or sorts them.
    pub(super) fn refresh_if_enabled_matters(&mut self, cx: &mut Context<Self>) {
        self.apply_rules(cx);
        if self.filter != Filter::All
            || self.sort.key == super::SortKey::Enabled
            || self.dual_active()
            || self.grouping_active()
        {
            self.refresh_query(cx);
        }
    }

    pub(super) fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = Some(index);
        self.details.clear();
        cx.notify();
    }

    pub(super) fn choose_directory(&mut self, game: bool, cx: &mut Context<Self>) {
        if self.busy || self.demo {
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(
                if game {
                    self.language.text("Game folder", "Папка игры")
                } else {
                    self.language.text("Mod folder", "Папка модов")
                }
                .into(),
            ),
        });
        self.io_task = Some(cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await
                && let Some(path) = paths.into_iter().next()
            {
                let result = cx
                    .background_spawn(async move {
                        if game && !path.join("Warhammer3.exe").is_file() {
                            return Err(wh3_core::message!(
                                "The folder does not contain Warhammer3.exe",
                                "В папке отсутствует Warhammer3.exe"
                            ));
                        }
                        Ok((
                            path.clone(),
                            if game {
                                Some(steam::for_game(path))
                            } else {
                                None
                            },
                        ))
                    })
                    .await;
                let _ = this.update(cx, |this, cx| match result {
                    Ok((path, settings)) => {
                        if let Some(settings) = settings {
                            this.settings.game_path = settings.game_path;
                            this.settings.roots = settings.roots;
                        } else if !this.settings.roots.iter().any(|(root, _)| root == &path) {
                            this.settings.roots.push((path, Source::Custom));
                        }
                        this.rescan(cx);
                        this.dirty = true;
                    }
                    Err(error) => {
                        this.status = error;
                        cx.notify();
                    }
                });
            }
        }));
    }

    pub(super) fn inspect(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(index) = self.selected else {
            return;
        };
        let path = self.catalog.mods[index].path.clone();
        self.busy = true;
        let task = cx.background_spawn(async move {
            pack::index(&path).map(|files| {
                files
                    .into_iter()
                    .map(|file| {
                        wh3_core::localization::Message::new(
                            format!(
                                "{} · {} B{}",
                                file.name,
                                file.size,
                                if file.compressed {
                                    " · compressed"
                                } else {
                                    ""
                                }
                            ),
                            format!(
                                "{} · {} Б{}",
                                file.name,
                                file.size,
                                if file.compressed { " · сжат" } else { "" }
                            ),
                        )
                    })
                    .collect()
            })
        });
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if this.selected == Some(index) {
                    match result {
                        Ok(files) => {
                            this.details = files;
                            this.show_report_tab(super::compat::Tab::Details, cx);
                        }
                        Err(e) => this.status = e.message(),
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn bulk_toggle(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let indices: Vec<usize> = self.shown().collect();
        let mut selected = self.enabled.clone();
        let always = self.always_enabled_indices();
        self.busy = true;
        let task = cx.background_spawn(async move {
            for index in indices {
                if enabled {
                    selected.insert(index);
                } else if !always.contains(&index) {
                    selected.remove(&index);
                }
            }
            selected
        });
        self.job = Some(cx.spawn(async move |this, cx| {
            let selected = task.await;
            let _ = this.update(cx, |this, cx| {
                this.enabled = selected;
                this.busy = false;
                this.dirty = true;
                this.refresh_query(cx);
                this.apply_rules(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }
}
