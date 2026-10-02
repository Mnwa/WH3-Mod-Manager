//! Bottom report panel: diagnostics, pack contents and compatibility sections.
use super::{Manager, compat::Tab, view::vertical_scrollbar};
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{
        Selectable, Sizable,
        button::{Button, ButtonVariants},
    },
    prelude::*,
    *,
};
use wh3_core::localization::Message;

impl Manager {
    fn report_lines(&self) -> &[Message] {
        let sections = &self.compat.sections;
        match self.report_tab {
            Tab::Diagnostics => &self.diagnostics,
            Tab::Details => &self.details,
            Tab::Files => &sections.files,
            Tab::Tables => &sections.tables,
            Tab::Keys => &sections.keys,
            Tab::Dependencies => &sections.dependencies,
            Tab::Startpos => &sections.startpos,
        }
    }

    fn report_tabs(&self) -> Vec<(Tab, &'static str, &'static str, usize)> {
        let l = self.language;
        let sections = &self.compat.sections;
        let mut tabs = vec![(
            Tab::Diagnostics,
            "report-messages",
            l.text("Messages", "Сообщения"),
            self.diagnostics.len(),
        )];
        if !self.details.is_empty() {
            tabs.push((
                Tab::Details,
                "report-pack",
                l.text("Pack files", "Файлы pack"),
                self.details.len(),
            ));
        }
        if self.compat.checked {
            tabs.extend([
                (
                    Tab::Files,
                    "report-files",
                    l.text("Overwritten files", "Перезаписанные файлы"),
                    sections.files.len(),
                ),
                (
                    Tab::Tables,
                    "report-tables",
                    l.text("Shared DB tables", "Общие DB-таблицы"),
                    sections.tables.len(),
                ),
                (
                    Tab::Keys,
                    "report-keys",
                    l.text("DB keys", "DB-ключи"),
                    sections.keys.len(),
                ),
                (
                    Tab::Dependencies,
                    "report-deps",
                    l.text("Dependencies", "Зависимости"),
                    sections.dependencies.len(),
                ),
                (
                    Tab::Startpos,
                    "report-startpos",
                    "Startpos",
                    sections.startpos.len(),
                ),
            ]);
        }
        tabs
    }

    pub(super) fn show_report_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.report_tab = tab;
        self.show_report = true;
        cx.notify();
    }

    pub(super) fn report_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let tabs = self.report_tabs();
        let count = self.report_lines().len();
        div()
            .h(px(200.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(theme::border())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .py_1()
                    .children(tabs.into_iter().map(|(tab, id, label, n)| {
                        Button::new(id)
                            .label(format!("{label} · {n}"))
                            .xsmall()
                            .ghost()
                            .cursor_pointer()
                            .selected(self.report_tab == tab)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.show_report_tab(tab, cx)),
                            )
                    }))
                    .child(div().flex_1())
                    .when(
                        self.report_tab == Tab::Dependencies && !self.compat.sections.required.is_empty(),
                        |bar| {
                            let ids = self.compat.sections.required.clone();
                            bar.child(
                                super::view::button(
                                    "report-get-required",
                                    self.language.text("Get required mods", "Установить нужные моды"),
                                    self.steam_available(),
                                )
                                .icon(IconName::Puzzle)
                                .tooltip(self.language.text(
                                    "Enable installed requirements and subscribe to missing ones; they are enabled after download",
                                    "Включить установленные зависимости и подписаться на отсутствующие; после загрузки они включатся",
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| this.fix_requirements(ids.clone(), cx))),
                            )
                        },
                    )
                    .child(
                        Button::new("report-close")
                            .icon(IconName::Close)
                            .xsmall()
                            .ghost()
                            .cursor_pointer()
                            .tooltip(self.language.text("Close (Esc)", "Закрыть (Esc)"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_report = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        uniform_list(
                            "report-lines",
                            count,
                            cx.processor(|this, range: std::ops::Range<usize>, _, _| {
                                let lines = this.report_lines();
                                range
                                    .map(|i| {
                                        div()
                                            .h(px(26.))
                                            .px_3()
                                            .text_xs()
                                            .truncate()
                                            .child(lines[i].text(this.language).to_owned())
                                    })
                                    .collect()
                            }),
                        )
                        .track_scroll(&self.report_scroll)
                        .size_full(),
                    )
                    .child(vertical_scrollbar(&self.report_scroll)),
            )
    }
}
