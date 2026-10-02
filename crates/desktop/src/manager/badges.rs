//! Row badges: small icons with tooltips that explain a mod's state without columns.
use super::Manager;
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{Icon, Sizable, tooltip::Tooltip},
    prelude::*,
    *,
};
use wh3_core::catalog::Source;

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
    /// The catalog lists data packs first for a name, so only the first entry is checked.
    fn shadowed_by_data(&self, index: usize) -> bool {
        let name = self.catalog.mods[index].name.to_lowercase();
        self.catalog
            .by_name
            .get(&name)
            .and_then(|all| all.first())
            .is_some_and(|&first| first != index && self.catalog.mods[first].source == Source::Data)
    }

    pub(super) fn badges(&self, index: usize) -> Vec<AnyElement> {
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
        if item.source != Source::Data && self.shadowed_by_data(index) {
            badges.push(
                badge(
                    ("shadowed", index),
                    IconName::Layers,
                    theme::muted(),
                    l.text(
                        "A copy with the same name is in the game's data folder; the game loads that one",
                        "Копия с тем же именем есть в папке data игры; игра загружает её",
                    )
                    .into(),
                )
                .into_any_element(),
            );
        }
        badges.extend(self.compat_badges(index));
        badges
    }
}
