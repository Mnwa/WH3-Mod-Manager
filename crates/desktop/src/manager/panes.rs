//! The mod lists: one table, or two panes (not enabled | enabled in load order), with
//! optional category groups in the main list, like the original's dual layout.
use super::{Manager, groups::GroupRow, row::Slot, view::vertical_scrollbar};
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

impl Manager {
    /// The table, grouped or plain; in the two-list layout this is the left pane.
    pub(super) fn main_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let compact = self.dual_active();
        let list = if self.grouping_active() {
            uniform_list(
                "mods-grouped",
                self.grouped.len(),
                cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                    this.rendered_rows += range.len();
                    range
                        .map(|position| match this.grouped[position] {
                            GroupRow::Mod(index) => {
                                let slot = Slot {
                                    row: ("gmod", position).into(),
                                    check: ("gcheck", position).into(),
                                    compact,
                                    draggable: false,
                                    enables_on_drop: false,
                                };
                                this.mod_row(index, slot, cx)
                            }
                            header => this.group_header(position, header, cx),
                        })
                        .collect()
                }),
            )
        } else {
            uniform_list(
                "mods",
                self.visible.len(),
                cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                    this.rendered_rows += range.len();
                    let draggable = this.sort.is_load_order();
                    range
                        .map(|position| {
                            let index = this.visible[position];
                            let slot = Slot {
                                compact,
                                ..Slot::table(index, draggable)
                            };
                            this.mod_row(index, slot, cx)
                        })
                        .collect()
                }),
            )
        };
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(list.track_scroll(&self.scroll).size_full())
            .child(vertical_scrollbar(&self.scroll))
            .into_any_element()
    }

    fn enabled_list(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.visible_enabled.is_empty() {
            return pane_message(self.language.text(
                "Tick mods on the left, or drag them here, to enable them.",
                "Отметьте моды слева или перетащите их сюда, чтобы включить.",
            ));
        }
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(
                uniform_list(
                    "mods-enabled",
                    self.visible_enabled.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        this.rendered_rows += range.len();
                        range
                            .map(|position| {
                                let index = this.visible_enabled[position];
                                let slot = Slot {
                                    row: ("emod", index).into(),
                                    check: ("echeck", index).into(),
                                    compact: true,
                                    draggable: true,
                                    enables_on_drop: true,
                                };
                                this.mod_row(index, slot, cx)
                            })
                            .collect()
                    }),
                )
                .track_scroll(&self.enabled_scroll)
                .size_full(),
            )
            .child(vertical_scrollbar(&self.enabled_scroll))
            .into_any_element()
    }

    fn pane_caption(&self, title: String, extra: Option<AnyElement>) -> Div {
        div()
            .h(px(32.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .bg(theme::panel())
            .border_b_1()
            .border_color(theme::border())
            .text_xs()
            .child(
                div()
                    .flex_1()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .children(extra)
    }

    fn categories_pill(&self, cx: &mut Context<Self>) -> AnyElement {
        let l = self.language;
        Button::new("group-pill")
            .icon(IconName::Tag)
            .label(l.text("Categories", "Категории"))
            .xsmall()
            .ghost()
            .selected(self.prefs.group_by_category)
            .cursor_pointer()
            .tooltip(l.text("Group by category", "Группировать по категориям"))
            .on_click(cx.listener(|this, _, _, cx| {
                this.update_prefs(|prefs| prefs.group_by_category ^= true, cx)
            }))
            .into_any_element()
    }

    /// The list area under the search bar.
    pub(super) fn lists(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.shown_count() == 0 {
            return self.empty_state(cx);
        }
        if !self.dual_active() {
            return div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .child(self.header(cx))
                .child(self.main_list(cx))
                .into_any_element();
        }
        let l = self.language;
        let left = crate::ui_text!(
            l,
            "Not enabled ({})",
            "Не включены ({})",
            self.visible.len()
        );
        let right = crate::ui_text!(
            l,
            "Enabled, in load order ({})",
            "Включены, в порядке загрузки ({})",
            self.visible_enabled.len()
        );
        let pane = || {
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .border_1()
                .border_color(theme::border())
                .rounded_md()
                .overflow_hidden()
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .gap_2()
            .px_3()
            .child(
                pane()
                    .child(self.pane_caption(left, Some(self.categories_pill(cx))))
                    .child(if self.visible.is_empty() {
                        pane_message(l.text(
                            "Every shown mod is enabled.",
                            "Все показанные моды включены.",
                        ))
                    } else {
                        self.main_list(cx)
                    }),
            )
            .child(
                pane()
                    .child(self.pane_caption(right, None))
                    .child(self.enabled_list(cx)),
            )
            .into_any_element()
    }
}

fn pane_message(text: &'static str) -> AnyElement {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .p_4()
        .text_sm()
        .text_center()
        .text_color(theme::muted())
        .child(text)
        .into_any_element()
}
