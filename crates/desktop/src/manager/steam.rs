//! Steam Workshop actions: metadata refresh, updates, subscriptions and collections.
use super::Manager;
use gpui_kit::*;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use wh3_core::{
    catalog::Catalog,
    message,
    workshop::{self, Item, Request, Response, Runtime, merge},
};

/// Workshop data fetched for the current catalog; refreshed, never persisted
/// except for the metadata it updates.
#[derive(Default)]
pub(crate) struct State {
    pub items: Arc<HashMap<u64, Item>>,
    /// Catalog indices by Workshop id, rebuilt with the catalog rather than per row.
    pub ids: HashMap<u64, Vec<usize>>,
    /// Catalog indices with a newer Workshop revision than the installed copy.
    pub outdated: HashSet<usize>,
    /// Items Steam is still working on, mapped to whether they should end up
    /// installed (download, subscribe) or removed (unsubscribe). The library is
    /// rescanned once every item reaches its state.
    pub pending: HashMap<u64, bool>,
    /// Items to enable once they appear in the catalog (subscribed requirements).
    pub enable_after: HashSet<u64>,
    /// A collection whose preset is created after its items are installed.
    pub collection: Option<(String, Vec<u64>)>,
    pub busy: bool,
    pub task: Option<Task<()>>,
    pub poll: Option<Task<()>>,
}

/// Blocks for a worker round trip (about a second); call it only inside
/// `background_spawn`/background executor tasks, never on the UI thread.
fn call(request: Request) -> wh3_core::Result<Response> {
    workshop::call(&Runtime::current()?, &request)
}

fn workshop_ids(catalog: &Catalog) -> Vec<u64> {
    let mut ids: Vec<u64> = merge::by_id(catalog).into_keys().collect();
    ids.sort_unstable();
    ids
}

impl Manager {
    pub(super) fn rebuild_workshop_ids(&mut self) {
        self.steam.ids = merge::by_id(&self.catalog);
    }

    pub(super) fn steam_available(&self) -> bool {
        cfg!(windows) && !self.demo && !self.busy && !self.steam.busy
    }

    /// Fetch titles, authors, requirements and update state for installed Workshop mods.
    pub(super) fn refresh_workshop(&mut self, cx: &mut Context<Self>) {
        if !cfg!(windows) || self.demo || self.steam.busy {
            return;
        }
        let ids = workshop_ids(&self.catalog);
        if ids.is_empty() {
            return;
        }
        let (catalog, mut metadata, generation) = (
            self.catalog.clone(),
            self.settings.metadata.clone(),
            self.generation,
        );
        self.steam.busy = true;
        let task = cx.background_spawn(async move {
            let Response::Details(mut items) = call(Request::Details { ids: ids.clone() })? else {
                return Err(wh3_core::Error::Steam("unexpected answer".into()));
            };
            let unknown = merge::unknown_required(&items);
            if !unknown.is_empty()
                && let Ok(Response::Details(extra)) = call(Request::Details { ids: unknown })
            {
                items.extend(extra);
            }
            let Response::States(states) = call(Request::State { ids })? else {
                return Err(wh3_core::Error::Steam("unexpected answer".into()));
            };
            let mut catalog = (*catalog).clone();
            let matched = merge::merge(&mut catalog, &mut metadata, &items);
            let outdated = merge::outdated(&catalog, &items, &states);
            Ok((catalog, metadata, items, outdated, matched))
        });
        self.steam.task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.steam.busy = false;
                match result {
                    // A rescan in the meantime replaced the catalog; its own refresh follows.
                    Ok(_) if generation != this.generation => {}
                    Ok((catalog, metadata, items, outdated, matched)) => {
                        this.catalog = Arc::new(catalog);
                        this.rebuild_workshop_ids();
                        this.settings.metadata = metadata;
                        this.steam.items =
                            Arc::new(items.into_iter().map(|item| (item.id, item)).collect());
                        this.steam.outdated = outdated;
                        this.rebuild_categories();
                        // Workshop revision times replace file times, so re-evaluate age.
                        this.refresh_outdated(cx);
                        this.status = message!(
                            "Workshop data updated for {} mods · {} have updates",
                            "Данные Workshop обновлены для {} модов · обновлений: {}",
                            matched,
                            this.steam.outdated.len()
                        );
                        this.refresh_query(cx);
                    }
                    Err(error) => this.diagnostics.push(error.message()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Subscribe, unsubscribe or download, then follow Steam until files are in place.
    pub(super) fn steam_action(&mut self, request: Request, cx: &mut Context<Self>) {
        if !self.steam_available() {
            return;
        }
        let ids = match &request {
            Request::Subscribe { ids }
            | Request::Unsubscribe { ids }
            | Request::Download { ids } => ids.clone(),
            _ => return,
        };
        if ids.is_empty() {
            return;
        }
        let install = !matches!(request, Request::Unsubscribe { .. });
        self.steam.busy = true;
        self.status = message!("Sending the request to Steam…", "Отправка запроса в Steam…");
        let task = cx.background_spawn(async move { call(request) });
        self.steam.task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.steam.busy = false;
                match result {
                    Ok(Response::Done { accepted, failed }) => {
                        this.diagnostics.extend(failed.iter().map(|(id, error)| {
                            message!("Workshop item {}: {}", "Мод Workshop {}: {}", id, error)
                        }));
                        if !failed.is_empty() {
                            this.show_report_tab(super::compat::Tab::Diagnostics, cx);
                        }
                        this.status = message!(
                            "Steam accepted {} of {} items",
                            "Steam принял {} из {} модов",
                            accepted.len(),
                            ids.len()
                        );
                        this.steam
                            .pending
                            .extend(accepted.into_iter().map(|id| (id, install)));
                        this.poll_downloads(cx);
                    }
                    Ok(_) => {}
                    Err(error) => this.status = error.message(),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Poll item state every two seconds; rescan once downloads are installed and
    /// unsubscribed items are gone, so the list never shows half-written files.
    fn poll_downloads(&mut self, cx: &mut Context<Self>) {
        let executor = cx.background_executor().clone();
        self.steam.poll = Some(cx.spawn(async move |this, cx| {
            for _ in 0..600 {
                executor.timer(Duration::from_secs(2)).await;
                let Ok(ids) = this.update(cx, |this, _| {
                    this.steam.pending.keys().copied().collect::<Vec<_>>()
                }) else {
                    return;
                };
                if ids.is_empty() {
                    break;
                }
                let states = executor
                    .spawn(async move { call(Request::State { ids }) })
                    .await;
                let Ok(Response::States(states)) = states else {
                    continue;
                };
                let installed = |s: &workshop::State| {
                    !s.busy()
                        && s.has(workshop::STATE_INSTALLED)
                        && !s.has(workshop::STATE_NEEDS_UPDATE)
                };
                let (downloaded, total) = states
                    .iter()
                    .fold((0, 0), |(d, t), s| (d + s.downloaded, t + s.total));
                let finished = this
                    .update(cx, |this, cx| {
                        for state in &states {
                            if this.steam.pending.get(&state.id) == Some(&installed(state)) {
                                this.steam.pending.remove(&state.id);
                            }
                        }
                        this.status = message!(
                            "Waiting for Steam: {} items · {:.1} / {:.1} MiB",
                            "Ожидание Steam: модов {} · {:.1} / {:.1} МБ",
                            this.steam.pending.len(),
                            downloaded as f64 / 1_048_576.,
                            total as f64 / 1_048_576.
                        );
                        cx.notify();
                        this.steam.pending.is_empty()
                    })
                    .unwrap_or(true);
                if finished {
                    break;
                }
            }
            let _ = this.update(cx, |this, cx| {
                this.steam.pending.clear();
                this.rescan(cx);
            });
        }));
    }

    /// After a rescan: enable subscribed requirements and build a pending collection preset.
    pub(super) fn after_steam_rescan(&mut self) {
        let ids = &self.steam.ids;
        let enable: Vec<u64> = self
            .steam
            .enable_after
            .iter()
            .copied()
            .filter(|id| ids.contains_key(id))
            .collect();
        for id in enable {
            self.steam.enable_after.remove(&id);
            if let Some(&index) = ids.get(&id).and_then(|indices| indices.first()) {
                self.enabled.insert(index);
                self.dirty = true;
            }
        }
        if let Some((title, items)) = self.steam.collection.take() {
            let mut preset = wh3_core::preset::Preset {
                name: title,
                mods: vec![],
                version: Some(2),
            };
            for id in items {
                if let Some(&index) = ids.get(&id).and_then(|indices| indices.first()) {
                    preset.mods.push(wh3_core::preset::Entry {
                        name: self.catalog.mods[index].name.to_string(),
                        is_enabled: true,
                        load_order: None,
                    });
                }
            }
            self.status = message!(
                "Preset “{}” created from the collection with {} mods",
                "Из коллекции создан пресет «{}»: модов {}",
                preset.name,
                preset.mods.len()
            );
            self.store_preset(preset);
            self.dirty = true;
        }
    }

    /// Subscribe to missing requirements and enable disabled ones.
    pub(super) fn fix_requirements(&mut self, ids: Vec<u64>, cx: &mut Context<Self>) {
        let installed = &self.steam.ids;
        let mut subscribe = Vec::new();
        for id in ids {
            match installed.get(&id).and_then(|indices| indices.first()) {
                Some(&index) => {
                    self.enabled.insert(index);
                    self.dirty = true;
                }
                None => subscribe.push(id),
            }
        }
        self.steam.enable_after.extend(subscribe.iter().copied());
        self.refresh_if_enabled_matters(cx);
        self.steam_action(Request::Subscribe { ids: subscribe }, cx);
    }

    /// Resolve a collection link through Steam, subscribe to its items and queue its preset.
    pub(super) fn import_collection(&mut self, link: String, cx: &mut Context<Self>) {
        let Some(id) = workshop::parse_id(&link) else {
            self.status = message!(
                "Not a Steam collection link or id",
                "Это не ссылка или id коллекции Steam"
            );
            cx.notify();
            return;
        };
        if !self.steam_available() {
            return;
        }
        self.steam.busy = true;
        let task = cx.background_spawn(async move {
            match call(Request::Details { ids: vec![id] })? {
                Response::Details(items) => Ok(items.into_iter().next()),
                _ => Ok::<_, wh3_core::Error>(None),
            }
        });
        self.steam.task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.steam.busy = false;
                match result {
                    Ok(Some(collection)) if !collection.required.is_empty() => {
                        let installed = &this.steam.ids;
                        let missing: Vec<u64> = collection
                            .required
                            .iter()
                            .copied()
                            .filter(|id| !installed.contains_key(id))
                            .collect();
                        this.steam.collection = Some((collection.title, collection.required));
                        if missing.is_empty() {
                            this.after_steam_rescan();
                        } else {
                            this.steam_action(Request::Subscribe { ids: missing }, cx);
                        }
                    }
                    Ok(_) => {
                        this.status = message!(
                            "The collection is empty or unavailable",
                            "Коллекция пуста или недоступна"
                        )
                    }
                    Err(error) => this.status = error.message(),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}
