#![allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::{v_flex, TitleBar};

pub mod accounts_tree;
pub mod dropdown_tree;
pub mod file;
pub mod state;
pub mod transactions_register;

pub struct Window {
    file: Entity<file::LedgerFile>,
}

impl Window {
    pub fn new(window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> Self {
        Self {
            file: cx.new(|cx| file::LedgerFile::new(window, cx)),
        }
    }
}

impl Render for Window {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(TitleBar::new().child(div().text_center().flex_1().child("ledger-desktop")))
            .child(div().size_full().child(self.file.clone()))
    }
}
