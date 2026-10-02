mod actions;
mod jobs;
mod metadata;
mod view;

use gpui_kit::{component::input::InputState, *};
use std::{
    collections::HashSet,
    sync::{Arc, atomic::AtomicBool},
};
use wh3_core::{catalog::Catalog, storage::Settings};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Enabled,
    Disabled,
}

pub struct Manager {
    pub(super) catalog: Arc<Catalog>,
    pub(super) order: Arc<Vec<usize>>,
    pub(super) ranks: Vec<usize>,
    pub(super) sort_name: bool,
    pub(super) enabled: HashSet<usize>,
    pub(super) visible: Vec<usize>,
    pub(super) selected: Option<usize>,
    pub(super) search: Entity<InputState>,
    pub(super) preset_name: Entity<InputState>,
    pub(super) filter: Filter,
    pub(super) settings: Settings,
    pub(super) status: SharedString,
    pub(super) diagnostics: Vec<SharedString>,
    pub(super) details: Vec<SharedString>,
    pub(super) show_report: bool,
    pub(super) busy: bool,
    pub(super) cancellable: bool,
    pub(super) demo: bool,
    pub(super) dirty: bool,
    pub(super) focus: FocusHandle,
    pub(super) scroll: UniformListScrollHandle,
    pub(super) cancel: Arc<AtomicBool>,
    pub(super) generation: u64,
    pub(super) query_generation: u64,
    pub(super) job: Option<Task<()>>,
    pub(super) search_task: Option<Task<()>>,
    pub(super) io_task: Option<Task<()>>,
    pub(super) _subscriptions: Vec<Subscription>,
    pub rendered_rows: usize,
}

impl Drop for Manager {
    fn drop(&mut self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

mod list;
mod shell;
