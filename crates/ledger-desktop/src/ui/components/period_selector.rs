#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    menu::DropdownMenu,
};
use period::Period;

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
#[action(namespace = balance_chart)]
pub struct SelectPeriod {
    pub period: Period,
}

pub fn period_selector(current_period: Period) -> impl IntoElement {
    Button::new("period-selector")
        .label(current_period.to_string())
        .ghost()
        .dropdown_menu(|menu, _window, _cx| {
            [
                Period::WTD,
                Period::D7,
                Period::MTD,
                Period::D30,
                Period::D90,
                Period::YTD,
                Period::Y1,
                Period::Y3,
                Period::All,
            ]
            .iter()
            .fold(menu, |menu, &period| {
                menu.menu(period.to_string(), Box::new(SelectPeriod { period }))
            })
        })
}
