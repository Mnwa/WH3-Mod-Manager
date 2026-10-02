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
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Поиск по названию, pack или Workshop ID…")
        });
        let preset_name = cx.new(|cx| InputState::new(window, cx).placeholder("Название пресета"));
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
            status: "Загрузка…".into(),
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
                        this.status = error.to_string().into();
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
            name: "Текущий".into(),
            mods: vec![],
            version: None,
        });
        self.apply_preset(&preset, cx);
        self.diagnostics
            .extend(scan.warnings.into_iter().map(SharedString::from));
        self.status = if self.demo {
            "Демонстрация · файлы игры не используются".into()
        } else {
            format!(
                "Найдено {} модов · предупреждений: {}",
                self.catalog.mods.len(),
                self.diagnostics.len()
            )
            .into()
        };
        cx.notify();
    }

    pub(super) fn rescan(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.demo {
            return;
        }
        self.settings.current = Some(self.capture("Текущий".into()));
        self.busy = true;
        self.cancellable = true;
        self.status = "Сканирование файлов…".into();
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
                    Err(error) => this.status = error.to_string().into(),
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

    pub(super) fn capture(&self, name: String) -> Preset {
        Preset::capture(name, &self.catalog, &self.order, &self.enabled)
    }

    pub(super) fn apply_preset(&mut self, preset: &Preset, cx: &mut Context<Self>) {
        let applied = preset.apply(&self.catalog);
        self.order = Arc::new(applied.order);
        self.rebuild_ranks();
        self.enabled = applied.enabled;
        self.status = format!(
            "Пресет «{}» · отсутствует модов: {}",
            preset.name,
            applied.missing.len()
        )
        .into();
        self.diagnostics = applied
            .missing
            .into_iter()
            .map(|name| format!("Не найден мод: {name}").into())
            .collect();
        self.dirty = true;
        self.refresh_query(cx);
        cx.notify();
    }

    pub(super) fn rebuild_ranks(&mut self) {
        self.ranks.resize(self.catalog.mods.len(), 0);
        for (rank, &index) in self.order.iter().enumerate() {
            self.ranks[index] = rank + 1;
        }
    }

    pub(super) fn save(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.demo {
            return;
        }
        self.settings.current = Some(self.capture("Текущий".into()));
        let settings = self.settings.clone();
        self.busy = true;
        self.status = "Сохранение…".into();
        let task = cx
            .background_spawn(async move { storage::save(&storage::settings_path()?, &settings) });
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(()) => {
                        this.dirty = false;
                        this.status = "Настройки и порядок модов сохранены".into();
                    }
                    Err(error) => this.status = error.to_string().into(),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(crate) fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.demo || (!self.dirty && !self.busy) {
            return true;
        }
        if self.busy {
            self.status = "Дождитесь завершения операции перед закрытием".into();
            cx.notify();
            return false;
        }
        self.settings.current = Some(self.capture("Текущий".into()));
        let settings = self.settings.clone();
        let handle = window.window_handle();
        self.busy = true;
        let task = cx
            .background_spawn(async move { storage::save(&storage::settings_path()?, &settings) });
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let succeeded = result.is_ok();
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(()) => this.dirty = false,
                    Err(error) => {
                        this.status =
                            format!("Не удалось сохранить перед закрытием: {error}").into()
                    }
                }
                cx.notify();
            });
            if succeeded {
                let _ = cx.update_window(handle, |_, window, _| window.remove_window());
            } else if let Ok(prompt) = cx.update_window(handle, |_, window, cx| {
                window.prompt(PromptLevel::Warning, "Не удалось сохранить библиотеку", Some("Можно остаться и исправить ошибку или закрыть окно без сохранения последних изменений."), &["Остаться", "Закрыть без сохранения"], cx)
            }) && prompt.await == Ok(1) {
                let _ = cx.update_window(handle, |_, window, _| window.remove_window());
            }
        }));
        false
    }
}
