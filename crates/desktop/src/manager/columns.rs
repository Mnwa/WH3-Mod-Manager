//! Resizable table columns: drag a header's right edge, double-click it to reset.
//! Widths live in the WHP2 preferences, never in the library.
use super::Manager;
use crate::theme;
use gpui_kit::{component::button::Button, prelude::*, *};
use wh3_core::preferences::Column;

/// The value carried while a column edge is dragged.
pub(super) struct ColumnDrag(Column);

impl Render for ColumnDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // The cursor already shows the resize; no floating preview is needed.
        div()
    }
}

impl Manager {
    pub(super) fn resizable_column(
        &self,
        column: Column,
        button: Button,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let l = self.language;
        let id = column as usize;
        div()
            .id(("column", id))
            .w(px(f32::from(self.prefs.width(column))))
            .flex_shrink_0()
            .relative()
            .flex()
            .items_center()
            .overflow_hidden()
            .child(button)
            .on_drag_move::<ColumnDrag>(cx.listener(
                move |this, event: &DragMoveEvent<ColumnDrag>, _, cx| {
                    if event.drag(cx).0 != column {
                        return;
                    }
                    let width = f32::from(event.event.position.x - event.bounds.origin.x);
                    this.update_prefs(|prefs| prefs.set_width(column, width), cx);
                },
            ))
            .child(
                div()
                    .id(("column-edge", id))
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(px(6.))
                    .cursor_col_resize()
                    .border_r_1()
                    .border_color(theme::border())
                    .hover(|edge| edge.bg(theme::border()))
                    .on_drag(ColumnDrag(column), |drag, _, _, cx| {
                        cx.new(|_| ColumnDrag(drag.0))
                    })
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                        if event.click_count() == 2 {
                            this.update_prefs(
                                |prefs| prefs.set_width(column, f32::from(column.default_width())),
                                cx,
                            );
                        }
                    }))
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(l.text(
                            "Drag to resize; double-click to reset",
                            "Потяните, чтобы изменить ширину; двойной клик — сбросить",
                        ))
                        .build(window, cx)
                    }),
            )
    }
}
