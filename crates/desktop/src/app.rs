use crate::{manager::Manager, theme};
use gpui_kit::{component::Root, prelude::*, *};

pub fn run() {
    let demo = std::env::args()
        .find_map(|arg| {
            arg.strip_prefix("--demo=")
                .and_then(|n| n.parse::<usize>().ok())
        })
        .map(|n| n.min(100_000));
    gpui_kit::application()
        .with_assets(crate::assets::AppAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::install(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1280.), px(820.)),
                    cx,
                ))),
                window_min_size: Some(size(px(1000.), px(600.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("WH3 Mod Manager".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            if let Err(error) = cx.open_window(options, |window, cx| {
                let manager = cx.new(|cx| Manager::new(demo, window, cx));
                let weak = manager.downgrade();
                window.on_window_should_close(cx, move |window, cx| {
                    weak.update(cx, |manager, cx| manager.request_close(window, cx))
                        .unwrap_or(true)
                });
                cx.new(|cx| Root::new(manager, window, cx))
            }) {
                eprintln!("Could not open the window: {error}");
                cx.quit();
            }
            cx.activate(true);
        });
}
