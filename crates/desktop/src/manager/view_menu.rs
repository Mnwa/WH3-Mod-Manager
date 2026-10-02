//! "View" menu beside the search bar: list layout, category groups, row size and
//! column widths. Everything here is a WHP2 preference.
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
use wh3_core::preferences::{Density, Layout, Preferences};

type Change = Box<dyn Fn(&mut Preferences)>;

impl Manager {
    pub(super) fn view_menu(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (manager, l, prefs) = (cx.entity().downgrade(), self.language, self.prefs);
        let grouped = self.grouping_active();
        Button::new("view")
            .icon(IconName::LayoutDashboard)
            .label(l.text("View", "Вид"))
            .small()
            .cursor_pointer()
            .dropdown_caret(true)
            .tooltip(l.text(
                "One or two lists, category groups, row size and column widths",
                "Один или два списка, группы по категориям, размер строк и ширина столбцов",
            ))
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu: PopupMenu, _, _| {
                let item = |label: &'static str, checked: bool, change: Change| {
                    let manager = manager.clone();
                    PopupMenuItem::new(label)
                        .checked(checked)
                        .on_click(move |_, _, cx| {
                            let _ = manager.update(cx, |this, cx| this.update_prefs(&change, cx));
                        })
                };
                let mut menu = menu
                    .label(l.text("Layout", "Расположение"))
                    .item(item(
                        l.text("One list", "Один список"),
                        prefs.layout == Layout::Single,
                        Box::new(|p| p.layout = Layout::Single),
                    ))
                    .item(item(
                        l.text(
                            "Two lists: not enabled | enabled",
                            "Два списка: не включены | включены",
                        ),
                        prefs.layout == Layout::Dual,
                        Box::new(|p| p.layout = Layout::Dual),
                    ))
                    .item(item(
                        l.text("Group by category", "Группировать по категориям"),
                        prefs.group_by_category,
                        Box::new(|p| p.group_by_category ^= true),
                    ));
                if grouped {
                    let groups = manager.clone();
                    menu = menu.item(
                        PopupMenuItem::new(l.text(
                            "Collapse or expand all groups",
                            "Свернуть или развернуть все группы",
                        ))
                        .on_click(move |_, _, cx| {
                            let _ = groups.update(cx, |this, cx| this.toggle_all_groups(cx));
                        }),
                    );
                }
                menu = menu.separator().label(l.text("Row size", "Размер строк"));
                for density in Density::ALL {
                    let label = match density {
                        Density::Compact => l.text("Compact", "Компактный"),
                        Density::Comfortable => l.text("Comfortable", "Обычный"),
                        Density::Roomy => l.text("Roomy", "Крупный"),
                    };
                    menu = menu.item(item(
                        label,
                        prefs.density == density,
                        Box::new(move |p| p.density = density),
                    ));
                }
                let reset = manager.clone();
                menu.separator().item(
                    PopupMenuItem::new(l.text("Reset column widths", "Сбросить ширину столбцов"))
                        .disabled(prefs.widths_are_default())
                        .on_click(move |_, _, cx| {
                            let _ = reset.update(cx, |this, cx| {
                                this.update_prefs(Preferences::reset_widths, cx)
                            });
                        }),
                )
            })
    }
}
