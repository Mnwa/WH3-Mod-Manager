use gpui_kit::*;
use std::time::Duration;
use wh3_core::{localization::Message, update};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug)]
pub enum Phase {
    Idle,
    Checking,
    Available(update::Available),
    Installing(String),
    Installed(String),
    Failed(Message),
}

/// The parent saves its library before the replaced executable is relaunched.
pub enum UpdaterEvent {
    RestartRequested,
}

pub struct Updater {
    phase: Phase,
    task: Option<Task<()>>,
}

impl EventEmitter<UpdaterEvent> for Updater {}

impl Updater {
    /// Only published Windows builds check automatically: a debug build's version
    /// does not come from a release tag, and demo mode never touches user files.
    pub fn new(demo: bool, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            phase: Phase::Idle,
            task: None,
        };
        if cfg!(windows) && !cfg!(debug_assertions) && !demo {
            this.schedule_check(Duration::from_secs(3), cx);
        }
        this
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn check(&mut self, cx: &mut Context<Self>) {
        if matches!(self.phase, Phase::Idle | Phase::Failed(_)) {
            self.schedule_check(Duration::ZERO, cx);
        }
    }

    fn schedule_check(&mut self, delay: Duration, cx: &mut Context<Self>) {
        self.phase = Phase::Checking;
        let executor = cx.background_executor().clone();
        self.task = Some(cx.spawn(async move |this, cx| {
            executor.timer(delay).await;
            let result = executor
                .spawn(async { update::check(CURRENT_VERSION) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.phase = match result {
                    Ok(Some(available)) => Phase::Available(available),
                    Ok(None) => Phase::Idle,
                    Err(error) => Phase::Failed(error.message()),
                };
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub fn install(&mut self, cx: &mut Context<Self>) {
        let Phase::Available(available) = &self.phase else {
            return;
        };
        self.phase = Phase::Installing(available.version.clone());
        let task = cx.background_spawn(async { update::install(CURRENT_VERSION) });
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.phase = match result {
                    Ok(version) => Phase::Installed(version),
                    Err(error) => Phase::Failed(error.message()),
                };
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub fn request_restart(&mut self, cx: &mut Context<Self>) {
        if matches!(self.phase, Phase::Installed(_)) {
            cx.emit(UpdaterEvent::RestartRequested);
        }
    }
}
