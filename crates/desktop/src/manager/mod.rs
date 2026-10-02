//! Mod-management screen: library state, background jobs and the views that render it.
mod actions;
mod badges;
mod categories;
mod columns;
mod compat;
mod data_menu;
mod dialogs;
mod empty;
mod flags;
mod game_folder;
mod groups;
mod header;
mod hunt;
mod hunt_view;
mod jobs;
mod language;
mod launch;
mod list;
mod metadata;
mod multi;
mod order;
mod panes;
mod persistence;
mod play_controls;
mod preset_view;
mod presets;
mod query;
mod reinstall;
mod report;
mod row;
mod row_menu;
mod row_menu_data;
mod rules;
mod rules_view;
mod selection;
mod settings_menu;
mod settings_staging;
mod sharing;
mod sidebar;
mod snapshot_view;
mod staging_control;
mod steam;
mod toolbar;
mod view;
mod view_menu;
mod watching;
mod workshop_menu;

use crate::update::Updater;
use gpui_kit::{component::input::InputState, *};
use std::{
    collections::HashSet,
    sync::{Arc, atomic::AtomicBool},
};
use wh3_core::{catalog::Catalog, localization::Message, storage::Settings};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Enabled,
    Disabled,
    Hidden,
}

/// Table sorting is a view concern and never changes the game load order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Order,
    Enabled,
    Title,
    Pack,
    Author,
    Updated,
    Size,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
    pub key: SortKey,
    pub descending: bool,
}

impl Sort {
    /// Dragging rows reorders the load order, so it is only meaningful in that view.
    pub fn is_load_order(self) -> bool {
        self.key == SortKey::Order && !self.descending
    }
}

pub struct Manager {
    pub(super) catalog: Arc<Catalog>,
    pub(super) order: Arc<Vec<usize>>,
    pub(super) ranks: Vec<usize>,
    pub(super) sort: Sort,
    pub(super) enabled: HashSet<usize>,
    pub(super) visible: Vec<usize>,
    pub(super) selected: Option<usize>,
    /// Multi-selection (catalog indices); `selected` is its focused row.
    pub(super) marked: std::collections::BTreeSet<usize>,
    pub(super) anchor: Option<usize>,
    pub(super) search: Entity<InputState>,
    pub(super) preset_name: Entity<InputState>,
    pub(super) filter: Filter,
    pub(super) category: Option<Arc<str>>,
    /// Category names with mod counts, rebuilt when metadata changes, not per frame.
    pub(super) categories: Vec<(Arc<str>, usize)>,
    pub(super) settings: Settings,
    pub(super) status: Message,
    pub(super) language: wh3_core::localization::Language,
    pub(super) preferences_task: Option<Task<()>>,
    /// The debounced preferences write.
    pub(super) preferences_save: Option<Task<()>>,
    /// Language, layout, row size and column widths (WHP2).
    pub(super) prefs: wh3_core::preferences::Preferences,
    /// Enabled mods in load order for the right pane of the two-list layout.
    pub(super) visible_enabled: Vec<usize>,
    /// Category groups of the main list, rebuilt with the query, not per frame.
    pub(super) groups: Arc<Vec<(Arc<str>, Vec<usize>)>>,
    /// Collapsed category groups; all start collapsed, like the original.
    pub(super) collapsed: Option<std::collections::HashSet<Arc<str>>>,
    /// Whether symbolic links can be created (administrator or Developer Mode).
    pub(super) can_link: bool,
    pub(super) preferences_busy: bool,
    pub(super) diagnostics: Vec<Message>,
    pub(super) details: Vec<Message>,
    pub(super) compat: compat::State,
    pub(super) report_tab: compat::Tab,
    pub(super) show_report: bool,
    pub(super) busy: bool,
    pub(super) cancellable: bool,
    pub(super) demo: bool,
    pub(super) dirty: bool,
    pub(super) focus: FocusHandle,
    pub(super) scroll: UniformListScrollHandle,
    pub(super) enabled_scroll: UniformListScrollHandle,
    pub(super) grouped: Vec<groups::GroupRow>,
    pub(super) report_scroll: UniformListScrollHandle,
    pub(super) preset_scroll: UniformListScrollHandle,
    pub(super) cancel: Arc<AtomicBool>,
    pub(super) generation: u64,
    pub(super) query_generation: u64,
    pub(super) job: Option<Task<()>>,
    pub(super) search_task: Option<Task<()>>,
    pub(super) io_task: Option<Task<()>>,
    pub(super) launch: launch::State,
    pub(super) steam: steam::State,
    pub(super) rules: rules::State,
    /// Folder watching, game process state and a launch queued until the game closes.
    pub(super) live: watching::State,
    /// The running problem-mod search, if any.
    pub(super) hunt: Option<hunt::State>,
    pub(super) updater: Entity<Updater>,
    pub(super) window_title: String,
    pub(super) _subscriptions: Vec<Subscription>,
    pub rendered_rows: usize,
}

impl Drop for Manager {
    fn drop(&mut self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
