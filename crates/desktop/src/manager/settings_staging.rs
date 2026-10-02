//! The "Workshop mods at launch" submenu: the original's Workshop mod staging.
use super::Manager;
use gpui_kit::{
    WeakEntity,
    component::menu::{PopupMenu, PopupMenuItem},
};
use wh3_core::{
    localization::Language,
    storage::{GameOptions, Staging},
};

fn size_label(bytes: u64) -> String {
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024. && unit < 4 {
        value /= 1024.;
        unit += 1;
    }
    format!("{value:.1} {}", ["B", "KB", "MB", "GB", "TB"][unit])
}

pub(super) fn staging_menu(
    menu: PopupMenu,
    manager: WeakEntity<Manager>,
    l: Language,
    options: GameOptions,
    can_link: bool,
    size: u64,
    busy: bool,
) -> PopupMenu {
    let mode = |label: &'static str, staging: Staging, enabled: bool| {
        let manager = manager.clone();
        PopupMenuItem::new(label)
            .checked(options.staging == staging)
            .disabled(busy || !enabled)
            .on_click(move |_, _, cx| {
                let _ = manager.update(cx, |this, cx| {
                    this.set_option(
                        |o| {
                            o.staging = staging;
                            // Cleanup only means something while staging is on.
                            o.clean_up_staging &= staging != Staging::Off;
                        },
                        cx,
                    )
                });
            })
    };
    let (cleanup, clear) = (manager.clone(), manager.clone());
    menu.label(l.text(
        "Folder: <game>\\whmm_copied_mods",
        "Папка: <игра>\\whmm_copied_mods",
    ))
    .item(mode(
        l.text(
            "Load them from the Workshop folder",
            "Загружать из папки Workshop",
        ),
        Staging::Off,
        true,
    ))
    .item(mode(
        l.text("Copy them into the game folder", "Копировать в папку игры"),
        Staging::Copy,
        true,
    ))
    .item(mode(
        if can_link {
            l.text(
                "Link them into the game folder",
                "Создавать ссылки в папке игры",
            )
        } else {
            l.text(
                "Link them (needs administrator or Developer Mode)",
                "Ссылки (нужен администратор или режим разработчика)",
            )
        },
        Staging::Symlink,
        can_link,
    ))
    .separator()
    .item(
        PopupMenuItem::new(l.text(
            "Delete them when the game closes",
            "Удалять их после закрытия игры",
        ))
        .checked(options.clean_up_staging)
        .disabled(busy || options.staging == Staging::Off)
        .on_click(move |_, _, cx| {
            let _ = cleanup.update(cx, |this, cx| {
                this.set_option(|o| o.clean_up_staging ^= true, cx)
            });
        }),
    )
    .item(
        PopupMenuItem::new(crate::ui_text!(
            l,
            "Delete prepared mods now ({})",
            "Удалить подготовленные моды сейчас ({})",
            size_label(size)
        ))
        .disabled(busy || size == 0)
        .on_click(move |_, _, cx| {
            let _ = clear.update(cx, |this, cx| this.clean_staging(cx));
        }),
    )
}
