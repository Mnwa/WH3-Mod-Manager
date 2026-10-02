use super::Manager;
use crate::theme;
use gpui_kit::{
    component::{Disableable, Sizable, button::Button},
    prelude::*,
    *,
};

pub(super) fn button(id: &'static str, label: &'static str, enabled: bool) -> Button {
    Button::new(id)
        .label(label)
        .small()
        .disabled(!enabled)
        .when(enabled, |b| b.cursor_pointer())
}

pub(super) fn cell(text: impl Into<SharedString>, width: f32) -> Div {
    div()
        .w(px(width))
        .flex_shrink_0()
        .px_2()
        .overflow_hidden()
        .text_ellipsis()
        .child(text.into())
}

impl Render for Manager {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::background())
            .text_color(theme::text())
            .text_sm()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let key = &event.keystroke;
                if this.focus.is_focused(window) && !key.modifiers.alt {
                    let position = this
                        .selected
                        .and_then(|index| this.visible.iter().position(|&i| i == index));
                    if key.key == "up" || key.key == "down" {
                        let next = match position {
                            Some(position) if key.key == "up" => position.saturating_sub(1),
                            Some(position) => {
                                (position + 1).min(this.visible.len().saturating_sub(1))
                            }
                            None => 0,
                        };
                        if let Some(&index) = this.visible.get(next) {
                            this.select(index, cx);
                            this.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
                        }
                    }
                    if key.key == "space"
                        && let Some(index) = this.selected
                    {
                        this.toggle(index, cx);
                    }
                }
                if (key.modifiers.control || key.modifiers.platform) && key.key == "f" {
                    use gpui_kit::Focusable;
                    this.search.read(cx).focus_handle(cx).focus(window, cx);
                }
                if key.key == "escape" {
                    this.show_report = false;
                    cx.notify();
                }
                if key.modifiers.alt && key.key == "up" {
                    this.move_selected(-1, cx);
                }
                if key.modifiers.alt && key.key == "down" {
                    this.move_selected(1, cx);
                }
                if (key.modifiers.control || key.modifiers.platform) && key.key == "s" {
                    this.save(cx);
                }
            }))
            .child(self.toolbar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(cx))
                    .child(self.workspace(cx)),
            )
            .when(self.show_report, |root| {
                let count = if self.details.is_empty() {
                    self.diagnostics.len()
                } else {
                    self.details.len()
                };
                root.child(
                    div()
                        .h(px(180.))
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .border_t_1()
                        .border_color(theme::border())
                        .child(div().px_3().py_1().text_color(theme::warning()).child(
                            crate::ui_text!(
                                self.language,
                                "Report · {count} entries · Esc to close",
                                "Отчёт · {count} записей · Esc — закрыть",
                                count = count
                            ),
                        ))
                        .child(
                            uniform_list(
                                "report-lines",
                                count,
                                cx.processor(|this, range: std::ops::Range<usize>, _, _| {
                                    let lines = if this.details.is_empty() {
                                        &this.diagnostics
                                    } else {
                                        &this.details
                                    };
                                    range
                                        .map(|i| {
                                            div()
                                                .h(px(28.))
                                                .px_3()
                                                .text_xs()
                                                .truncate()
                                                .child(lines[i].text(this.language).to_owned())
                                        })
                                        .collect()
                                }),
                            )
                            .flex_1()
                            .min_h_0(),
                        ),
                )
            })
            .child(
                div()
                    .h(px(30.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_4()
                    .px_3()
                    .border_t_1()
                    .border_color(theme::border())
                    .text_xs()
                    .text_color(theme::muted())
                    .child(crate::ui_text!(
                        self.language,
                        "{} / {} mods · {} enabled",
                        "{} / {} модов · включено {}",
                        self.visible.len(),
                        self.catalog.mods.len(),
                        self.enabled.len()
                    ))
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .child(self.status.text(self.language).to_owned()),
                    )
                    .when(self.dirty, |bar| {
                        bar.child(
                            div()
                                .text_color(theme::warning())
                                .child(self.language.text("● Unsaved", "● Не сохранено")),
                        )
                    }),
            )
    }
}
