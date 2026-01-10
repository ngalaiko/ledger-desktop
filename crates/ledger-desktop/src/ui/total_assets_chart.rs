use std::collections::HashMap;

use gpui::{div, App, Entity, Window};
use gpui::{prelude::*, Subscription};

use ledger::{Balance, CurrencyConverter};
use state::AppState;

use crate::data::total_assets::{self, TotalAssets};
use crate::ui::components::line_chart::LineChart;

pub fn init(cx: &mut App) -> Entity<TotalAssetsChart> {
    cx.new(|cx| TotalAssetsChart::new(cx))
}

pub struct TotalAssetsChart {
    chart: Entity<LineChart>,
    _subscriptions: Vec<Subscription>,
}

impl TotalAssetsChart {
    fn new(cx: &mut Context<Self>) -> Self {
        let total_assets = TotalAssets::global(cx);
        let mut subscriptions = vec![];
        subscriptions.push(
            // observe total assets changes and refresh chart data
            cx.observe(&total_assets, |this, _total_assets, cx| {
                this.refresh_data(cx);
            }),
        );
        Self {
            chart: cx.new(|cx| LineChart::new(cx)),
            _subscriptions: subscriptions,
        }
    }

    pub fn refresh_data(&mut self, cx: &mut Context<Self>) {
        let period = AppState::get_period(cx);
        let total_assets = total_assets::TotalAssets::global(cx);
        let total_assets = total_assets.read(cx);

        // Collect all dates from total assets data
        let all_dates: Vec<chrono::NaiveDate> = total_assets.iter().map(|(d, _)| *d).collect();

        if all_dates.is_empty() {
            return;
        }

        let min_date = *all_dates.first().expect("at least one date exists");
        let max_date = *all_dates.last().expect("at least one date exists");

        // Calculate period start date based on max_date (latest transaction)
        let period_start = period.start_date(max_date).unwrap_or(min_date);

        // Apply period filter: use max of period_start and min_date
        let filtered_start = period_start.max(min_date);

        let converter = ledger::File::currency_converter(cx).expect("todo");
        let target_commodity = AppState::get_commodity(cx);

        let mut plot_dates = Vec::new();
        let mut plot_balances = Vec::new();

        // Iterate through each day in the filtered range
        let mut current_date = filtered_start;
        while current_date <= max_date {
            // Find the balance for this date (or the most recent one before it)
            let balance = total_assets
                .iter()
                .filter(|(d, _)| **d <= current_date)
                .last()
                .map(|(_, b)| b.clone())
                .unwrap_or_else(Balance::new);

            let converted_balance = convert_balance(
                converter,
                &balance,
                target_commodity.as_deref(),
                current_date,
            );

            plot_dates.push(current_date);
            plot_balances.push(converted_balance);

            current_date += chrono::Duration::days(1);
        }

        // Convert Balance data to HashMap<String, Vec<Option<f64>>> format
        let values = convert_balances_to_values(&plot_balances);

        self.chart.update(cx, |chart, cx| {
            chart.refresh_data(&plot_dates, values, cx);
        });
    }
}

fn convert_balance(
    converter: &CurrencyConverter,
    balance: &Balance,
    target_commodity: Option<&str>,
    at_date: chrono::NaiveDate,
) -> Balance {
    let Some(target_commodity) = target_commodity else {
        // No conversion, return balance as-is
        return balance.clone();
    };

    let mut converted_balance = Balance::new();
    for amount in balance.iter() {
        if let Some(converted_amount) = converter.convert(amount, target_commodity, at_date) {
            converted_balance.add_amount(converted_amount);
        } else {
            converted_balance.add_amount(amount.clone());
        }
    }
    converted_balance
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

impl Render for TotalAssetsChart {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.chart.clone())
    }
}
