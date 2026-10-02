//! Bar under the table with actions for the selected mod.
use super::{Manager, row_menu::workshop_url, view::button};
use crate::theme;
use gpui_kit::{assets::IconName, prelude::*, *};
use wh3_core::catalog::Source;

impl Manager {
    pub(super) fn selection_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let l = self.language;
        let selected = self.selected.is_some() && !self.busy;
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .border_t_1()
            .border_color(theme::border())
            .bg(theme::panel())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        button("top", l.text("Top", "В начало"), selected)
                            .icon(IconName::ArrowUpToLine)
                            .tooltip(l.text(
                                "Move to the top of the load order (Alt+Home)",
                                "В начало порядка загрузки (Alt+Home)",
                            ))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(index) = this.selected {
                                    this.move_to_edge(index, true, cx);
                                }
                            })),
                    )
                    .child(
                        button("up", l.text("Up", "Выше"), selected)
                            .icon(IconName::ArrowUp)
                            .tooltip(l.text("Load earlier (Alt+↑)", "Загружать раньше (Alt+↑)"))
                            .on_click(cx.listener(|this, _, _, cx| this.move_selected(-1, cx))),
                    )
                    .child(
                        button("down", l.text("Down", "Ниже"), selected)
                            .icon(IconName::ArrowDown)
                            .tooltip(l.text("Load later (Alt+↓)", "Загружать позже (Alt+↓)"))
                            .on_click(cx.listener(|this, _, _, cx| this.move_selected(1, cx))),
                    )
                    .child(
                        button("bottom", l.text("Bottom", "В конец"), selected)
                            .icon(IconName::ArrowDownToLine)
                            .tooltip(l.text(
                                "Move to the bottom of the load order (Alt+End)",
                                "В конец порядка загрузки (Alt+End)",
                            ))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(index) = this.selected {
                                    this.move_to_edge(index, false, cx);
                                }
                            })),
                    )
                    .child(
                        button(
                            "inspect",
                            l.text("Pack files", "Файлы pack"),
                            selected && !self.demo,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.inspect(cx))),
                    )
                    .child(
                        button(
                            "workshop",
                            "Workshop",
                            self.selected
                                .is_some_and(|i| !self.catalog.mods[i].workshop_id.is_empty()),
                        )
                        .icon(IconName::Globe)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(i) = this.selected {
                                cx.open_url(&workshop_url(&this.catalog.mods[i].workshop_id));
                            }
                        })),
                    )
                    .child(div().flex_1())
                    .child(button("report", l.text("Report", "Отчёт"), true).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.show_report = !this.show_report;
                            cx.notify();
                        }),
                    )),
            )
            .when_some(self.selected, |bar, index| {
                let item = &self.catalog.mods[index];
                let source = match item.source {
                    Source::Data => "Data",
                    Source::Workshop => "Workshop",
                    Source::Custom => l.text("Local folder", "Локальная папка"),
                };
                let mut parts = vec![source.to_owned(), item.path.display().to_string()];
                if !item.metadata.categories.is_empty() {
                    parts.push(item.metadata.categories.join(", "));
                }
                if !item.dependencies.is_empty() {
                    parts.push(crate::ui_text!(
                        l,
                        "needs {}",
                        "требует {}",
                        item.dependencies.join(", ")
                    ));
                }
                bar.child(
                    div()
                        .text_xs()
                        .text_color(theme::muted())
                        .truncate()
                        .child(parts.join(" · ")),
                )
            })
    }
}
