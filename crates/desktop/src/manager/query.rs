use super::{Filter, Manager, SortKey};
use gpui_kit::*;
use std::{collections::HashSet, sync::Arc, time::Duration};
use wh3_core::catalog::Catalog;

/// Indices of the catalog entries named by a set of lowercase pack names.
pub(super) fn indices_of<'a>(
    catalog: &Catalog,
    names: impl IntoIterator<Item = &'a String>,
) -> HashSet<usize> {
    names
        .into_iter()
        .filter_map(|name| catalog.by_name.get(name))
        .flatten()
        .copied()
        .collect()
}

impl Manager {
    pub(super) fn hidden_indices(&self) -> HashSet<usize> {
        indices_of(&self.catalog, &self.settings.hidden)
    }

    pub(super) fn is_hidden(&self, index: usize) -> bool {
        self.settings
            .hidden
            .contains(&self.catalog.mods[index].name.to_lowercase())
    }

    pub(super) fn is_always_enabled(&self, index: usize) -> bool {
        self.settings
            .always_enabled
            .contains(&self.catalog.mods[index].name.to_lowercase())
    }

    /// Filter and sort on a background thread; a newer query discards older results.
    pub(super) fn refresh_query(&mut self, cx: &mut Context<Self>) {
        self.query_generation += 1;
        let generation = self.query_generation;
        let query = self.search.read(cx).value().to_string();
        let catalog = self.catalog.clone();
        let order = self.order.clone();
        let (filter, sort, category) = (self.filter, self.sort, self.category.clone());
        let hidden = self.hidden_indices();
        let enabled = (filter == Filter::Enabled
            || filter == Filter::Disabled
            || sort.key == SortKey::Enabled)
            .then(|| self.enabled.clone());
        let executor = cx.background_executor().clone();
        self.search_task = Some(cx.spawn(async move |this, cx| {
            executor.timer(Duration::from_millis(75)).await;
            let visible = executor
                .spawn(async move {
                    let mut visible = catalog.query(&query, &order);
                    visible.retain(|i| hidden.contains(i) == (filter == Filter::Hidden));
                    if let Some(enabled) = &enabled {
                        match filter {
                            Filter::Enabled => visible.retain(|i| enabled.contains(i)),
                            Filter::Disabled => visible.retain(|i| !enabled.contains(i)),
                            Filter::All | Filter::Hidden => {}
                        }
                    }
                    if let Some(category) = category {
                        visible.retain(|&i| {
                            catalog.mods[i]
                                .metadata
                                .categories
                                .iter()
                                .any(|c| c.as_str() == &*category)
                        });
                    }
                    sort_rows(&catalog, &mut visible, sort.key, enabled.as_ref());
                    if sort.descending {
                        visible.reverse();
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

/// `visible` arrives in load order, so stable sorts keep it as the tie-breaker.
fn sort_rows(
    catalog: &Arc<Catalog>,
    visible: &mut [usize],
    key: SortKey,
    enabled: Option<&HashSet<usize>>,
) {
    let mods = &catalog.mods;
    match key {
        SortKey::Order => {}
        SortKey::Enabled => {
            visible.sort_by_key(|i| !enabled.is_some_and(|enabled| enabled.contains(i)))
        }
        SortKey::Title => visible.sort_by_cached_key(|&i| mods[i].title.to_lowercase()),
        SortKey::Pack => visible.sort_by_cached_key(|&i| mods[i].name.to_lowercase()),
        // Mods without an author sort last, as in the original manager.
        SortKey::Author => visible.sort_by_cached_key(|&i| {
            let author = &mods[i].metadata.author;
            (author.is_empty(), author.to_lowercase())
        }),
        SortKey::Updated => visible.sort_by_key(|&i| std::cmp::Reverse(mods[i].modified)),
        SortKey::Size => visible.sort_by_key(|&i| std::cmp::Reverse(mods[i].size)),
    }
}
