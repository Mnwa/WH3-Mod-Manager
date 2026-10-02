//! Workshop staging around a launch: progress while preparing, cleanup after the game
//! closes, and the game's process priority.
use super::Manager;
use gpui_kit::*;
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use wh3_core::{game_process, message, staging, storage::Staging};

impl Manager {
    /// Show "Preparing Workshop mods: n / total" until the launch job finishes.
    pub(super) fn follow_staging(
        &mut self,
        progress: Arc<staging::Progress>,
        cx: &mut Context<Self>,
    ) {
        let executor = cx.background_executor().clone();
        self.live.staging_poll = Some(cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_millis(250)).await;
                let (done, total) = (
                    progress.done.load(Ordering::Relaxed),
                    progress.total.load(Ordering::Relaxed),
                );
                let alive = this.update(cx, |this, cx| {
                    this.status = message!(
                        "Preparing Workshop mods in the game folder: {} / {}",
                        "Подготовка модов Workshop в папке игры: {} / {}",
                        done,
                        total
                    );
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        }));
    }

    pub(super) fn refresh_staging_size(&mut self, cx: &mut Context<Self>) {
        let Some(game) = self.settings.game_path.clone().filter(|_| !self.demo) else {
            return;
        };
        let task = cx.background_spawn(async move { staging::size(&game) });
        cx.spawn(async move |this, cx| {
            let size = task.await;
            let _ = this.update(cx, |this, cx| {
                this.live.staging_size = size;
                cx.notify();
            });
        })
        .detach();
    }

    /// Delete the prepared copies or links; refused while the game may be using them.
    pub(super) fn clean_staging(&mut self, cx: &mut Context<Self>) {
        let Some(game) = self.settings.game_path.clone().filter(|_| !self.demo) else {
            return;
        };
        if self.live.game_running {
            self.status = message!(
                "Close the game before deleting the prepared Workshop mods",
                "Закройте игру, прежде чем удалять подготовленные моды Workshop"
            );
            cx.notify();
            return;
        }
        let task = cx.background_spawn(async move { staging::clean(&game) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.status = match result {
                    Ok(()) => {
                        this.live.staged = false;
                        message!(
                            "Prepared Workshop mods were deleted from the game folder",
                            "Подготовленные моды Workshop удалены из папки игры"
                        )
                    }
                    Err(error) => error.message(),
                };
                this.refresh_staging_size(cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Clean up a folder left by a previous session that closed with the game.
    pub(super) fn startup_staging_cleanup(&mut self, running: bool, cx: &mut Context<Self>) {
        self.refresh_staging_size(cx);
        if running {
            // The running game may be using it; clean when it exits.
            self.live.staged = true;
        } else if self.settings.options.clean_up_staging {
            self.clean_staging(cx);
        }
    }

    pub(super) fn on_game_state(&mut self, running: bool, cx: &mut Context<Self>) {
        let options = self.settings.options;
        if running && options.raise_priority {
            let task = cx.background_spawn(async { game_process::raise_priority() });
            cx.spawn(async move |this, cx| {
                if let Err(error) = task.await {
                    let _ = this.update(cx, |this, cx| {
                        // Report a refusal once per session, as the original does.
                        if !this.live.priority_warned {
                            this.live.priority_warned = true;
                            this.diagnostics.push(error.message());
                            this.status = error.message();
                            cx.notify();
                        }
                    });
                }
            })
            .detach();
        }
        // Also covers a folder left by an earlier session that closed with the game.
        if !running
            && options.clean_up_staging
            && options.staging != Staging::Off
            && (self.live.staged || self.live.staging_size > 0)
        {
            self.clean_staging(cx);
        }
    }
}
