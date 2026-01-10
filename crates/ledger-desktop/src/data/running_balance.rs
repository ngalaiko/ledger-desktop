use std::collections::{BTreeMap, HashMap};

use gpui::{App, AppContext, Context, Entity, Global, Subscription};
use ledger::{Account, Balance};

pub fn init(cx: &mut App) {
    RunningBalance::set_global(cx.new(|cx| RunningBalance::new(cx)), cx);
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

        let ledger_file = ledger::File::global(cx);
        subscriptions.push(
            // observe ledger file transactions and recalculate running balance
            cx.observe(&ledger_file, |this, _ledger_file, cx| {
                match ledger::File::transactions(cx) {
                    Ok(txs) => {
                        let data = Self::calculate(txs);
                        this.data = data;
                        cx.notify();
                    }
                    Err(_) => {
                        this.data.clear();
                        cx.notify();
                    }
                };
            }),
        );

        Self {
            data: HashMap::new(),
            _subscriptions: subscriptions,
        }
    }

    fn calculate(
        transactions: &[ledger::Transaction],
    ) -> HashMap<Account, BTreeMap<chrono::NaiveDate, Balance>> {
        let mut data: HashMap<Account, BTreeMap<chrono::NaiveDate, Balance>> = HashMap::new();
        for transaction in transactions.iter() {
            let date = transaction.date.clone();
            for posting in &transaction.postings {
                let account = posting.account.clone();
                let amount = posting.amount.value.clone();

                let account_balances = data.entry(account).or_insert_with(BTreeMap::new);

                if let Some(balance) = account_balances.get_mut(&date) {
                    balance.add_amount(amount);
                } else {
                    let previous_balance = account_balances
                        .range(..date)
                        .next_back()
                        .map(|(_, b)| b.clone())
                        .unwrap_or_else(Balance::new);
                    let mut new_balance = previous_balance;
                    new_balance.add_amount(amount);
                    account_balances.insert(date, new_balance);
                }
            }
        }

        data
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
