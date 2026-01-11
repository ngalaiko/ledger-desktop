use gpui::prelude::*;
use gpui::App;
use gpui::Entity;
use gpui::SharedString;
use gpui_component::button::Button;
use gpui_component::button::ButtonVariants;
use gpui_component::h_flex;
use gpui_component::Disableable;

use crate::icons::IconName;

pub fn init(cx: &mut App) -> Entity<PeriodSelector> {
    cx.new(|cx| PeriodSelector::new(cx))
}

pub struct PeriodSelector {}

impl PeriodSelector {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl Render for PeriodSelector {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let period_idx = state::AppState::get_period_idx(cx);
        h_flex()
            .child(
                Button::new(SharedString::from("period-back"))
                    .ghost()
                    .icon(IconName::ArrowLeft)
                    .on_click(cx.listener(|_this, _event, _window, cx| {
                        state::AppState::update_period_prev(cx);
                    })),
            )
            .child(
                Button::new("period-today")
                    .ghost()
                    .child("Today")
                    .on_click(cx.listener(|_this, _event, _window, cx| {
                        state::AppState::update_period_today(cx);
                    })),
            )
            .child(
                Button::new(SharedString::from("period-next"))
                    .ghost()
                    .disabled(period_idx == 0)
                    .icon(IconName::ArrowRight)
                    .on_click(cx.listener(|_this, _event, _window, cx| {
                        state::AppState::update_period_next(cx);
                    })),
            )
    }
}
