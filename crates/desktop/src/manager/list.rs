//! Search bar, column header and the virtualized mod table.
use super::{Manager, SortKey, view::button};
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{Icon, Sizable, button::Button, input::Input},
    prelude::*,
    *,
};

impl Manager {
    fn sort_name(&self) -> &'static str {
        let l = self.language;
        match self.sort.key {
            SortKey::Order => l.text("load order", "порядку загрузки"),
            SortKey::Enabled => l.text("enabled state", "включённости"),
            SortKey::Title => l.text("title", "названию"),
            SortKey::Pack => l.text("pack", "pack"),
            SortKey::Author => l.text("author", "автору"),
            SortKey::Updated => l.text("update date", "дате обновления"),
            SortKey::Size => l.text("size", "размеру"),
        }
    }

    /// Sorting by a column hides the real load order, so say so and offer the way back.
    fn sorted_notice(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let l = self.language;
        div()
            .flex()
            .items_center()
            .gap_2()
            .text_xs()
            .text_color(theme::warning())
            .child(crate::ui_text!(
                l,
                "Sorted by {}: dragging is off",
                "Сортировка по {}: перетаскивание выключено",
                self.sort_name()
            ))
            .child(
                Button::new("sort-reset")
                    .label(l.text("Show load order", "Показать порядок загрузки"))
                    .xsmall()
                    .outline()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.reset_sort(cx))),
            )
    }

    fn search_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let l = self.language;
        div()
            .flex()
            .items_center()
            .gap_3()
            .p_3()
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&self.search)
                        .small()
                        .cleanable(true)
                        .prefix(Icon::new(IconName::Search).small().text_color(theme::muted())),
                ),
            )
            .when(!self.sort.is_load_order(), |bar| bar.child(self.sorted_notice(cx)))
            .child(self.view_menu(cx))
            .child(
                button(
                    "check",
                    l.text("Check compatibility", "Проверить совместимость"),
                    !self.busy && !self.demo,
                )
                .icon(IconName::ShieldAlert)
                .tooltip(l.text(
                    "Find enabled mods that overwrite each other, miss a requirement or both change the campaign start",
                    "Найти включённые моды, которые перекрывают друг друга, требуют отсутствующий мод или вместе меняют старт кампании",
                ))
                .on_click(cx.listener(|this, _, _, cx| this.check(cx))),
            )
    }

    pub(super) fn workspace(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let first_start = self.catalog.mods.is_empty();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .when(!first_start, |workspace| {
                workspace
                    .child(self.search_bar(cx))
                    .children(self.hunt_banner(cx))
            })
            .child(self.lists(cx))
            .when(!first_start, |workspace| {
                workspace.child(self.selection_bar(cx))
            })
    }
}
