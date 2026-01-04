use core::fmt;

use chrono::Datelike;
#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    menu::DropdownMenu,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
pub enum Period {
    WTD,
    D7,
    MTD,
    D30,
    D90,
    YTD,
    Y1,
    Y3,
    All,
}

impl Period {
    pub fn start_date(&self, reference_date: chrono::NaiveDate) -> Option<chrono::NaiveDate> {
        match self {
            Period::WTD => {
                // Week-to-date: start of current week (Monday)
                let days_from_monday = reference_date.weekday().num_days_from_monday();
                Some(reference_date - chrono::Duration::days(days_from_monday as i64))
            }
            Period::D7 => {
                // Last 7 days
                Some(reference_date - chrono::Duration::days(7))
            }
            Period::MTD => {
                // Month-to-date: first day of current month
                reference_date.with_day(1)
            }
            Period::D30 => {
                // Last 30 days
                Some(reference_date - chrono::Duration::days(30))
            }
            Period::D90 => {
                // Last 90 days
                Some(reference_date - chrono::Duration::days(90))
            }
            Period::YTD => {
                // Year-to-date: January 1st of current year
                reference_date.with_month(1).and_then(|d| d.with_day(1))
            }
            Period::Y1 => {
                // Last 1 year
                Some(reference_date - chrono::Duration::days(365))
            }
            Period::Y3 => {
                // Last 3 years
                Some(reference_date - chrono::Duration::days(365 * 3))
            }
            Period::All => {
                // No filtering
                None
            }
        }
    }
}

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Period::WTD => "WTD",
            Period::D7 => "7D",
            Period::MTD => "MTD",
            Period::D30 => "30D",
            Period::D90 => "90D",
            Period::YTD => "YTD",
            Period::Y1 => "1Y",
            Period::Y3 => "3Y",
            Period::All => "All",
        };
        write!(f, "{s}")
    }
}

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
