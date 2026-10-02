//! Save, Continue (with the campaign save picker) and Play at the end of the top bar.
use super::{Manager, launch::RECENT_SAVES, view::button, watching::Queued};
use gpui_kit::{
    assets::IconName,
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants, DropdownButton},
        menu::{PopupMenu, PopupMenuItem},
    },
    prelude::*,
    *,
};

impl Manager {
    /// "Continue" loads the newest save; its arrow lists recent saves with both ways to
    /// load them, so the save picker is found where people look for "continue".
    fn continue_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (manager, l) = (cx.entity().downgrade(), self.language);
        let saves = self.launch.saves.clone();
        let playable = self.can_play();
        let latest = saves.first().map(|save| save.name.clone());
        let running = self.live.game_running;
        let queued = self.live.queued == Some(Queued::Continue);
        let tooltip = match (&latest, running) {
            (None, _) => l
                .text("No campaign saves yet", "Сохранений кампании пока нет")
                .to_owned(),
            (Some(_), true) if queued => l
                .text(
                    "Your newest save loads as soon as the game closes. Click to cancel.",
                    "Последнее сохранение загрузится, как только игра закроется. Нажмите, чтобы отменить.",
                )
                .to_owned(),
            (Some(_), true) => l
                .text(
                    "The game is running. Click to continue your newest save once it closes.",
                    "Игра запущена. Нажмите, чтобы продолжить последнее сохранение после её закрытия.",
                )
                .to_owned(),
            (Some(name), false) => crate::ui_text!(
                l,
                "Load your newest campaign save: {}",
                "Загрузить последнее сохранение кампании: {}",
                name
            ),
        };
        DropdownButton::new("continue")
            .small()
            .disabled(!playable)
            .button(
                Button::new("continue-latest")
                    .label(if queued {
                        l.text("Continues after exit", "Продолжит после выхода")
                    } else {
                        l.text("Continue", "Продолжить")
                    })
                    .when(queued, |b| b.icon(IconName::Clock))
                    .disabled(!playable || latest.is_none())
                    .when(playable && latest.is_some(), |b| b.cursor_pointer())
                    .tooltip(tooltip)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.live.game_running {
                            this.toggle_queued(Queued::Continue, cx);
                        } else {
                            this.play(latest.clone(), cx);
                        }
                    })),
            )
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu: PopupMenu, window, cx| {
                if saves.is_empty() {
                    return menu
                        .label(l.text("No campaign saves yet", "Сохранений кампании пока нет"));
                }
                let menu = menu.label(l.text("Recent campaign saves", "Последние сохранения"));
                saves.iter().take(RECENT_SAVES).fold(menu, |menu, save| {
                    let (load, mods) = (manager.clone(), manager.clone());
                    let (name, path) = (save.name.clone(), save.path.clone());
                    menu.submenu(save.name.clone(), window, cx, move |menu, _, _| {
                        let (load, mods, name, path) =
                            (load.clone(), mods.clone(), name.clone(), path.clone());
                        menu.item(
                            PopupMenuItem::new(
                                l.text("Load with the current mods", "Загрузить с текущими модами"),
                            )
                            .icon(IconName::Play)
                            .disabled(!playable)
                            .on_click(move |_, _, cx| {
                                let _ =
                                    load.update(cx, |this, cx| this.play(Some(name.clone()), cx));
                            }),
                        )
                        .item(
                            PopupMenuItem::new(l.text(
                                "Enable exactly the mods this save used",
                                "Включить ровно те моды, что были в сохранении",
                            ))
                            .icon(IconName::ListChecks)
                            .on_click(move |_, _, cx| {
                                let _ = mods.update(cx, |this, cx| {
                                    this.enable_mods_from_save(path.clone(), cx)
                                });
                            }),
                        )
                    })
                })
            })
    }

    pub(super) fn play_controls(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let l = self.language;
        let idle = !self.busy && !self.demo;
        let running = self.live.game_running;
        let queued_play = self.live.queued == Some(Queued::Play);
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                button("save", l.text("Save", "Сохранить"), idle && self.dirty)
                    .icon(IconName::Save)
                    .tooltip(l.text(
                        "Save the mod list and settings (Ctrl+S). Play saves too.",
                        "Сохранить список модов и настройки (Ctrl+S). «Играть» тоже сохраняет.",
                    ))
                    .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
            )
            .child(self.continue_button(cx))
            .when(running, |bar| {
                bar.child(
                    Button::new("close-game")
                        .icon(IconName::Close)
                        .small()
                        .ghost()
                        .cursor_pointer()
                        .accessibility_label(l.text("Close the game", "Закрыть игру"))
                        .tooltip(l.text(
                            "Close the running game",
                            "Закрыть запущенную игру",
                        ))
                        .on_click(cx.listener(|this, _, window, cx| this.close_game(window, cx))),
                )
            })
            .child(
                Button::new("play")
                    .icon(if running { IconName::Clock } else { IconName::Play })
                    .label(match (running, queued_play) {
                        (true, true) => l.text("Starts after exit", "Запустится после выхода"),
                        (true, false) => l.text("Game is running", "Игра запущена"),
                        _ => l.text("Play", "Играть"),
                    })
                    .small()
                    .primary()
                    .when(running && !queued_play, |b| b.outline())
                    .disabled(!self.can_play())
                    .when(self.can_play(), |b| b.cursor_pointer())
                    .tooltip(match (running, queued_play) {
                        (true, true) => l
                            .text(
                                "The game starts again with this list as soon as it closes. Click to cancel.",
                                "Игра запустится снова с этим списком, как только закроется. Нажмите, чтобы отменить.",
                            )
                            .to_owned(),
                        (true, false) => l
                            .text(
                                "The game is running. Click to start it again with this list once it closes.",
                                "Игра запущена. Нажмите, чтобы перезапустить её с этим списком после закрытия.",
                            )
                            .to_owned(),
                        _ => crate::ui_text!(
                            l,
                            "Save and start the game with {} enabled mods",
                            "Сохранить и запустить игру с включёнными модами: {}",
                            self.enabled.len()
                        ),
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.live.game_running {
                            this.toggle_queued(Queued::Play, cx);
                        } else {
                            this.play(None, cx);
                        }
                    })),
            )
    }
}
