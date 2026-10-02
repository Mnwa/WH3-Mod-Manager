use super::Manager;
use super::view::{button, cell};
use crate::theme;
use gpui_kit::{
    component::{Disableable, Sizable, button::ButtonVariants, checkbox::Checkbox, input::Input},
    prelude::*,
    *,
};
use wh3_core::catalog::Source;
impl Manager {
    pub(super) fn row(&self, position: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let index = self.visible[position];
        let item = &self.catalog.mods[index];
        let title = SharedString::from(item.title.clone());
        let name = SharedString::from(item.name.clone());
        let source = match item.source {
            Source::Data => "Data",
            Source::Workshop => "Workshop",
            Source::Custom => self.language.text("Local", "Локальный"),
        };
        div()
            .id(("mod", index))
            .w_full()
            .flex()
            .items_center()
            .h(px(theme::ROW_HEIGHT))
            .border_b_1()
            .border_color(theme::border())
            .when(self.selected == Some(index), |row| {
                row.bg(theme::selection())
            })
            .hover(|row| row.bg(theme::selection()))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.focus.focus(window, cx);
                this.select(index, cx);
            }))
            .child(cell(format!("{}", self.ranks[index]), 48.).text_color(theme::accent()))
            .child(
                div().w(px(40.)).flex_shrink_0().child(
                    Checkbox::new(("check", index))
                        .checked(self.enabled.contains(&index))
                        .disabled(self.busy)
                        .accessibility_label(crate::ui_text!(
                            self.language,
                            "Enable {}",
                            "Включить {}",
                            item.title
                        ))
                        .on_click(cx.listener(move |this, _, _, cx| this.toggle(index, cx))),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px_2()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(title),
            )
            .child(cell(name, 220.).text_color(theme::muted()))
            .child(
                cell(
                    if item.movie {
                        self.language.text("Movie / auto", "Movie / авто")
                    } else {
                        source
                    },
                    100.,
                )
                .text_color(if item.movie {
                    theme::warning()
                } else {
                    theme::muted()
                }),
            )
            .child(
                cell(
                    crate::ui_text!(
                        self.language,
                        "{:.1} MiB",
                        "{:.1} МБ",
                        item.size as f64 / 1_048_576.
                    ),
                    85.,
                )
                .text_color(theme::muted()),
            )
    }

    pub(super) fn workspace(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .p_3()
                    .child(div().flex_1().child(Input::new(&self.search).small()))
                    .child(
                        button(
                            "check",
                            self.language.text("Check conflicts", "Проверить конфликты"),
                            !self.busy && !self.demo,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.check(cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(32.))
                    .bg(theme::panel())
                    .text_xs()
                    .text_color(theme::muted())
                    .child(cell("№", 48.))
                    .child(cell("✓", 40.))
                    .child(
                        div().flex_1().px_2().child(
                            button(
                                "sort",
                                if self.sort_name {
                                    self.language.text("Name ↑", "Название ↑")
                                } else {
                                    self.language.text("Load order ↕", "Порядок загрузки ↕")
                                },
                                true,
                            )
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.sort_name = !this.sort_name;
                                this.refresh_query(cx);
                                cx.notify();
                            })),
                        ),
                    )
                    .child(cell("PACK", 220.))
                    .child(cell(self.language.text("SOURCE", "ИСТОЧНИК"), 100.))
                    .child(cell(self.language.text("SIZE", "РАЗМЕР"), 85.)),
            )
            .child(if self.visible.is_empty() {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(if self.busy {
                        self.language
                            .text("Loading library…", "Загрузка библиотеки…")
                    } else {
                        self.language.text("No mods found", "Моды не найдены")
                    })
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme::muted())
                            .child(self.language.text(
                                "Select the game folder or add a folder containing .pack files.",
                                "Выберите папку игры или добавьте папку с .pack файлами.",
                            )),
                    )
                    .into_any_element()
            } else {
                uniform_list(
                    "mods",
                    self.visible.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        this.rendered_rows += range.len();
                        range.map(|position| this.row(position, cx)).collect()
                    }),
                )
                .track_scroll(&self.scroll)
                .w_full()
                .flex_1()
                .min_h_0()
                .into_any_element()
            })
            .child(self.selection_bar(cx))
    }
}
