//! Right-click menu of a mod row, mirroring the original manager's mod dropdown.
use super::Manager;
use gpui_kit::{
    assets::IconName,
    component::menu::{PopupMenu, PopupMenuItem},
    *,
};
use std::{path::PathBuf, sync::Arc};
use wh3_core::workshop::Request;

pub(super) fn workshop_url(id: &str) -> String {
    format!("https://steamcommunity.com/sharedfiles/filedetails/?id={id}")
}

/// Everything the menu needs, captured once so the builder stays `'static`.
struct Snapshot {
    index: usize,
    enabled: bool,
    always: bool,
    hidden: bool,
    workshop: Arc<str>,
    path: PathBuf,
    name: Arc<str>,
    own: Vec<String>,
    categories: Vec<Arc<str>>,
    title: Arc<str>,
    /// Required Workshop items that are not enabled (installed or not).
    required: Vec<u64>,
    steam: bool,
    pinned: bool,
    /// Number of mods the action applies to (the selection or this row).
    group: usize,
    in_data: bool,
    can_link: bool,
}

impl Manager {
    pub(super) fn row_menu(
        &self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        let item = &self.catalog.mods[index];
        let snapshot = Arc::new(Snapshot {
            index,
            enabled: self.enabled.contains(&index),
            always: self.is_always_enabled(index),
            hidden: self.is_hidden(index),
            workshop: item.workshop_id.clone(),
            path: item.path.clone(),
            name: item.name.clone(),
            own: item.metadata.categories.clone(),
            categories: self
                .categories
                .iter()
                .map(|(name, _)| name.clone())
                .collect(),
            title: item.title.clone(),
            required: self.unmet_requirements(index),
            steam: self.steam_available(),
            pinned: self.rules.pinned.contains(&index),
            group: self.targets(index).len(),
            in_data: item.source == wh3_core::catalog::Source::Data,
            can_link: self.can_link,
        });
        let (manager, l, busy, demo) =
            (cx.entity().downgrade(), self.language, self.busy, self.demo);
        move |menu, window, cx| {
            let s = snapshot.clone();
            let index = s.index;
            let act = |action: fn(&mut Manager, usize, &mut Context<Manager>)| {
                let manager = manager.clone();
                move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    let _ = manager.update(cx, |this, cx| action(this, index, cx));
                }
            };
            let manager_for_categories = manager.clone();
            let s_for_categories = s.clone();
            let mut menu = menu
                .item(
                    PopupMenuItem::new(match (s.enabled, s.group) {
                        (true, 1) => l.text("Disable", "Отключить").to_owned(),
                        (false, 1) => l.text("Enable", "Включить").to_owned(),
                        (true, n) => crate::ui_text!(
                            l,
                            "Disable selected ({})",
                            "Отключить выбранные ({})",
                            n
                        ),
                        (false, n) => {
                            crate::ui_text!(l, "Enable selected ({})", "Включить выбранные ({})", n)
                        }
                    })
                    .disabled(busy || (s.enabled && s.always && s.group == 1))
                    .on_click(act(|this, index, cx| this.toggle_targets(index, cx))),
                )
                .item(
                    PopupMenuItem::new(l.text("Move to top", "В начало порядка"))
                        .icon(IconName::ArrowUpToLine)
                        .disabled(busy)
                        .on_click(act(|this, index, cx| this.move_to_edge(index, true, cx))),
                )
                .item(
                    PopupMenuItem::new(l.text("Move to bottom", "В конец порядка"))
                        .icon(IconName::ArrowDownToLine)
                        .disabled(busy)
                        .on_click(act(|this, index, cx| this.move_to_edge(index, false, cx))),
                )
                .item({
                    let manager = manager.clone();
                    PopupMenuItem::new(l.text("Load before…", "Загружать перед…"))
                        .disabled(busy)
                        .on_click(move |_, window, cx| {
                            let _ = manager
                                .update(cx, |this, cx| this.prompt_rule(index, true, window, cx));
                        })
                })
                .item({
                    let manager = manager.clone();
                    PopupMenuItem::new(l.text("Load after…", "Загружать после…"))
                        .disabled(busy)
                        .on_click(move |_, window, cx| {
                            let _ = manager
                                .update(cx, |this, cx| this.prompt_rule(index, false, window, cx));
                        })
                })
                .item(
                    PopupMenuItem::new(l.text("Pinned position", "Закреплённая позиция"))
                        .checked(s.pinned)
                        .disabled(busy)
                        .on_click(act(|this, index, cx| this.toggle_pin(index, cx))),
                )
                .separator()
                .item(
                    PopupMenuItem::new(l.text("Keep always enabled", "Всегда включён"))
                        .checked(s.always)
                        .disabled(busy)
                        .on_click(act(|this, index, cx| this.toggle_always_enabled(index, cx))),
                )
                .item(
                    PopupMenuItem::new(l.text("Hide from list", "Скрыть из списка"))
                        .checked(s.hidden)
                        .disabled(busy)
                        .on_click(act(|this, index, cx| this.hide_targets(index, cx))),
                )
                .submenu(
                    l.text("Categories", "Категории"),
                    window,
                    cx,
                    move |menu, _, _| {
                        let s = s_for_categories.clone();
                        let mut menu = menu;
                        for category in &s.categories {
                            let (manager, category_name) =
                                (manager_for_categories.clone(), category.clone());
                            let index = s.index;
                            menu = menu.item(
                                PopupMenuItem::new(category.to_string())
                                    .checked(s.own.iter().any(|own| own.as_str() == &**category))
                                    .disabled(busy)
                                    .on_click(move |_, _, cx| {
                                        let _ = manager.update(cx, |this, cx| {
                                            this.toggle_category(index, &category_name, cx)
                                        });
                                    }),
                            );
                        }
                        let (manager, index) = (manager_for_categories.clone(), s.index);
                        menu.separator().item(
                            PopupMenuItem::new(l.text("Edit categories…", "Изменить категории…"))
                                .icon(IconName::Tag)
                                .disabled(busy)
                                .on_click(move |_, window, cx| {
                                    let _ = manager.update(cx, |this, cx| {
                                        this.edit_categories(index, window, cx)
                                    });
                                }),
                        )
                    },
                )
                .separator();
            if !s.workshop.is_empty() {
                let (page, steam) = (
                    workshop_url(&s.workshop),
                    format!("steam://url/CommunityFilePage/{}", s.workshop),
                );
                menu = menu
                    .item(
                        PopupMenuItem::new(
                            l.text("Open Workshop page", "Открыть страницу в Workshop"),
                        )
                        .icon(IconName::Globe)
                        .on_click(move |_, _, cx| cx.open_url(&page)),
                    )
                    .item(
                        PopupMenuItem::new(l.text("Open in Steam", "Открыть в Steam"))
                            .icon(IconName::ExternalLink)
                            .disabled(demo)
                            .on_click(move |_, _, cx| cx.open_url(&steam)),
                    );
                if let Ok(id) = s.workshop.parse::<u64>() {
                    let (update, unsubscribe, reinstall, title) = (
                        manager.clone(),
                        manager.clone(),
                        manager.clone(),
                        s.title.clone(),
                    );
                    menu = menu
                        .item(
                            PopupMenuItem::new(
                                l.text("Update from Workshop", "Обновить из Workshop"),
                            )
                            .icon(IconName::Download)
                            .disabled(!s.steam)
                            .on_click(move |_, _, cx| {
                                let _ = update.update(cx, |this, cx| {
                                    this.steam_action(Request::Download { ids: vec![id] }, cx)
                                });
                            }),
                        )
                        .item(
                            PopupMenuItem::new(
                                l.text("Reinstall from Workshop…", "Переустановить из Workshop…"),
                            )
                            .disabled(!s.steam)
                            .on_click(move |_, window, cx| {
                                let _ = reinstall.update(cx, |this, cx| {
                                    this.confirm_reinstall(id, index, window, cx)
                                });
                            }),
                        )
                        .item(
                            PopupMenuItem::new(l.text("Unsubscribe…", "Отписаться…"))
                                .disabled(!s.steam)
                                .on_click(move |_, window, cx| {
                                    let title = title.clone();
                                    let _ = unsubscribe.update(cx, |this, cx| {
                                        this.confirm_unsubscribe(id, &title, window, cx)
                                    });
                                }),
                        );
                }
            }
            if !s.required.is_empty() {
                let (fix, ids) = (manager.clone(), s.required.clone());
                menu = menu.item(
                    PopupMenuItem::new(crate::ui_text!(
                        l,
                        "Get required mods ({})",
                        "Установить нужные моды ({})",
                        s.required.len()
                    ))
                    .icon(IconName::Puzzle)
                    .disabled(!s.steam)
                    .on_click(move |_, _, cx| {
                        let ids = ids.clone();
                        let _ = fix.update(cx, |this, cx| this.fix_requirements(ids, cx));
                    }),
                );
            }
            let (reveal, copy_path, copy_name) = (
                s.path.clone(),
                s.path.to_string_lossy().into_owned(),
                s.name.to_string(),
            );
            let menu = super::row_menu_data::data_items(
                menu,
                manager.clone(),
                l,
                index,
                s.in_data,
                s.can_link,
                busy || demo,
            );
            menu.item(
                PopupMenuItem::new(l.text("Show in folder", "Показать в папке"))
                    .icon(IconName::FolderOpen)
                    .disabled(demo)
                    .on_click(move |_, _, cx| cx.reveal_path(&reveal)),
            )
            .item(
                PopupMenuItem::new(l.text("Copy path", "Копировать путь"))
                    .icon(IconName::Copy)
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy_path.clone()))
                    }),
            )
            .item(
                PopupMenuItem::new(l.text("Copy pack name", "Копировать имя pack")).on_click(
                    move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy_name.clone()))
                    },
                ),
            )
            .item(
                PopupMenuItem::new(l.text("Pack files", "Файлы pack"))
                    .icon(IconName::FileText)
                    .disabled(busy || demo)
                    .on_click(act(|this, index, cx| {
                        this.select(index, cx);
                        this.inspect(cx);
                    })),
            )
        }
    }
}

impl Manager {
    /// Workshop requirements of `index` that are missing or disabled.
    fn unmet_requirements(&self, index: usize) -> Vec<u64> {
        let required = &self.catalog.mods[index].metadata.req_mod_id_to_name;
        if required.is_empty() {
            return vec![];
        }
        let ids = &self.steam.ids;
        required
            .iter()
            .filter_map(|(id, _)| id.parse::<u64>().ok())
            .filter(|id| {
                !ids.get(id)
                    .is_some_and(|indices| indices.iter().any(|i| self.enabled.contains(i)))
            })
            .collect()
    }

    fn confirm_unsubscribe(
        &mut self,
        id: u64,
        title: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let l = self.language;
        self.confirm(
            crate::ui_text!(l, "Unsubscribe from “{}”?", "Отписаться от «{}»?", title),
            l.text(
                "Steam removes the mod's files. You can subscribe again from its Workshop page.",
                "Steam удалит файлы мода. Подписаться снова можно на его странице в Workshop.",
            ),
            l.text("Unsubscribe", "Отписаться"),
            move |this, cx| this.steam_action(Request::Unsubscribe { ids: vec![id] }, cx),
            window,
            cx,
        );
    }
}
