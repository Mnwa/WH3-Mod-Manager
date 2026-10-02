//! One virtualized row of the mod table.
use super::header::{
    AUTHOR_WIDTH, CHECK_WIDTH, ORDER_WIDTH, PACK_WIDTH, SIZE_WIDTH, THUMB_WIDTH, UPDATED_WIDTH,
};
use super::{Manager, order::DraggedMod, view::cell};
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{
        Disableable, Icon, Sizable, checkbox::Checkbox, menu::ContextMenuExt, tooltip::Tooltip,
    },
    prelude::*,
    *,
};
use std::time::SystemTime;
use wh3_core::{catalog::Source, localization::Language};

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

/// An icon with a tooltip; badges explain row state without extra columns.
pub(super) fn badge(
    id: (&'static str, usize),
    icon: IconName,
    color: Hsla,
    tip: String,
) -> impl IntoElement {
    div()
        .id(id)
        .flex_shrink_0()
        .child(Icon::new(icon).small().text_color(color))
        .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
}

impl Manager {
    fn thumbnail(&self, index: usize) -> AnyElement {
        let frame = div()
            .size(px(32.))
            .rounded_sm()
            .overflow_hidden()
            .bg(theme::thumbnail())
            .flex()
            .items_center()
            .justify_center();
        match &self.catalog.mods[index].thumbnail {
            // GPUI decodes and caches the image off the UI thread.
            Some(path) => frame
                .child(img(path.clone()).size(px(32.)).object_fit(ObjectFit::Cover))
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

    fn badges(&self, index: usize) -> Vec<AnyElement> {
        let l = self.language;
        let item = &self.catalog.mods[index];
        let mut badges = Vec::new();
        if self.is_always_enabled(index) {
            badges.push(
                badge(
                    ("always", index),
                    IconName::Lock,
                    theme::always_enabled(),
                    l.text("Always enabled", "Всегда включён").into(),
                )
                .into_any_element(),
            );
        }
        if item.movie {
            badges.push(badge(("movie", index), IconName::Film, theme::warning(), l.text(
                "Movie pack: loads with high priority; movie packs in data always load",
                "Movie pack: загружается с высоким приоритетом; movie packs из data загружаются всегда",
            ).into()).into_any_element());
        }
        if let Some(files) = self.compat.outdated.get(&index) {
            badges.push(
                badge(
                    ("outdated", index),
                    IconName::Clock,
                    theme::warning(),
                    crate::ui_text!(
                        l,
                        "Older than the last game update and overwrites {} vanilla DB/Lua files, e.g. {}",
                        "Старше последнего обновления игры и перезаписывает ванильных DB/Lua-файлов: {}, например {}",
                        files.len(),
                        files.first().map(String::as_str).unwrap_or_default()
                    ),
                )
                .into_any_element(),
            );
        }
        if self.steam.outdated.contains(&index) {
            badges.push(
                badge(
                    ("update", index),
                    IconName::Download,
                    theme::accent(),
                    l.text(
                        "A newer version is on the Workshop; right-click → Update from Workshop",
                        "В Workshop есть новая версия; правый клик → Обновить из Workshop",
                    )
                    .into(),
                )
                .into_any_element(),
            );
        }
        badges.extend(self.compat_badges(index));
        badges
    }

    pub(super) fn row(&self, position: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let index = self.visible[position];
        let item = &self.catalog.mods[index];
        let l = self.language;
        let title = SharedString::from(item.title.clone());
        let always = self.is_always_enabled(index);
        let draggable = self.sort.is_load_order() && !self.busy;
        let pack_color = match item.source {
            Source::Data => theme::data_pack(),
            Source::Workshop => theme::muted(),
            Source::Custom => theme::accent(),
        };
        div()
            .id(("mod", index))
            .w_full()
            .flex()
            .items_center()
            .h(px(theme::ROW_HEIGHT))
            .pr(px(12.))
            .border_b_1()
            .border_color(theme::border())
            .when(
                self.selected == Some(index) || self.marked.contains(&index),
                |row| row.bg(theme::selection()),
            )
            .hover(|row| row.bg(theme::selection()))
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
                    this.move_onto(dragged.index, index, cx)
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
                    .text_color(theme::accent())
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
                    Checkbox::new(("check", index))
                        .checked(self.enabled.contains(&index))
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
                    .w(px(THUMB_WIDTH))
                    .flex_shrink_0()
                    .child(self.thumbnail(index)),
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
                            .when(always, |title| title.text_color(theme::always_enabled()))
                            .child(title),
                    )
                    .children(self.badges(index)),
            )
            .child(cell(item.name.clone(), PACK_WIDTH).text_color(pack_color))
            .child(cell(item.metadata.author.clone(), AUTHOR_WIDTH).text_color(theme::muted()))
            .child(cell(age(l, item.modified), UPDATED_WIDTH).text_color(theme::muted()))
            .child(
                cell(
                    crate::ui_text!(l, "{:.1} MiB", "{:.1} МБ", item.size as f64 / 1_048_576.),
                    SIZE_WIDTH,
                )
                .text_color(theme::muted()),
            )
            .context_menu(self.row_menu(index, cx))
    }
}
