//! Keeps the screen in step with the disk and the game: rescans after mod files change,
//! refreshes saves, notices the running game and starts a queued launch when it closes.
use super::Manager;
use gpui_kit::*;
use std::sync::Arc;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use wh3_core::{catalog::Source, game_process, localization::Message, message, watch};

/// Folders must be quiet this long before a rescan, so a download is read once, whole.
const QUIET: Duration = Duration::from_secs(2);
/// The game is looked for every this many ticks of the one-second loop.
const GAME_EVERY: u32 = 2;

/// What to start once the running game closes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Queued {
    Play,
    Continue,
}

struct Restore {
    generation: u64,
    dirty: bool,
    selected: Option<PathBuf>,
    status: Message,
    enabled: Vec<(String, Option<String>)>,
}

struct Vanished {
    name: String,
    after: Option<String>,
    at: Instant,
}

#[derive(Default)]
pub(crate) struct State {
    watcher: Option<watch::Watcher>,
    roots: Vec<(PathBuf, Source)>,
    seen_packs: u64,
    seen_saves: u64,
    changed_at: Option<Instant>,
    /// Restores dirty state and selection after an automatic rescan of this generation.
    restore: Option<Restore>,
    /// Enabled mods whose file disappeared, to re-enable when it comes back.
    vanished: Vec<Vanished>,
    /// A launch waiting until changed mod files have been rescanned (`Some(save)`).
    pub play_after_refresh: Option<Option<String>>,
    /// Staging ran since start, so cleanup after exit applies.
    pub staged: bool,
    /// Bytes in the staging folder, refreshed after launches and cleanups.
    pub staging_size: u64,
    pub priority_warned: bool,
    pub startup_cleanup_done: bool,
    pub staging_poll: Option<Task<()>>,
    pub game_running: bool,
    pub queued: Option<Queued>,
    checking_game: bool,
    ticks: u32,
    pub loop_task: Option<Task<()>>,
    setup: Option<Task<()>>,
    game_task: Option<Task<()>>,
}

impl State {
    /// Mod files changed and the rescan has not finished yet.
    pub fn files_settling(&self) -> bool {
        self.changed_at.is_some() || self.restore.is_some()
    }
}

impl Manager {
    /// One loop for every live check; it only reads counters on the UI thread.
    pub(super) fn start_live_updates(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            return;
        }
        let executor = cx.background_executor().clone();
        self.live.loop_task = Some(cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_secs(1)).await;
                if this.update(cx, |this, cx| this.live_tick(cx)).is_err() {
                    break;
                }
            }
        }));
    }

    fn live_tick(&mut self, cx: &mut Context<Self>) {
        let signals = self.live.watcher.as_ref().map(|w| w.signals.clone());
        if let Some(signals) = signals {
            let packs = signals.packs();
            if packs != self.live.seen_packs {
                self.live.seen_packs = packs;
                self.live.changed_at = Some(Instant::now());
            } else if self.live.changed_at.is_some_and(|at| at.elapsed() >= QUIET)
                && !self.busy
                && self.steam.pending.is_empty()
            {
                // Steam's own download poll rescans when it finishes; wait for it.
                self.live.changed_at = None;
                let status = message!(
                    "Mod files changed on disk; the list is up to date",
                    "Файлы модов изменились на диске; список обновлён"
                );
                self.rescan_keeping_state(Some(status), cx);
            }
            let saves = signals.saves();
            if saves != self.live.seen_saves {
                self.live.seen_saves = saves;
                self.refresh_saves(cx);
            }
        }
        self.live.ticks = self.live.ticks.wrapping_add(1);
        if cfg!(windows) && self.live.ticks.is_multiple_of(GAME_EVERY) {
            self.check_game(cx);
        }
    }

    fn check_game(&mut self, cx: &mut Context<Self>) {
        if self.live.checking_game {
            return;
        }
        self.live.checking_game = true;
        let task = cx.background_spawn(async { game_process::running() });
        self.live.game_task = Some(cx.spawn(async move |this, cx| {
            let running = task.await.unwrap_or(false);
            let _ = this.update(cx, |this, cx| {
                this.live.checking_game = false;
                if !this.live.startup_cleanup_done {
                    this.live.startup_cleanup_done = true;
                    this.startup_staging_cleanup(running, cx);
                }
                if running == this.live.game_running {
                    return;
                }
                this.live.game_running = running;
                this.on_game_state(running, cx);
                if !running {
                    this.refresh_saves(cx);
                    match this.live.queued.take() {
                        Some(Queued::Play) => this.play(None, cx),
                        Some(Queued::Continue) => {
                            let latest = this.launch.saves.first().map(|s| s.name.clone());
                            this.play(latest, cx);
                        }
                        None => {}
                    }
                }
                cx.notify();
            });
        }));
    }

    /// Watch the current folders; called after every scan, cheap when nothing changed.
    pub(super) fn follow_folders(&mut self, cx: &mut Context<Self>) {
        if self.demo || (self.live.watcher.is_some() && self.live.roots == self.settings.roots) {
            return;
        }
        let roots = self.settings.roots.clone();
        let saves = self.launch.folder.clone().or_else(wh3_core::saves::folder);
        self.live.roots = roots.clone();
        let task = cx.background_spawn(async move { watch::watch(&roots, saves.as_deref()) });
        self.live.setup = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(watcher) => {
                        this.live.seen_packs = watcher.signals.packs();
                        this.live.seen_saves = watcher.signals.saves();
                        this.live.watcher = Some(watcher);
                    }
                    Err(error) => {
                        this.live.watcher = None;
                        this.diagnostics.push(error.message());
                    }
                }
                cx.notify();
            });
        }));
    }

    /// A rescan the user did not ask for keeps their unsaved state and selection.
    /// `status` replaces the scan summary; `None` keeps the current message.
    pub(super) fn rescan_keeping_state(&mut self, status: Option<Message>, cx: &mut Context<Self>) {
        let selected = self
            .selected
            .and_then(|index| self.catalog.mods.get(index))
            .map(|item| item.path.clone());
        // Each enabled mod with the enabled mod before it, to put it back if Steam
        // removes and re-adds its file across two rescans (an update).
        let mut previous = None;
        let mut enabled = Vec::new();
        for &index in self.order.iter().filter(|i| self.enabled.contains(i)) {
            let name = self.catalog.mods[index].name.to_lowercase();
            enabled.push((name.clone(), previous.replace(name)));
        }
        let restore = Restore {
            generation: 0,
            dirty: self.dirty,
            selected,
            status: status.unwrap_or_else(|| self.status.clone()),
            enabled,
        };
        self.rescan(cx);
        self.live.restore = Some(Restore {
            generation: self.generation,
            ..restore
        });
    }

    /// Called at the end of `apply_scan`.
    pub(super) fn finish_refresh_from_disk(&mut self, cx: &mut Context<Self>) {
        let Some(restore) = self.live.restore.take() else {
            return;
        };
        if restore.generation != self.generation {
            return;
        }
        self.dirty = restore.dirty;
        self.selected = restore
            .selected
            .and_then(|path| self.catalog.mods.iter().position(|item| item.path == path));
        self.marked.extend(self.selected);
        self.status = restore.status;
        let by_name = &self.catalog.by_name;
        for (name, after) in restore.enabled {
            if !by_name.contains_key(&name) && !self.live.vanished.iter().any(|v| v.name == name) {
                self.live.vanished.push(Vanished {
                    name,
                    after,
                    at: Instant::now(),
                });
            }
        }
        self.restore_vanished(cx);
        if let Some(save) = self.live.play_after_refresh.take() {
            self.play(save, cx);
        }
    }

    /// Re-enable mods whose file came back, next to the mod they followed.
    fn restore_vanished(&mut self, cx: &mut Context<Self>) {
        self.live
            .vanished
            .retain(|v| v.at.elapsed() < Duration::from_secs(15 * 60));
        let mut restored = 0;
        for vanished in std::mem::take(&mut self.live.vanished) {
            let lookup = |name: &str| {
                self.catalog
                    .by_name
                    .get(name)
                    .and_then(|i| i.first())
                    .copied()
            };
            let Some(index) = lookup(&vanished.name) else {
                self.live.vanished.push(vanished);
                continue;
            };
            let after = vanished.after.as_deref().and_then(lookup);
            let order = Arc::make_mut(&mut self.order);
            if let (Some(from), Some(anchor)) = (
                order.iter().position(|&i| i == index),
                after.and_then(|a| order.iter().position(|&i| i == a)),
            ) {
                let item = order.remove(from);
                let to = if from < anchor { anchor } else { anchor + 1 };
                order.insert(to.min(order.len()), item);
            }
            self.enabled.insert(index);
            restored += 1;
        }
        if restored > 0 {
            self.rebuild_ranks();
            self.dirty = true;
            self.status = message!(
                "{} mods came back after an update and were enabled again",
                "Модов вернулось после обновления и снова включено: {}",
                restored
            );
            self.refresh_if_enabled_matters(cx);
            self.refresh_query(cx);
        }
    }

    pub(super) fn toggle_queued(&mut self, queued: Queued, cx: &mut Context<Self>) {
        self.live.queued = if self.live.queued == Some(queued) {
            None
        } else {
            Some(queued)
        };
        cx.notify();
    }

    pub(super) fn close_game(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let l = self.language;
        self.confirm(
            l.text(
                "Close Total War: WARHAMMER III?",
                "Закрыть Total War: WARHAMMER III?",
            )
            .into(),
            l.text(
                "The game is ended immediately; progress since your last save is lost.",
                "Игра будет завершена сразу; прогресс после последнего сохранения потеряется.",
            ),
            l.text("Close the game", "Закрыть игру"),
            |this, cx| {
                let task = cx.background_spawn(async { game_process::close() });
                cx.spawn(async move |this, cx| {
                    let result = task.await;
                    let _ = this.update(cx, |this, cx| {
                        this.status = match result {
                            Ok(()) => message!("The game was closed", "Игра закрыта"),
                            Err(error) => error.message(),
                        };
                        cx.notify();
                    });
                })
                .detach();
                this.status = message!("Closing the game…", "Закрываем игру…");
                cx.notify();
            },
            window,
            cx,
        );
    }
}
