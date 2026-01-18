use std::collections::HashMap;

use gpui::{div, App, Entity, Window};
use gpui::{prelude::*, Subscription};

use ledger::Balance;

use crate::data::revenue::Revenue;
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
        subscriptions.push(
            observe_multiple(
                cx,
                (&Revenue::global(cx), &AppState::global(cx)),
                |this, cx| {
                    let revenue = Revenue::global(cx);
                    let app_state = AppState::global(cx);
                    this.chart.update(cx, |this, cx| {
                        let app_state = app_state.read(cx);
                        let (dates, values) = calculate(
                            revenue.read(cx),
                            app_state.values.get_period_interval(),
                        );
                        this.refresh_data(&dates, values, cx);
                    });
                    cx.notify();
                },
            ),
        );
        Self {
            chart: cx.new(LineChart::new),
            _subscriptions: subscriptions,
        }
    }
}

fn calculate(
    revenue: &Revenue,
    (min_date, max_date): (chrono::NaiveDate, chrono::NaiveDate),
) -> (Vec<chrono::NaiveDate>, HashMap<String, Vec<Option<f64>>>) {
    let mut plot_dates = Vec::new();
    let mut plot_balances = Vec::new();
    let mut cumulative_balance = Balance::default();

    // Iterate through each day in the filtered range
    let mut current_date = min_date;
    while current_date <= max_date {
        // Find the revenue for this exact date and add to cumulative
        if let Some((_, daily_balance)) = revenue.iter().find(|(d, _)| **d == current_date) {
            for amount in daily_balance.iter() {
                cumulative_balance.add_amount(amount.clone());
            }
        }

        plot_dates.push(current_date);
        plot_balances.push(cumulative_balance.clone());

        current_date += chrono::Duration::days(1);
    }

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
