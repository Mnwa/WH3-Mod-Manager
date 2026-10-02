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
                button(
                    "game-folder",
                    self.language.text("Game folder", "Папка игры"),
                    !self.busy && !self.demo,
                )
                .on_click(cx.listener(|this, _, _, cx| this.choose_directory(true, cx))),
            )
            .child(
                button(
                    "add-folder",
                    self.language.text("+ Mod folder", "+ Папка модов"),
                    !self.busy && !self.demo,
                )
                .on_click(cx.listener(|this, _, _, cx| this.choose_directory(false, cx))),
            )
            .child(
                button(
                    "rescan",
                    self.language.text("Refresh", "Обновить"),
                    !self.busy && !self.demo,
                )
                .on_click(cx.listener(|this, _, _, cx| this.rescan(cx))),
            )
            .child(div().flex_1())
            .child(
                button(
                    "language",
                    self.language.text("Русский", "English"),
                    !self.preferences_busy,
                )
                .tooltip(
                    self.language
                        .text("Switch to Russian", "Переключить на английский"),
                )
                .on_click(cx.listener(|this, _, window, cx| this.change_language(window, cx))),
            )
            .when(self.cancellable, |bar| {
                bar.child(
                    button("cancel", self.language.text("Cancel", "Отменить"), true).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.cancel
                                .store(true, std::sync::atomic::Ordering::Relaxed);
                            this.status = wh3_core::message!("Cancelling…", "Отмена операции…");
                            cx.notify();
                        }),
                    ),
                )
            })
            .child(
                button(
                    "save",
                    self.language.text("Save", "Сохранить"),
                    !self.busy && !self.demo && self.dirty,
                )
                .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
            )
            .child(
                button(
                    "play",
                    self.language.text("Play", "Играть"),
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
                    .child(self.language.text("LIBRARY", "БИБЛИОТЕКА")),
            )
            .children(
                [
                    (
                        Filter::All,
                        "all",
                        self.language.text("All mods", "Все моды"),
                    ),
                    (
                        Filter::Enabled,
                        "enabled",
                        self.language.text("Enabled", "Включённые"),
                    ),
                    (
                        Filter::Disabled,
                        "disabled",
                        self.language.text("Disabled", "Отключённые"),
                    ),
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
                        button(
                            "enable-visible",
                            self.language.text("Enable", "Включить"),
                            available,
                        )
                        .tooltip(self.language.text(
                            "Enable all mods in the current search results",
                            "Включить все моды текущего результата поиска",
                        ))
                        .on_click(cx.listener(|this, _, _, cx| this.bulk_toggle(true, cx))),
                    )
                    .child(
                        button(
                            "disable-visible",
                            self.language.text("Disable", "Отключить"),
                            available,
                        )
                        .tooltip(self.language.text(
                            "Disable all mods in the current search results",
                            "Отключить все моды текущего результата поиска",
                        ))
                        .on_click(cx.listener(|this, _, _, cx| this.bulk_toggle(false, cx))),
                    ),
            )
            .child(
                div()
                    .mt_5()
                    .mb_2()
                    .text_xs()
                    .text_color(theme::muted())
                    .child(self.language.text("PRESETS", "ПРЕСЕТЫ")),
            )
            .child(Input::new(&self.preset_name).small())
            .child(
                button(
                    "save-preset",
                    self.language.text("Save preset", "Сохранить пресет"),
                    available,
                )
                .on_click(cx.listener(|this, _, _, cx| this.save_preset(cx))),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button("import", self.language.text("Import", "Импорт"), available)
                            .on_click(cx.listener(|this, _, _, cx| this.import(cx))),
                    )
                    .child(
                        button("export", self.language.text("Export", "Экспорт"), available)
                            .on_click(cx.listener(|this, _, _, cx| this.export(cx))),
                    ),
            )
            .child(
                button(
                    "import-meta",
                    self.language.text("Import metadata", "Импорт метаданных"),
                    available,
                )
                .on_click(cx.listener(|this, _, _, cx| this.import_metadata(cx))),
            )
            .child(
                button(
                    "export-original-meta",
                    self.language
                        .text("Original metadata…", "Мета из оригинала…"),
                    available,
                )
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
}
