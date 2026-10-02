//! One virtualized row of the mod table.
use super::header::{CHECK_WIDTH, ORDER_WIDTH};
use super::{Manager, order::DraggedMod, view::cell};
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{Disableable, Icon, Sizable, checkbox::Checkbox, menu::ContextMenuExt},
    prelude::*,
    *,
};
use std::time::SystemTime;
use wh3_core::{
    catalog::Source,
    localization::Language,
    preferences::{Column, Density},
};

/// Row and thumbnail sizes for a row-size preference.
#[derive(Clone, Copy)]
pub(super) struct Metrics {
    pub row: f32,
    pub thumb: f32,
    pub large_text: bool,
}

impl Metrics {
    pub fn of(density: Density) -> Self {
        match density {
            Density::Compact => Self {
                row: 30.,
                thumb: 22.,
                large_text: false,
            },
            Density::Comfortable => Self {
                row: theme::ROW_HEIGHT,
                thumb: 32.,
                large_text: false,
            },
            Density::Roomy => Self {
                row: 60.,
                thumb: 48.,
                large_text: true,
            },
        }
    }
}

/// How a row appears in a particular list.
pub(super) struct Slot {
    pub row: ElementId,
    pub check: ElementId,
    /// Two-list panes show only the title column.
    pub compact: bool,
    pub draggable: bool,
    pub enables_on_drop: bool,
}

impl Slot {
    /// The main table keeps the ids tests and accessibility rely on.
    pub fn table(index: usize, draggable: bool) -> Self {
        Self {
            row: ("mod", index).into(),
            check: ("check", index).into(),
            compact: false,
            draggable,
            enables_on_drop: false,
        }
    }
}

/// Coarse relative age, like the original's "~N ago" column.
fn age(language: Language, modified: Option<SystemTime>) -> String {
    let Some(elapsed) = modified.and_then(|time| SystemTime::now().duration_since(time).ok())
    else {
        return String::new();
    };
    let days = elapsed.as_secs() / 86_400;
    match days {
        0 => language.text("today", "сегодня").into(),
        1..=29 => crate::ui_text!(language, "{} d ago", "{} дн. назад", days),
        30..=364 => crate::ui_text!(language, "{} mo ago", "{} мес. назад", days / 30),
        _ => crate::ui_text!(language, "{} y ago", "{} г. назад", days / 365),
    }
}

/// Pack size in MiB, or GiB once it no longer fits the column.
fn size(language: Language, bytes: u64) -> String {
    let mib = bytes as f64 / 1_048_576.;
    if mib >= 1024. {
        crate::ui_text!(language, "{:.1} GiB", "{:.1} ГБ", mib / 1024.)
    } else {
        crate::ui_text!(language, "{:.1} MiB", "{:.1} МБ", mib)
    }
}

impl Manager {
    fn thumbnail(&self, index: usize, side: f32) -> AnyElement {
        let frame = div()
            .size(px(side))
            .rounded_sm()
            .overflow_hidden()
            .border_1()
            .border_color(theme::border())
            .bg(theme::thumbnail())
            .flex()
            .items_center()
            .justify_center();
        match &self.catalog.mods[index].thumbnail {
            // GPUI decodes and caches the image off the UI thread.
            Some(path) => frame
                .child(
                    img(path.clone())
                        .size(px(side))
                        .object_fit(ObjectFit::Cover),
                )
                .into_any_element(),
            None => frame
                .child(
                    Icon::new(IconName::Package)
                        .small()
                        .text_color(theme::border()),
                )
                .into_any_element(),
        }
    }

    /// One mod row. `slot` keeps ids unique per list and says how the row behaves there.
    pub(super) fn mod_row(&self, index: usize, slot: Slot, cx: &mut Context<Self>) -> AnyElement {
        let item = &self.catalog.mods[index];
        let l = self.language;
        let title = SharedString::from(item.title.clone());
        let always = self.is_always_enabled(index);
        let enabled = self.enabled.contains(&index);
        let draggable = slot.draggable && !self.busy;
        let metrics = Metrics::of(self.prefs.density);
        let pack_color = match item.source {
            Source::Data if item.linked => theme::accent(),
            Source::Data => theme::data_pack(),
            Source::Workshop => theme::muted(),
            Source::Custom => theme::accent(),
        };
        let enables_on_drop = slot.enables_on_drop;
        let width = |column| self.prefs.width(column) as f32;
        div()
            .id(slot.row)
            .w_full()
            .flex()
            .items_center()
            .h(px(metrics.row))
            .pr(px(12.))
            .border_b_1()
            .border_color(theme::border())
            .when(metrics.large_text, |row| row.text_base())
            .map(|row| {
                if self.selected == Some(index) || self.marked.contains(&index) {
                    row.bg(theme::selection())
                } else {
                    row.hover(|row| row.bg(theme::raised()))
                }
            })
            .when(draggable, |row| {
                row.on_drag(
                    DraggedMod {
                        index,
                        title: title.clone(),
                    },
                    |drag, _, _, cx| cx.new(|_| drag.clone()),
                )
                .drag_over::<DraggedMod>(|style, _, _, _| {
                    style.border_t_2().border_color(theme::accent())
                })
                .on_drop(cx.listener(move |this, dragged: &DraggedMod, _, cx| {
                    this.move_onto(dragged.index, index, cx);
                    // Dropping a disabled mod among the enabled ones enables it.
                    if enables_on_drop && !this.enabled.contains(&dragged.index) {
                        this.toggle(dragged.index, cx);
                    }
                }))
            })
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                this.focus.focus(window, cx);
                this.click_row(index, event.modifiers(), cx);
            }))
            .child(
                div()
                    .w(px(ORDER_WIDTH))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .text_color(theme::muted())
                    .child(
                        Icon::new(IconName::GripVertical)
                            .xsmall()
                            .text_color(if draggable {
                                theme::muted()
                            } else {
                                theme::border()
                            }),
                    )
                    .child(self.ranks[index].to_string()),
            )
            .child(
                div().w(px(CHECK_WIDTH)).flex_shrink_0().child(
                    Checkbox::new(slot.check)
                        .checked(enabled)
                        .disabled(self.busy)
                        .accessibility_label(crate::ui_text!(
                            l,
                            "Enable {}",
                            "Включить {}",
                            item.title
                        ))
                        .on_click(cx.listener(move |this, _, _, cx| this.toggle(index, cx))),
                ),
            )
            .child(
                div()
                    .w(px(metrics.thumb + 16.))
                    .flex_shrink_0()
                    .child(self.thumbnail(index, metrics.thumb)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            // Enabled mods read brighter, so what will load stands out.
                            .text_color(if always {
                                theme::always_enabled()
                            } else if enabled {
                                theme::text()
                            } else {
                                theme::text_dim()
                            })
                            .child(title),
                    )
                    .children(self.badges(index)),
            )
            .when(!slot.compact, |row| {
                row.child(cell(item.name.clone(), width(Column::Pack)).text_color(pack_color))
                    .child(
                        cell(item.metadata.author.clone(), width(Column::Author))
                            .text_color(theme::muted()),
                    )
                    .child(
                        cell(age(l, item.modified), width(Column::Updated))
                            .text_color(theme::muted()),
                    )
                    .child(cell(size(l, item.size), width(Column::Size)).text_color(theme::muted()))
            })
            .context_menu(self.row_menu(index, cx))
            .into_any_element()
    }
}
