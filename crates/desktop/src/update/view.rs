use super::{Phase, Updater};
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{
        Sizable,
        button::{Button, ButtonVariants},
    },
    prelude::*,
    *,
};
use wh3_core::localization::Language;

/// Toolbar control for the update flow; nothing is shown while idle or checking.
pub fn update_button<T: 'static>(
    updater: &Entity<Updater>,
    language: Language,
    cx: &mut Context<T>,
) -> Option<AnyElement> {
    let l = language;
    let entity = updater.clone();
    let button = match updater.read(cx).phase() {
        Phase::Idle | Phase::Checking => return None,
        Phase::Available(available) => Button::new("update")
            .icon(IconName::Download)
            .label(crate::ui_text!(
                l,
                "Update to {}",
                "Обновить до {}",
                available.version
            ))
            .success()
            .cursor_pointer()
            .tooltip(l.text(
                "Download the release from GitHub, verify SHA-256 and replace this executable",
                "Скачать релиз с GitHub, проверить SHA-256 и заменить исполняемый файл",
            ))
            .on_click(move |_, _, cx| entity.update(cx, |u, cx| u.install(cx))),
        Phase::Installing(version) => Button::new("update")
            .label(crate::ui_text!(
                l,
                "Updating to {}…",
                "Обновление до {}…",
                version
            ))
            .loading(true),
        Phase::Installed(version) => Button::new("update")
            .icon(IconName::RefreshCw)
            .label(crate::ui_text!(
                l,
                "Restart into {}",
                "Перезапустить в {}",
                version
            ))
            .success()
            .cursor_pointer()
            .tooltip(l.text(
                "Save the library and restart the updated manager",
                "Сохранить библиотеку и перезапустить обновлённый менеджер",
            ))
            .on_click(move |_, _, cx| entity.update(cx, |u, cx| u.request_restart(cx))),
        Phase::Failed(message) => {
            let tip = message.text(l).to_owned();
            Button::new("update")
                .icon(IconName::TriangleAlert)
                .ghost()
                .text_color(theme::warning())
                .cursor_pointer()
                .tooltip(crate::ui_text!(
                    l,
                    "{} · click to retry",
                    "{} · нажмите, чтобы повторить",
                    tip
                ))
                .on_click(move |_, _, cx| entity.update(cx, |u, cx| u.check(cx)))
        }
    };
    Some(button.small().into_any_element())
}
