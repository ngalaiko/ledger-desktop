use std::collections::BTreeMap;

use gpui::{App, AppContext, Context, Entity, Global};
use ledger::Balance;

use super::running_balance::RunningBalance;

pub fn init(cx: &mut App) {
    TotalAssets::set_global(cx.new(TotalAssets::new), cx);
}

struct GlobalTotalAssets(Entity<TotalAssets>);

impl Global for GlobalTotalAssets {}

#[derive(Debug)]
pub struct TotalAssets {
    data: BTreeMap<chrono::NaiveDate, Balance>,
    _subscriptions: Vec<gpui::Subscription>,
}

impl TotalAssets {
    pub fn global(cx: &App) -> Entity<TotalAssets> {
        cx.global::<GlobalTotalAssets>().0.clone()
    }

    pub(crate) fn set_global(total_assets: Entity<TotalAssets>, cx: &mut App) {
        cx.set_global(GlobalTotalAssets(total_assets));
    }

    fn new(cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![];
        let running_balance = RunningBalance::global(cx);

        subscriptions.push(
            // observe running balance changes and recalculate total assets
            cx.observe(&running_balance, |this, running_balance, cx| {
                let running_balance = running_balance.read(cx);
                let data = calculate(running_balance);
                this.data = data;
                cx.notify();
            }),
        );

        Self {
            data: BTreeMap::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&chrono::NaiveDate, &Balance)> {
        self.data.iter()
    }
}

fn calculate(running_balance: &RunningBalance) -> BTreeMap<chrono::NaiveDate, Balance> {
    let accounts = running_balance
        .iter()
        .filter_map(|(account, _)| match account.type_of {
            ledger::AccountType::Assets | ledger::AccountType::Liabilities => Some(account),
            ledger::AccountType::Unknown
            | ledger::AccountType::Expenses
            | ledger::AccountType::Revenue => None,
        })
        .collect::<Vec<_>>();
    let all_dates = running_balance
        .iter()
        .filter(|(account, _)| {
            matches!(
                account.type_of,
                ledger::AccountType::Assets | ledger::AccountType::Liabilities
            )
        })
        .flat_map(|(_, date_balances)| date_balances.keys())
        .copied()
        .collect::<std::collections::BTreeSet<_>>();

    if all_dates.is_empty() {
        return BTreeMap::new();
    }

    let min_date = *all_dates.first().expect("at least one date exists");
    let max_date = *all_dates.last().expect("at least one date exists");

    let mut total_assets: BTreeMap<chrono::NaiveDate, Balance> = BTreeMap::new();
    let mut current_date = min_date;
    while current_date <= max_date {
        let mut total_balance = Balance::new();
        for account in &accounts {
            let balance = running_balance.get_balance(account, current_date);
            total_balance.add(&balance);
        }
        total_assets.insert(current_date, total_balance);
        current_date += chrono::Duration::days(1);
    }
    total_assets
}
