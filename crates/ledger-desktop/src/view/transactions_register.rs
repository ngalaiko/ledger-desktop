use std::collections::HashSet;

#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::{
    h_flex,
    table::{Column, Table, TableDelegate, TableState},
};
use state::AppState;

use ledger::{Account, Transaction};

use crate::{
    data::{currency_converter::CurrencyConverter, transactions::convert_transaction},
    util::observe_multiple,
};

pub fn init(window: &mut Window, cx: &mut App) -> Entity<RegisterView> {
    cx.new(|cx| RegisterView::new(window, cx))
}

pub struct RegisterView {
    table_state: Entity<TableState<TransactionTableDelegate>>,
    _subscriptions: Vec<Subscription>,
}

impl RegisterView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let table_state =
            cx.new(|cx| TableState::new(TransactionTableDelegate::new(vec![]), window, cx));
        let mut subscriptions = vec![];

        subscriptions.push(observe_multiple(
            cx,
            (
                &ledger::File::global(cx),
                &AppState::global(cx),
                &CurrencyConverter::global(cx),
            ),
            |this, cx| {
                match ledger::File::transactions(cx) {
                    Ok(_) => {
                        let converter = CurrencyConverter::global(cx).read(cx);
                        let transactions = ledger::File::transactions(cx).expect("todo");
                        let visible_accounts = AppState::get_selected_accounts(cx);
                        let visible_transactions = transactions
                            .iter()
                            .filter_map(|transaction| {
                                filter_map_visible_transaction(transaction, &visible_accounts)
                            })
                            .map(|tx| {
                                convert_transaction(converter, &tx, AppState::get_commodity(cx))
                            })
                            .collect::<Vec<_>>();
                        this.table_state.update(cx, |table_state, cx| {
                            let delegate = table_state.delegate_mut();
                            delegate.transactions = visible_transactions;
                            table_state.refresh(cx);
                        });
                    }
                    Err(_) => {
                        // Clear table on error
                        this.table_state.update(cx, |table_state, cx| {
                            let delegate = table_state.delegate_mut();
                            delegate.transactions.clear();
                            table_state.refresh(cx);
                        });
                    }
                }
            },
        ));

        Self {
            table_state,
            _subscriptions: subscriptions,
        }
    }
}

fn filter_map_visible_transaction(
    transaction: &Transaction,
    visible_accounts: &HashSet<Account>,
) -> Option<Transaction> {
    let matching_postings = transaction
        .postings
        .iter()
        .filter(|posting| {
            visible_accounts
                .iter()
                .any(|filter| filter.is_parent_of(&posting.account))
        })
        .collect::<Vec<_>>();

    if matching_postings.is_empty() {
        // No matching postings, skip this transaction
        None
    } else {
        Some(Transaction {
            postings: matching_postings.into_iter().cloned().collect(),
            ..transaction.clone()
        })
    }
}

impl Render for RegisterView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Table::new(&self.table_state)
    }
}

struct TransactionTableDelegate {
    transactions: Vec<Transaction>,
    columns: Vec<Column>,
}

impl TransactionTableDelegate {
    fn new(transactions: Vec<Transaction>) -> Self {
        let columns = vec![
            Column::new("date", "Date").width(px(100.0)),
            Column::new("description", "Description").width(px(300.0)),
            Column::new("account", "Account").width(px(250.0)),
            Column::new("amount", "Amount")
                .width(px(120.0))
                .text_right(),
        ];
        Self {
            transactions,
            columns,
        }
    }

    // Helper to get the transaction and posting index for a given row
    fn get_row_data(&self, row_ix: usize) -> Option<(usize, usize, bool)> {
        let mut current_row = 0;
        for (tx_ix, transaction) in self.transactions.iter().enumerate() {
            for (posting_ix, _) in transaction.postings.iter().enumerate() {
                if current_row == row_ix {
                    return Some((tx_ix, posting_ix, posting_ix == 0));
                }
                current_row += 1;
            }
        }
        None
    }
}

impl TableDelegate for TransactionTableDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        4
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.transactions.iter().map(|t| t.postings.len()).sum()
    }

    fn column(&self, col_ix: usize, _cx: &App) -> &Column {
        &self.columns[col_ix]
    }

    fn render_tr(
        &mut self,
        row_ix: usize,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> Stateful<Div> {
        // Get the transaction index for this row to determine background color
        let bg_color = if let Some((tx_ix, _, _)) = self.get_row_data(row_ix) {
            if tx_ix % 2 == 0 {
                rgb(0x000d_0d0d) // Same as table background for even transactions
            } else {
                rgb(0x0015_1515) // Slightly lighter for odd transactions
            }
        } else {
            rgb(0x000d_0d0d)
        };

        h_flex().id(("row", row_ix)).bg(bg_color)
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        if let Some((tx_ix, posting_ix, is_first)) = self.get_row_data(row_ix) {
            let transaction = &self.transactions[tx_ix];
            let posting = &transaction.postings[posting_ix];

            match col_ix {
                0 => {
                    // Date
                    if is_first {
                        div().child(transaction.date.format("%Y-%m-%d").to_string())
                    } else {
                        div() // Empty for subsequent postings
                    }
                }
                1 => {
                    // Description
                    if is_first {
                        div().child(transaction.description.clone())
                    } else {
                        div() // Empty for subsequent postings
                    }
                }
                2 => {
                    // Account
                    div()
                        .text_color(rgb(0x00ff_ff80))
                        .child(posting.account.to_string())
                }
                3 => {
                    // Amount
                    div()
                        .text_color(rgb(0x0080_ff80))
                        .child(posting.amount.to_string())
                }
                _ => div(),
            }
        } else {
            div()
        }
    }
}
