//! Play, Continue, campaign saves and game start options.
use super::{Manager, presets::LAST_LAUNCH};
use gpui_kit::*;
use std::{path::PathBuf, sync::Arc};
use wh3_core::{
    launch,
    saves::{self, Save},
    staging,
    storage::{self, GameOptions, Staging},
};

/// Recent saves, refreshed after scans and launches rather than during render.
#[derive(Default)]
pub(crate) struct State {
    pub saves: Arc<Vec<Save>>,
    pub folder: Option<PathBuf>,
    pub task: Option<Task<()>>,
    pub outdated_task: Option<Task<()>>,
    /// Vanilla DB/Lua paths per game folder, shared by outdated checks.
    pub vanilla: Option<(PathBuf, Arc<std::collections::HashSet<String>>)>,
}

/// Number of saves offered in the menu; the folder can hold hundreds.
pub(super) const RECENT_SAVES: usize = 15;

pub(super) fn launch_options(options: GameOptions) -> launch::Options {
    launch::Options {
        skip_intro_movies: options.skip_intro_movies,
        script_logging: options.script_logging,
        auto_start_custom_battle: options.auto_start_custom_battle,
        make_units_generals: options.make_units_generals,
    }
}

impl Manager {
    pub(super) fn refresh_saves(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            return;
        }
        let Some(folder) = saves::folder() else {
            return;
        };
        self.launch.folder = Some(folder.clone());
        let task = cx.background_spawn(async move { saves::list(&folder) });
        self.launch.task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(saves) => this.launch.saves = Arc::new(saves),
                    Err(error) => this.diagnostics.push(error.message()),
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn set_option(
        &mut self,
        change: impl FnOnce(&mut GameOptions),
        cx: &mut Context<Self>,
    ) {
        change(&mut self.settings.options);
        self.dirty = true;
        cx.notify();
    }

    pub(super) fn can_play(&self) -> bool {
        !self.busy && !self.demo && self.settings.game_path.is_some() && cfg!(windows)
    }

    /// `save` is a file name from the save folder, as the original passes it.
    pub(super) fn play(&mut self, save: Option<String>, cx: &mut Context<Self>) {
        if !self.can_play() {
            return;
        }
        if self.live.game_running {
            self.status = wh3_core::message!(
                "The game is already running; close it first",
                "Игра уже запущена; сначала закройте её"
            );
            cx.notify();
            return;
        }
        // Steam may be replacing a mod right now (it removes, then re-adds the file);
        // launch once the folders are quiet and rescanned, like the original's delay.
        if self.live.files_settling() {
            self.live.play_after_refresh = Some(save);
            self.status = wh3_core::message!(
                "Mod files are changing; the game starts once they settle",
                "Файлы модов меняются; игра запустится, когда они успокоятся"
            );
            cx.notify();
            return;
        }
        let Some(game) = self.settings.game_path.clone() else {
            return;
        };
        let snapshot = self.capture(LAST_LAUNCH.into());
        self.store_preset(snapshot);
        // Play saves first, so the list survives even if the manager is closed with the game.
        self.settings.current = Some(self.capture("Current".into()));
        let settings = self.settings.clone();
        let (catalog, order, enabled) = (
            self.catalog.clone(),
            self.order.clone(),
            self.enabled.clone(),
        );
        let options = launch_options(self.settings.options);
        let close = self.settings.options.close_on_play;
        let rules = super::rules::Inputs::new(&self.settings, self.rules.pack_rules.clone());
        let pinned = self.rules.pinned.clone();
        let (mode, fresh) = (
            self.settings.options.staging,
            self.settings.options.clean_up_staging,
        );
        let progress = Arc::new(staging::Progress::default());
        self.busy = true;
        if mode != Staging::Off {
            self.cancel
                .store(false, std::sync::atomic::Ordering::Relaxed);
            self.cancellable = true;
            self.follow_staging(progress.clone(), cx);
        }
        let cancel = self.cancel.clone();
        let task = cx.background_spawn(async move {
            // Rules are applied again here so a pending debounced pass cannot be skipped.
            storage::save(&storage::settings_path()?, &settings)?;
            let (_, applied) = rules.run(&catalog, &order, &enabled, &pinned);
            let temp = launch::temp_pack_dir()?;
            let staged;
            let catalog = if mode == Staging::Off {
                &*catalog
            } else {
                staged =
                    staging::stage(&game, &catalog, &enabled, mode, fresh, &cancel, &progress)?;
                &staged
            };
            launch::prepare(&game, catalog, &applied.order, &enabled, &options, &temp)?;
            launch::start(&game, save.as_deref())?;
            Ok::<_, wh3_core::Error>(applied.order)
        });
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.cancellable = false;
                this.live.staging_poll = None;
                if mode != Staging::Off {
                    this.refresh_staging_size(cx);
                }
                match result {
                    Ok(order) => {
                        this.live.staged |= mode != Staging::Off;
                        this.dirty = false;
                        if order.len() == this.order.len() && *this.order != order {
                            this.order = Arc::new(order);
                            this.rebuild_ranks();
                            this.refresh_query(cx);
                            this.dirty = true;
                        }
                        this.status = wh3_core::message!(
                            "Launch command sent to the game",
                            "Команда запуска передана игре"
                        );
                        if close {
                            this.persist_then_quit(false, cx);
                        }
                    }
                    Err(error) => {
                        this.dirty = true;
                        this.status = error.message();
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Enable exactly the packs recorded in a save, like "Load mods from save".
    pub(super) fn enable_mods_from_save(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        let task = cx.background_spawn(async move { saves::mods(&path) });
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(names) => this.enable_exactly(&names),
                    Err(error) => this.status = error.message(),
                }
                this.refresh_if_enabled_matters(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn enable_exactly(&mut self, names: &[String]) {
        let mut missing = Vec::new();
        self.enabled.clear();
        for name in names {
            match self
                .catalog
                .by_name
                .get(&name.to_lowercase())
                .and_then(|i| i.first())
            {
                Some(&index) => {
                    self.enabled.insert(index);
                }
                None => missing.push(wh3_core::message!(
                    "Mod not found: {}",
                    "Не найден мод: {}",
                    name
                )),
            }
        }
        self.enforce_always_enabled();
        self.status = wh3_core::message!(
            "Enabled {} mods from the save · {} missing",
            "Включено модов из сохранения: {} · отсутствует: {}",
            names.len() - missing.len(),
            missing.len()
        );
        if !missing.is_empty() {
            self.show_report = true;
        }
        self.diagnostics = missing;
        self.dirty = true;
    }

    pub(super) fn restart_for_update(&mut self, cx: &mut Context<Self>) {
        self.persist_then_quit(true, cx);
    }

    /// Save unsaved library changes, optionally start the updated executable, then quit.
    fn persist_then_quit(&mut self, relaunch: bool, cx: &mut Context<Self>) {
        if self.busy || self.preferences_busy {
            self.status = wh3_core::message!(
                "Wait for the current operation to finish",
                "Дождитесь завершения текущей операции"
            );
            cx.notify();
            return;
        }
        let settings = (!self.demo && self.dirty).then(|| {
            self.settings.current = Some(self.capture("Current".into()));
            self.settings.clone()
        });
        self.busy = true;
        let task = cx.background_spawn(async move {
            if let Some(settings) = settings {
                storage::save(&storage::settings_path()?, &settings)?;
            }
            if relaunch {
                wh3_core::update::relaunch()?;
            }
            Ok::<_, wh3_core::Error>(())
        });
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(()) => {
                        this.dirty = false;
                        cx.quit();
                    }
                    Err(error) => this.status = error.message(),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}
