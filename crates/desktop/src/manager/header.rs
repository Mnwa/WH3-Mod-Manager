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
use wh3_core::preferences::Column;

pub(super) const ORDER_WIDTH: f32 = 60.;
pub(super) const CHECK_WIDTH: f32 = 36.;

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

    pub(super) fn reset_sort(&mut self, cx: &mut Context<Self>) {
        self.sort = Sort {
            key: SortKey::Order,
            descending: false,
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
            .tooltip(if key == SortKey::Order {
                self.language.text(
                    "Load order: the game loads mods from the top down",
                    "Порядок загрузки: игра загружает моды сверху вниз",
                )
            } else {
                self.language.text(
                    "Sort the table by this column; the load order stays the same",
                    "Сортировать таблицу по этому столбцу; порядок загрузки не меняется",
                )
            })
            .on_click(cx.listener(move |this, _, _, cx| this.set_sort(key, cx)))
    }

    pub(super) fn header(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let l = self.language;
        let thumb = super::row::Metrics::of(self.prefs.density).thumb + 16.;
        // Flex columns keep each sort button at its content width, aligned with the cells.
        let column = |width: f32| div().w(px(width)).flex_shrink_0().flex().overflow_hidden();
        let resizable = |this: &Self, key: Column, button: Button, cx: &mut Context<Self>| {
            this.resizable_column(key, button, cx)
        };
        let sort = |this: &Self, key, id, label, cx: &mut Context<Self>| {
            this.sort_button(key, id, label, cx)
        };
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
                l.text("#", "№"),
                cx,
            )))
            .child(column(CHECK_WIDTH).child(self.sort_button(
                SortKey::Enabled,
                "sort-enabled",
                "✓",
                cx,
            )))
            .child(column(thumb))
            .child(div().flex_1().min_w_0().flex().child(self.sort_button(
                SortKey::Title,
                "sort-title",
                l.text("Title", "Название"),
                cx,
            )))
            .child({
                let button = sort(self, SortKey::Pack, "sort-pack", "Pack", cx);
                resizable(self, Column::Pack, button, cx)
            })
            .child({
                let button = sort(
                    self,
                    SortKey::Author,
                    "sort-author",
                    l.text("Author", "Автор"),
                    cx,
                );
                resizable(self, Column::Author, button, cx)
            })
            .child({
                let label = l.text("Updated", "Обновлён");
                let button = sort(self, SortKey::Updated, "sort-updated", label, cx);
                resizable(self, Column::Updated, button, cx)
            })
            .child({
                let button = sort(
                    self,
                    SortKey::Size,
                    "sort-size",
                    l.text("Size", "Размер"),
                    cx,
                );
                resizable(self, Column::Size, button, cx)
            })
    }
}
