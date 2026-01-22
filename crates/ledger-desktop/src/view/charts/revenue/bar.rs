use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use fastnum::D128;
use gpui::{prelude::*, Subscription};
use gpui::{App, Entity, Window};
use gpui_component::v_flex;

use ledger::{AccountType, Balance};

use crate::data::balance::DailyBalance;
use crate::util::observe_multiple;
use crate::view::components::charts::{bar, Label};
use state::{period::Period, AppState};

use super::summary;

pub fn init(cx: &mut App) -> Entity<Chart> {
    cx.new(Chart::new)
}

pub struct Chart {
    summary: Entity<summary::Summary>,
    inner: Entity<bar::Chart>,
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
                this.inner.update(cx, |this, cx| {
                    let app_state = app_state.read(cx);
                    let daily_balance = daily_balance.read(cx);

                    // Calculate current period data
                    let current_interval = app_state.values.get_period_interval();
                    let period = app_state.values.period.clone();
                    let (bars, values) =
                        calculate_aggregated(daily_balance, current_interval, &period);

                    let values = values
                        .into_iter()
                        .map(|(k, v)| (Label::for_commodity(cx, &k), v))
                        .collect();
                    // higher_is_better = true for revenue
                    this.refresh_data(&bars, values, None, true, cx);
                });
                cx.notify();
            },
        ));
        Self {
            summary: summary::init(cx),
            inner: cx.new(bar::Chart::new),
            _subscriptions: subscriptions,
        }
    }
}

/// Calculate aggregated bar data based on period type
fn calculate_aggregated(
    daily_balance: &DailyBalance,
    range: std::ops::Range<NaiveDate>,
    period: &Period,
) -> (Vec<bar::BarData>, HashMap<String, Vec<Option<f64>>>) {
    match period {
        Period::Year => aggregate_by_month(daily_balance, range),
        Period::Month => aggregate_by_week(daily_balance, range),
        Period::Week => aggregate_by_day(daily_balance, range),
    }
}

/// Aggregate daily revenue data by month (for Year period)
fn aggregate_by_month(
    daily_balance: &DailyBalance,
    range: std::ops::Range<NaiveDate>,
) -> (Vec<bar::BarData>, HashMap<String, Vec<Option<f64>>>) {
    let revenue_accounts: Vec<_> = daily_balance
        .iter()
        .filter(|(account, _)| account.type_of == AccountType::Revenue)
        .collect();

    let mut months: Vec<(u32, i32, NaiveDate, NaiveDate)> = vec![];
    let mut current_date = range.start;

    while range.contains(&current_date) {
        let month = current_date.month();
        let year = current_date.year();

        if months.is_empty() || months.last().map(|m| (m.0, m.1)) != Some((month, year)) {
            months.push((month, year, current_date, current_date));
        } else if let Some(last) = months.last_mut() {
            last.3 = current_date;
        }

        current_date += chrono::Duration::days(1);
    }

    let mut bars = Vec::new();
    let mut all_balances: Vec<Balance> = Vec::new();

    for (month, _year, start_date, end_date) in &months {
        let label = match month {
            1 => "Jan",
            2 => "Feb",
            3 => "Mar",
            4 => "Apr",
            5 => "May",
            6 => "Jun",
            7 => "Jul",
            8 => "Aug",
            9 => "Sep",
            10 => "Oct",
            11 => "Nov",
            12 => "Dec",
            _ => "",
        }
        .to_string();

        bars.push(bar::BarData {
            label,
            start_date: *start_date,
            end_date: *end_date,
        });

        let mut month_balance = Balance::default();
        let mut date = *start_date;
        while date <= *end_date && range.contains(&date) {
            for (account, _) in &revenue_accounts {
                let daily = daily_balance.get_daily_balance(account, date);
                month_balance.add(&daily);
            }
            date += chrono::Duration::days(1);
        }
        all_balances.push(month_balance);
    }

    let values = convert_balances_to_values(&all_balances);
    (bars, values)
}

/// Aggregate daily revenue data by week (for Month period)
fn aggregate_by_week(
    daily_balance: &DailyBalance,
    range: std::ops::Range<NaiveDate>,
) -> (Vec<bar::BarData>, HashMap<String, Vec<Option<f64>>>) {
    let revenue_accounts: Vec<_> = daily_balance
        .iter()
        .filter(|(account, _)| account.type_of == AccountType::Revenue)
        .collect();

    let mut weeks: Vec<(NaiveDate, NaiveDate)> = vec![];
    let mut current_date = range.start;
    let mut week_start = current_date;

    while range.contains(&current_date) {
        let days_in_week = (current_date - week_start).num_days();

        if days_in_week >= 7 {
            weeks.push((week_start, current_date - chrono::Duration::days(1)));
            week_start = current_date;
        }

        current_date += chrono::Duration::days(1);
    }

    if week_start < range.end {
        let end_date = range.end - chrono::Duration::days(1);
        weeks.push((week_start, end_date));
    }

    let mut bars = Vec::new();
    let mut all_balances: Vec<Balance> = Vec::new();

    for (start_date, end_date) in &weeks {
        let label = if start_date.day() == end_date.day() {
            format!("{}", start_date.day())
        } else {
            format!("{}-{}", start_date.day(), end_date.day())
        };

        bars.push(bar::BarData {
            label,
            start_date: *start_date,
            end_date: *end_date,
        });

        let mut week_balance = Balance::default();
        let mut date = *start_date;
        while date <= *end_date && range.contains(&date) {
            for (account, _) in &revenue_accounts {
                let daily = daily_balance.get_daily_balance(account, date);
                week_balance.add(&daily);
            }
            date += chrono::Duration::days(1);
        }
        all_balances.push(week_balance);
    }

    let values = convert_balances_to_values(&all_balances);
    (bars, values)
}

/// Aggregate daily revenue data by day (for Week period)
fn aggregate_by_day(
    daily_balance: &DailyBalance,
    range: std::ops::Range<NaiveDate>,
) -> (Vec<bar::BarData>, HashMap<String, Vec<Option<f64>>>) {
    let revenue_accounts: Vec<_> = daily_balance
        .iter()
        .filter(|(account, _)| account.type_of == AccountType::Revenue)
        .collect();

    let mut bars = Vec::new();
    let mut all_balances: Vec<Balance> = Vec::new();

    let mut current_date = range.start;
    while range.contains(&current_date) {
        let label = current_date.format("%a").to_string();

        bars.push(bar::BarData {
            label,
            start_date: current_date,
            end_date: current_date,
        });

        let mut day_balance = Balance::default();
        for (account, _) in &revenue_accounts {
            let daily = daily_balance.get_daily_balance(account, current_date);
            day_balance.add(&daily);
        }
        all_balances.push(day_balance);

        current_date += chrono::Duration::days(1);
    }

    let values = convert_balances_to_values(&all_balances);
    (bars, values)
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
                let value = amount.map_or(D128::ZERO, |a| a.value);
                // Negate revenue values (revenue is negative in ledger convention)
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
            .child(self.inner.clone())
    }
}
