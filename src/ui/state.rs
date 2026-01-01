#[allow(clippy::wildcard_imports)]
use gpui::*;

use futures_lite::StreamExt;

use std::collections::{BTreeMap, HashMap};

use crate::ledger::accounts::{Account, Balance, TreeNode};
use crate::ledger::transactions::{CurrencyAmount, Transaction};
use crate::ledger::LedgerHandle;

pub struct State {
    pub accounts: TreeNode,
    pub transactions: Vec<Transaction>,
    pub running_balance: RunningBalance,
    pub error: Option<String>,

    ledger_handle: LedgerHandle,
}

impl State {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let ledger_handle = LedgerHandle::spawn(cx, None);
        let mut ledger_state = Self {
            accounts: TreeNode::new(),
            running_balance: RunningBalance::new(),
            transactions: Vec::new(),
            error: None,
            ledger_handle,
        };
        ledger_state.reload_state(cx);
        ledger_state
    }

    fn reload_state(&mut self, cx: &mut Context<Self>) {
        let ledger = self.ledger_handle.clone();

        self.accounts.clear();
        self.transactions.clear();
        self.error = None;

        cx.notify();

        cx.spawn(async move |this, cx| {
            let Ok(mut stream) = ledger.transactions().await else {
                this.update(cx, |this, cx| {
                    this.error = Some("Failed to start ledger process".into());
                    cx.notify();
                })
                .map_err(|e| {
                    eprintln!("Error updating state with error: {}", e);
                })
                .ok();
                return;
            };

            loop {
                match stream.next().await {
                    Some(Ok(transaction)) => {
                        this.update(cx, |this, _cx| {
                            for posting in transaction.postings.iter() {
                                this.accounts.add_account(&posting.account);
                                this.running_balance.record(
                                    transaction.time,
                                    &posting.account,
                                    &posting.amount.value,
                                );
                            }

                            this.transactions.push(transaction.clone());
                        })
                        .map_err(|e| {
                            eprintln!("Error updating state: {}", e);
                        })
                        .ok();
                    }
                    None => {
                        this.update(cx, |_this, cx| {
                            cx.notify();
                        })
                        .map_err(|e| {
                            eprintln!("Error finalizing state: {}", e);
                        })
                        .ok();
                        break;
                    }
                    Some(Err(e)) => {
                        eprintln!("Error parsing transaction: {}", e);
                        this.update(cx, |this, cx| {
                            this.error = Some(format!("Error parsing transaction: {}", e));
                            cx.notify();
                        })
                        .map_err(|e| {
                            eprintln!("Error updating state with error: {}", e);
                        })
                        .ok();
                        break;
                    }
                }
            }
        })
        .detach();
    }
}

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

    pub fn record(&mut self, date: chrono::NaiveDate, account: &Account, amount: &CurrencyAmount) {
        // Record for the account itself
        self.record_single(date, account, amount);

        // Also record for all parent accounts
        for ancestor in account.ancestors() {
            self.record_single(date, &ancestor, amount);
        }
    }

    fn record_single(
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
