//! A scan-owned indicator; progress and animation refresh this view, not the mod table.
use crate::theme;
use gpui_kit::{
    component::{Sizable, Size, progress::Progress, spinner::Spinner, tooltip::Tooltip},
    prelude::*,
    *,
};
use std::{
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};
use wh3_core::{localization::Language, scan};

pub(super) struct ScanProgress {
    progress: Arc<Mutex<scan::Progress>>,
    shown: scan::Progress,
    language: Language,
    size: Size,
    _task: Task<()>,
}

impl ScanProgress {
    pub(super) fn new(language: Language, size: Size, cx: &mut Context<Self>) -> Self {
        let progress = Arc::default();
        let executor = cx.background_executor().clone();
        let task = cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_millis(100)).await;
                if this
                    .update(cx, |this, cx| {
                        let progress =
                            *this.progress.lock().unwrap_or_else(PoisonError::into_inner);
                        if progress != this.shown {
                            this.shown = progress;
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            progress,
            shown: Default::default(),
            language,
            size,
            _task: task,
        }
    }

    pub(super) fn reporter(&self) -> impl FnMut(scan::Progress) + Send + 'static + use<> {
        let progress = self.progress.clone();
        move |state| *progress.lock().unwrap_or_else(PoisonError::into_inner) = state
    }

    pub(super) fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        self.language = language;
        cx.notify();
    }
}

impl Render for ScanProgress {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let l = self.language;
        let label = match self.shown.total {
            Some(total) => crate::ui_text!(
                l,
                "Packs scanned: {} of {}; mods found: {}",
                "Проверено pack-файлов: {} из {}; найдено модов: {}",
                self.shown.completed,
                total,
                self.shown.mods
            ),
            None => l
                .text("Finding pack files…", "Поиск pack-файлов…")
                .to_owned(),
        };
        let value = match self.shown.total {
            Some(0) => 100.,
            Some(total) => self.shown.completed as f32 / total as f32 * 100.,
            None => 0.,
        };
        div()
            .id("scan-progress")
            .flex()
            .items_center()
            .gap_2()
            .flex_shrink_0()
            .py_2()
            .tooltip({
                let label = label.clone();
                move |window, cx| Tooltip::new(label.clone()).build(window, cx)
            })
            .child(Spinner::new().with_size(self.size).color(theme::muted()))
            .child(
                div().w(px(160.)).child(
                    Progress::new("scan-progress-bar")
                        .small()
                        .color(theme::brass())
                        .loading(self.shown.total.is_none())
                        .value(value)
                        .accessibility_label(label),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{Duration, Language, ScanProgress, Size, scan};
    use gpui_kit::{AppContext, TestAppContext};

    #[gpui_kit::test]
    fn polls_the_latest_count_and_resets_for_a_new_scan(cx: &mut TestAppContext) {
        let old = cx.new(|cx| ScanProgress::new(Language::English, Size::Small, cx));
        let mut report = old.read_with(cx, |progress, _| progress.reporter());
        cx.run_until_parked();
        report(scan::Progress {
            total: Some(100),
            completed: 12,
            mods: 10,
        });
        let latest = scan::Progress {
            total: Some(100),
            completed: 34,
            mods: 20,
        };
        report(latest);
        assert_eq!(
            old.read_with(cx, |progress, _| progress.shown),
            scan::Progress::default()
        );
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        assert_eq!(old.read_with(cx, |progress, _| progress.shown), latest);

        drop(old);
        let next = cx.new(|cx| ScanProgress::new(Language::Russian, Size::Small, cx));
        cx.run_until_parked();
        report(scan::Progress {
            total: Some(100),
            completed: 99,
            mods: 50,
        });
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        assert_eq!(
            next.read_with(cx, |progress, _| progress.shown),
            scan::Progress::default()
        );
    }
}
