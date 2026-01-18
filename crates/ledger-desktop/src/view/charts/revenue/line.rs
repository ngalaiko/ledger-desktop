use std::collections::HashMap;

use fastnum::D128;
use gpui::{div, App, Entity, Window};
use gpui::{prelude::*, Subscription};

use ledger::{AccountType, Balance};

use crate::data::balance::DailyBalance;
use crate::util::observe_multiple;
use crate::view::components::line_chart::LineChart;
use state::AppState;

pub fn init(cx: &mut App) -> Entity<Chart> {
    cx.new(Chart::new)
}

pub struct Chart {
    chart: Entity<LineChart>,
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
                    let (dates, values) = calculate(
                        daily_balance.read(cx),
                        app_state.values.get_period_interval(),
                    );
                    this.refresh_data(&dates, values, cx);
                });
                cx.notify();
            },
        ));
        Self {
            chart: cx.new(LineChart::new),
            _subscriptions: subscriptions,
        }
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
        div().size_full().child(self.chart.clone())
    }
}
