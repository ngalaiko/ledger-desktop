use fastnum::D128;
#[allow(clippy::wildcard_imports)]
use gpui::*;

use futures_lite::StreamExt;

use std::collections::{BTreeMap, HashMap};

use ledger::{Account, Balance, CurrencyAmount, LedgerHandle, Price, Transaction, TreeNode};

pub fn init(cx: &mut App) -> Entity<LedgerState> {
    cx.new(|cx| LedgerState::new(cx))
}

pub struct LedgerState {
    pub accounts: TreeNode,
    pub transactions: Vec<Transaction>,
    pub running_balance: RunningBalance,
    pub currency_converter: CurrencyConverter,
    pub error: Option<String>,

    ledger_handle: LedgerHandle,
}

impl LedgerState {
    fn new(cx: &mut Context<Self>) -> Self {
        let ledger_handle = LedgerHandle::spawn(cx, None);
        let mut ledger_state = Self {
            accounts: TreeNode::new(),
            running_balance: RunningBalance::new(),
            currency_converter: CurrencyConverter::new(),
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

        // Load transactions concurrently
        let ledger_for_transactions = ledger.clone();
        cx.spawn(async move |this, cx| {
            let Ok(mut stream) = ledger_for_transactions.transactions().await else {
                this.update(cx, |this, cx| {
                    this.error = Some("Failed to start ledger process for transactions".into());
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
                                this.running_balance.record_diff(
                                    transaction.date,
                                    &posting.account,
                                    &posting.amount.value,
                                );
                                if let Some(cost) = &posting.amount.cost {
                                    this.currency_converter.record(Price {
                                        date: transaction.date,
                                        commodity: posting.amount.value.commodity.clone(),
                                        value: cost.clone(),
                                    });
                                }
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

        // Load prices concurrently
        cx.spawn(async move |this, cx| {
            let Ok(mut stream) = ledger.prices().await else {
                this.update(cx, |this, cx| {
                    this.error = Some("Failed to start ledger process for prices".into());
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
                    Some(Ok(price)) => {
                        this.update(cx, |this, _cx| {
                            this.currency_converter.record(price);
                        })
                        .map_err(|e| {
                            eprintln!("Error updating prices: {}", e);
                        })
                        .ok();
                    }
                    None => {
                        this.update(cx, |_this, cx| {
                            cx.notify();
                        })
                        .map_err(|e| {
                            eprintln!("Error finalizing prices: {}", e);
                        })
                        .ok();
                        break;
                    }
                    Some(Err(e)) => {
                        eprintln!("Error parsing price: {}", e);
                        this.update(cx, |this, cx| {
                            this.error = Some(format!("Error parsing price: {}", e));
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

pub struct CurrencyConverter {
    // From commodity -> to commodity -> date -> price
    history: HashMap<String, HashMap<String, BTreeMap<chrono::NaiveDate, D128>>>,
}

impl CurrencyConverter {
    pub fn new() -> Self {
        Self {
            history: HashMap::new(),
        }
    }

    pub fn record(&mut self, price: Price) {
        {
            let to_map = self
                .history
                .entry(price.commodity.clone())
                .or_insert_with(HashMap::new);
            let date_map = to_map
                .entry(price.value.commodity.clone())
                .or_insert_with(BTreeMap::new);
            date_map.insert(price.date, price.value.value);
        }

        {
            let from_map = self
                .history
                .entry(price.value.commodity)
                .or_insert_with(HashMap::new);
            let date_map = from_map
                .entry(price.commodity)
                .or_insert_with(BTreeMap::new);
            date_map.insert(price.date, D128::ONE / price.value.value);
        }
    }

    pub fn convert(
        &self,
        amount: &CurrencyAmount,
        target_commodity: &str,
        at_date: chrono::NaiveDate,
    ) -> Option<CurrencyAmount> {
        if amount.commodity == target_commodity {
            return Some(amount.clone());
        }

        let to_map = self.history.get(&amount.commodity)?;
        let date_map = to_map.get(target_commodity)?;
        let (_, price) = date_map.range(..=at_date).next_back()?;

        Some(CurrencyAmount {
            value: amount.value * (*price),
            commodity: target_commodity.to_string(),
        })
    }

    pub fn available_commodities(&self) -> Vec<String> {
        let mut commodities: Vec<String> = self.history.keys().cloned().collect();
        commodities.sort();
        commodities
    }
}
