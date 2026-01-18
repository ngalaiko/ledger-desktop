use std::collections::HashMap;

use fastnum::D128;
use gpui::{App, Entity, Window};
use gpui::{prelude::*, Subscription};
use gpui_component::v_flex;

use ledger::{AccountType, Balance};

use crate::data::balance::DailyBalance;
use crate::util::observe_multiple;
use crate::view::components::charts::{line, Label};
use state::AppState;

use super::summary;

pub fn init(cx: &mut App) -> Entity<Chart> {
    cx.new(Chart::new)
}

pub struct Chart {
    summary: Entity<summary::Summary>,
    chart: Entity<line::Chart>,
    _subscriptions: Vec<Subscription>,
}

impl Chart {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![];
        subscriptions.push(observe_multiple(
            cx,
            (&DailyBalance::global(cx), &AppState::global(cx)),
            |this, cx| {
                let daily_balance = DailyBalance::global(cx);
                let app_state = AppState::global(cx);
                this.chart.update(cx, |this, cx| {
                    let app_state = app_state.read(cx);
                    let daily_balance = daily_balance.read(cx);

                    // Calculate current period data
                    let current_interval = app_state.values.get_period_interval();
                    let (dates, values) = calculate(daily_balance, current_interval);

                    // Calculate previous period data
                    let prev_interval = app_state.values.get_previous_period_interval();
                    let (prev_dates, prev_values) = calculate(daily_balance, prev_interval);

                    // Align previous period values to current period indices
                    let previous_period = align_previous_period(dates.len(), prev_dates, prev_values);

                    let values = values
                        .into_iter()
                        .map(|(k, v)| (Label::for_commodity(cx, &k), v))
                        .collect();
                    this.refresh_data(&dates, values, Some(previous_period), cx);
                });
                cx.notify();
            },
        ));
        Self {
            summary: summary::init(cx),
            chart: cx.new(line::Chart::new),
            _subscriptions: subscriptions,
        }
    }
}

/// Align previous period data to current period by index (day 1 -> day 1, etc.)
fn align_previous_period(
    current_len: usize,
    prev_dates: Vec<chrono::NaiveDate>,
    prev_values: HashMap<String, Vec<Option<f64>>>,
) -> line::PreviousPeriodData {
    // Pad or truncate previous values to match current period length
    let aligned_values: HashMap<String, Vec<Option<f64>>> = prev_values
        .into_iter()
        .map(|(commodity, values)| {
            let mut aligned = values;
            // Pad with None if previous period is shorter
            while aligned.len() < current_len {
                aligned.push(None);
            }
            // Truncate if previous period is longer
            aligned.truncate(current_len);
            (commodity, aligned)
        })
        .collect();

    // Pad dates similarly
    let mut aligned_dates = prev_dates;
    while aligned_dates.len() < current_len {
        // Use the last date for padding (won't be displayed anyway since value is None)
        if let Some(last) = aligned_dates.last().copied() {
            aligned_dates.push(last);
        }
    }
    aligned_dates.truncate(current_len);

    line::PreviousPeriodData {
        dates: aligned_dates,
        values: aligned_values,
    }
}

fn calculate(
    daily_balance: &DailyBalance,
    (min_date, max_date): (chrono::NaiveDate, chrono::NaiveDate),
) -> (Vec<chrono::NaiveDate>, HashMap<String, Vec<Option<f64>>>) {
    // Collect revenue accounts
    let revenue_accounts: Vec<_> = daily_balance
        .iter()
        .filter(|(account, _)| account.type_of == AccountType::Revenue)
        .collect();

    let mut plot_dates = Vec::new();
    let mut plot_balances = Vec::new();
    let mut cumulative_balance = Balance::default();

    // Iterate through each day in the filtered range
    let mut current_date = min_date;
    while current_date <= max_date {
        // Sum daily balances across all revenue accounts for this date
        for (account, _) in &revenue_accounts {
            let daily = daily_balance.get_daily_balance(account, current_date);
            cumulative_balance.add(&daily);
        }

        plot_dates.push(current_date);
        plot_balances.push(cumulative_balance.clone());

        current_date += chrono::Duration::days(1);
    }

    // Negate values since revenue is negative in ledger
    let values = convert_balances_to_values(&plot_balances);
    (plot_dates, values)
}

fn convert_balances_to_values(balances: &[Balance]) -> HashMap<String, Vec<Option<f64>>> {
    let mut all_commodities = std::collections::HashSet::new();
    for balance in balances {
        for amount in balance.iter() {
            all_commodities.insert(amount.commodity.clone());
        }
    }

    let mut values: HashMap<String, Vec<Option<f64>>> = HashMap::new();
    for commodity in all_commodities {
        let commodity_values: Vec<Option<f64>> = balances
            .iter()
            .map(|balance| {
                let amount = balance.get_amount(&commodity);
                let value = amount.map(|a| a.value.clone()).unwrap_or(D128::ZERO);
                Some(-value.to_f64())
            })
            .collect();
        values.insert(commodity, commodity_values);
    }

    values
}

impl Render for Chart {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(self.summary.clone())
            .child(self.chart.clone())
    }
}
