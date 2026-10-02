//! Search bar, column header and the virtualized mod table.
use super::{
    Manager,
    view::{button, vertical_scrollbar},
};
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{Sizable, input::Input},
    prelude::*,
    *,
};

impl Manager {
    fn search_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let l = self.language;
        div()
            .flex()
            .items_center()
            .gap_2()
            .p_3()
            .child(div().flex_1().child(Input::new(&self.search).small()))
            .when(!self.sort.is_load_order(), |bar| {
                bar.child(
                    div()
                        .text_xs()
                        .text_color(theme::warning())
                        .child(l.text("Sorted view · drag disabled", "Сортировка · перетаскивание выключено")),
                )
            })
            .child(
                button("check", l.text("Check compatibility", "Проверить совместимость"), !self.busy && !self.demo)
                    .icon(IconName::ShieldAlert)
                    .tooltip(l.text(
                        "Find overwritten files, shared DB tables, missing dependencies and startpos conflicts among enabled mods",
                        "Найти перезаписанные файлы, общие DB-таблицы, отсутствующие зависимости и конфликты startpos у включённых модов",
                    ))
                    .on_click(cx.listener(|this, _, _, cx| this.check(cx))),
            )
    }

    fn empty_state(&self) -> AnyElement {
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
    }

    fn table(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(
                uniform_list(
                    "mods",
                    self.visible.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        this.rendered_rows += range.len();
                        range.map(|position| this.row(position, cx)).collect()
                    }),
                )
                .track_scroll(&self.scroll)
                .size_full(),
            )
            .child(vertical_scrollbar(&self.scroll))
            .into_any_element()
    }

    pub(super) fn workspace(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(self.search_bar(cx))
            .child(self.header(cx))
            .child(if self.visible.is_empty() {
                self.empty_state()
            } else {
                self.table(cx)
            })
            .child(self.selection_bar(cx))
    }
}
