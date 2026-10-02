use super::{Filter, Manager, Sort, SortKey};
use crate::update::{Updater, UpdaterEvent};
use gpui_kit::{
    component::input::{InputEvent, InputState},
    *,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use wh3_core::{catalog::Catalog, preset::Preset, scan, steam, storage};

impl Manager {
    pub fn new(demo: Option<usize>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        crate::i18n::install();
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(
                wh3_core::localization::Language::default().text(
                    "Search by title, author, pack or Workshop ID (Ctrl+F)",
                    "Поиск по названию, автору, pack или Workshop ID (Ctrl+F)",
                ),
            )
        });
        let preset_name = cx.new(|cx| {
            InputState::new(window, cx).placeholder(
                wh3_core::localization::Language::default()
                    .text("New preset name", "Название нового пресета"),
            )
        });
        let subscription = cx.subscribe(&search, |this: &mut Self, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                this.refresh_query(cx);
            }
        });
        let updater = cx.new(|cx| Updater::new(demo.is_some(), cx));
        let restart = cx.subscribe(&updater, |this: &mut Self, _, event, cx| match event {
            UpdaterEvent::RestartRequested => this.restart_for_update(cx),
        });
        let update_phase = cx.observe(&updater, |_, _, cx| cx.notify());
        let mut this = Self {
            catalog: Arc::default(),
            order: Arc::default(),
            ranks: vec![],
            sort: Sort {
                key: SortKey::Order,
                descending: false,
            },
            enabled: Default::default(),
            visible: vec![],
            selected: None,
            marked: Default::default(),
            anchor: None,
            search,
            preset_name,
            filter: Filter::All,
            category: None,
            categories: vec![],
            settings: Default::default(),
            language: Default::default(),
            preferences_task: None,
            preferences_save: None,
            prefs: Default::default(),
            visible_enabled: vec![],
            groups: Arc::default(),
            collapsed: None,
            can_link: false,
            preferences_busy: false,
            status: wh3_core::message!("Loading…", "Загрузка…"),
            diagnostics: vec![],
            details: vec![],
            compat: Default::default(),
            report_tab: super::compat::Tab::Diagnostics,
            show_report: false,
            busy: true,
            cancellable: demo.is_none(),
            demo: demo.is_some(),
            dirty: false,
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            enabled_scroll: UniformListScrollHandle::new(),
            grouped: vec![],
            report_scroll: UniformListScrollHandle::new(),
            preset_scroll: UniformListScrollHandle::new(),
            cancel: Arc::new(AtomicBool::new(false)),
            generation: 0,
            query_generation: 0,
            job: None,
            search_task: None,
            io_task: None,
            launch: Default::default(),
            steam: Default::default(),
            rules: Default::default(),
            live: Default::default(),
            hunt: None,
            updater,
            window_title: String::new(),
            _subscriptions: vec![subscription, restart, update_phase],
            rendered_rows: 0,
        };
        this.load_language(window, cx);
        this.start_live_updates(cx);
        this.probe_links(cx);
        let cancel = this.cancel.clone();
        let task = cx.background_spawn(async move {
            if let Some(count) = demo {
                return Ok((
                    storage::Settings::default(),
                    scan::Scan {
                        catalog: Catalog::demo(count),
                        warnings: vec![],
                    },
                ));
            }
            let mut settings = storage::load(&storage::settings_path()?)?;
            if settings.game_path.is_none() && settings.roots.is_empty() {
                settings = steam::discover();
            }
            let mut result = scan::scan(&settings.roots, &cancel)?;
            wh3_core::metadata::apply(&mut result.catalog, &settings.metadata);
            Ok::<_, wh3_core::Error>((settings, result))
        });
        this.job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.cancellable = false;
                match result {
                    Ok((settings, scan)) => {
                        this.settings = settings;
                        this.apply_scan(scan, cx);
                        this.dirty = false;
                        this.refresh_saves(cx);
                    }
                    Err(error) => {
                        this.status = error.message();
                        cx.notify();
                    }
                }
            });
        }));
        this
    }

    pub(super) fn apply_scan(&mut self, scan: scan::Scan, cx: &mut Context<Self>) {
        self.catalog = Arc::new(scan.catalog);
        self.selected = None;
        self.marked.clear();
        self.anchor = None;
        self.details.clear();
        self.compat = Default::default();
        self.rebuild_categories();
        let preset = self.settings.current.clone().unwrap_or(Preset {
            name: "Current".into(),
            mods: vec![],
            version: None,
        });
        self.apply_preset(&preset, cx);
        // Automatic rescans repeat the same warnings; keep each one once.
        for warning in scan.warnings {
            let english = wh3_core::localization::Language::English;
            if !self
                .diagnostics
                .iter()
                .any(|m| m.text(english) == warning.text(english))
            {
                self.diagnostics.push(warning);
            }
        }
        self.rebuild_workshop_ids();
        self.refresh_outdated(cx);
        self.refresh_workshop(cx);
        self.scan_pack_rules(cx);
        self.status = if self.demo {
            wh3_core::message!(
                "Demo · no game files are used",
                "Демонстрация · файлы игры не используются"
            )
        } else {
            wh3_core::message!(
                "Found {} mods · {} warnings",
                "Найдено {} модов · предупреждений: {}",
                self.catalog.mods.len(),
                self.diagnostics.len()
            )
        };
        // Follow-ups report their own outcome, so they run after the scan summary.
        self.finish_refresh_from_disk(cx);
        self.after_steam_rescan();
        self.finish_shared_import(cx);
        self.continue_reinstall(cx);
        self.follow_folders(cx);
        cx.notify();
    }

    pub(super) fn rescan(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.demo {
            return;
        }
        self.settings.current = Some(self.capture("Current".into()));
        self.busy = true;
        self.cancellable = true;
        self.status = wh3_core::message!("Scanning files…", "Сканирование файлов…");
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.generation += 1;
        let generation = self.generation;
        let cancel = self.cancel.clone();
        let roots = self.settings.roots.clone();
        let metadata = self.settings.metadata.clone();
        let task = cx.background_spawn(async move {
            let mut result = scan::scan(&roots, &cancel)?;
            wh3_core::metadata::apply(&mut result.catalog, &metadata);
            Ok::<_, wh3_core::Error>(result)
        });
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.generation {
                    return;
                }
                this.busy = false;
                this.cancellable = false;
                match result {
                    Ok(scan) => this.apply_scan(scan, cx),
                    Err(error) => this.status = error.message(),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}
