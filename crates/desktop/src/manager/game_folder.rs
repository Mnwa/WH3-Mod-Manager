//! Copy or link mods into the game's `data` folder and remove them again. Each action
//! is started by the user, names its files and asks before deleting anything.
use super::Manager;
use gpui_kit::*;
use std::{path::PathBuf, sync::Arc};
use wh3_core::{
    catalog::Source,
    data_folder::{self, Outcome, Placement},
    message,
};

impl Manager {
    /// Probe once whether links can be created (administrator or Developer Mode).
    pub(super) fn probe_links(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            return;
        }
        let task = cx.background_spawn(async { data_folder::can_link(&std::env::temp_dir()) });
        cx.spawn(async move |this, cx| {
            let can_link = task.await;
            let _ = this.update(cx, |this, cx| {
                this.can_link = can_link;
                cx.notify();
            });
        })
        .detach();
    }

    /// Enabled mods (or `indices`) that live outside `data`.
    pub(super) fn outside_data(&self, indices: impl IntoIterator<Item = usize>) -> Vec<PathBuf> {
        indices
            .into_iter()
            .filter_map(|index| self.catalog.mods.get(index))
            .filter(|item| item.source != Source::Data)
            .map(|item| item.path.clone())
            .collect()
    }

    pub(super) fn enabled_outside_data(&self) -> Vec<PathBuf> {
        let order = self.order.clone();
        self.outside_data(order.iter().copied().filter(|i| self.enabled.contains(i)))
    }

    pub(super) fn place_in_data(
        &mut self,
        packs: Vec<PathBuf>,
        placement: Placement,
        cx: &mut Context<Self>,
    ) {
        let Some(game) = self.settings.game_path.clone() else {
            return;
        };
        if self.busy || self.demo || packs.is_empty() {
            return;
        }
        self.busy = true;
        self.status = match placement {
            Placement::Copy => message!("Copying mods into data…", "Копирование модов в data…"),
            Placement::Link => message!("Linking mods into data…", "Создание ссылок в data…"),
        };
        let cancel = self.cancel.clone();
        let task = cx
            .background_spawn(async move { data_folder::place(&game, &packs, placement, &cancel) });
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.report_outcome(result, cx);
                this.rescan_keeping_state(None, cx);
            });
        }));
        cx.notify();
    }

    fn report_outcome(&mut self, result: wh3_core::Result<Outcome>, cx: &mut Context<Self>) {
        match result {
            Ok(outcome) => {
                self.status = message!(
                    "Done: {} files, {} skipped (see Report)",
                    "Готово: файлов {}, пропущено {} (см. отчёт)",
                    outcome.done.len(),
                    outcome.failed.len()
                );
                if !outcome.failed.is_empty() {
                    self.diagnostics.extend(outcome.failed);
                    self.show_report_tab(super::compat::Tab::Diagnostics, cx);
                }
            }
            Err(error) => self.status = error.message(),
        }
        cx.notify();
    }

    /// Delete packs from `data` after a confirmation that names how many and why.
    pub(super) fn confirm_remove_from_data(
        &mut self,
        packs: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if packs.is_empty() || self.busy || self.demo {
            return;
        }
        let l = self.language;
        let title = crate::ui_text!(
            l,
            "Delete {} packs from the game's data folder?",
            "Удалить pack-файлы из папки data игры ({})?",
            packs.len()
        );
        let packs = Arc::new(packs);
        self.confirm(
            title,
            l.text(
                "The files are deleted, not moved to the Recycle Bin. Links are removed without touching the mods they point to. A newer version you keep only in data is lost.",
                "Файлы удаляются без корзины. Ссылки удаляются, а моды, на которые они указывают, остаются. Новая версия, которая есть только в data, будет потеряна.",
            ),
            l.text("Delete", "Удалить"),
            move |this, cx| this.remove_from_data(packs.to_vec(), cx),
            window,
            cx,
        );
    }

    fn remove_from_data(&mut self, packs: Vec<PathBuf>, cx: &mut Context<Self>) {
        let Some(game) = self.settings.game_path.clone() else {
            return;
        };
        self.busy = true;
        let task = cx.background_spawn(async move { Ok(data_folder::remove_all(&game, &packs)) });
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.report_outcome(result, cx);
                this.rescan_keeping_state(None, cx);
            });
        }));
        cx.notify();
    }

    /// Data packs the original's "Clean data" / "Clean symbolic links" would remove.
    pub(super) fn data_cleanup(&self, links_only: bool) -> Vec<PathBuf> {
        data_folder::removable(&self.catalog, links_only)
    }
}
