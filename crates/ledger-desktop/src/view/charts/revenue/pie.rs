use std::collections::HashMap;

use gpui::{div, App, Entity, Window};
use gpui::{prelude::*, Subscription};

use ledger::AccountType;

use crate::data::transactions::Transactions;
use crate::util::observe_multiple;
use crate::view::components::pie_chart::PieChart;
use state::AppState;

pub fn init(cx: &mut App) -> Entity<Chart> {
    cx.new(Chart::new)
}

pub struct Chart {
    chart: Entity<PieChart>,
    _subscriptions: Vec<Subscription>,
}

impl Chart {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![];
        subscriptions.push(
            observe_multiple(
                cx,
                (&Transactions::global(cx), &AppState::global(cx)),
                |this, cx| {
                    let transactions = Transactions::global(cx);
                    let app_state = AppState::global(cx);
                    this.chart.update(cx, |this, cx| {
                        let app_state = app_state.read(cx);
                        let values = calculate(
                            transactions.read(cx),
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
            chart: cx.new(PieChart::new),
            _subscriptions: subscriptions,
        }
    }
}

fn calculate(
    transactions: &Transactions,
    (from_date, to_date): (chrono::NaiveDate, chrono::NaiveDate),
    target_commodity: Option<&str>,
) -> HashMap<String, f64> {
    let Some(target_commodity) = target_commodity else {
        return HashMap::new();
    };

    let mut values: HashMap<String, f64> = HashMap::new();

    // Sum revenue by account for transactions within the period
    for tx in transactions.as_slice() {
        if tx.date < from_date || tx.date > to_date {
            continue;
        }

        for posting in &tx.postings {
            if posting.account.type_of != AccountType::Revenue {
                continue;
            }

            // Check if the posting is in the target commodity
            if posting.amount.value.commodity == target_commodity {
                // Revenue postings are typically negative (credits), so we negate to show positive values
                let value = -posting.amount.value.value.to_f64();
                if value > 0.0 {
                    let account_name = posting.account.to_string();
                    *values.entry(account_name).or_default() += value;
                }
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
