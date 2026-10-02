//! Left panel: which mods to show, categories, load-order rules and presets.
use super::{Filter, Manager, view::vertical_scrollbar};
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
use std::sync::Arc;

/// Section heading: sentence case, quiet, so the controls under it carry the weight.
pub(super) fn caption(text: &'static str) -> Div {
    div()
        .mt_4()
        .mb_1()
        .px_1()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::muted())
        .child(text)
}

/// A short explanation under a control, written for someone who never read a guide.
pub(super) fn hint(text: impl Into<SharedString>) -> Div {
    div()
        .px_1()
        .text_xs()
        .text_color(theme::muted())
        .child(text.into())
}

/// A navigation entry with a right-aligned count.
fn nav_item(
    id: impl Into<ElementId>,
    label: SharedString,
    count: Option<usize>,
    active: bool,
) -> Button {
    Button::new(id)
        .ghost()
        .small()
        .w_full()
        .selected(active)
        .cursor_pointer()
        .accessibility_label(label.clone())
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(div().min_w_0().truncate().child(label))
                .children(count.map(|n| {
                    div()
                        .text_xs()
                        .text_color(if active {
                            theme::text()
                        } else {
                            theme::muted()
                        })
                        .child(n.to_string())
                })),
        )
}

impl Manager {
    /// Counts come from the enabled and hidden sets, never from a library scan.
    fn filter_counts(&self) -> [usize; 4] {
        let hidden = self.hidden_indices();
        let hidden_enabled = hidden.iter().filter(|i| self.enabled.contains(i)).count();
        let shown = self.catalog.mods.len().saturating_sub(hidden.len());
        let enabled = self.enabled.len().saturating_sub(hidden_enabled);
        [shown, enabled, shown.saturating_sub(enabled), hidden.len()]
    }

    fn filters(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let l = self.language;
        let [all, enabled, disabled, hidden] = self.filter_counts();
        div().flex().flex_col().gap_0p5().children(
            [
                (Filter::All, "all", l.text("All mods", "Все моды"), all),
                (
                    Filter::Enabled,
                    "enabled",
                    l.text("Enabled", "Включённые"),
                    enabled,
                ),
                (
                    Filter::Disabled,
                    "disabled",
                    l.text("Disabled", "Отключённые"),
                    disabled,
                ),
                (
                    Filter::Hidden,
                    "hidden",
                    l.text("Hidden", "Скрытые"),
                    hidden,
                ),
            ]
            .map(|(filter, id, label, count)| {
                nav_item(id, label.into(), Some(count), self.filter == filter).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.filter = filter;
                        this.refresh_query(cx);
                        cx.notify();
                    }),
                )
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
                            let (category, label, count): (Option<Arc<str>>, SharedString, _) =
                                match i {
                                    0 => (
                                        None,
                                        this.language
                                            .text("Any category", "Любая категория")
                                            .into(),
                                        None,
                                    ),
                                    _ => {
                                        let (name, count) = &this.categories[i - 1];
                                        (Some(name.clone()), name.to_string().into(), Some(*count))
                                    }
                                };
                            let active = this.category == category;
                            div().h(px(30.)).py_0p5().child(
                                nav_item(("category", i), label, count, active).on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.set_category_filter(category.clone(), cx)
                                    }),
                                ),
                            )
                        })
                        .collect()
                }),
            )
            .size_full(),
        )
    }

    fn load_order_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let l = self.language;
        let troubled =
            !self.rules.overridden.is_empty() || !self.rules.resolution.dropped.is_empty();
        let pinned = self.rules.pinned.len();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(caption(l.text("Load order", "Порядок загрузки")))
            .child(hint(l.text(
                "Mods higher in the list win when they change the same thing.",
                "Мод выше в списке побеждает, если моды меняют одно и то же.",
            )))
            .child(
                Button::new("rules")
                    .icon(if troubled {
                        IconName::TriangleAlert
                    } else {
                        IconName::ListOrdered
                    })
                    .label(crate::ui_text!(
                        l,
                        "Order rules ({})",
                        "Правила порядка ({})",
                        self.rules.resolution.accepted.len()
                    ))
                    .small()
                    .w_full()
                    .cursor_pointer()
                    .tooltip(l.text(
                        "“Load A before B” rules from you and from mods; applied automatically",
                        "Правила «A перед B» от вас и от модов; применяются автоматически",
                    ))
                    .on_click(cx.listener(|this, _, window, cx| this.open_rules(window, cx))),
            )
            .when(pinned > 0, |section| {
                section.child(hint(crate::ui_text!(
                    l,
                    "{} mods stay where you dragged them.",
                    "Перетащенных вручную модов: {}. Они остаются на месте.",
                    pinned
                )))
            })
    }

    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let l = self.language;
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
            .child(caption(l.text("Show", "Показать")).mt_0())
            .child(self.filters(cx))
            .when(!self.categories.is_empty(), |bar| {
                bar.child(caption(l.text("Categories", "Категории")))
                    .child(self.category_list(cx))
            })
            .child(self.load_order_section(cx))
            .child(self.presets_section(cx))
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
