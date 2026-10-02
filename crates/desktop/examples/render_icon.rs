//! Renders `assets/logo.svg` into the multi-size `assets/app.ico` embedded in the
//! Windows executable. Run after changing the logo:
//! `cargo run -p wh3-mod-manager --example render_icon --locked`.
#![allow(clippy::expect_used)]

use resvg::{tiny_skia, usvg};

/// Sizes Explorer, the taskbar and Alt+Tab pick from at 100–200 % scaling.
const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

fn main() {
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let svg = std::fs::read(assets.join("logo.svg")).expect("read logo.svg");
    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default()).expect("parse logo.svg");
    let images: Vec<Vec<u8>> = SIZES
        .iter()
        .map(|&size| {
            let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("pixmap");
            let scale = size as f32 / tree.size().width();
            resvg::render(
                &tree,
                tiny_skia::Transform::from_scale(scale, scale),
                &mut pixmap.as_mut(),
            );
            pixmap.encode_png().expect("encode png")
        })
        .collect();
    // ICONDIR, one ICONDIRENTRY per size, then PNG payloads (valid since Windows Vista).
    let mut ico = vec![0, 0, 1, 0];
    ico.extend((SIZES.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * SIZES.len();
    for (&size, png) in SIZES.iter().zip(&images) {
        let side = if size >= 256 { 0 } else { size as u8 };
        ico.extend([side, side, 0, 0]);
        ico.extend(1u16.to_le_bytes());
        ico.extend(32u16.to_le_bytes());
        ico.extend((png.len() as u32).to_le_bytes());
        ico.extend((offset as u32).to_le_bytes());
        offset += png.len();
    }
    for png in &images {
        ico.extend(png);
    }
    std::fs::write(assets.join("app.ico"), ico).expect("write app.ico");
}
