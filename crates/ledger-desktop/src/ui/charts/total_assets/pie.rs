use std::collections::HashMap;

use gpui::{div, App, Entity, Window};
use gpui::{prelude::*, Subscription};

use ledger::AccountType;

use crate::data::currency_converter::CurrencyConverter;
use crate::data::running_balance::RunningBalance;
use state::AppState;

use crate::ui::components::pie_chart::PieChart;

pub fn init(cx: &mut App) -> Entity<Chart> {
    cx.new(|cx| Chart::new(cx))
}

pub struct Chart {
    chart: Entity<PieChart>,
    _subscriptions: Vec<Subscription>,
}

impl Chart {
    fn new(cx: &mut Context<Self>) -> Self {
        let running_balance = RunningBalance::global(cx);
        let mut subscriptions = vec![];
        subscriptions.push(cx.observe(&running_balance, |this, _running_balance, cx| {
            this.refresh_data(cx);
        }));
        Self {
            chart: cx.new(|cx| PieChart::new(cx)),
            _subscriptions: subscriptions,
        }
    }

    pub fn refresh_data(&mut self, cx: &mut Context<Self>) {
        let running_balance = RunningBalance::global(cx);
        let running_balance = running_balance.read(cx);

        let (_, max_date) = AppState::get_period_interval(cx);

        let converter = CurrencyConverter::global(cx).read(cx);
        let target_commodity = AppState::get_commodity(cx);

        let Some(target_commodity) = target_commodity else {
            // No commodity selected, clear the chart
            self.chart.update(cx, |chart, cx| {
                chart.refresh_data(HashMap::new(), cx);
            });
            return;
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

            let balance = converter.convert_balance(
                &running_balance.get_balance(account, max_date),
                &target_commodity,
                max_date,
            );

            // Try to get the target commodity amount directly
            let amount = if let Some(amount) = balance.get_amount(&target_commodity) {
                Some(amount.clone())
            } else {
                None
            };

            if let Some(amount) = amount {
                let value = amount.value.to_f64();
                if value > 0.0 {
                    // Use the full account path as the label
                    let account_name = account.to_string();
                    values.insert(account_name, value);
                }
            }
        }

        self.chart.update(cx, |chart, cx| {
            chart.refresh_data(values, cx);
        });
    }
}

impl Render for Chart {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.chart.clone())
    }
}
