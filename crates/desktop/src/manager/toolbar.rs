//! Top bar: brand, folders, Workshop, settings, language and the play controls.
//! Setup lives on the left, the actions that end in a game launch on the right.
use super::Manager;
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{
        Disableable, Selectable, Sizable,
        button::{Button, ButtonVariants},
        menu::{DropdownMenu as _, PopupMenu, PopupMenuItem},
    },
    prelude::*,
    *,
};
use wh3_core::localization::Language;

/// A thin rule between groups of toolbar controls.
pub(super) fn divider() -> Div {
    div().w(px(1.)).h(px(20.)).mx_1().bg(theme::border())
}

impl Manager {
    fn brand(&self) -> impl IntoElement + use<> {
        div()
            .flex()
            .items_center()
            .gap_2()
            .mr_2()
            .child(img(crate::assets::LOGO).size(px(24.)))
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .whitespace_nowrap()
                    .child("WH3 Mod Manager"),
            )
    }

    fn folders_menu(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (manager, l) = (cx.entity().downgrade(), self.language);
        let idle = !self.busy && !self.demo;
        let game = self.settings.game_path.clone();
        let mods = self.settings.roots.len();
        Button::new("folders")
            .icon(IconName::FolderCog)
            .label(l.text("Folders", "Папки"))
            .small()
            .dropdown_caret(true)
            .disabled(!idle)
            .when(idle, |b| b.cursor_pointer())
            .tooltip(l.text(
                "Where the game and your mods are",
                "Где находятся игра и ваши моды",
            ))
            .dropdown_menu(move |menu: PopupMenu, window, cx| {
                let (choose, add, rescan, reveal) = (
                    manager.clone(),
                    manager.clone(),
                    manager.clone(),
                    game.clone(),
                );
                let menu = menu.label(match &game {
                    Some(path) => crate::ui_text!(l, "Game: {}", "Игра: {}", path.display()),
                    None => l
                        .text("Game folder not found", "Папка игры не найдена")
                        .into(),
                });
                menu.item(
                    PopupMenuItem::new(l.text("Choose the game folder…", "Выбрать папку игры…"))
                        .icon(IconName::Folder)
                        .on_click(move |_, _, cx| {
                            let _ = choose.update(cx, |this, cx| this.choose_directory(true, cx));
                        }),
                )
                .item(
                    PopupMenuItem::new(l.text("Open the game folder", "Открыть папку игры"))
                        .icon(IconName::FolderOpen)
                        .disabled(reveal.is_none())
                        .on_click(move |_, _, cx| {
                            if let Some(path) = &reveal {
                                cx.reveal_path(path);
                            }
                        }),
                )
                .separator()
                .label(crate::ui_text!(
                    l,
                    "Mods are read from {} folders",
                    "Моды читаются из папок: {}",
                    mods
                ))
                .item(
                    PopupMenuItem::new(l.text(
                        "Add a folder with .pack files…",
                        "Добавить папку с .pack-файлами…",
                    ))
                    .icon(IconName::FolderPlus)
                    .on_click(move |_, _, cx| {
                        let _ = add.update(cx, |this, cx| this.choose_directory(false, cx));
                    }),
                )
                .item(
                    PopupMenuItem::new(l.text("Rescan all folders", "Пересканировать папки"))
                        .icon(IconName::RefreshCw)
                        .on_click(move |_, _, cx| {
                            let _ = rescan.update(cx, |this, cx| this.rescan(cx));
                        }),
                )
                .separator()
                .submenu(
                    l.text("Game data folder", "Папка data игры"),
                    window,
                    cx,
                    {
                        let manager = manager.clone();
                        // Computed when the submenu opens, never while the toolbar renders.
                        move |menu, _, cx| match manager.upgrade() {
                            Some(entity) => {
                                let tools = entity.read(cx).data_tools();
                                super::data_menu::data_menu(menu, manager.clone(), l, &tools)
                            }
                            None => menu,
                        }
                    },
                )
            })
    }

    /// Both languages stay visible in their own names, so the switch can be found
    /// even by someone who cannot read the current one.
    fn language_switch(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let available = !self.preferences_busy;
        div()
            .id("language-switch")
            .flex()
            .items_center()
            .gap_0p5()
            .p_0p5()
            .rounded_md()
            .border_1()
            .border_color(theme::border())
            .child(
                div().pl_1().pr_0p5().child(
                    gpui_kit::component::Icon::new(IconName::Languages)
                        .xsmall()
                        .text_color(theme::muted()),
                ),
            )
            .children(Language::ALL.map(|language| {
                let active = language == self.language;
                Button::new(match language {
                    Language::English => "language-en",
                    Language::Russian => "language-ru",
                })
                .label(language.code().to_uppercase())
                .xsmall()
                .ghost()
                .selected(active)
                .disabled(!available)
                .when(available && !active, |b| b.cursor_pointer())
                .accessibility_label(language.native_name())
                .tooltip(language.text("Interface language: English", "Язык интерфейса: русский"))
                .on_click(
                    cx.listener(move |this, _, window, cx| this.set_language(language, window, cx)),
                )
            }))
    }

    pub(super) fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let l = self.language;
        let idle = !self.busy && !self.demo;
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .h(px(52.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(theme::border())
            .bg(theme::panel())
            .child(self.brand())
            .child(self.folders_menu(cx))
            .child(
                Button::new("rescan")
                    .icon(IconName::RefreshCw)
                    .small()
                    .ghost()
                    .disabled(!idle)
                    .when(idle, |b| b.cursor_pointer())
                    .accessibility_label(l.text("Rescan mods", "Пересканировать моды"))
                    .tooltip(l.text(
                        "Rescan mods: pick up new and updated .pack files",
                        "Пересканировать моды: найти новые и обновлённые .pack-файлы",
                    ))
                    .on_click(cx.listener(|this, _, _, cx| this.rescan(cx))),
            )
            .child(div().flex_1())
            .children(crate::update::update_button(&self.updater, l, cx))
            .child(self.workshop_menu(cx))
            .child(self.settings_menu(cx))
            .child(self.language_switch(cx))
            .child(divider())
            .when(self.cancellable, |bar| {
                bar.child(
                    super::view::button("cancel", l.text("Cancel", "Отменить"), true).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.cancel
                                .store(true, std::sync::atomic::Ordering::Relaxed);
                            this.status = wh3_core::message!("Cancelling…", "Отмена операции…");
                            cx.notify();
                        }),
                    ),
                )
            })
            .child(self.play_controls(cx))
    }
}
