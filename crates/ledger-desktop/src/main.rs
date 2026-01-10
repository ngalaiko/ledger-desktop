mod data;
mod ui;

use gpui::prelude::*;
use gpui::{
    point, px, size, Application, Bounds, TitlebarOptions, WindowBackgroundAppearance,
    WindowBounds, WindowDecorations, WindowKind, WindowOptions,
};
use gpui_component::Root;
use gpui_component_assets::Assets;

fn main() {
    Application::new().with_assets(Assets).run(move |cx| {
        let bounds = Bounds::centered(None, size(px(920.0), px(700.0)), cx);

        let opts = WindowOptions {
            window_background: WindowBackgroundAppearance::Opaque,
            window_decorations: Some(WindowDecorations::Client),
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::Normal,
            titlebar: Some(TitlebarOptions {
                title: Some("ledger-desktop".into()),
                traffic_light_position: Some(point(px(9.0), px(9.0))),
                appears_transparent: true,
            }),
            ..WindowOptions::default()
        };

        // create the main window
        cx.open_window(opts, |window: &mut gpui::Window, cx| {
            // bring window to front and give it focus
            cx.activate(true);
            cx.new(|cx| {
                gpui_component::init(cx);
                state::init(cx);
                ledger::init::<&str>(None, cx);
                data::init(cx);

                Root::new(ui::init(window, cx), window, cx)
            })
        })
        .ok();
    });
}
