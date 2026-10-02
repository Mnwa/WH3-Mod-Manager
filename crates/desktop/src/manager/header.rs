//! Sortable column header of the mod table.
use super::{Manager, Sort, SortKey};
use crate::theme;
use gpui_kit::{
    component::{
        Sizable,
        button::{Button, ButtonVariants},
    },
    prelude::*,
    *,
};

pub(super) const ORDER_WIDTH: f32 = 60.;
pub(super) const CHECK_WIDTH: f32 = 36.;
pub(super) const THUMB_WIDTH: f32 = 48.;
pub(super) const PACK_WIDTH: f32 = 180.;
pub(super) const AUTHOR_WIDTH: f32 = 120.;
pub(super) const UPDATED_WIDTH: f32 = 92.;
pub(super) const SIZE_WIDTH: f32 = 72.;

impl Manager {
    pub(super) fn set_sort(&mut self, key: SortKey, cx: &mut Context<Self>) {
        self.sort = if self.sort.key == key {
            Sort {
                key,
                descending: !self.sort.descending,
            }
        } else {
            Sort {
                key,
                descending: false,
            }
        };
        self.refresh_query(cx);
        cx.notify();
    }

    fn sort_button(
        &self,
        key: SortKey,
        id: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> Button {
        let active = self.sort.key == key;
        let arrow = match (active, self.sort.descending) {
            (false, _) => "",
            (true, false) => " ↑",
            (true, true) => " ↓",
        };
        Button::new(id)
            .label(format!("{label}{arrow}"))
            .ghost()
            .xsmall()
            .cursor_pointer()
            .when(active, |b| b.text_color(theme::text()))
            .tooltip(self.language.text(
                "Sort the table; the load order is not changed",
                "Сортировать таблицу; порядок загрузки не меняется",
            ))
            .on_click(cx.listener(move |this, _, _, cx| this.set_sort(key, cx)))
    }

    pub(super) fn header(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let l = self.language;
        // Flex columns keep each sort button at its content width, aligned with the cells.
        let column = |width: f32| div().w(px(width)).flex_shrink_0().flex().overflow_hidden();
        div()
            .flex()
            .items_center()
            .h(px(32.))
            .pr(px(12.))
            .bg(theme::panel())
            .text_xs()
            .text_color(theme::muted())
            .child(column(ORDER_WIDTH).child(self.sort_button(
                SortKey::Order,
                "sort-order",
                "№",
                cx,
            )))
            .child(column(CHECK_WIDTH).child(self.sort_button(
                SortKey::Enabled,
                "sort-enabled",
                "✓",
                cx,
            )))
            .child(column(THUMB_WIDTH))
            .child(div().flex_1().min_w_0().flex().child(self.sort_button(
                SortKey::Title,
                "sort-title",
                l.text("Title", "Название"),
                cx,
            )))
            .child(column(PACK_WIDTH).child(self.sort_button(
                SortKey::Pack,
                "sort-pack",
                "Pack",
                cx,
            )))
            .child(column(AUTHOR_WIDTH).child(self.sort_button(
                SortKey::Author,
                "sort-author",
                l.text("Author", "Автор"),
                cx,
            )))
            .child(column(UPDATED_WIDTH).child(self.sort_button(
                SortKey::Updated,
                "sort-updated",
                l.text("Updated", "Обновлён"),
                cx,
            )))
            .child(column(SIZE_WIDTH).child(self.sort_button(
                SortKey::Size,
                "sort-size",
                l.text("Size", "Размер"),
                cx,
            )))
    }
}
