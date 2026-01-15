use std::collections::HashMap;

use gpui::{div, App, Entity, Window};
use gpui::{prelude::*, Subscription};

use ledger::Balance;

use crate::util::observe_multiple;
use state::AppState;

use crate::data::total_assets::TotalAssets;
use crate::view::components::line_chart::LineChart;

pub fn init(cx: &mut App) -> Entity<Chart> {
    cx.new(|cx| Chart::new(cx))
}

pub struct Chart {
    chart: Entity<LineChart>,
    _subscriptions: Vec<Subscription>,
}

impl Chart {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![];
        subscriptions.push(
            // observe total assets changes and refresh chart data
            observe_multiple(
                cx,
                (&TotalAssets::global(cx), &AppState::global(cx)),
                |this, cx| {
                    let total_assets = TotalAssets::global(cx);
                    let app_state = AppState::global(cx);
                    this.chart.update(cx, |this, cx| {
                        let app_state = app_state.read(cx);
                        let (dates, values) = calculate(
                            &total_assets.read(cx),
                            app_state.values.get_period_interval(),
                        );
                        this.refresh_data(&dates, values, cx);
                    });
                    cx.notify();
                },
            ),
        );
        Self {
            chart: cx.new(|cx| LineChart::new(cx)),
            _subscriptions: subscriptions,
        }
    }
}

fn calculate(
    total_assets: &TotalAssets,
    (min_date, max_date): (chrono::NaiveDate, chrono::NaiveDate),
) -> (Vec<chrono::NaiveDate>, HashMap<String, Vec<Option<f64>>>) {
    let mut plot_dates = Vec::new();
    let mut plot_balances = Vec::new();

    // Iterate through each day in the filtered range
    let mut current_date = min_date;
    while current_date <= max_date {
        // Find the balance for this date (or the most recent one before it)
        // Balance is already converted to target commodity upstream
        let balance = total_assets
            .iter()
            .filter(|(d, _)| **d <= current_date)
            .last()
            .map(|(_, b)| b.clone())
            .unwrap_or_else(Balance::new);

        plot_dates.push(current_date);
        plot_balances.push(balance);

        current_date += chrono::Duration::days(1);
    }

    // Convert Balance data to HashMap<String, Vec<Option<f64>>> format
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
