mod accounts;
mod ledger;
mod sexpr;
mod transactions;
mod ui;

#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::Root;
use gpui_component_assets::Assets;

fn main() {
    Application::new().with_assets(Assets).run(move |cx| {
        gpui_component::init(cx);

        cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("ledger-desktop".into()),
                    appears_transparent: true,
                    ..TitlebarOptions::default()
                }),
                ..WindowOptions::default()
            },
            |window, cx| {
                let view = cx.new(|cx| ui::Window::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .ok();

        cx.activate(true);
    });
}
