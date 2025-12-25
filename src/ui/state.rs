#[allow(clippy::wildcard_imports)]
use gpui::*;

use futures_lite::StreamExt;

use crate::{accounts::TreeNode, ledger::LedgerHandle, transactions::Transaction};

pub struct State {
    pub accounts: TreeNode,
    pub transactions: Vec<Transaction>,
    pub error: Option<String>,

    ledger_handle: LedgerHandle,
}

impl State {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let ledger_handle = LedgerHandle::spawn(cx, None);
        let mut ledger_state = Self {
            accounts: TreeNode::new(),
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
                .ok();
                return;
            };

            loop {
                match stream.next().await {
                    Some(Ok(transaction)) => {
                        this.update(cx, |this, _cx| {
                            for posting in transaction.postings.iter() {
                                this.accounts.add_account(&posting.account);
                                this.accounts
                                    .add_amount_to_account(&posting.account, &posting.amount);
                            }

                            this.transactions.push(transaction.clone());
                        })
                        .ok();
                    }
                    None => {
                        this.update(cx, |_this, cx| {
                            cx.notify();
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
                        .ok();
                        break;
                    }
                }
            }
        })
        .detach();
    }
}
