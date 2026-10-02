//! Root layout, keyboard shortcuts and the status bar.
use super::Manager;
use crate::theme;
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::Button,
        scroll::{Scrollbar, ScrollbarMode},
    },
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

/// Always-visible scrollbar overlay for a virtualized list inside a `relative` box,
/// so long libraries show their position and can be dragged directly.
pub(super) fn vertical_scrollbar(handle: &UniformListScrollHandle) -> impl IntoElement + use<> {
    div()
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .w(Scrollbar::width())
        .child(
            Scrollbar::vertical(handle)
                .viewport_from_layout()
                .mode(ScrollbarMode::Always),
        )
}

pub(super) fn cell(text: impl Into<SharedString>, width: f32) -> Div {
    div()
        .w(px(width))
        .flex_shrink_0()
        .px_2()
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        .child(text.into())
}

impl Manager {
    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let command = key.modifiers.control || key.modifiers.platform;
        if self.focus.is_focused(window) && !key.modifiers.alt {
            let position = self
                .selected
                .and_then(|index| self.visible.iter().position(|&i| i == index));
            if key.key == "up" || key.key == "down" {
                let next = match position {
                    Some(position) if key.key == "up" => position.saturating_sub(1),
                    Some(position) => (position + 1).min(self.visible.len().saturating_sub(1)),
                    None => 0,
                };
                if let Some(&index) = self.visible.get(next) {
                    let modifiers = Modifiers {
                        shift: key.modifiers.shift,
                        ..Default::default()
                    };
                    self.click_row(index, modifiers, cx);
                    self.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
                }
            }
            if key.key == "space"
                && let Some(index) = self.selected
            {
                self.toggle_targets(index, cx);
            }
            if command && key.key == "a" {
                self.select_all_visible(cx);
            }
        }
        if command && key.key == "f" {
            use gpui_kit::Focusable;
            self.search.read(cx).focus_handle(cx).focus(window, cx);
        }
        if key.key == "escape" {
            self.show_report = false;
            cx.notify();
        }
        if key.modifiers.alt {
            match (key.key.as_str(), self.selected) {
                ("up", _) => self.move_selected(-1, cx),
                ("down", _) => self.move_selected(1, cx),
                ("home", Some(index)) => self.move_to_edge(index, true, cx),
                ("end", Some(index)) => self.move_to_edge(index, false, cx),
                _ => {}
            }
        }
        if command && key.key == "s" {
            self.save(cx);
        }
    }

    fn status_bar(&self) -> impl IntoElement + use<> {
        let l = self.language;
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
                l,
                "{} / {} mods · {} enabled",
                "{} / {} модов · включено {}",
                self.visible.len(),
                self.catalog.mods.len(),
                self.enabled.len()
            ))
            .when(self.marked.len() > 1, |bar| {
                bar.child(div().text_color(theme::accent()).child(crate::ui_text!(
                    l,
                    "{} selected",
                    "выбрано {}",
                    self.marked.len()
                )))
            })
            .child(
                div()
                    .flex_1()
                    .truncate()
                    .child(self.status.text(l).to_owned()),
            )
            .when(self.dirty, |bar| {
                bar.child(
                    div()
                        .text_color(theme::warning())
                        .child(l.text("● Unsaved", "● Не сохранено")),
                )
            })
    }

    /// Mirrors the original's "N mods enabled" title without a per-frame platform call.
    fn sync_title(&mut self, window: &mut Window) {
        let title = crate::ui_text!(
            self.language,
            "WH3 Mod Manager v{}: {} mods enabled",
            "WH3 Mod Manager v{}: включено модов: {}",
            env!("CARGO_PKG_VERSION"),
            self.enabled.len()
        );
        if title != self.window_title {
            window.set_window_title(&title);
            self.window_title = title;
        }
    }
}

impl Render for Manager {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_title(window);
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::background())
            .text_color(theme::text())
            .text_sm()
            .track_focus(&self.focus)
            .on_key_down(
                cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    this.on_key(event, window, cx)
                }),
            )
            .child(self.toolbar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(cx))
                    .child(self.workspace(cx)),
            )
            .when(self.show_report, |root| root.child(self.report_panel(cx)))
            .child(self.status_bar())
    }
}
