//! Bar under the table. With nothing selected it teaches the table and acts on the
//! shown mods; with a selection it holds that selection's actions and details.
use super::{
    Manager,
    row_menu::workshop_url,
    view::{button, icon_button},
};
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{Icon, Sizable},
    prelude::*,
    *,
};
use wh3_core::catalog::Source;

impl Manager {
    fn report_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let l = self.language;
        let count = self.diagnostics.len();
        let label = match (self.show_report, count) {
            (true, _) => l.text("Hide report", "Скрыть отчёт").to_owned(),
            (false, 0) => l.text("Report", "Отчёт").to_owned(),
            (false, n) => crate::ui_text!(l, "Report ({})", "Отчёт ({})", n),
        };
        button("report", label, true)
            .when(count > 0 && !self.show_report, |b| b.icon(IconName::Info))
            .tooltip(l.text(
                "Compatibility results, pack contents and warnings (Esc closes)",
                "Результаты проверки, содержимое pack и предупреждения (Esc закрывает)",
            ))
            .on_click(cx.listener(|this, _, _, cx| {
                this.show_report = !this.show_report;
                cx.notify();
            }))
    }

    fn idle_actions(&self, cx: &mut Context<Self>) -> Div {
        let l = self.language;
        let available = !self.busy && self.shown_count() > 0;
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(Icon::new(IconName::MousePointerClick).small().text_color(theme::muted()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(theme::muted())
                    .child(l.text(
                        "Click a mod to select it, drag the ⋮⋮ handle to reorder, right-click for more.",
                        "Нажмите на мод, чтобы выбрать его, тяните за ⋮⋮, чтобы переставить, правый клик — остальное.",
                    )),
            )
            .child(
                button(
                    "enable-visible",
                    l.text("Enable all shown", "Включить показанные"),
                    available,
                )
                .tooltip(crate::ui_text!(
                    l,
                    "Enable the {} mods shown in the list",
                    "Включить показанные в списке моды: {}",
                    self.shown_count()
                ))
                .on_click(cx.listener(|this, _, _, cx| this.bulk_toggle(true, cx))),
            )
            .child(
                button(
                    "disable-visible",
                    l.text("Disable all shown", "Отключить показанные"),
                    available,
                )
                .tooltip(l.text(
                    "Disable the mods shown in the list, except always-enabled ones",
                    "Отключить показанные моды, кроме всегда включённых",
                ))
                .on_click(cx.listener(|this, _, _, cx| this.bulk_toggle(false, cx))),
            )
    }

    fn selected_actions(&self, index: usize, cx: &mut Context<Self>) -> Div {
        let l = self.language;
        let available = !self.busy;
        let movable = available && self.sort.is_load_order();
        let item = &self.catalog.mods[index];
        let group = self.targets(index).len();
        let enabled = self.enabled.contains(&index);
        let toggle = match (enabled, group) {
            (true, 1) => l.text("Disable", "Отключить").to_owned(),
            (false, 1) => l.text("Enable", "Включить").to_owned(),
            (true, n) => crate::ui_text!(l, "Disable {} selected", "Отключить выбранные ({})", n),
            (false, n) => crate::ui_text!(l, "Enable {} selected", "Включить выбранные ({})", n),
        };
        let move_button =
            |id: &'static str, icon: IconName, label: &'static str, tip: &'static str| {
                icon_button(id, movable)
                    .icon(icon)
                    .accessibility_label(label)
                    .tooltip(tip)
            };
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                button("toggle-selected", toggle, available)
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_targets(index, cx))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme::muted())
                    .ml_2()
                    .child(l.text("Move", "Сдвинуть")),
            )
            .child(
                move_button(
                    "top",
                    IconName::ArrowUpToLine,
                    l.text("To the top", "В начало"),
                    l.text(
                        "To the top of the load order (Alt+Home)",
                        "В начало порядка загрузки (Alt+Home)",
                    ),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.move_to_edge(index, true, cx))),
            )
            .child(
                move_button(
                    "up",
                    IconName::ArrowUp,
                    l.text("Up", "Выше"),
                    l.text("Load earlier (Alt+↑)", "Загружать раньше (Alt+↑)"),
                )
                .on_click(cx.listener(|this, _, _, cx| this.move_selected(-1, cx))),
            )
            .child(
                move_button(
                    "down",
                    IconName::ArrowDown,
                    l.text("Down", "Ниже"),
                    l.text("Load later (Alt+↓)", "Загружать позже (Alt+↓)"),
                )
                .on_click(cx.listener(|this, _, _, cx| this.move_selected(1, cx))),
            )
            .child(
                move_button(
                    "bottom",
                    IconName::ArrowDownToLine,
                    l.text("To the bottom", "В конец"),
                    l.text(
                        "To the bottom of the load order (Alt+End)",
                        "В конец порядка загрузки (Alt+End)",
                    ),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.move_to_edge(index, false, cx))),
            )
            .child(div().flex_1())
            .when(!item.workshop_id.is_empty(), |bar| {
                let id = item.workshop_id.clone();
                bar.child(
                    button(
                        "workshop",
                        l.text("Workshop page", "Страница в Workshop"),
                        true,
                    )
                    .icon(IconName::Globe)
                    .on_click(move |_, _, cx| cx.open_url(&workshop_url(&id))),
                )
            })
            .child(
                button(
                    "inspect",
                    l.text("Files inside", "Файлы внутри"),
                    available && !self.demo,
                )
                .icon(IconName::FileText)
                .tooltip(l.text(
                    "List the files packed in this mod",
                    "Показать файлы, упакованные в этот мод",
                ))
                .on_click(cx.listener(|this, _, _, cx| this.inspect(cx))),
            )
    }

    fn selection_details(&self, index: usize) -> impl IntoElement + use<> {
        let l = self.language;
        let item = &self.catalog.mods[index];
        let source = match item.source {
            Source::Data => l.text("Game data folder", "Папка data игры"),
            Source::Workshop => "Steam Workshop",
            Source::Custom => l.text("Your mod folder", "Ваша папка модов"),
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
        div()
            .text_xs()
            .text_color(theme::muted())
            .truncate()
            .child(parts.join("  ·  "))
    }

    pub(super) fn selection_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let selected = self.selected.filter(|&i| i < self.catalog.mods.len());
        div()
            .flex()
            .flex_col()
            .gap_2()
            .px_3()
            .py_2()
            .border_t_1()
            .border_color(theme::border())
            .bg(theme::panel())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().min_w_0().child(match selected {
                        Some(index) => self.selected_actions(index, cx),
                        None => self.idle_actions(cx),
                    }))
                    .child(self.report_button(cx)),
            )
            .when_some(selected, |bar, index| {
                bar.child(self.selection_details(index))
            })
    }
}
