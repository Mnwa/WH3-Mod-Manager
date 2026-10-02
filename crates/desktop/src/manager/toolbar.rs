//! Top bar: folders, game start options, saves, language, Save and Play.
use super::{Manager, launch::RECENT_SAVES, view::button};
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{
        Sizable,
        button::{Button, ButtonVariants},
        menu::{DropdownMenu as _, PopupMenu, PopupMenuItem},
    },
    prelude::*,
    *,
};
use wh3_core::storage::GameOptions;

type Toggle = fn(&mut GameOptions);

impl Manager {
    fn options_menu(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (manager, l, options, busy) = (
            cx.entity().downgrade(),
            self.language,
            self.settings.options,
            self.busy,
        );
        let entries: [(&'static str, bool, Toggle); 4] = [
            (
                l.text("Skip intro movies", "Пропустить заставки"),
                options.skip_intro_movies,
                |o| o.skip_intro_movies ^= true,
            ),
            (
                l.text("Script logging", "Логирование скриптов"),
                options.script_logging,
                |o| o.script_logging ^= true,
            ),
            (
                l.text("Auto-start custom battle", "Автостарт своей битвы"),
                options.auto_start_custom_battle,
                |o| o.auto_start_custom_battle ^= true,
            ),
            (
                l.text("Make units generals", "Юниты как генералы"),
                options.make_units_generals,
                |o| o.make_units_generals ^= true,
            ),
        ];
        let close: (&'static str, bool, Toggle) = (
            l.text("Close manager on Play", "Закрывать менеджер при запуске"),
            options.close_on_play,
            |o| o.close_on_play ^= true,
        );
        Button::new("options")
            .icon(IconName::Settings)
            .label(l.text("Options", "Опции"))
            .small()
            .cursor_pointer()
            .dropdown_caret(true)
            .tooltip(l.text(
                "Game start parameters; keep them in sync for multiplayer",
                "Параметры запуска игры; для мультиплеера они должны совпадать",
            ))
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu: PopupMenu, _, _| {
                let toggle = |(label, checked, change): (&'static str, bool, Toggle)| {
                    let manager = manager.clone();
                    PopupMenuItem::new(label)
                        .checked(checked)
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            let _ = manager.update(cx, |this, cx| this.set_option(change, cx));
                        })
                };
                let mut menu = menu.label(l.text("Game start", "Запуск игры"));
                for entry in entries {
                    menu = menu.item(toggle(entry));
                }
                let (copy, update, import, export) = (
                    manager.clone(),
                    manager.clone(),
                    manager.clone(),
                    manager.clone(),
                );
                menu.separator()
                    .item(toggle(close))
                    .separator()
                    .item(
                        PopupMenuItem::new(l.text(
                            "Copy enabled mod list",
                            "Копировать список включённых модов",
                        ))
                        .icon(IconName::Copy)
                        .on_click(move |_, _, cx| {
                            let _ = copy.update(cx, |this, cx| this.copy_mod_list(cx));
                        }),
                    )
                    .item(
                        PopupMenuItem::new(l.text("Check for updates", "Проверить обновления"))
                            .icon(IconName::RefreshCw)
                            .on_click(move |_, _, cx| {
                                let _ = update.update(cx, |this, cx| {
                                    this.updater.update(cx, |u, cx| u.check(cx))
                                });
                            }),
                    )
                    .separator()
                    .label(l.text("Original manager", "Оригинальный менеджер"))
                    .item(
                        PopupMenuItem::new(l.text(
                            "Import metadata or config.json…",
                            "Импорт метаданных или config.json…",
                        ))
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            let _ = import.update(cx, |this, cx| this.import_metadata(cx));
                        }),
                    )
                    .item(
                        PopupMenuItem::new(l.text(
                            "Export metadata from config.json…",
                            "Экспорт метаданных из config.json…",
                        ))
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            let _ = export.update(cx, |this, cx| this.export_original_metadata(cx));
                        }),
                    )
            })
    }

    fn saves_menu(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (manager, l) = (cx.entity().downgrade(), self.language);
        let saves = self.launch.saves.clone();
        let playable = self.can_play();
        Button::new("saves")
            .icon(IconName::FolderOpen)
            .small()
            .cursor_pointer()
            .tooltip(l.text("Campaign saves", "Сохранения кампании"))
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu: PopupMenu, window, cx| {
                if saves.is_empty() {
                    return menu.label(l.text("No saves found", "Сохранения не найдены"));
                }
                saves.iter().take(RECENT_SAVES).fold(menu, |menu, save| {
                    let (load, mods) = (manager.clone(), manager.clone());
                    let (name, path) = (save.name.clone(), save.path.clone());
                    menu.submenu(save.name.clone(), window, cx, move |menu, _, _| {
                        let (load, mods, name, path) =
                            (load.clone(), mods.clone(), name.clone(), path.clone());
                        menu.item(
                            PopupMenuItem::new(
                                l.text("Load with current mods", "Загрузить с текущими модами"),
                            )
                            .icon(IconName::Play)
                            .disabled(!playable)
                            .on_click(move |_, _, cx| {
                                let _ =
                                    load.update(cx, |this, cx| this.play(Some(name.clone()), cx));
                            }),
                        )
                        .item(
                            PopupMenuItem::new(
                                l.text("Enable mods from this save", "Включить моды из сохранения"),
                            )
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

    fn copy_mod_list(&mut self, cx: &mut Context<Self>) {
        let list: Vec<&str> = self
            .order
            .iter()
            .filter(|i| self.enabled.contains(i))
            .map(|&i| &*self.catalog.mods[i].title)
            .collect();
        cx.write_to_clipboard(ClipboardItem::new_string(list.join("\n")));
        self.status = wh3_core::message!(
            "Copied {} mod titles",
            "Скопировано названий модов: {}",
            list.len()
        );
        cx.notify();
    }

    pub(super) fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let l = self.language;
        let idle = !self.busy && !self.demo;
        let latest = self.launch.saves.first().map(|save| save.name.clone());
        div()
            .flex()
            .items_center()
            .gap_2()
            .p_3()
            .border_b_1()
            .border_color(theme::border())
            .bg(theme::panel())
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mr_4()
                    .child("WH3 / MOD MANAGER"),
            )
            .child(
                button("game-folder", l.text("Game folder", "Папка игры"), idle)
                    .on_click(cx.listener(|this, _, _, cx| this.choose_directory(true, cx))),
            )
            .child(
                button("add-folder", l.text("Mod folder", "Папка модов"), idle)
                    .icon(IconName::FolderPlus)
                    .on_click(cx.listener(|this, _, _, cx| this.choose_directory(false, cx))),
            )
            .child(
                button("rescan", l.text("Refresh", "Обновить"), idle)
                    .icon(IconName::RefreshCw)
                    .on_click(cx.listener(|this, _, _, cx| this.rescan(cx))),
            )
            .child(div().flex_1())
            .children(crate::update::update_button(&self.updater, l, cx))
            .child(self.workshop_menu(cx))
            .child(self.options_menu(cx))
            .child(
                button(
                    "language",
                    l.text("Русский", "English"),
                    !self.preferences_busy,
                )
                .tooltip(l.text("Switch to Russian", "Переключить на английский"))
                .on_click(cx.listener(|this, _, window, cx| this.change_language(window, cx))),
            )
            .when(self.cancellable, |bar| {
                bar.child(
                    button("cancel", l.text("Cancel", "Отменить"), true).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.cancel
                                .store(true, std::sync::atomic::Ordering::Relaxed);
                            this.status = wh3_core::message!("Cancelling…", "Отмена операции…");
                            cx.notify();
                        },
                    )),
                )
            })
            .child(
                button("save", l.text("Save", "Сохранить"), idle && self.dirty)
                    .icon(IconName::Save)
                    .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
            )
            .child(self.saves_menu(cx))
            .child(
                button(
                    "continue",
                    l.text("Continue", "Продолжить"),
                    self.can_play() && latest.is_some(),
                )
                .tooltip(match &latest {
                    Some(name) => crate::ui_text!(
                        l,
                        "Load the newest save: {}",
                        "Загрузить последнее сохранение: {}",
                        name
                    ),
                    None => l
                        .text("No campaign saves found", "Сохранения кампании не найдены")
                        .into(),
                })
                .on_click(cx.listener(move |this, _, _, cx| this.play(latest.clone(), cx))),
            )
            .child(
                button("play", l.text("Play", "Играть"), self.can_play())
                    .icon(IconName::Play)
                    .primary()
                    .on_click(cx.listener(|this, _, _, cx| this.play(None, cx))),
            )
    }
}
