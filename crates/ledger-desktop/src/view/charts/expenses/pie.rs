use std::collections::HashMap;
use std::ops::Range;

use gpui::{div, App, Entity, Window};
use gpui::{prelude::*, Subscription};

use ledger::{Account, AccountType};

use crate::data::balance::DailyBalance;
use crate::util::observe_multiple;
use crate::view::components::charts::{pie, Label};
use state::AppState;

pub fn init(cx: &mut App) -> Entity<Chart> {
    cx.new(Chart::new)
}

pub struct Chart {
    chart: Entity<pie::Chart>,
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
                    let values = calculate(
                        daily_balance.read(cx),
                        app_state.values.get_period_interval(),
                        app_state.values.commodity.as_deref(),
                    );
                    let values = values
                        .into_iter()
                        .map(|(k, v)| (Label::for_account(cx, &k), v))
                        .collect();
                    this.refresh_data(values, cx);
                });
                cx.notify();
            },
        ));
        Self {
            chart: cx.new(pie::Chart::new),
            _subscriptions: subscriptions,
        }
    }
}

fn calculate(
    daily_balance: &DailyBalance,
    date_range: Range<chrono::NaiveDate>,
    target_commodity: Option<&str>,
) -> HashMap<Account, f64> {
    let Some(target_commodity) = target_commodity else {
        return HashMap::new();
    };

    let mut values: HashMap<Account, f64> = HashMap::new();

    // Sum expenses by account for the period
    for (account, date_balances) in daily_balance.iter() {
        if account.type_of != AccountType::Expenses {
            continue;
        }

        let mut account_total = 0.0;
        for (date, balance) in date_balances {
            if !date_range.contains(date) {
                continue;
            }

            if let Some(amount) = balance.get_amount(target_commodity) {
                account_total += amount.value.to_f64();
            }
        }

        if account_total > 0.0 {
            values.insert(account.clone(), account_total);
        }
    }

    values
}

impl Render for Chart {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.chart.clone())
    }
}
