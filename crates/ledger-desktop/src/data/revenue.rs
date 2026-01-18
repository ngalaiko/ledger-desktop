use std::collections::BTreeMap;

use gpui::{App, AppContext, Context, Entity, Global};
use ledger::Balance;

use super::transactions::Transactions;

pub fn init(cx: &mut App) {
    Revenue::set_global(cx.new(Revenue::new), cx);
}

struct GlobalRevenue(Entity<Revenue>);

impl Global for GlobalRevenue {}

#[derive(Debug)]
pub struct Revenue {
    data: BTreeMap<chrono::NaiveDate, Balance>,
    _subscriptions: Vec<gpui::Subscription>,
}

impl Revenue {
    pub fn global(cx: &App) -> Entity<Revenue> {
        cx.global::<GlobalRevenue>().0.clone()
    }

    pub(crate) fn set_global(revenue: Entity<Revenue>, cx: &mut App) {
        cx.set_global(GlobalRevenue(revenue));
    }

    fn new(cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![];
        let transactions = Transactions::global(cx);

        subscriptions.push(cx.observe(&transactions, |this, transactions, cx| {
            let transactions = transactions.read(cx);
            let data = calculate(transactions);
            this.data = data;
            cx.notify();
        }));

        Self {
            data: BTreeMap::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&chrono::NaiveDate, &Balance)> {
        self.data.iter()
    }
}

fn calculate(transactions: &Transactions) -> BTreeMap<chrono::NaiveDate, Balance> {
    let mut revenue: BTreeMap<chrono::NaiveDate, Balance> = BTreeMap::new();

    for tx in transactions.as_slice() {
        for posting in &tx.postings {
            if posting.account.type_of == ledger::AccountType::Revenue {
                let balance = revenue.entry(tx.date).or_default();
                balance.add_amount(posting.amount.value.clone());
            }
        }
    }

    revenue
}
