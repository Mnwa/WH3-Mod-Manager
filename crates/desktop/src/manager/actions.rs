use super::{Filter, Manager};
use gpui_kit::*;
use std::sync::{Arc, atomic::Ordering};
use wh3_core::{catalog::Source, conflict, launch, pack, steam};

impl Manager {
    pub(super) fn toggle(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if !self.enabled.remove(&index) {
            self.enabled.insert(index);
        }
        self.dirty = true;
        if self.filter != Filter::All {
            self.refresh_query(cx);
        }
        cx.notify();
    }

    pub(super) fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = Some(index);
        self.details.clear();
        cx.notify();
    }

    pub(super) fn move_selected(&mut self, direction: isize, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(index) = self.selected else {
            return;
        };
        let order = Arc::make_mut(&mut self.order);
        let Some(position) = order.iter().position(|&i| i == index) else {
            return;
        };
        let next = position
            .saturating_add_signed(direction)
            .min(order.len().saturating_sub(1));
        order.swap(position, next);
        self.rebuild_ranks();
        self.sort_name = false;
        self.dirty = true;
        self.refresh_query(cx);
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
                            this.show_report = true;
                        }
                        Err(e) => this.status = e.message(),
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn check(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.demo {
            return;
        }
        self.busy = true;
        self.cancellable = true;
        self.status = wh3_core::message!(
            "Checking overlapping files and dependencies…",
            "Проверка совпадающих файлов и зависимостей…"
        );
        self.cancel.store(false, Ordering::Relaxed);
        let (catalog, order, enabled, cancel) = (
            self.catalog.clone(),
            self.order.clone(),
            self.enabled.clone(),
            self.cancel.clone(),
        );
        let task = cx.background_spawn(async move {
            let report = conflict::check(&catalog, &order, &enabled, &cancel)?;
            let count = report.collisions.len();
            let lines: Vec<wh3_core::localization::Message> = report
                .warnings
                .into_iter()
                .chain(report.collisions.into_iter().map(|collision| {
                    format!(
                        "{}  ←  {}",
                        collision.file,
                        collision
                            .mods
                            .iter()
                            .map(|&i| catalog.mods[i].name.as_ref())
                            .collect::<Vec<_>>()
                            .join(" / ")
                    )
                    .into()
                }))
                .collect();
            Ok::<_, wh3_core::Error>((count, lines))
        });
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.cancellable = false;
                match result {
                    Ok((count, lines)) => { this.status = wh3_core::message!("Overlapping paths: {count}. An overlap does not always mean incompatibility.", "Совпадающих путей: {count}. Совпадение не всегда означает несовместимость.", count = count); this.diagnostics = lines; this.details.clear(); this.show_report = true; }
                    Err(error) => this.status = error.message(),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn play(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.demo {
            return;
        }
        let Some(game) = self.settings.game_path.clone() else {
            return;
        };
        let (catalog, order, enabled) = (
            self.catalog.clone(),
            self.order.clone(),
            self.enabled.clone(),
        );
        self.busy = true;
        let task = cx.background_spawn(async move {
            launch::prepare(&game, &catalog, &order, &enabled)?;
            launch::start(&game)
        });
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.status = match result {
                    Ok(()) => wh3_core::message!(
                        "Launch command sent to the game",
                        "Команда запуска передана игре"
                    ),
                    Err(e) => e.message(),
                };
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn bulk_toggle(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let indices = self.visible.clone();
        let mut selected = self.enabled.clone();
        self.busy = true;
        let task = cx.background_spawn(async move {
            for index in indices {
                if enabled {
                    selected.insert(index);
                } else {
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
                cx.notify();
            });
        }));
        cx.notify();
    }
}
