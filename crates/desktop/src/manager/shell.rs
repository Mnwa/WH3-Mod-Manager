use super::view::button;
use super::{Filter, Manager};
use crate::theme;
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::Input,
    },
    prelude::*,
    *,
};
impl Manager {
    pub(super) fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex()
            .items_center()
            .gap_2()
            .p_3()
            .border_b_1()
            .border_color(theme::border())
            .bg(theme::panel())
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mr_4()
                    .child("WH3 / MOD MANAGER"),
            )
            .child(
                button("game-folder", "Папка игры", !self.busy && !self.demo)
                    .on_click(cx.listener(|this, _, _, cx| this.choose_directory(true, cx))),
            )
            .child(
                button("add-folder", "+ Папка модов", !self.busy && !self.demo)
                    .on_click(cx.listener(|this, _, _, cx| this.choose_directory(false, cx))),
            )
            .child(
                button("rescan", "Обновить", !self.busy && !self.demo)
                    .on_click(cx.listener(|this, _, _, cx| this.rescan(cx))),
            )
            .child(div().flex_1())
            .when(self.cancellable, |bar| {
                bar.child(button("cancel", "Отменить", true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.cancel
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                        this.status = "Отмена операции…".into();
                        cx.notify();
                    },
                )))
            })
            .child(
                button("save", "Сохранить", !self.busy && !self.demo && self.dirty)
                    .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
            )
            .child(
                button(
                    "play",
                    "Играть",
                    !self.busy && !self.demo && self.settings.game_path.is_some() && cfg!(windows),
                )
                .primary()
                .on_click(cx.listener(|this, _, _, cx| this.play(cx))),
            )
    }

    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let available = !self.busy;
        div()
            .w(px(224.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .p_3()
            .gap_2()
            .bg(theme::panel())
            .border_r_1()
            .border_color(theme::border())
            .child(
                div()
                    .text_xs()
                    .text_color(theme::muted())
                    .mb_2()
                    .child("БИБЛИОТЕКА"),
            )
            .children(
                [
                    (Filter::All, "all", "Все моды"),
                    (Filter::Enabled, "enabled", "Включённые"),
                    (Filter::Disabled, "disabled", "Отключённые"),
                ]
                .map(|(filter, id, label)| {
                    button(id, label, true)
                        .w_full()
                        .when(self.filter == filter, |b| b.primary())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.filter = filter;
                            this.refresh_query(cx);
                            cx.notify();
                        }))
                }),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button("enable-visible", "Включить", available)
                            .tooltip("Включить все моды текущего результата поиска")
                            .on_click(cx.listener(|this, _, _, cx| this.bulk_toggle(true, cx))),
                    )
                    .child(
                        button("disable-visible", "Отключить", available)
                            .tooltip("Отключить все моды текущего результата поиска")
                            .on_click(cx.listener(|this, _, _, cx| this.bulk_toggle(false, cx))),
                    ),
            )
            .child(
                div()
                    .mt_5()
                    .mb_2()
                    .text_xs()
                    .text_color(theme::muted())
                    .child("ПРЕСЕТЫ"),
            )
            .child(Input::new(&self.preset_name).small())
            .child(
                button("save-preset", "Сохранить пресет", available)
                    .on_click(cx.listener(|this, _, _, cx| this.save_preset(cx))),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button("import", "Импорт", available)
                            .on_click(cx.listener(|this, _, _, cx| this.import(cx))),
                    )
                    .child(
                        button("export", "Экспорт", available)
                            .on_click(cx.listener(|this, _, _, cx| this.export(cx))),
                    ),
            )
            .child(
                button("import-meta", "Импорт метаданных", available)
                    .on_click(cx.listener(|this, _, _, cx| this.import_metadata(cx))),
            )
            .child(
                button("export-original-meta", "Мета из оригинала…", available)
                    .on_click(cx.listener(|this, _, _, cx| this.export_original_metadata(cx))),
            )
            .child(
                uniform_list(
                    "presets",
                    self.settings.presets.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|index| {
                                let name: SharedString =
                                    this.settings.presets[index].name.clone().into();
                                div().h(px(36.)).py_1().child(
                                    Button::new(("preset", index))
                                        .label(name)
                                        .small()
                                        .w_full()
                                        .disabled(this.busy)
                                        .when(!this.busy, |b| b.cursor_pointer())
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let preset = this.settings.presets[index].clone();
                                            this.apply_preset(&preset, cx);
                                        })),
                                )
                            })
                            .collect()
                    }),
                )
                .flex_1()
                .min_h_0(),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(theme::border())
                    .pt_3()
                    .text_xs()
                    .text_color(theme::muted())
                    .child("Warhammer III · Steam")
                    .child(div().mt_1().child("Rust / GPUI")),
            )
    }

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
                        button("up", "↑ Выше", !self.busy && self.selected.is_some())
                            .on_click(cx.listener(|this, _, _, cx| this.move_selected(-1, cx))),
                    )
                    .child(
                        button("down", "↓ Ниже", !self.busy && self.selected.is_some())
                            .on_click(cx.listener(|this, _, _, cx| this.move_selected(1, cx))),
                    )
                    .child(
                        button(
                            "inspect",
                            "Файлы pack",
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
                    .child(button("report", "Отчёт", true).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.show_report = !this.show_report;
                            cx.notify();
                        },
                    ))),
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
