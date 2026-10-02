//! Category editing and the sidebar category filter.
use super::{Manager, dialogs::TextPrompt};
use gpui_kit::*;
use std::{collections::BTreeMap, sync::Arc};

impl Manager {
    /// Counted once per metadata change so rendering never scans the library.
    pub(super) fn rebuild_categories(&mut self) {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for item in &self.catalog.mods {
            for category in &item.metadata.categories {
                *counts.entry(category).or_default() += 1;
            }
        }
        self.categories = counts
            .into_iter()
            .map(|(name, count)| (Arc::from(name), count))
            .collect();
        if let Some(category) = &self.category
            && !self.categories.iter().any(|(name, _)| name == category)
        {
            self.category = None;
        }
    }

    pub(super) fn set_category_filter(
        &mut self,
        category: Option<Arc<str>>,
        cx: &mut Context<Self>,
    ) {
        self.category = category;
        self.refresh_query(cx);
        cx.notify();
    }

    /// Categories are user metadata keyed by pack name, like the original's `modUserData`.
    pub(super) fn set_categories(
        &mut self,
        index: usize,
        mut categories: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        categories.retain(|c| !c.trim().is_empty());
        categories.iter_mut().for_each(|c| *c = c.trim().to_owned());
        categories.dedup();
        let name = self.catalog.mods[index].name.to_string();
        self.settings
            .metadata
            .entry(name)
            .or_default()
            .categories
            .clone_from(&categories);
        Arc::make_mut(&mut self.catalog)
            .update_metadata(index, |meta| meta.categories = categories);
        self.rebuild_categories();
        self.dirty = true;
        self.refresh_query(cx);
        cx.notify();
    }

    pub(super) fn toggle_category(&mut self, index: usize, category: &str, cx: &mut Context<Self>) {
        let mut categories = self.catalog.mods[index].metadata.categories.clone();
        if let Some(position) = categories.iter().position(|c| c == category) {
            categories.remove(position);
        } else {
            categories.push(category.to_owned());
        }
        self.set_categories(index, categories, cx);
    }

    pub(super) fn edit_categories(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        let l = self.language;
        let item = &self.catalog.mods[index];
        let prompt = TextPrompt {
            title: l.text("Categories", "Категории"),
            subtitle: item.title.to_string().into(),
            placeholder: l.text("Comma-separated categories", "Категории через запятую"),
            value: item.metadata.categories.join(", "),
            confirm: l.text("Save", "Сохранить"),
        };
        self.prompt_text(
            prompt,
            move |this, value, cx| {
                this.set_categories(index, value.split(',').map(str::to_owned).collect(), cx)
            },
            window,
            cx,
        );
    }
}
