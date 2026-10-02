//! Snapshots the manager saves by itself, shown apart from the user's presets with
//! names that say when they were taken.
use super::{
    Manager,
    presets::{PresetAction, Snapshot},
    sidebar::caption,
};
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{
        Disableable, Icon, Sizable,
        button::{Button, ButtonVariants},
    },
    prelude::*,
    *,
};

impl Manager {
    /// Indices of presets the user named, in stored order.
    pub(super) fn user_presets(&self) -> Vec<usize> {
        self.settings
            .presets
            .iter()
            .enumerate()
            .filter(|(_, preset)| Snapshot::from_name(&preset.name).is_none())
            .map(|(index, _)| index)
            .collect()
    }

    pub(super) fn snapshots_section(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let l = self.language;
        let rows: Vec<_> = Snapshot::ALL
            .into_iter()
            .filter_map(|snapshot| {
                let index = self
                    .settings
                    .presets
                    .iter()
                    .position(|preset| preset.name == snapshot.stored_name())?;
                Some(self.snapshot_row(snapshot, index, cx))
            })
            .collect();
        if rows.is_empty() {
            return None;
        }
        Some(
            div()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap_0p5()
                .pt_1()
                .border_t_1()
                .border_color(theme::border())
                .child(caption(l.text("Saved automatically", "Сохраняются автоматически")).mt_2())
                .children(rows)
                .into_any_element(),
        )
    }

    fn snapshot_row(
        &self,
        snapshot: Snapshot,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let (l, busy) = (self.language, self.busy);
        let label = snapshot.label(l);
        // At most three snapshots, each the size of one mod list: cheap enough to count here.
        let enabled = self.settings.presets[index]
            .mods
            .iter()
            .filter(|entry| entry.is_enabled)
            .count();
        let icon = match snapshot {
            Snapshot::AppStart => IconName::AppWindow,
            Snapshot::LastLaunch => IconName::Play,
            Snapshot::BeforeShared => IconName::Share2,
            Snapshot::BeforeSearch => IconName::Search,
        };
        Button::new(("snapshot", index))
            .ghost()
            .small()
            .w_full()
            .disabled(busy)
            .when(!busy, |b| b.cursor_pointer())
            .accessibility_label(label)
            .tooltip(crate::ui_text!(
                l,
                "{} Click to enable exactly these {} mods.",
                "{} Нажмите, чтобы включить ровно эти моды ({}).",
                snapshot.description(l),
                enabled
            ))
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Icon::new(icon).xsmall().text_color(theme::muted()))
                    .child(div().flex_1().min_w_0().truncate().child(label))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::muted())
                            .child(enabled.to_string()),
                    ),
            )
            .on_click(
                cx.listener(move |this, _, _, cx| {
                    this.preset_action(index, PresetAction::Apply, cx)
                }),
            )
    }
}
