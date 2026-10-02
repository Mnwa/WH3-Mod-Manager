//! Left panel: list filters, categories and presets.
use super::{
    Filter, Manager,
    presets::PresetAction,
    view::{button, vertical_scrollbar},
};
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{
        Disableable, Selectable, Sizable,
        button::{Button, ButtonVariants},
        input::Input,
        menu::{DropdownMenu as _, PopupMenuItem},
    },
    prelude::*,
    *,
};
use std::sync::Arc;

fn caption(text: &'static str) -> Div {
    div()
        .mt_3()
        .mb_1()
        .text_xs()
        .text_color(theme::muted())
        .child(text)
}

impl Manager {
    fn filters(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let l = self.language;
        let hidden = self.settings.hidden.len();
        div().flex().flex_col().gap_1().children(
            [
                (
                    Filter::All,
                    "all",
                    l.text("All mods", "Все моды").to_string(),
                ),
                (
                    Filter::Enabled,
                    "enabled",
                    l.text("Enabled", "Включённые").to_string(),
                ),
                (
                    Filter::Disabled,
                    "disabled",
                    l.text("Disabled", "Отключённые").to_string(),
                ),
                (
                    Filter::Hidden,
                    "hidden",
                    crate::ui_text!(l, "Hidden ({})", "Скрытые ({})", hidden),
                ),
            ]
            .map(|(filter, id, label)| {
                Button::new(id)
                    .label(label)
                    .small()
                    .w_full()
                    .cursor_pointer()
                    .when(self.filter == filter, |b| b.primary())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.filter = filter;
                        this.refresh_query(cx);
                        cx.notify();
                    }))
            }),
        )
    }

    fn category_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let height = (self.categories.len() + 1).min(5) as f32 * 30.;
        div().h(px(height)).relative().child(
            uniform_list(
                "categories",
                self.categories.len() + 1,
                cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                    range
                        .map(|i| {
                            let (category, label): (Option<Arc<str>>, SharedString) = match i {
                                0 => (
                                    None,
                                    this.language.text("All categories", "Все категории").into(),
                                ),
                                _ => {
                                    let (name, count) = &this.categories[i - 1];
                                    (Some(name.clone()), format!("{name} · {count}").into())
                                }
                            };
                            let active = this.category == category;
                            div().h(px(30.)).py_0p5().child(
                                Button::new(("category", i))
                                    .label(label)
                                    .xsmall()
                                    .w_full()
                                    .ghost()
                                    .when(active, |b| b.selected(true))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_category_filter(category.clone(), cx)
                                    })),
                            )
                        })
                        .collect()
                }),
            )
            .size_full(),
        )
    }

    fn preset_row(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (l, busy, manager) = (self.language, self.busy, cx.entity().downgrade());
        let name: SharedString = self.settings.presets[index].name.clone().into();
        div()
            .h(px(34.))
            .py_0p5()
            .flex()
            .gap_1()
            .child(
                Button::new(("preset", index))
                    .label(name.clone())
                    .small()
                    .flex_1()
                    .min_w_0()
                    .disabled(busy)
                    .when(!busy, |b| b.cursor_pointer())
                    .tooltip(l.text(
                        "Apply: enable exactly these mods in this order",
                        "Применить: включить ровно эти моды в этом порядке",
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.preset_action(index, PresetAction::Apply, cx)
                    })),
            )
            .child(
                Button::new(("preset-menu", index))
                    .icon(IconName::Ellipsis)
                    .small()
                    .ghost()
                    .disabled(busy)
                    .when(!busy, |b| b.cursor_pointer())
                    .tooltip(crate::ui_text!(
                        l,
                        "Preset actions: {}",
                        "Действия с пресетом: {}",
                        name
                    ))
                    .dropdown_menu(move |menu, _, _| {
                        let item = |label: &'static str, action: PresetAction| {
                            let manager = manager.clone();
                            PopupMenuItem::new(label).on_click(move |_, _, cx| {
                                let _ = manager
                                    .update(cx, |this, cx| this.preset_action(index, action, cx));
                            })
                        };
                        menu.item(item(l.text("Apply", "Применить"), PresetAction::Apply))
                            .item(item(
                                l.text("Enable its mods too", "Добавить его моды"),
                                PresetAction::Merge,
                            ))
                            .item(item(
                                l.text("Disable its mods", "Отключить его моды"),
                                PresetAction::Subtract,
                            ))
                            .separator()
                            .item(item(
                                l.text("Replace with current list", "Заменить текущим списком"),
                                PresetAction::Replace,
                            ))
                            .item(item(l.text("Delete", "Удалить"), PresetAction::Delete))
                    }),
            )
    }

    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (l, available) = (self.language, !self.busy);
        div()
            .w(px(240.))
            .flex_shrink_0()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .p_3()
            .gap_1()
            .bg(theme::panel())
            .border_r_1()
            .border_color(theme::border())
            .child(caption(l.text("LIBRARY", "БИБЛИОТЕКА")).mt_0())
            .child(self.filters(cx))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .mt_1()
                    .child(
                        button("enable-visible", l.text("Enable", "Включить"), available)
                            .tooltip(l.text("Enable all mods in the current search results", "Включить все моды текущего результата поиска"))
                            .on_click(cx.listener(|this, _, _, cx| this.bulk_toggle(true, cx))),
                    )
                    .child(
                        button("disable-visible", l.text("Disable", "Отключить"), available)
                            .tooltip(l.text(
                                "Disable all mods in the current search results, except always-enabled ones",
                                "Отключить все моды текущего результата поиска, кроме всегда включённых",
                            ))
                            .on_click(cx.listener(|this, _, _, cx| this.bulk_toggle(false, cx))),
                    ),
            )
            .when(!self.categories.is_empty(), |bar| {
                bar.child(caption(l.text("CATEGORIES", "КАТЕГОРИИ"))).child(self.category_list(cx))
            })
            .child(caption(l.text("LOAD ORDER", "ПОРЯДОК ЗАГРУЗКИ")))
            .child(
                Button::new("rules")
                    .label(crate::ui_text!(
                        l,
                        "Rules · {} · pinned {}",
                        "Правила · {} · закреплено {}",
                        self.rules.resolution.accepted.len(),
                        self.rules.pinned.len()
                    ))
                    .small()
                    .w_full()
                    .cursor_pointer()
                    .when(!self.rules.overridden.is_empty() || !self.rules.resolution.dropped.is_empty(), |b| {
                        b.icon(IconName::TriangleAlert)
                    })
                    .tooltip(l.text(
                        "Before/after rules from you and from mods; applied automatically and before launch",
                        "Правила «перед/после» от вас и из модов; применяются автоматически и перед запуском",
                    ))
                    .on_click(cx.listener(|this, _, window, cx| this.open_rules(window, cx))),
            )
            .child(caption(l.text("PRESETS", "ПРЕСЕТЫ")))
            .child(Input::new(&self.preset_name).small())
            .child(
                button("save-preset", l.text("Save preset", "Сохранить пресет"), available)
                    .on_click(cx.listener(|this, _, _, cx| this.save_preset(cx))),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(button("import", l.text("Import", "Импорт"), available).on_click(cx.listener(|this, _, _, cx| this.import(cx))))
                    .child(button("export", l.text("Export", "Экспорт"), available).on_click(cx.listener(|this, _, _, cx| this.export(cx)))),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        uniform_list(
                            "presets",
                            self.settings.presets.len(),
                            cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                                range.map(|index| this.preset_row(index, cx)).collect()
                            }),
                        )
                        .track_scroll(&self.preset_scroll)
                        .size_full(),
                    )
                    .child(vertical_scrollbar(&self.preset_scroll)),
            )
    }
}
