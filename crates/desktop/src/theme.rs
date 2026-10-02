//! Shared palette. Surfaces are quiet gunmetal; brass is reserved for the logo,
//! enabled checkboxes and Play, so the eye always finds "what will run" first.
use gpui_kit::{
    App, Hsla,
    component::{Theme, ThemeMode},
    rgb,
};

pub const ROW_HEIGHT: f32 = 40.;
pub fn background() -> Hsla {
    rgb(0x15181d).into()
}
pub fn panel() -> Hsla {
    rgb(0x1b1f25).into()
}
/// Hover and raised surfaces inside panels.
pub fn raised() -> Hsla {
    rgb(0x242932).into()
}
pub fn border() -> Hsla {
    rgb(0x2e343e).into()
}
pub fn text() -> Hsla {
    rgb(0xebe7df).into()
}
/// Titles of disabled mods: readable, but quieter than the ones that will load.
pub fn text_dim() -> Hsla {
    rgb(0xb5b1a9).into()
}
pub fn muted() -> Hsla {
    rgb(0x9aa2ad).into()
}
/// Links, custom-folder packs and informational highlights.
pub fn accent() -> Hsla {
    rgb(0x7fa7d9).into()
}
/// The brand color: Play, enabled checkboxes and the active filter.
pub fn brass() -> Hsla {
    rgb(0xd4a64a).into()
}
pub fn selection() -> Hsla {
    rgb(0x2a3444).into()
}
pub fn warning() -> Hsla {
    rgb(0xe9a23b).into()
}
pub fn danger() -> Hsla {
    rgb(0xe5534b).into()
}
pub fn success() -> Hsla {
    rgb(0x6cbf84).into()
}
/// Data packs are highlighted like the original's orange pack names.
pub fn data_pack() -> Hsla {
    rgb(0xe39a5b).into()
}
/// Always-enabled mods use the original's violet accent.
pub fn always_enabled() -> Hsla {
    rgb(0xb18cff).into()
}
pub fn thumbnail() -> Hsla {
    rgb(0x1e232b).into()
}

pub fn install(cx: &mut App) {
    // Apply shared tokens to the kit components as well.
    let theme = Theme::global_mut(cx);
    let mut dark = (*theme.dark_theme).clone();
    let colors = &mut dark.colors;
    colors.background = Some("#15181d".into());
    colors.border = Some("#2e343e".into());
    colors.input = Some("#4a5260".into());
    colors.ring = Some("#d4a64a".into());
    colors.primary = Some("#d4a64a".into());
    colors.primary_hover = Some("#e0b55c".into());
    colors.primary_active = Some("#bf9340".into());
    colors.primary_foreground = Some("#1b1408".into());
    colors.button_primary = Some("#d4a64a".into());
    colors.button_primary_hover = Some("#e0b55c".into());
    colors.button_primary_active = Some("#bf9340".into());
    colors.button_primary_foreground = Some("#1b1408".into());
    colors.secondary = Some("#242932".into());
    colors.secondary_hover = Some("#2c323c".into());
    colors.secondary_active = Some("#333a46".into());
    colors.popover = Some("#1f232a".into());
    colors.accent = Some("#2a3444".into());
    colors.list_active = Some("#2a3444".into());
    colors.list_active_border = Some("#d4a64a".into());
    theme.dark_theme = std::rc::Rc::new(dark);
    Theme::change(ThemeMode::Dark, None, cx);
}
