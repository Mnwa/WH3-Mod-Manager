//! Banner above the table while the problem-mod search runs.
use super::{Manager, view::button};
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{Icon, button::ButtonVariants as _},
    prelude::*,
    *,
};

impl Manager {
    pub(super) fn hunt_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let hunt = self.hunt.as_ref()?;
        let l = self.language;
        let search = &hunt.search;
        let (title, detail) = if hunt.found {
            let detail = if search.suspects.len() == 1 {
                l.text(
                    "Restore your list without it, or as it was.",
                    "Верните свой список без него или как было.",
                )
            } else {
                l.text(
                    "These mods need each other, so they were tested together. Restore your list with or without them.",
                    "Эти моды нужны друг другу, поэтому проверялись вместе. Верните свой список — с ними или без них.",
                )
            };
            let detail = if hunt.dependents.is_empty() {
                detail.to_owned()
            } else {
                crate::ui_text!(
                    l,
                    "{} Restoring without it also switches off {} mods that require it.",
                    "{} Без него отключатся и моды, которым он нужен: {}.",
                    detail,
                    hunt.dependents.len()
                )
            };
            (
                crate::ui_text!(
                    l,
                    "Found it: {}",
                    "Нашли: {}",
                    self.hunt_names(&search.suspects)
                ),
                detail,
            )
        } else {
            (
                crate::ui_text!(
                    l,
                    "Finding the problem mod: test {} with {} of {} suspects enabled",
                    "Поиск проблемного мода: проверка {}, включено {} из {} подозреваемых",
                    search.step,
                    search.testing.len(),
                    search.suspects.len()
                ),
                crate::ui_text!(
                    l,
                    "Play, see whether the problem happens, then answer. At most {} more tests.",
                    "Запустите игру, проверьте, есть ли проблема, и ответьте. Осталось не больше {} проверок.",
                    search.remaining_steps()
                ),
            )
        };
        let actions = if hunt.found {
            div()
                .flex()
                .gap_2()
                .child(
                    button(
                        "hunt-without",
                        l.text("Restore without it", "Вернуть без него"),
                        true,
                    )
                    .primary()
                    .on_click(cx.listener(|this, _, _, cx| this.end_hunt(true, cx))),
                )
                .child(
                    button(
                        "hunt-restore",
                        l.text("Restore as it was", "Вернуть как было"),
                        true,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.end_hunt(false, cx))),
                )
        } else {
            div()
                .flex()
                .gap_2()
                .child(
                    button(
                        "hunt-still",
                        l.text("Problem is still there", "Проблема осталась"),
                        !self.busy,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.answer_hunt(true, cx))),
                )
                .child(
                    button(
                        "hunt-gone",
                        l.text("Problem is gone", "Проблемы нет"),
                        !self.busy,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.answer_hunt(false, cx))),
                )
                .child(
                    button(
                        "hunt-stop",
                        l.text("Stop and restore", "Остановить и вернуть"),
                        true,
                    )
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| this.end_hunt(false, cx))),
                )
        };
        Some(
            div()
                .mx_3()
                .mb_2()
                .p_3()
                .flex()
                .items_center()
                .gap_3()
                .rounded_md()
                .border_1()
                .border_color(theme::brass())
                .bg(theme::panel())
                .child(Icon::new(IconName::Search).text_color(theme::brass()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
                                .child(title),
                        )
                        .child(div().text_xs().text_color(theme::muted()).child(detail)),
                )
                .child(actions)
                .when(!hunt.history.is_empty(), |banner| {
                    banner.child(
                        button("hunt-undo", l.text("Undo answer", "Отменить ответ"), true)
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| this.undo_hunt_answer(cx))),
                    )
                })
                .into_any_element(),
        )
    }
}
