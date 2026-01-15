use std::collections::{BTreeMap, HashMap};

use gpui::{App, AppContext, Context, Entity, Global, Subscription};
use ledger::{Account, Balance};

use super::transactions::Transactions;

pub fn init(cx: &mut App) {
    RunningBalance::set_global(cx.new(RunningBalance::new), cx);
}

struct GlobalRunningBalance(Entity<RunningBalance>);

impl Global for GlobalRunningBalance {}

pub struct RunningBalance {
    data: HashMap<Account, BTreeMap<chrono::NaiveDate, Balance>>,
    _subscriptions: Vec<Subscription>,
}

impl RunningBalance {
    pub fn global(cx: &App) -> Entity<RunningBalance> {
        cx.global::<GlobalRunningBalance>().0.clone()
    }

    pub(crate) fn set_global(running_balance: Entity<RunningBalance>, cx: &mut App) {
        cx.set_global(GlobalRunningBalance(running_balance));
    }

    fn new(cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![];

        let transactions = Transactions::global(cx);
        subscriptions.push(cx.observe(&transactions, |this, transactions, cx| {
            this.data = calculate(transactions.read(cx).as_slice());
            cx.notify();
        }));

        Self {
            data: HashMap::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Account, &BTreeMap<chrono::NaiveDate, Balance>)> {
        self.data.iter()
    }

    pub fn get_balance(&self, account: &Account, date: chrono::NaiveDate) -> Balance {
        if let Some(account_balances) = self.data.get(account) {
            if let Some((_, balance)) = account_balances.range(..=date).next_back() {
                return balance.clone();
            }
        }
        Balance::new()
    }
}

fn calculate(
    transactions: &[ledger::Transaction],
) -> HashMap<Account, BTreeMap<chrono::NaiveDate, Balance>> {
    let mut result: HashMap<Account, BTreeMap<chrono::NaiveDate, Balance>> = HashMap::new();
    for transaction in transactions {
        let tx_date = transaction.date;
        for posting in &transaction.postings {
            let account = posting.account.clone();
            let amount = posting.amount.value.clone();

            let account_balances = result.entry(account).or_default();

            if let Some(balance) = account_balances.get_mut(&tx_date) {
                balance.add_amount(amount);
            } else {
                let previous_balance = account_balances
                    .range(..tx_date)
                    .next_back()
                    .map(|(_, b)| b.clone())
                    .unwrap_or_default();
                let mut new_balance = previous_balance;
                new_balance.add_amount(amount);
                account_balances.insert(tx_date, new_balance);
            }
        }
    }

    result
}
