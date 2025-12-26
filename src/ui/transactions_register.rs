#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::{
    h_flex,
    resizable::v_resizable,
    table::{Column, Table, TableDelegate, TableState},
    v_flex,
};

use crate::{accounts::Account, transactions::Transaction};

use super::{
    balance_chart::{BalanceChart, DataPoint},
    state::State,
};

pub struct RegisterView {
    state: Entity<State>,
    chart_state: Entity<BalanceChart>,
    table_state: Entity<TableState<TransactionTableDelegate>>,
    account_filter: Option<Account>,
}

impl RegisterView {
    pub fn new(
        state: Entity<State>,
        account_filter: Option<Account>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let table_state =
            cx.new(|cx| TableState::new(TransactionTableDelegate::new(vec![]), window, cx));
        let chart_state = cx.new(|_cx| BalanceChart::new());

        cx.observe(&state, |this, _state, cx| {
            this.rebuild_visible_transactions(cx);
        })
        .detach();

        Self {
            state,
            chart_state,
            table_state,
            account_filter,
        }
    }

    fn rebuild_visible_transactions(&mut self, cx: &mut Context<Self>) {
        let visible_transactions = self
            .state
            .read(cx)
            .transactions
            .iter()
            .filter_map(|transaction| {
                if let Some(account) = &self.account_filter {
                    let matching_postings = transaction
                        .postings
                        .iter()
                        .filter(|posting| {
                            posting.account.eq(&account) || account.is_parent_of(&posting.account)
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
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let (chart_data_points, commodities) = build_chart_data_points(&visible_transactions);
        self.chart_state.update(cx, |chart_state, _cx| {
            chart_state.set_data(chart_data_points, commodities);
        });
        self.table_state.update(cx, |table_state, cx| {
            let delegate = table_state.delegate_mut();
            delegate.transactions = visible_transactions;
            table_state.refresh(cx);
        });
    }

    pub fn set_account_filter(&mut self, filter: Option<Account>, cx: &mut Context<Self>) {
        self.account_filter = filter;
        self.rebuild_visible_transactions(cx);
    }
}

fn build_chart_data_points(transactions: &[Transaction]) -> (Vec<DataPoint>, Vec<String>) {
    use std::collections::{HashMap, HashSet};

    if transactions.is_empty() {
        return (vec![], vec![]);
    }

    // First pass: collect all unique commodities
    let mut all_commodities = HashSet::new();
    for transaction in transactions {
        for posting in &transaction.postings {
            all_commodities.insert(posting.amount.value.commodity.clone());
        }
    }

    // Sort commodities alphabetically for consistent ordering
    let mut commodities: Vec<String> = all_commodities.into_iter().collect();
    commodities.sort();

    let min_date = transactions
        .first()
        .map(|t| t.time)
        .expect("transactions are not empty");
    let max_date = transactions
        .last()
        .map(|t| t.time)
        .expect("transactions are not empty");

    let mut data_points = Vec::new();
    let mut balances = HashMap::<String, f64>::new();

    // Initialize all commodities with 0.0
    for commodity in &commodities {
        balances.insert(commodity.clone(), 0.0);
    }

    let mut transaction_idx = 0;

    // Iterate through each day
    let mut current_date = min_date;
    while current_date <= max_date {
        // Process all transactions on this date
        while transaction_idx < transactions.len()
            && transactions[transaction_idx].time == current_date
        {
            for posting in &transactions[transaction_idx].postings {
                let commodity = posting.amount.value.commodity.clone();
                let value: f64 = posting
                    .amount
                    .value
                    .value
                    .to_string()
                    .parse()
                    .unwrap_or(0.0);

                *balances.entry(commodity).or_insert(0.0) += value;
            }
            transaction_idx += 1;
        }

        // Create a data point with all commodities in consistent order
        let ordered_balances: Vec<(String, f64)> = commodities
            .iter()
            .map(|commodity| (commodity.clone(), balances[commodity]))
            .collect();

        data_points.push(DataPoint {
            date: current_date,
            balances: ordered_balances,
        });

        current_date += chrono::Duration::days(1);
    }

    (data_points, commodities)
}

impl Render for RegisterView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(self.chart_state.clone())
            .child(Table::new(&self.table_state))
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
                        div().child(transaction.time.format("%Y-%m-%d").to_string())
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
