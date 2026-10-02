//! Dialogs for load-order rules: adding a rule for one mod and the rules overview.
use super::{Manager, view::button};
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{
        Sizable, WindowExt as _,
        button::{Button, ButtonVariants as _},
        checkbox::Checkbox,
        searchable_list::SearchableVec,
        select::{Select, SelectState},
        v_flex,
    },
    prelude::*,
    *,
};
use wh3_core::load_order::{DropReason, Rule, RuleSource};

fn reason(l: wh3_core::localization::Language, reason: &DropReason) -> String {
    match reason {
        DropReason::Disabled | DropReason::DisabledPack => {
            l.text("switched off", "выключено").into()
        }
        DropReason::Invalid => l.text("invalid", "некорректно").into(),
        DropReason::MissingPack(name) => {
            crate::ui_text!(l, "{} is not installed", "{} не установлен", name)
        }
        DropReason::OverriddenByUser => l
            .text("replaced by your rule", "заменено вашим правилом")
            .into(),
        DropReason::Contradiction => l
            .text(
                "contradicts another mod's rule",
                "противоречит правилу другого мода",
            )
            .into(),
        DropReason::Cycle => l.text("would create a cycle", "создало бы цикл").into(),
    }
}

fn describe(rule: &Rule) -> String {
    format!("{}  →  {}", rule.before, rule.after)
}

impl Manager {
    /// Pick the other pack for "`index` loads before/after …".
    pub(super) fn prompt_rule(
        &mut self,
        index: usize,
        before: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let l = self.language;
        let own = self.catalog.mods[index].name.clone();
        let mut names: Vec<SharedString> = self
            .catalog
            .mods
            .iter()
            .filter(|item| item.name != own)
            .map(|item| SharedString::from(item.name.clone()))
            .collect();
        names.sort_unstable_by_key(|name| name.to_lowercase());
        names.dedup();
        let state = cx.new(|cx| {
            SelectState::new(SearchableVec::new(names), None, window, cx).searchable(true)
        });
        let title = if before {
            crate::ui_text!(l, "Load {} before…", "Загружать {} перед…", own)
        } else {
            crate::ui_text!(l, "Load {} after…", "Загружать {} после…", own)
        };
        let manager = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let (state, manager) = (state.clone(), manager.clone());
            dialog
                .title(title.clone())
                .w(px(520.))
                .child(
                    v_flex()
                        .gap_2()
                        .child(div().text_sm().text_color(theme::muted()).child(l.text(
                            "Earlier mods win conflicts. The rule is applied now and before every launch.",
                            "Мод выше в порядке побеждает в конфликтах. Правило применяется сразу и перед каждым запуском.",
                        )))
                        .child(Select::new(&state).placeholder(l.text("Choose a pack", "Выберите pack")).search_placeholder(l.text("Search…", "Поиск…"))),
                )
                .footer(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(button("rule-cancel", l.text("Cancel", "Отмена"), true).on_click(|_, window, cx| window.close_dialog(cx)))
                        .child(button("rule-add", l.text("Add rule", "Добавить правило"), true).primary().on_click(move |_, window, cx| {
                            let Some(other) = state.read(cx).selected_value().cloned() else { return };
                            let _ = manager.update(cx, |this, cx| this.add_rule(index, &other, before, cx));
                            window.close_dialog(cx);
                        })),
                )
        });
    }

    /// Overview of user rules, pack rules, dropped rules and pins; it reads the
    /// manager on every render so changes made inside it show immediately.
    pub(super) fn open_rules(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let manager = cx.entity().downgrade();
        let l = self.language;
        window.open_dialog(cx, move |dialog, _, cx| {
            let Some(entity) = manager.upgrade() else { return dialog };
            let this = entity.read(cx);
            let resolution = this.rules.resolution.clone();
            let mut rows: Vec<AnyElement> = Vec::new();
            let section = |text: &'static str| div().mt_2().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(theme::muted()).child(text).into_any_element();
            rows.push(section(l.text("Your rules", "Ваши правила")));
            if this.settings.rules.is_empty() {
                rows.push(div().text_sm().text_color(theme::muted()).child(l.text(
                    "None yet. Right-click a mod and choose Load before… or Load after…",
                    "Пока нет. Нажмите на мод правой кнопкой → «Загружать перед…» или «Загружать после…»",
                )).into_any_element());
            }
            for (i, rule) in this.settings.rules.iter().enumerate() {
                let (manager, rule_copy) = (manager.clone(), rule.clone());
                rows.push(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().text_sm().truncate().child(describe(rule)))
                        .child(
                            Button::new(("rule-remove", i))
                                .icon(IconName::Close)
                                .xsmall()
                                .ghost()
                                .cursor_pointer()
                                .tooltip(l.text("Remove rule", "Удалить правило"))
                                .on_click(move |_, _, cx| {
                                    let _ = manager.update(cx, |this, cx| this.remove_rule(&rule_copy, cx));
                                }),
                        )
                        .into_any_element(),
                );
            }
            rows.push(section(l.text("Rules shipped with mods (untick to ignore)", "Правила из модов (снимите галочку, чтобы игнорировать)")));
            if this.rules.pack_rules.is_empty() {
                rows.push(div().text_sm().text_color(theme::muted()).child(l.text(
                    "None of your mods ship load-order rules.",
                    "Ни один из ваших модов не содержит правил порядка.",
                )).into_any_element());
            }
            for (i, rule) in this.rules.pack_rules.iter().enumerate() {
                let enabled = !this.settings.disabled_rules.contains(rule.key().as_str());
                let source = match &rule.source {
                    RuleSource::Pack(name) => name.clone(),
                    RuleSource::User => String::new(),
                };
                let (manager, rule_copy) = (manager.clone(), rule.clone());
                rows.push(
                    Checkbox::new(("pack-rule", i))
                        .checked(enabled)
                        .label(format!("{} · {}", describe(rule), source))
                        .on_click(move |_, _, cx| {
                            let _ = manager.update(cx, |this, cx| this.toggle_pack_rule(&rule_copy, cx));
                        })
                        .into_any_element(),
                );
            }
            if !resolution.dropped.is_empty() || !this.rules.overridden.is_empty() {
                rows.push(section(l.text("Not applied", "Не применяются")));
                for (rule, why) in &resolution.dropped {
                    rows.push(div().text_xs().text_color(theme::warning()).child(format!("{} · {}", describe(rule), reason(l, why))).into_any_element());
                }
                for rule in &this.rules.overridden {
                    rows.push(
                        div()
                            .text_xs()
                            .text_color(theme::warning())
                            .child(format!("{} · {}", describe(rule), l.text("a pinned mod blocks it", "мешает закреплённый мод")))
                            .into_any_element(),
                    );
                }
            }
            let pins = this.rules.pinned.len();
            let clear = manager.clone();
            dialog
                .title(l.text("Load-order rules", "Правила порядка загрузки"))
                .w(px(640.))
                .child(div().id("rules-list").max_h(px(420.)).overflow_y_scroll().child(v_flex().gap_1().children(rows)))
                .footer(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().text_xs().text_color(theme::muted()).child(crate::ui_text!(
                            l,
                            "Pinned (moved by hand): {}. Rules do not move pinned mods.",
                            "Закреплено (перемещено вручную): {}. Правила не двигают закреплённые моды.",
                            pins
                        )))
                        .child(button("rules-unpin", l.text("Unpin all", "Открепить все"), pins > 0).on_click(move |_, _, cx| {
                            let _ = clear.update(cx, |this, cx| this.clear_pins(cx));
                        }))
                        .child(button("rules-close", l.text("Close", "Закрыть"), true).on_click(|_, window, cx| window.close_dialog(cx))),
                )
        });
    }
}
