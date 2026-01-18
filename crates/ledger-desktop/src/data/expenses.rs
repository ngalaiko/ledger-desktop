use std::collections::BTreeMap;

use gpui::{App, AppContext, Context, Entity, Global};
use ledger::Balance;

use super::transactions::Transactions;

pub fn init(cx: &mut App) {
    Expenses::set_global(cx.new(Expenses::new), cx);
}

struct GlobalExpenses(Entity<Expenses>);

impl Global for GlobalExpenses {}

#[derive(Debug)]
pub struct Expenses {
    data: BTreeMap<chrono::NaiveDate, Balance>,
    _subscriptions: Vec<gpui::Subscription>,
}

impl Expenses {
    pub fn global(cx: &App) -> Entity<Expenses> {
        cx.global::<GlobalExpenses>().0.clone()
    }

    pub(crate) fn set_global(expenses: Entity<Expenses>, cx: &mut App) {
        cx.set_global(GlobalExpenses(expenses));
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
    let mut expenses: BTreeMap<chrono::NaiveDate, Balance> = BTreeMap::new();

    for tx in transactions.as_slice() {
        for posting in &tx.postings {
            if posting.account.type_of == ledger::AccountType::Expenses {
                let balance = expenses.entry(tx.date).or_default();
                balance.add_amount(posting.amount.value.clone());
            }
        }
    }

    expenses
}
