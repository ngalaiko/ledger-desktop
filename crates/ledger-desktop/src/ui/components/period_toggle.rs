use gpui::prelude::*;
use gpui::App;
use gpui::Entity;
use gpui::SharedString;
use gpui_component::{
    button::{Button, ButtonGroup},
    Selectable,
};
use state::period::Period;

pub fn init(cx: &mut App) -> Entity<PeriodToggle> {
    cx.new(|cx| PeriodToggle::new(cx))
}

pub struct PeriodToggle {
    periods: Vec<Period>,
}

impl PeriodToggle {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            periods: vec![Period::Week, Period::Month, Period::Year],
        }
    }
}

impl Render for PeriodToggle {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = state::AppState::get_period(cx);
        let button = ButtonGroup::new("period-toggle").outline().compact();
        self.periods
            .iter()
            .fold(button, |button, &period| {
                button.child(
                    Button::new(SharedString::from(format!("period-{}", period)))
                        .label(period.to_string())
                        .selected(selected == period),
                )
            })
            .on_click(cx.listener({
                |this, selected: &Vec<usize>, _windiow, cx| {
                    if let Some(&ix) = selected.first() {
                        let selected = this.periods[ix];
                        state::AppState::update_period(selected, cx);
                    }
                    cx.notify();
                }
            }))
    }
}
