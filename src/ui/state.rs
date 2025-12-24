use std::collections::HashSet;

#[allow(clippy::wildcard_imports)]
use gpui::*;

use futures_lite::StreamExt;

use crate::{accounts::Account, ledger::LedgerHandle, transactions::Transaction};

#[derive(Debug)]
pub enum StateUpdatedEvent {
    Reset,
    NewTransaction { transaction: Transaction },
    NewAccount { account: Account },
    Error { message: String },
}

pub struct LedgerState {
    accounts: HashSet<Account>,
    ledger_handle: LedgerHandle,
}

impl EventEmitter<StateUpdatedEvent> for LedgerState {}

impl LedgerState {
    pub fn new( cx: &mut Context<Self>) -> Self {
        let ledger_handle = LedgerHandle::spawn(cx, None);
        let mut ledger_state = Self {
            accounts: HashSet::new(),
            ledger_handle,
        };
        ledger_state.reload_state(cx);
        ledger_state
    }

    fn reload_state(&mut self, cx: &mut Context<Self>) {
        let ledger = self.ledger_handle.clone();

        let account_filter = "".trim(); // todo: get from input
        let command = if account_filter.is_empty() {
            "lisp --lisp-date-format seconds".to_string()
        } else {
            // Escape forward slashes and backslashes to prevent injection
            let escaped = account_filter.replace('\\', "\\\\").replace('/', "\\/");
            format!("lisp --lisp-date-format seconds account =~ /{}/", escaped)
        };

        self.accounts.clear();
        cx.emit(StateUpdatedEvent::Reset);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let Ok(stream) = ledger.stream(&command).await else {
                this.update(cx, |_this, cx| {
                    cx.emit(StateUpdatedEvent::Error {
                        message: "Failed to start ledger command".to_string(),
                    });
                    cx.notify();
                })
                .ok();
                return;
            };
            let mut stream = stream.sexpr().transactions();

            loop {
                match stream.next().await {
                    Some(Ok(transaction)) => {
                        this.update(cx, |this, cx| {
                            for posting in transaction.postings.iter() {
                                if this.accounts.insert(posting.account.clone()) {
                                    cx.emit(StateUpdatedEvent::NewAccount {
                                        account: posting.account.clone(),
                                    });
                                }
                            }

                            cx.emit(StateUpdatedEvent::NewTransaction { transaction });
                            cx.notify();
                        })
                        .ok();
                    }
                    None => {
                        // Command completed successfully
                        break;
                    }
                    Some(Err(e)) => {
                        this.update(cx, |_this, cx| {
                            cx.emit(StateUpdatedEvent::Error {
                                message: format!("Error parsing transaction: {}", e),
                            });
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
