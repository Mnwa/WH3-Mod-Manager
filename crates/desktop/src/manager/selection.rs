use super::{Manager, view::button};
use crate::theme;
use gpui_kit::{prelude::*, *};

impl Manager {
    pub(super) fn selection_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
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
                        button(
                            "up",
                            self.language.text("↑ Move up", "↑ Выше"),
                            !self.busy && self.selected.is_some(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.move_selected(-1, cx))),
                    )
                    .child(
                        button(
                            "down",
                            self.language.text("↓ Move down", "↓ Ниже"),
                            !self.busy && self.selected.is_some(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.move_selected(1, cx))),
                    )
                    .child(
                        button(
                            "inspect",
                            self.language.text("Pack files", "Файлы pack"),
                            !self.busy && self.selected.is_some() && !self.demo,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.inspect(cx))),
                    )
                    .child(
                        button(
                            "workshop",
                            "Workshop",
                            self.selected
                                .is_some_and(|i| !self.catalog.mods[i].workshop_id.is_empty())
                                && !self.demo,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(i) = this.selected {
                                cx.open_url(&format!(
                                    "https://steamcommunity.com/sharedfiles/filedetails/?id={}",
                                    this.catalog.mods[i].workshop_id
                                ));
                            }
                        })),
                    )
                    .child(div().flex_1())
                    .child(
                        button("report", self.language.text("Report", "Отчёт"), true).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.show_report = !this.show_report;
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .when_some(self.selected, |bar, index| {
                let item = &self.catalog.mods[index];
                bar.child(
                    div()
                        .text_xs()
                        .text_color(theme::muted())
                        .truncate()
                        .child(format!(
                            "{} · {} · {}",
                            item.path.display(),
                            item.metadata.author,
                            item.metadata.categories.join(", ")
                        )),
                )
            })
    }
}
