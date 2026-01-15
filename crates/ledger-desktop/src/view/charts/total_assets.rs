use gpui::{div, prelude::*, App};
use gpui::{Entity, IntoElement, Render};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{h_flex, v_flex};

use ui::icons::IconName;

mod line;
mod pie;

pub fn init(cx: &mut App) -> Entity<TotalAssets> {
    cx.new(TotalAssets::new)
}

pub struct TotalAssets {
    line: Entity<line::Chart>,
    pie: Entity<pie::Chart>,
}

impl TotalAssets {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            line: line::init(cx),
            pie: pie::init(cx),
        }
    }
}

impl Render for TotalAssets {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let selected_tab = state::AppState::get_selected_total_assets_tab_idx(cx);
        let container = v_flex().size_full().child(
            h_flex()
                .justify_end()
                .child(
                    Button::new("total-assets-line-chart")
                        .icon(IconName::ChartLine)
                        .ghost()
                        .tooltip("Line Chart")
                        .on_click(cx.listener(|_this, _event, _window, cx| {
                            state::AppState::update_selected_total_assets_tab_idx(0, cx);
                        })),
                )
                .child(
                    Button::new("total-assets-pie-chart")
                        .icon(IconName::ChartPie)
                        .ghost()
                        .tooltip("Pie Chart")
                        .on_click(cx.listener(|_this, _event, _window, cx| {
                            state::AppState::update_selected_total_assets_tab_idx(1, cx);
                        })),
                ),
        );

        match selected_tab {
            0 => container.child(self.line.clone()),
            1 => container.child(self.pie.clone()),
            _ => div().child("Invalid tab index"),
        }
    }
}
