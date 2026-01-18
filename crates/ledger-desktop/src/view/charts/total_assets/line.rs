use std::collections::HashMap;

use gpui::{div, App, Entity, Window};
use gpui::{prelude::*, Subscription};

use ledger::{AccountType, Balance};

use crate::data::running_balance::RunningBalance;
use crate::util::observe_multiple;
use crate::view::components::charts::{line, Label};
use state::AppState;

pub fn init(cx: &mut App) -> Entity<Chart> {
    cx.new(Chart::new)
}

pub struct Chart {
    chart: Entity<line::Chart>,
    _subscriptions: Vec<Subscription>,
}

impl Chart {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![];
        subscriptions.push(observe_multiple(
            cx,
            (&RunningBalance::global(cx), &AppState::global(cx)),
            |this, cx| {
                let running_balance = RunningBalance::global(cx);
                let app_state = AppState::global(cx);
                this.chart.update(cx, |this, cx| {
                    let app_state = app_state.read(cx);
                    let (dates, values) = calculate(
                        running_balance.read(cx),
                        app_state.values.get_period_interval(),
                    );
                    let values = values
                        .into_iter()
                        .map(|(k, v)| (Label::for_commodity(cx, &k), v))
                        .collect();
                    this.refresh_data(&dates, values, None, cx);
                });
                cx.notify();
            },
        ));
        Self {
            chart: cx.new(line::Chart::new),
            _subscriptions: subscriptions,
        }
    }
}

fn calculate(
    running_balance: &RunningBalance,
    (min_date, max_date): (chrono::NaiveDate, chrono::NaiveDate),
) -> (Vec<chrono::NaiveDate>, HashMap<String, Vec<Option<f64>>>) {
    // Collect accounts that are Assets or Liabilities
    let accounts: Vec<_> = running_balance
        .iter()
        .filter_map(|(account, _)| {
            matches!(
                account.type_of,
                AccountType::Assets | AccountType::Liabilities
            )
            .then_some(account)
        })
        .collect();

    let mut plot_dates = Vec::new();
    let mut plot_balances = Vec::new();

    // Iterate through each day in the filtered range
    let mut current_date = min_date;
    while current_date <= max_date {
        // Sum balances across all asset and liability accounts for this date
        let mut total_balance = Balance::default();
        for account in &accounts {
            let balance = running_balance.get_balance(account, current_date);
            total_balance.add(&balance);
        }

        plot_dates.push(current_date);
        plot_balances.push(total_balance);

        current_date += chrono::Duration::days(1);
    }

    let values = convert_balances_to_values(&plot_balances);
    (plot_dates, values)
}

fn convert_balances_to_values(balances: &[Balance]) -> HashMap<String, Vec<Option<f64>>> {
    // Collect all unique commodities across all dates
    let mut all_commodities = std::collections::HashSet::new();
    for balance in balances {
        for amount in balance.iter() {
            all_commodities.insert(amount.commodity.clone());
        }
    }

    // For each commodity, create a Vec<Option<f64>> with values for each date
    let mut values: HashMap<String, Vec<Option<f64>>> = HashMap::new();
    for commodity in all_commodities {
        let commodity_values: Vec<Option<f64>> = balances
            .iter()
            .map(|balance| {
                balance
                    .get_amount(&commodity)
                    .map(|amount| amount.value.to_f64())
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
