use super::{Filter, Manager, SortKey};
use gpui_kit::*;
use std::{collections::HashSet, sync::Arc, time::Duration};
use wh3_core::catalog::Catalog;
use wh3_core::preferences::Layout;

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

    /// Every listed mod: the main list and, in the two-list layout, the enabled pane.
    pub(super) fn shown(&self) -> impl Iterator<Item = usize> + '_ {
        self.visible.iter().chain(&self.visible_enabled).copied()
    }

    pub(super) fn shown_count(&self) -> usize {
        self.visible.len() + self.visible_enabled.len()
    }

    /// The list a row belongs to, for keyboard movement and Shift ranges.
    pub(super) fn pane_of(&self, index: usize) -> &[usize] {
        if self.dual_active() && self.enabled.contains(&index) {
            &self.visible_enabled
        } else {
            &self.visible
        }
    }

    /// The two-list layout applies to every view except the hidden mods.
    pub(super) fn dual_active(&self) -> bool {
        self.prefs.layout == Layout::Dual && self.filter != Filter::Hidden
    }

    pub(super) fn grouping_active(&self) -> bool {
        self.prefs.group_by_category && self.filter != Filter::Hidden
    }

    /// Filter, split, sort and group on a background thread; a newer query discards
    /// older results.
    pub(super) fn refresh_query(&mut self, cx: &mut Context<Self>) {
        self.query_generation += 1;
        let generation = self.query_generation;
        let query = self.search.read(cx).value().to_string();
        let catalog = self.catalog.clone();
        let order = self.order.clone();
        let (dual, grouping) = (self.dual_active(), self.grouping_active());
        // The two lists already separate enabled mods, so those filters do not apply.
        let filter = if dual { Filter::All } else { self.filter };
        let (sort, category) = (self.sort, self.category.clone());
        let hidden = self.hidden_indices();
        let enabled = (dual
            || filter == Filter::Enabled
            || filter == Filter::Disabled
            || sort.key == SortKey::Enabled)
            .then(|| self.enabled.clone());
        let executor = cx.background_executor().clone();
        self.search_task = Some(cx.spawn(async move |this, cx| {
            executor.timer(Duration::from_millis(75)).await;
            let (visible, right, groups) = executor
                .spawn(async move {
                    let mut visible = catalog.query(&query, &order);
                    visible.retain(|i| hidden.contains(i) == (filter == Filter::Hidden));
                    if let Some(category) = category {
                        visible.retain(|&i| {
                            catalog.mods[i]
                                .metadata
                                .categories
                                .iter()
                                .any(|c| c.as_str() == &*category)
                        });
                    }
                    let mut right = Vec::new();
                    if let Some(enabled) = &enabled {
                        match filter {
                            _ if dual => {
                                // `visible` is still in load order here.
                                right = visible
                                    .iter()
                                    .copied()
                                    .filter(|i| enabled.contains(i))
                                    .collect();
                                visible.retain(|i| !enabled.contains(i));
                            }
                            Filter::Enabled => visible.retain(|i| enabled.contains(i)),
                            Filter::Disabled => visible.retain(|i| !enabled.contains(i)),
                            Filter::All | Filter::Hidden => {}
                        }
                    }
                    sort_rows(&catalog, &mut visible, sort.key, enabled.as_ref());
                    if sort.descending {
                        visible.reverse();
                    }
                    let groups = if grouping {
                        group_by_category(&catalog, &visible)
                    } else {
                        vec![]
                    };
                    (visible, right, groups)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.query_generation {
                    return;
                }
                this.visible = visible;
                this.visible_enabled = right;
                this.groups = Arc::new(groups);
                this.rebuild_grouped();
                cx.notify();
            });
        }));
    }
}

/// Every mod is listed under each of its categories; mods without one go to the
/// unnamed group, which comes first. Rows keep the list's order inside a group.
fn group_by_category(catalog: &Catalog, rows: &[usize]) -> Vec<(Arc<str>, Vec<usize>)> {
    let mut groups: std::collections::BTreeMap<(bool, String), (Arc<str>, Vec<usize>)> =
        Default::default();
    for &index in rows {
        let categories = &catalog.mods[index].metadata.categories;
        if categories.is_empty() {
            groups
                .entry((false, String::new()))
                .or_insert_with(|| (Arc::from(""), vec![]))
                .1
                .push(index);
        }
        for category in categories {
            groups
                .entry((true, category.to_lowercase()))
                .or_insert_with(|| (Arc::from(category.as_str()), vec![]))
                .1
                .push(index);
        }
    }
    groups.into_values().collect()
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
