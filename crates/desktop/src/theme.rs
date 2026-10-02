use gpui_kit::{
    App, Hsla,
    component::{Theme, ThemeMode},
    rgb,
};

pub const ROW_HEIGHT: f32 = 40.;
pub fn background() -> Hsla {
    rgb(0x17191d).into()
}
pub fn panel() -> Hsla {
    rgb(0x1e2127).into()
}
pub fn border() -> Hsla {
    rgb(0x343941).into()
}
pub fn text() -> Hsla {
    rgb(0xe6e9ee).into()
}
pub fn muted() -> Hsla {
    rgb(0xa4adbb).into()
}
pub fn accent() -> Hsla {
    rgb(0x619cff).into()
}
pub fn selection() -> Hsla {
    rgb(0x28374e).into()
}
pub fn warning() -> Hsla {
    rgb(0xe8b66a).into()
}

pub fn danger() -> Hsla {
    rgb(0xef6461).into()
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
    rgb(0x262a31).into()
}

pub fn install(cx: &mut App) {
    // Apply shared tokens to the kit components as well.
    let theme = Theme::global_mut(cx);
    let mut dark = (*theme.dark_theme).clone();
    dark.colors.primary = Some("#3c78d8".into());
    dark.colors.primary_foreground = Some("#ffffff".into());
    theme.dark_theme = std::rc::Rc::new(dark);
    Theme::change(ThemeMode::Dark, None, cx);
}
