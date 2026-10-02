//! "Game data folder" submenu of Folders: the original's "Content Mods Vs Data Mods".
use super::Manager;
use gpui_kit::{
    WeakEntity,
    component::menu::{PopupMenu, PopupMenuItem},
};
use std::{path::PathBuf, sync::Arc};
use wh3_core::{data_folder::Placement, localization::Language};

/// What the submenu offers, captured when the menu opens.
#[derive(Clone)]
pub(super) struct DataTools {
    pub enabled_outside: Arc<Vec<PathBuf>>,
    pub duplicates: Arc<Vec<PathBuf>>,
    pub links: Arc<Vec<PathBuf>>,
    pub can_link: bool,
}

impl Manager {
    pub(super) fn data_tools(&self) -> DataTools {
        DataTools {
            enabled_outside: Arc::new(self.enabled_outside_data()),
            duplicates: Arc::new(self.data_cleanup(false)),
            links: Arc::new(self.data_cleanup(true)),
            can_link: self.can_link,
        }
    }
}

pub(super) fn data_menu(
    menu: PopupMenu,
    manager: WeakEntity<Manager>,
    l: Language,
    tools: &DataTools,
) -> PopupMenu {
    let place = |label: String, placement: Placement, enabled: bool| {
        let (manager, packs) = (manager.clone(), tools.enabled_outside.clone());
        PopupMenuItem::new(label)
            .disabled(!enabled || packs.is_empty())
            .on_click(move |_, _, cx| {
                let packs = packs.to_vec();
                let _ = manager.update(cx, |this, cx| this.place_in_data(packs, placement, cx));
            })
    };
    let remove = |label: String, packs: Arc<Vec<PathBuf>>| {
        let manager = manager.clone();
        PopupMenuItem::new(label)
            .disabled(packs.is_empty())
            .on_click(move |_, window, cx| {
                let packs = packs.to_vec();
                let _ = manager.update(cx, |this, cx| {
                    this.confirm_remove_from_data(packs, window, cx)
                });
            })
    };
    let count = tools.enabled_outside.len();
    menu.label(l.text(
        "Copies in data replace the Workshop version",
        "Копии в data заменяют версию из Workshop",
    ))
    .item(place(
        crate::ui_text!(
            l,
            "Copy {} enabled mods into data",
            "Скопировать включённые моды в data ({})",
            count
        ),
        Placement::Copy,
        true,
    ))
    .item(place(
        if tools.can_link {
            crate::ui_text!(
                l,
                "Link {} enabled mods into data (no extra space)",
                "Создать ссылки на включённые моды в data ({}, без лишнего места)",
                count
            )
        } else {
            l.text(
                "Link enabled mods into data (needs administrator or Developer Mode)",
                "Ссылки на включённые моды в data (нужен администратор или режим разработчика)",
            )
            .to_owned()
        },
        Placement::Link,
        tools.can_link,
    ))
    .separator()
    .item(remove(
        crate::ui_text!(
            l,
            "Remove data copies that also exist elsewhere ({})…",
            "Удалить из data копии, которые есть и в других папках ({})…",
            tools.duplicates.len()
        ),
        tools.duplicates.clone(),
    ))
    .item(remove(
        crate::ui_text!(
            l,
            "Remove links from data ({})…",
            "Удалить ссылки из data ({})…",
            tools.links.len()
        ),
        tools.links.clone(),
    ))
}
