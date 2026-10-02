//! Toolbar menu with Steam Workshop actions for the whole library.
use super::{Manager, dialogs::TextPrompt};
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
use wh3_core::workshop::Request;

/// A menu entry's effect on the manager.
type MenuAction = Box<dyn Fn(&mut Manager, &mut Window, &mut Context<Manager>)>;

impl Manager {
    fn outdated_ids(&self) -> Vec<u64> {
        let mut ids: Vec<u64> = self
            .steam
            .outdated
            .iter()
            .filter_map(|&i| self.catalog.mods[i].workshop_id.parse().ok())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    fn open_collection_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let l = self.language;
        let prompt = TextPrompt {
            title: l.text("Import Steam collection", "Импорт коллекции Steam"),
            subtitle: l
                .text(
                    "Missing mods are subscribed; a preset with the collection's mods is created once they are installed.",
                    "На недостающие моды оформляется подписка; после их установки создаётся пресет с модами коллекции.",
                )
                .into(),
            placeholder: "https://steamcommunity.com/sharedfiles/filedetails/?id=…",
            value: String::new(),
            confirm: l.text("Import", "Импортировать"),
        };
        self.prompt_text(
            prompt,
            |this, link, cx| this.import_collection(link, cx),
            window,
            cx,
        );
    }

    pub(super) fn workshop_menu(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (manager, l) = (cx.entity().downgrade(), self.language);
        let (available, outdated) = (self.steam_available(), self.outdated_ids());
        let enabled: Vec<u64> = self
            .enabled
            .iter()
            .filter_map(|&i| self.catalog.mods[i].workshop_id.parse().ok())
            .collect();
        Button::new("workshop-menu")
            .icon(IconName::Download)
            .label(if outdated.is_empty() {
                "Workshop".to_owned()
            } else {
                format!("Workshop · {}", outdated.len())
            })
            .small()
            .cursor_pointer()
            .loading(self.steam.busy || !self.steam.pending.is_empty())
            .tooltip(l.text(
                "Steam Workshop: refresh mod data, update, subscribe (Steam must be running)",
                "Steam Workshop: обновить данные, обновить моды, подписки (Steam должен быть запущен)",
            ))
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu: PopupMenu, _, _| {
                let action = |label: String, enabled: bool, run: MenuAction| {
                    let manager = manager.clone();
                    PopupMenuItem::new(label).disabled(!enabled).on_click(move |_, window, cx| {
                        let _ = manager.update(cx, |this, cx| run(this, window, cx));
                    })
                };
                let (outdated, enabled_ids) = (outdated.clone(), enabled.clone());
                menu.item(action(
                    l.text("Refresh Workshop data", "Обновить данные Workshop").into(),
                    available,
                    Box::new(|this, _, cx| this.refresh_workshop(cx)),
                ))
                .item(action(
                    crate::ui_text!(l, "Update outdated mods ({})", "Обновить устаревшие моды ({})", outdated.len()),
                    available && !outdated.is_empty(),
                    Box::new(move |this, _, cx| this.steam_action(Request::Download { ids: outdated.clone() }, cx)),
                ))
                .item(action(
                    crate::ui_text!(
                        l,
                        "Re-download enabled Workshop mods ({})",
                        "Перекачать включённые моды Workshop ({})",
                        enabled_ids.len()
                    ),
                    available && !enabled_ids.is_empty(),
                    Box::new(move |this, _, cx| this.steam_action(Request::Download { ids: enabled_ids.clone() }, cx)),
                ))
                .separator()
                .item(action(
                    l.text("Import Steam collection…", "Импорт коллекции Steam…").into(),
                    available,
                    Box::new(|this, window, cx| this.open_collection_prompt(window, cx)),
                ))
            })
    }
}
