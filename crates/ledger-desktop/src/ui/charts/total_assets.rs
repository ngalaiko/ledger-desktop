use gpui::{div, prelude::*, App, Subscription};
use gpui::{Entity, IntoElement, Render};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{h_flex, v_flex};
use state::AppState;

use crate::icons::IconName;

mod line;
mod pie;

pub fn init(cx: &mut App) -> Entity<TotalAssets> {
    cx.new(|cx| TotalAssets::new(cx))
}

pub struct TotalAssets {
    line: Entity<line::Chart>,
    pie: Entity<pie::Chart>,
    _subscriptions: Vec<Subscription>,
}

impl TotalAssets {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![];
        subscriptions.push(
            // observe state changes and update views accordingly
            cx.observe(&AppState::global(cx), move |this, _app_state, cx| {
                this.line.update(cx, |this, cx| {
                    this.refresh_data(cx);
                });
                this.pie.update(cx, |this, cx| {
                    this.refresh_data(cx);
                });
            }),
        );
        subscriptions.push(
            // observe ledger file changes and update views accordingly
            cx.observe(&ledger::File::global(cx), move |this, _file, cx| {
                this.line.update(cx, |this, cx| {
                    this.refresh_data(cx);
                });
                this.pie.update(cx, |this, cx| {
                    this.refresh_data(cx);
                });
            }),
        );
        Self {
            line: line::init(cx),
            pie: pie::init(cx),
            _subscriptions: subscriptions,
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
