//! Settings menu: game start options, manager behavior and the original manager's data.
use super::Manager;
use gpui_kit::{
    assets::IconName,
    component::{
        Sizable,
        button::Button,
        menu::{DropdownMenu as _, PopupMenu, PopupMenuItem},
    },
    prelude::*,
    *,
};
use wh3_core::storage::GameOptions;

type Toggle = fn(&mut GameOptions);

impl Manager {
    pub(super) fn settings_menu(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (manager, l, options, busy) = (
            cx.entity().downgrade(),
            self.language,
            self.settings.options,
            self.busy,
        );
        let hunting = self.hunt.is_some();
        let entries: [(&'static str, bool, Toggle); 5] = [
            (
                l.text("Skip intro movies", "Пропускать заставки"),
                options.skip_intro_movies,
                |o| o.skip_intro_movies ^= true,
            ),
            (
                l.text("Write script logs", "Писать логи скриптов"),
                options.script_logging,
                |o| o.script_logging ^= true,
            ),
            (
                l.text(
                    "Start a custom battle right away",
                    "Сразу начинать свою битву",
                ),
                options.auto_start_custom_battle,
                |o| o.auto_start_custom_battle ^= true,
            ),
            (
                l.text(
                    "Let units be picked as generals",
                    "Юниты могут быть генералами",
                ),
                options.make_units_generals,
                |o| o.make_units_generals ^= true,
            ),
            (
                l.text(
                    "Raise the game's priority (helps when alt-tabbed)",
                    "Повышать приоритет игры (помогает при Alt+Tab)",
                ),
                options.raise_priority,
                |o| o.raise_priority ^= true,
            ),
        ];
        let (can_link, staging_size) = (self.can_link, self.live.staging_size);
        let close: (&'static str, bool, Toggle) = (
            l.text(
                "Close the manager when the game starts",
                "Закрывать менеджер при запуске игры",
            ),
            options.close_on_play,
            |o| o.close_on_play ^= true,
        );
        Button::new("options")
            .icon(IconName::Settings)
            .label(l.text("Settings", "Настройки"))
            .small()
            .cursor_pointer()
            .dropdown_caret(true)
            .tooltip(l.text(
                "Game start options, updates and import from the original manager",
                "Параметры запуска игры, обновления и импорт из оригинального менеджера",
            ))
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu: PopupMenu, window, cx| {
                let toggle = |(label, checked, change): (&'static str, bool, Toggle)| {
                    let manager = manager.clone();
                    PopupMenuItem::new(label)
                        .checked(checked)
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            let _ = manager.update(cx, |this, cx| this.set_option(change, cx));
                        })
                };
                let mut menu = menu.label(l.text(
                    "When the game starts (keep equal with multiplayer friends)",
                    "При запуске игры (в мультиплеере — как у друзей)",
                ));
                for entry in entries {
                    menu = menu.item(toggle(entry));
                }
                let (copy, update, import, export, hunt) = (
                    manager.clone(),
                    manager.clone(),
                    manager.clone(),
                    manager.clone(),
                    manager.clone(),
                );
                let staging = manager.clone();
                menu.separator()
                    .submenu(
                        l.text("Workshop mods at launch", "Моды Workshop при запуске"),
                        window,
                        cx,
                        move |menu, _, _| {
                            super::settings_staging::staging_menu(
                                menu,
                                staging.clone(),
                                l,
                                options,
                                can_link,
                                staging_size,
                                busy,
                            )
                        },
                    )
                    .item(toggle(close))
                    .item(
                        PopupMenuItem::new(l.text(
                            "Copy the enabled mod list",
                            "Копировать список включённых модов",
                        ))
                        .icon(IconName::Copy)
                        .on_click(move |_, _, cx| {
                            let _ = copy.update(cx, |this, cx| this.copy_mod_list(cx));
                        }),
                    )
                    .item(
                        PopupMenuItem::new(l.text(
                            "Find the mod that causes a problem…",
                            "Найти мод, который вызывает проблему…",
                        ))
                        .icon(IconName::Search)
                        .disabled(busy || hunting)
                        .on_click(move |_, window, cx| {
                            let _ = hunt.update(cx, |this, cx| this.confirm_hunt(window, cx));
                        }),
                    )
                    .item(
                        PopupMenuItem::new(
                            l.text("Check for a new version", "Проверить новую версию"),
                        )
                        .icon(IconName::RefreshCw)
                        .on_click(move |_, _, cx| {
                            let _ = update.update(cx, |this, cx| {
                                this.updater.update(cx, |u, cx| u.check(cx))
                            });
                        }),
                    )
                    .separator()
                    .label(l.text(
                        "Coming from the original WH3 Mod Manager?",
                        "Переходите с оригинального WH3 Mod Manager?",
                    ))
                    .item(
                        PopupMenuItem::new(l.text(
                            "Import its config.json or a metadata file…",
                            "Импортировать его config.json или файл метаданных…",
                        ))
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            let _ = import.update(cx, |this, cx| this.import_metadata(cx));
                        }),
                    )
                    .item(
                        PopupMenuItem::new(l.text(
                            "Save its config.json as a metadata file…",
                            "Сохранить его config.json как файл метаданных…",
                        ))
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            let _ = export.update(cx, |this, cx| this.export_original_metadata(cx));
                        }),
                    )
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
}
