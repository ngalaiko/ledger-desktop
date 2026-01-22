use gpui::{div, prelude::*, App};
use gpui::{Entity, IntoElement, Render};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{h_flex, v_flex};

use ui::icons::IconName;

mod bar;
mod line;
mod pie;
mod summary;

pub fn init(cx: &mut App) -> Entity<Expenses> {
    cx.new(Expenses::new)
}

pub struct Expenses {
    line: Entity<line::Chart>,
    pie: Entity<pie::Chart>,
    bar: Entity<bar::Chart>,
}

impl Expenses {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            line: line::init(cx),
            pie: pie::init(cx),
            bar: bar::init(cx),
        }
    }
}

impl Render for Expenses {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let selected_tab = state::AppState::get_selected_expenses_tab_idx(cx);
        let container = v_flex().size_full().child(
            h_flex()
                .justify_end()
                .child(
                    Button::new("expenses-line-chart")
                        .icon(IconName::ChartLine)
                        .ghost()
                        .tooltip("Line Chart")
                        .on_click(cx.listener(|_this, _event, _window, cx| {
                            state::AppState::update_selected_expenses_tab_idx(0, cx);
                        })),
                )
                .child(
                    Button::new("expenses-bar-chart")
                        .icon(IconName::ChartBar)
                        .ghost()
                        .tooltip("Bar Chart")
                        .on_click(cx.listener(|_this, _event, _window, cx| {
                            state::AppState::update_selected_expenses_tab_idx(1, cx);
                        })),
                )
                .child(
                    Button::new("expenses-pie-chart")
                        .icon(IconName::ChartPie)
                        .ghost()
                        .tooltip("Pie Chart")
                        .on_click(cx.listener(|_this, _event, _window, cx| {
                            state::AppState::update_selected_expenses_tab_idx(2, cx);
                        })),
                ),
        );

        match selected_tab {
            0 => container.child(self.line.clone()),
            1 => container.child(self.bar.clone()),
            2 => container.child(self.pie.clone()),
            _ => div().child("Invalid tab index"),
        }
    }
}
