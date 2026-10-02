//! Category groups of the main list: each mod under every category, collapsible,
//! with an enable/disable button per group, like the original's category view.
use super::{Manager, row::Metrics};
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{
        Icon, Sizable,
        button::{Button, ButtonVariants},
    },
    prelude::*,
    *,
};
use std::{collections::HashSet, sync::Arc};

/// One line of the grouped list; rebuilt with the query or a collapse, not per frame.
#[derive(Clone, Copy)]
pub(crate) enum GroupRow {
    Header {
        group: usize,
        collapsed: bool,
        enabled: usize,
        total: usize,
    },
    Mod(usize),
}

impl Manager {
    /// Flatten groups into rows; every group starts collapsed the first time.
    pub(super) fn rebuild_grouped(&mut self) {
        let groups = self.groups.clone();
        if groups.is_empty() {
            self.grouped.clear();
            return;
        }
        let collapsed = self
            .collapsed
            .get_or_insert_with(|| groups.iter().map(|(name, _)| name.clone()).collect());
        let mut rows = Vec::new();
        for (group, (name, members)) in groups.iter().enumerate() {
            let shut = collapsed.contains(name);
            rows.push(GroupRow::Header {
                group,
                collapsed: shut,
                enabled: members.iter().filter(|i| self.enabled.contains(i)).count(),
                total: members.len(),
            });
            if !shut {
                rows.extend(members.iter().map(|&index| GroupRow::Mod(index)));
            }
        }
        self.grouped = rows;
    }

    fn toggle_group(&mut self, name: Arc<str>, cx: &mut Context<Self>) {
        let collapsed = self.collapsed.get_or_insert_with(HashSet::new);
        if !collapsed.remove(&name) {
            collapsed.insert(name);
        }
        self.rebuild_grouped();
        cx.notify();
    }

    /// Expand every group if any is collapsed, otherwise collapse them all.
    pub(super) fn toggle_all_groups(&mut self, cx: &mut Context<Self>) {
        let names: HashSet<Arc<str>> = self.groups.iter().map(|(name, _)| name.clone()).collect();
        let collapsed = self.collapsed.get_or_insert_with(HashSet::new);
        *collapsed = if collapsed.is_empty() {
            names
        } else {
            HashSet::new()
        };
        self.rebuild_grouped();
        cx.notify();
    }

    /// Enable a group's disabled mods, or disable them all once all are enabled.
    fn toggle_group_mods(&mut self, group: usize, cx: &mut Context<Self>) {
        let Some((_, members)) = self.groups.get(group) else {
            return;
        };
        let members = members.clone();
        let enable = members.iter().any(|i| !self.enabled.contains(i));
        self.set_enabled(&members, enable, cx);
    }

    pub(super) fn group_header(
        &self,
        position: usize,
        row: GroupRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let GroupRow::Header {
            group,
            collapsed,
            enabled,
            total,
        } = row
        else {
            return div().into_any_element();
        };
        let l = self.language;
        let name = self.groups[group].0.clone();
        let label: SharedString = if name.is_empty() {
            l.text("Uncategorized", "Без категории").into()
        } else {
            name.to_string().into()
        };
        let all_on = enabled == total;
        let toggle_name = name.clone();
        div()
            .id(("group", position))
            .h(px(Metrics::of(self.prefs.density).row))
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .bg(theme::panel())
            .border_b_1()
            .border_color(theme::border())
            .cursor_pointer()
            .hover(|header| header.bg(theme::raised()))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_group(toggle_name.clone(), cx)))
            .child(
                Icon::new(if collapsed {
                    IconName::ChevronRight
                } else {
                    IconName::ChevronDown
                })
                .small()
                .text_color(theme::muted()),
            )
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .truncate()
                    .child(label),
            )
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(theme::muted())
                    .child(crate::ui_text!(
                        l,
                        "{} of {} enabled",
                        "включено {} из {}",
                        enabled,
                        total
                    )),
            )
            .child(
                Button::new(("group-toggle", position))
                    .label(if all_on {
                        l.text("Disable all", "Отключить все")
                    } else {
                        l.text("Enable all", "Включить все")
                    })
                    .xsmall()
                    .ghost()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.toggle_group_mods(group, cx)
                    })),
            )
            .into_any_element()
    }
}
