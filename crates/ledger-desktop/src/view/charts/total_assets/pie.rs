use std::collections::HashMap;

use gpui::{div, App, Entity, Window};
use gpui::{prelude::*, Subscription};

use ledger::AccountType;

use crate::data::running_balance::RunningBalance;
use crate::util::observe_multiple;
use state::AppState;

use crate::view::components::pie_chart::PieChart;

pub fn init(cx: &mut App) -> Entity<Chart> {
    cx.new(|cx| Chart::new(cx))
}

pub struct Chart {
    chart: Entity<PieChart>,
    _subscriptions: Vec<Subscription>,
}

impl Chart {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![];
        subscriptions.push(
            // observe running balance changes and refresh chart data
            observe_multiple(
                cx,
                (&RunningBalance::global(cx), &AppState::global(cx)),
                |this, cx| {
                    let running_balance = RunningBalance::global(cx);
                    let app_state = AppState::global(cx);
                    this.chart.update(cx, |this, cx| {
                        let app_state = app_state.read(cx);
                        let values = calculate(
                            &running_balance.read(cx),
                            app_state.values.get_period_interval(),
                            app_state.values.commodity.as_deref(),
                        );
                        this.refresh_data(values, cx);
                    });
                    cx.notify();
                },
            ),
        );
        Self {
            chart: cx.new(|cx| PieChart::new(cx)),
            _subscriptions: subscriptions,
        }
    }
}

fn calculate(
    running_balance: &RunningBalance,
    (_from, max_date): (chrono::NaiveDate, chrono::NaiveDate),
    target_commodity: Option<&str>,
) -> HashMap<String, f64> {
    let Some(target_commodity) = target_commodity else {
        return HashMap::new();
    };

    let mut values: HashMap<String, f64> = HashMap::new();

    // For each account, get balance at max_date and extract the target commodity value
    for (account, _) in running_balance.iter() {
        // Only include asset and liability accounts
        if !matches!(
            account.type_of,
            AccountType::Assets | AccountType::Liabilities
        ) {
            continue;
        }

        let balance = running_balance.get_balance(account, max_date);

        // Try to get the target commodity amount directly (already converted)
        if let Some(amount) = balance.get_amount(target_commodity) {
            let value = amount.value.to_f64();
            if value > 0.0 {
                // Use the full account path as the label
                let account_name = account.to_string();
                values.insert(account_name, value);
            }
        }
    }

    values
}

impl Render for Chart {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.chart.clone())
    }
}
