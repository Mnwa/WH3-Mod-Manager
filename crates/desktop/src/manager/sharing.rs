//! "Share mod list": copy the enabled mods as text, or use text from a friend. The
//! format is the original manager's, so both managers can swap lists.
use super::{Manager, sidebar::hint, view::button};
use gpui_kit::{
    assets::IconName,
    component::{
        WindowExt as _,
        button::ButtonVariants as _,
        input::{Input, InputState},
        v_flex,
    },
    prelude::*,
    *,
};
use wh3_core::{message, share, workshop::Request};

/// Snapshot of the list before a shared one replaces it, so nothing is lost.
const BEFORE_SHARED: &str = "Before shared list";

impl Manager {
    fn copy_shared_list(&mut self, cx: &mut Context<Self>) {
        let text = share::serialize(&self.catalog, &self.order, &self.enabled);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.status = message!(
            "Copied {} mods. Send the text to a friend; they paste it under Share.",
            "Скопировано модов: {}. Отправьте текст другу — он вставит его в «Поделиться».",
            self.enabled.len()
        );
        cx.notify();
    }

    pub(super) fn open_share(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let l = self.language;
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(l.text(
                "Paste the text you received, e.g. 2789900000;0|local:my_mod.pack;1",
                "Вставьте полученный текст, например 2789900000;0|local:my_mod.pack;1",
            ))
        });
        let manager = cx.entity().downgrade();
        let enabled = self.enabled.len();
        window.open_dialog(cx, move |dialog, _, _| {
            let (copy, apply, input) = (manager.clone(), manager.clone(), input.clone());
            let section = |text: &'static str| {
                div()
                    .mt_2()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(text)
            };
            dialog
                .title(l.text("Share mod list", "Поделиться списком модов"))
                .w(px(560.))
                .child(
                    v_flex()
                        .gap_2()
                        .child(section(l.text("Send your list", "Отправить свой список")))
                        .child(hint(l.text(
                            "Copies your enabled mods and their order as text. It also works in the original WH3 Mod Manager.",
                            "Копирует включённые моды и их порядок как текст. Он подходит и для оригинального WH3 Mod Manager.",
                        )))
                        .child(
                            div().child(
                                button(
                                    "share-copy",
                                    crate::ui_text!(
                                        l,
                                        "Copy my list ({} mods)",
                                        "Скопировать мой список (модов: {})",
                                        enabled
                                    ),
                                    enabled > 0,
                                )
                                .icon(IconName::Copy)
                                .on_click(move |_, _, cx| {
                                    let _ = copy.update(cx, |this, cx| this.copy_shared_list(cx));
                                }),
                            ),
                        )
                        .child(section(l.text("Use a friend’s list", "Взять список друга")))
                        .child(hint(l.text(
                            "Enables exactly those mods in that order. Missing Workshop mods are subscribed first; your current list is kept as the preset “Before shared list”.",
                            "Включит ровно эти моды в этом порядке. Недостающие моды из Workshop сначала будут подписаны; текущий список сохранится как пресет «Before shared list».",
                        )))
                        .child(Input::new(&input)),
                )
                .footer(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            button("share-cancel", l.text("Close", "Закрыть"), true)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            button("share-apply", l.text("Use this list", "Взять этот список"), true)
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let text = input.read(cx).value().to_string();
                                    let _ = apply.update(cx, |this, cx| this.use_shared_list(&text, cx));
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    fn use_shared_list(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let shared = share::parse(text);
        if shared.is_empty() {
            self.status = message!(
                "That text is not a shared mod list",
                "Этот текст не похож на список модов"
            );
            cx.notify();
            return;
        }
        let resolved = share::resolve(&shared, &self.catalog, &self.steam.ids, String::new());
        if !resolved.missing_workshop.is_empty() && self.steam_available() {
            self.steam.shared = Some(shared);
            self.status = message!(
                "Subscribing to {} missing mods; the list is applied once Steam has them",
                "Подписка на недостающие моды: {}; список применится, когда Steam их скачает",
                resolved.missing_workshop.len()
            );
            self.steam_action(
                Request::Subscribe {
                    ids: resolved.missing_workshop,
                },
                cx,
            );
        } else {
            self.apply_shared(resolved, cx);
        }
        cx.notify();
    }

    /// Called after a rescan: the subscribed mods of a pending shared list are in place.
    pub(super) fn finish_shared_import(&mut self, cx: &mut Context<Self>) {
        if !self.steam.pending.is_empty() {
            return;
        }
        if let Some(shared) = self.steam.shared.take() {
            let resolved = share::resolve(&shared, &self.catalog, &self.steam.ids, String::new());
            self.apply_shared(resolved, cx);
        }
    }

    fn apply_shared(&mut self, resolved: share::Resolution, cx: &mut Context<Self>) {
        let before = self.capture(BEFORE_SHARED.into());
        self.store_preset(before);
        self.apply_preset(&resolved.preset, cx);
        let missing: Vec<_> = resolved
            .missing_local
            .iter()
            .map(|name| message!("Not installed: {}", "Не установлен: {}", name))
            .chain(resolved.missing_workshop.iter().map(|id| {
                message!(
                    "Workshop item {} is not installed",
                    "Мод Workshop {} не установлен",
                    id
                )
            }))
            .collect();
        self.status = message!(
            "Shared list applied: {} mods enabled, {} missing",
            "Список применён: включено модов {}, не хватает {}",
            resolved.preset.mods.len(),
            missing.len()
        );
        if !missing.is_empty() {
            self.diagnostics.extend(missing);
            self.show_report_tab(super::compat::Tab::Diagnostics, cx);
        }
        self.dirty = true;
    }
}
