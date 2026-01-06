use std::collections::{BTreeMap, HashMap};

use crate::{Account, Balance, CurrencyAmount};

#[derive(Debug, Clone)]
pub struct RunningBalance {
    // Track historical balances per account per date
    // Account -> Date -> Balance (multi-commodity)
    balances: HashMap<Account, BTreeMap<chrono::NaiveDate, Balance>>,
}

impl RunningBalance {
    pub fn new() -> Self {
        Self {
            balances: HashMap::new(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Account, &BTreeMap<chrono::NaiveDate, Balance>)> {
        self.balances.iter()
    }

    pub fn record_diff(
        &mut self,
        date: chrono::NaiveDate,
        account: &Account,
        amount: &CurrencyAmount,
    ) {
        let account_balances = self
            .balances
            .entry(account.clone())
            .or_insert_with(BTreeMap::new);

        if let Some(balance) = account_balances.get_mut(&date) {
            balance.add_amount(amount.clone());
        } else {
            let previous_balance = account_balances
                .range(..date)
                .next_back()
                .map(|(_, b)| b.clone())
                .unwrap_or_else(Balance::new);
            let mut new_balance = previous_balance;
            new_balance.add_amount(amount.clone());
            account_balances.insert(date, new_balance);
        }
    }

    pub fn get_balance(&self, account: &Account, date: chrono::NaiveDate) -> Balance {
        if let Some(account_balances) = self.balances.get(account) {
            if let Some((_, balance)) = account_balances.range(..=date).next_back() {
                return balance.clone();
            }
        }
        Balance::new()
    }
}
