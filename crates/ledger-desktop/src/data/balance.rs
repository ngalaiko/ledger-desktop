use std::collections::{BTreeMap, HashMap};

use gpui::{App, AppContext, Context, Entity, Global};
use ledger::{Account, Balance};

use super::transactions::Transactions;

pub fn init(cx: &mut App) {
    DailyBalance::set_global(cx.new(DailyBalance::new), cx);
}

struct GlobalDailyBalance(Entity<DailyBalance>);

impl Global for GlobalDailyBalance {}

/// Stores daily balance changes (not cumulative) per account
#[derive(Debug)]
pub struct DailyBalance {
    data: HashMap<Account, BTreeMap<chrono::NaiveDate, Balance>>,
    _subscriptions: Vec<gpui::Subscription>,
}

impl DailyBalance {
    pub fn global(cx: &App) -> Entity<DailyBalance> {
        cx.global::<GlobalDailyBalance>().0.clone()
    }

    pub(crate) fn set_global(balance: Entity<DailyBalance>, cx: &mut App) {
        cx.set_global(GlobalDailyBalance(balance));
    }

    fn new(cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![];
        let transactions = Transactions::global(cx);

        subscriptions.push(cx.observe(&transactions, |this, transactions, cx| {
            let transactions = transactions.read(cx);
            this.data = calculate(transactions);
            cx.notify();
        }));

        Self {
            data: HashMap::new(),
            _subscriptions: subscriptions,
        }
    }

    /// Iterate over all accounts and their daily balances
    pub fn iter(&self) -> impl Iterator<Item = (&Account, &BTreeMap<chrono::NaiveDate, Balance>)> {
        self.data.iter()
    }

    /// Get the daily balance change for a specific account on a specific date
    pub fn get_daily_balance(&self, account: &Account, date: chrono::NaiveDate) -> Balance {
        self.data
            .get(account)
            .and_then(|dates| dates.get(&date))
            .cloned()
            .unwrap_or_default()
    }
}

fn calculate(
    transactions: &Transactions,
) -> HashMap<Account, BTreeMap<chrono::NaiveDate, Balance>> {
    let mut result: HashMap<Account, BTreeMap<chrono::NaiveDate, Balance>> = HashMap::new();

    for transaction in transactions.as_slice() {
        let tx_date = transaction.date;
        for posting in &transaction.postings {
            let account = posting.account.clone();
            let amount = posting.amount.value.clone();

            let account_balances = result.entry(account).or_default();
            let balance = account_balances.entry(tx_date).or_default();
            balance.add_amount(amount);
        }
    }

    result
}
