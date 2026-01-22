use std::collections::HashMap;

use chrono::NaiveDate;
use gpui::{div, App, Entity, Window};
use gpui::{prelude::*, Subscription};
use gpui_component::{h_flex, v_flex, ActiveTheme, StyledExt};

use ledger::{AccountType, Balance};

use crate::data::balance::DailyBalance;
use crate::util::observe_multiple;
use state::AppState;

pub fn init(cx: &mut App) -> Entity<Summary> {
    cx.new(Summary::new)
}

pub struct Summary {
    current_total: HashMap<String, f64>,
    previous_total: HashMap<String, f64>,
    _subscriptions: Vec<Subscription>,
}

impl Summary {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![];
        subscriptions.push(observe_multiple(
            cx,
            (&DailyBalance::global(cx), &AppState::global(cx)),
            |this, cx| {
                let daily_balance = DailyBalance::global(cx);
                let app_state = AppState::global(cx);

                let app_state = app_state.read(cx);
                let daily_balance = daily_balance.read(cx);

                // Calculate current period data
                let current_interval = app_state.values.get_period_interval();
                this.current_total = calculate(daily_balance, &current_interval);

                // Calculate previous period data
                let prev_interval = app_state.values.get_previous_period_interval();
                this.previous_total = calculate(daily_balance, &prev_interval);

                cx.notify();
            },
        ));
        Self {
            current_total: HashMap::new(),
            previous_total: HashMap::new(),
            _subscriptions: subscriptions,
        }
    }
}

fn calculate(
    daily_balance: &DailyBalance,
    date_range: &std::ops::Range<NaiveDate>,
) -> HashMap<String, f64> {
    // Collect expense accounts
    let expense_accounts: Vec<_> = daily_balance
        .iter()
        .filter(|(account, _)| account.type_of == AccountType::Expenses)
        .collect();

    let mut total_balance = Balance::default();

    // Sum daily balances across all expense accounts for the entire period
    let mut current_date = date_range.start;
    while date_range.contains(&current_date) {
        for (account, _) in &expense_accounts {
            let daily = daily_balance.get_daily_balance(account, current_date);
            total_balance.add(&daily);
        }
        current_date += chrono::Duration::days(1);
    }

    // Convert balance to HashMap<commodity, value>
    let mut result = HashMap::new();
    for amount in total_balance.iter() {
        let value = amount.value;
        result.insert(amount.commodity.clone(), value.to_f64());
    }
    result
}

#[allow(clippy::cast_possible_truncation)]
fn format_value(value: f64) -> String {
    let abs_value = value.abs();
    if abs_value >= 1_000_000_000.0 {
        format!("{:.1}B", value / 1_000_000_000.0)
    } else if abs_value >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if abs_value >= 1_000.0 {
        format_with_spaces(value as i64)
    } else if abs_value >= 1.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

fn format_with_spaces(value: i64) -> String {
    let abs = value.abs();
    let s = abs.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(' ');
        }
        result.push(c);
    }
    if value < 0 {
        result.push('-');
    }
    result.chars().rev().collect()
}

impl Render for Summary {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let green = cx.theme().colors.green;
        let red = cx.theme().colors.red;
        let muted = cx.theme().muted_foreground;

        // Collect all commodities from both current and previous periods
        let mut all_commodities: Vec<_> = self
            .current_total
            .keys()
            .chain(self.previous_total.keys())
            .cloned()
            .collect();
        all_commodities.sort();
        all_commodities.dedup();

        // Handle no data
        if all_commodities.is_empty() {
            return v_flex()
                .gap_1()
                .child(div().text_sm().text_color(muted).child("Expenses"))
                .child(div().text_2xl().font_semibold().child("0"));
        }

        v_flex()
            .gap_1()
            .children(all_commodities.into_iter().map(|commodity| {
                let current = self.current_total.get(&commodity).copied().unwrap_or(0.0);
                let previous = self.previous_total.get(&commodity).copied();
                let diff = previous.map(|prev| current - prev);

                let (indicator, diff_color) = match diff {
                    Some(d) if d < 0.0 => ("▼", green),
                    Some(d) if d > 0.0 => ("▲", red),
                    _ => ("", muted),
                };

                v_flex()
                    .gap_1()
                    // Title
                    .child(div().text_sm().text_color(muted).child("Expenses"))
                    // Current total (large)
                    .child(div().text_2xl().font_semibold().child(format!(
                        "{} {}",
                        format_value(current),
                        commodity
                    )))
                    // Difference from previous period
                    .when_some(diff, |el, d| {
                        if d.abs() > f64::EPSILON {
                            el.child(
                                h_flex()
                                    .gap_1()
                                    .text_sm()
                                    .text_color(diff_color)
                                    .child(indicator)
                                    .child(format!("{} {}", format_value(d.abs()), commodity)),
                            )
                        } else {
                            el
                        }
                    })
            }))
    }
}
