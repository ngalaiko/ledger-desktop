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

        cx.open_window(WindowOptions::default(), |window, cx| {
            let view = cx.new(|cx| ui::file::LedgerFile::new(window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .ok();

        cx.activate(true);
    });
}
