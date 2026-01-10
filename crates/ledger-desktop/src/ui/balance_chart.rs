use std::collections::HashMap;

use gpui::prelude::*;
use gpui::{div, App, Entity, Window};

use ledger::{Balance, CurrencyConverter};
use state::AppState;

use crate::ui::components::line_chart::LineChart;

pub fn init(cx: &mut App) -> Entity<BalanceChart> {
    cx.new(|cx| BalanceChart::new(cx))
}

pub struct BalanceChart {
    chart: Entity<LineChart>,
}

impl BalanceChart {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            chart: cx.new(|cx| LineChart::new(cx)),
        }
    }

    pub fn refresh_data(&mut self, cx: &mut Context<Self>) {
        let visible_accounts = AppState::get_selected_accounts(cx);
        let period = AppState::get_period(cx);
        let running_balance = ledger::File::running_balance(cx).expect("todo");

        // Collect all dates where any visible account has transactions
        let all_dates = running_balance
            .iter()
            .filter(|(account, _)| {
                // Check if account is in visible accounts or a child of any visible account
                visible_accounts
                    .iter()
                    .any(|visible_account| visible_account.is_parent_of(account))
            })
            .flat_map(|(_, date_balances)| date_balances.keys())
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();

        if all_dates.is_empty() {
            self.chart.update(cx, |chart, cx| {
                chart.refresh_data(&[], HashMap::new(), cx);
            });
            return;
        }

        let min_date = *all_dates.first().expect("at least one date exists");
        let max_date = *all_dates.last().expect("at least one date exists");

        // Calculate period start date based on max_date (latest transaction)
        let period_start = period.start_date(max_date).unwrap_or(min_date);

        // Apply period filter: use max of period_start and min_date
        let filtered_start = period_start.max(min_date);

        let mut plot_dates = Vec::new();
        let mut plot_balances = Vec::new();

        let converter = ledger::File::currency_converter(cx).expect("todo");
        let mut current_date = filtered_start;
        while current_date <= max_date {
            // Aggregate balances from all visible accounts at this date
            let mut daily_balance = Balance::new();

            for visible_account in &visible_accounts {
                let balance = running_balance.get_balance(visible_account, current_date);
                let converted_balance = convert_balance(
                    converter,
                    &balance,
                    AppState::get_commodity(cx).as_deref(),
                    current_date,
                );
                daily_balance.add(&converted_balance);
            }

            plot_dates.push(current_date);
            plot_balances.push(daily_balance);

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

impl Render for BalanceChart {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.chart.clone())
    }
}
