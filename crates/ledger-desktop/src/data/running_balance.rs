use std::collections::{BTreeMap, BTreeSet, HashMap};

use gpui::{App, AppContext, Context, Entity, Global, Subscription};
use ledger::{Account, Balance};

use super::balance::DailyBalance;

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

        let daily_balance = DailyBalance::global(cx);
        subscriptions.push(cx.observe(&daily_balance, |this, daily_balance, cx| {
            this.data = calculate(daily_balance.read(cx));
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
    daily_balance: &DailyBalance,
) -> HashMap<Account, BTreeMap<chrono::NaiveDate, Balance>> {
    let mut result: HashMap<Account, BTreeMap<chrono::NaiveDate, Balance>> = HashMap::new();

    for (account, daily_balances) in daily_balance.iter() {
        // Collect all dates for this account
        let dates: BTreeSet<_> = daily_balances.keys().copied().collect();

        if dates.is_empty() {
            continue;
        }

        let range = dates.first().expect("at least one date").clone()
            ..=dates.last().expect("at least one date").clone();

        let mut running = Balance::default();
        let mut account_running: BTreeMap<chrono::NaiveDate, Balance> = BTreeMap::new();

        let mut current_date = range.start().clone();
        while range.contains(&current_date) {
            if let Some(daily) = daily_balances.get(&current_date) {
                running.add(daily);
            }
            account_running.insert(current_date, running.clone());
            current_date += chrono::Duration::days(1);
        }

        result.insert(account.clone(), account_running);
    }

    result
}
