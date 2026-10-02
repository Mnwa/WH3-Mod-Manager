//! Sidebar presets: save the current list under a name, apply or manage saved ones.
use super::{
    Manager,
    presets::PresetAction,
    sidebar::{caption, hint},
};
use gpui_kit::{
    assets::IconName,
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::Input,
        menu::{DropdownMenu as _, PopupMenuItem},
    },
    prelude::*,
    *,
};

impl Manager {
    pub(super) fn presets_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (l, available) = (self.language, !self.busy);
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(caption(l.text("Presets", "Пресеты")))
                    .child(
                        div()
                            .mt_4()
                            .flex()
                            .child(
                                Button::new("share")
                                    .icon(IconName::Share2)
                                    .label(l.text("Share", "Поделиться"))
                                    .xsmall()
                                    .ghost()
                                    .disabled(!available)
                                    .when(available, |b| b.cursor_pointer())
                                    .tooltip(l.text(
                                        "Copy your list as text for a friend, or use theirs",
                                        "Скопировать список текстом для друга или взять его список",
                                    ))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_share(window, cx)
                                    })),
                            )
                            .child(self.preset_files_menu(cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.preset_name).small()),
                    )
                    .child(
                        Button::new("save-preset")
                            .icon(IconName::Plus)
                            .small()
                            .disabled(!available)
                            .when(available, |b| b.cursor_pointer())
                            .accessibility_label(l.text("Save preset", "Сохранить пресет"))
                            .tooltip(l.text(
                                "Save the enabled mods and their order as a preset",
                                "Сохранить включённые моды и их порядок как пресет",
                            ))
                            .on_click(cx.listener(|this, _, _, cx| this.save_preset(cx))),
                    ),
            )
            .when(self.settings.presets.is_empty(), |section| {
                section.child(hint(l.text(
                    "Name the current mod list to switch back to it in one click.",
                    "Назовите текущий список модов, чтобы возвращаться к нему в один клик.",
                )))
            })
    }

    fn preset_files_menu(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (l, available, manager) = (self.language, !self.busy, cx.entity().downgrade());
        Button::new("preset-files")
            .icon(IconName::Ellipsis)
            .xsmall()
            .ghost()
            .disabled(!available)
            .when(available, |b| b.cursor_pointer())
            .accessibility_label(l.text("Preset files", "Файлы пресетов"))
            .tooltip(l.text(
                "Import or export preset files",
                "Импорт и экспорт файлов пресетов",
            ))
            .dropdown_menu(move |menu, _, _| {
                let (import, export) = (manager.clone(), manager.clone());
                menu.item(
                    PopupMenuItem::new(l.text(
                        "Import presets from a file…",
                        "Импортировать пресеты из файла…",
                    ))
                    .on_click(move |_, _, cx| {
                        let _ = import.update(cx, |this, cx| this.import(cx));
                    }),
                )
                .item(
                    PopupMenuItem::new(l.text(
                        "Export the current list to a file…",
                        "Экспортировать текущий список в файл…",
                    ))
                    .on_click(move |_, _, cx| {
                        let _ = export.update(cx, |this, cx| this.export(cx));
                    }),
                )
            })
    }

    pub(super) fn preset_row(
        &self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let (l, busy, manager) = (self.language, self.busy, cx.entity().downgrade());
        let name: SharedString = self.settings.presets[index].name.clone().into();
        div()
            .w_full()
            .h(px(32.))
            .py_0p5()
            .flex()
            .gap_1()
            .child(
                Button::new(("preset", index))
                    .child(div().w_full().min_w_0().truncate().child(name.clone()))
                    .small()
                    .ghost()
                    .flex_1()
                    .min_w_0()
                    .disabled(busy)
                    .when(!busy, |b| b.cursor_pointer())
                    .tooltip(l.text(
                        "Apply: enable exactly these mods in this order",
                        "Применить: включить ровно эти моды в этом порядке",
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.preset_action(index, PresetAction::Apply, cx)
                    })),
            )
            .child(
                Button::new(("preset-menu", index))
                    .icon(IconName::Ellipsis)
                    .small()
                    .ghost()
                    .disabled(busy)
                    .when(!busy, |b| b.cursor_pointer())
                    .accessibility_label(crate::ui_text!(
                        l,
                        "Preset actions: {}",
                        "Действия с пресетом: {}",
                        name
                    ))
                    .tooltip(crate::ui_text!(l, "More for “{}”", "Ещё для «{}»", name))
                    .dropdown_menu(move |menu, _, _| {
                        let item = |label: &'static str, action: PresetAction| {
                            let manager = manager.clone();
                            PopupMenuItem::new(label).on_click(move |_, _, cx| {
                                let _ = manager
                                    .update(cx, |this, cx| this.preset_action(index, action, cx));
                            })
                        };
                        menu.item(item(
                            l.text("Apply (only these mods)", "Применить (только эти моды)"),
                            PresetAction::Apply,
                        ))
                        .item(item(
                            l.text("Also enable its mods", "Дополнительно включить его моды"),
                            PresetAction::Merge,
                        ))
                        .item(item(
                            l.text("Disable its mods", "Отключить его моды"),
                            PresetAction::Subtract,
                        ))
                        .separator()
                        .item(item(
                            l.text(
                                "Overwrite with the current list",
                                "Перезаписать текущим списком",
                            ),
                            PresetAction::Replace,
                        ))
                        .item(item(l.text("Delete", "Удалить"), PresetAction::Delete))
                    }),
            )
    }
}
