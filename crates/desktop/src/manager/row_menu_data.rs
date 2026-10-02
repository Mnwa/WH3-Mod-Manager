//! Row-menu entries for the game's data folder; they apply to the whole selection.
use super::Manager;
use gpui_kit::{
    WeakEntity,
    assets::IconName,
    component::menu::{PopupMenu, PopupMenuItem},
};
use wh3_core::{catalog::Source, data_folder::Placement, localization::Language};

pub(super) fn data_items(
    menu: PopupMenu,
    manager: WeakEntity<Manager>,
    l: Language,
    index: usize,
    in_data: bool,
    can_link: bool,
    blocked: bool,
) -> PopupMenu {
    let menu = menu.separator();
    if in_data {
        return menu.item(
            PopupMenuItem::new(l.text("Delete from the data folder…", "Удалить из папки data…"))
                .icon(IconName::Delete)
                .disabled(blocked)
                .on_click(move |_, window, cx| {
                    let _ = manager.update(cx, |this, cx| {
                        let packs = this
                            .targets(index)
                            .into_iter()
                            .map(|i| &this.catalog.mods[i])
                            .filter(|item| item.source == Source::Data)
                            .map(|item| item.path.clone())
                            .collect();
                        this.confirm_remove_from_data(packs, window, cx)
                    });
                }),
        );
    }
    let place = |label: &'static str, placement: Placement, enabled: bool| {
        let manager = manager.clone();
        PopupMenuItem::new(label)
            .disabled(blocked || !enabled)
            .on_click(move |_, _, cx| {
                let _ = manager.update(cx, |this, cx| {
                    let packs = this.outside_data(this.targets(index));
                    this.place_in_data(packs, placement, cx)
                });
            })
    };
    menu.item(place(
        l.text(
            "Copy into the game's data folder",
            "Скопировать в папку data игры",
        ),
        Placement::Copy,
        true,
    ))
    .item(place(
        if can_link {
            l.text(
                "Link into the game's data folder",
                "Создать ссылку в папке data игры",
            )
        } else {
            l.text(
                "Link into data (needs administrator or Developer Mode)",
                "Ссылка в data (нужен администратор или режим разработчика)",
            )
        },
        Placement::Link,
        can_link,
    ))
}
