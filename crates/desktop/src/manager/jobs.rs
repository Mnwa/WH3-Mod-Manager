use super::{Filter, Manager};
use gpui_kit::{
    component::input::{InputEvent, InputState},
    *,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use wh3_core::{catalog::Catalog, preset::Preset, scan, steam, storage};

impl Manager {
    pub fn new(demo: Option<usize>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        crate::i18n::install();
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(
                wh3_core::localization::Language::default().text(
                    "Search by name, pack or Workshop ID…",
                    "Поиск по названию, pack или Workshop ID…",
                ),
            )
        });
        let preset_name = cx.new(|cx| {
            InputState::new(window, cx).placeholder(
                wh3_core::localization::Language::default().text("Preset name", "Название пресета"),
            )
        });
        let subscription = cx.subscribe(&search, |this: &mut Self, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                this.refresh_query(cx);
            }
        });
        let mut this = Self {
            catalog: Arc::default(),
            order: Arc::default(),
            ranks: vec![],
            sort_name: false,
            enabled: Default::default(),
            visible: vec![],
            selected: None,
            search,
            preset_name,
            filter: Filter::All,
            settings: Default::default(),
            language: Default::default(),
            preferences_task: None,
            preferences_busy: false,
            status: wh3_core::message!("Loading…", "Загрузка…"),
            diagnostics: vec![],
            details: vec![],
            show_report: false,
            busy: true,
            cancellable: demo.is_none(),
            demo: demo.is_some(),
            dirty: false,
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            cancel: Arc::new(AtomicBool::new(false)),
            generation: 0,
            query_generation: 0,
            job: None,
            search_task: None,
            io_task: None,
            _subscriptions: vec![subscription],
            rendered_rows: 0,
        };
        this.load_language(window, cx);
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
        self.details.clear();
        let preset = self.settings.current.clone().unwrap_or(Preset {
            name: "Current".into(),
            mods: vec![],
            version: None,
        });
        self.apply_preset(&preset, cx);
        self.diagnostics.extend(scan.warnings);
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

    pub(super) fn refresh_query(&mut self, cx: &mut Context<Self>) {
        self.query_generation += 1;
        let generation = self.query_generation;
        let query = self.search.read(cx).value().to_string();
        let catalog = self.catalog.clone();
        let order = self.order.clone();
        let filter = self.filter;
        let sort_name = self.sort_name;
        let enabled = if filter == Filter::All {
            None
        } else {
            Some(self.enabled.clone())
        };
        let executor = cx.background_executor().clone();
        self.search_task = Some(cx.spawn(async move |this, cx| {
            executor.timer(Duration::from_millis(75)).await;
            let visible = executor
                .spawn(async move {
                    let mut visible = catalog.query(&query, &order);
                    if sort_name {
                        visible.sort_by(|&a, &b| {
                            catalog.mods[a]
                                .title
                                .cmp(&catalog.mods[b].title)
                                .then(a.cmp(&b))
                        });
                    }
                    if let Some(enabled) = enabled {
                        visible.retain(|i| enabled.contains(i) == (filter == Filter::Enabled));
                    }
                    visible
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.query_generation {
                    return;
                }
                this.visible = visible;
                cx.notify();
            });
        }));
    }
}
