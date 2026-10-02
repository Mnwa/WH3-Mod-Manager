#![allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(target_os = "macos")]
fn main() {
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, HeadlessAppContext, component::Root, px, size};
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    use wh3_mod_manager::{manager::Manager, theme};
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        theme::install(cx);
    });
    for count in [1_000, 10_000, 100_000] {
        let mut manager = None;
        let window = cx
            .open_window(size(px(1280.), px(820.)), |window, cx| {
                let view = cx.new(|cx| Manager::new(Some(count), window, cx));
                manager = Some(view.clone());
                cx.new(|cx| Root::new(view, window, cx))
            })
            .unwrap();
        let manager = manager.unwrap();
        for _ in 0..20 {
            cx.run_until_parked();
            cx.advance_clock(Duration::from_millis(100));
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
                .unwrap();
        }
        let mut samples = Vec::new();
        for _ in 0..50 {
            cx.update(|cx| {
                manager.update(cx, |view, cx| {
                    view.rendered_rows = 0;
                    cx.notify();
                })
            });
            let started = Instant::now();
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
                .unwrap();
            samples.push(started.elapsed().as_secs_f64() * 1000.);
            let rows = cx.update(|cx| manager.read(cx).rendered_rows);
            assert!(
                rows > 0 && rows < 100,
                "virtualization: {count} mods rendered {rows} rows"
            );
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "mods={count} frame_p50_ms={:.3} frame_p95_ms={:.3}",
            samples[25], samples[47]
        );
        let image = cx
            .capture_screenshot(window.into())
            .expect("Metal renderer");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/visual");
        std::fs::create_dir_all(&dir).unwrap();
        image.save(dir.join(format!("mods-{count}.png"))).unwrap();
        cx.update_window(window.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("check", 0usize), cx);
            assert_eq!(window.find(("check", 0usize)).checked(), Some(true));
            window.click("enabled", cx);
        })
        .unwrap();
        for _ in 0..4 {
            cx.run_until_parked();
            cx.advance_clock(Duration::from_millis(100));
            cx.update_window(window.into(), |_, window, cx| window.render_frame(cx))
                .unwrap();
        }
        cx.update_window(window.into(), |_, window, cx| {
            assert!(window.try_find(("check", 1usize)).is_none());
            window.click("all", cx);
            window.press("cmd-f", cx);
            window.input(&format!("mod_{:06}.pack", count - 1), cx);
        })
        .unwrap();
        for _ in 0..4 {
            cx.run_until_parked();
            cx.advance_clock(Duration::from_millis(100));
            cx.update_window(window.into(), |_, window, cx| window.render_frame(cx))
                .unwrap();
        }
        cx.update_window(window.into(), |_, window, _| {
            assert!(window.find(("check", count - 1)).visible());
            assert!(window.try_find(("check", 0usize)).is_none());
        })
        .unwrap();
        cx.update_window(window.into(), |_, window, _| window.remove_window())
            .unwrap();
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("SKIP visual: GPUI Metal headless test requires macOS");
}
