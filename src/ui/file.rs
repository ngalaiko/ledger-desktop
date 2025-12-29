use gpui::*;
use gpui_component::{
    resizable::{h_resizable, resizable_panel},
    v_flex,
};

use crate::transactions::Transaction;

use super::{
    accounts_tree::AccountsTreeView, balance_chart::BalanceChart, state::State,
    transactions_register::RegisterView,
};

pub struct LedgerFile {
    chart_state: Entity<BalanceChart>,
    register_view: Entity<RegisterView>,
    accounts_tree: Entity<AccountsTreeView>,

    state: Entity<State>,
}

impl LedgerFile {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| State::new(cx));
        let accounts_tree = cx.new(|cx| AccountsTreeView::new(state.clone(), cx));
        let register_view = cx.new(|cx| RegisterView::new(window, cx));
        let chart_state = cx.new(|cx| BalanceChart::new(cx));

        cx.observe(&accounts_tree, |this, accounts_tree, cx| {
            accounts_tree.update(cx, |accounts_tree, cx| {
                let visible_transactions = this
                    .state
                    .read(cx)
                    .transactions
                    .iter()
                    .filter_map(|transaction| {
                        let matching_postings = transaction
                            .postings
                            .iter()
                            .filter(|posting| {
                                accounts_tree
                                    .selected_accounts()
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
                    })
                    .collect::<Vec<_>>();

                this.register_view.update(cx, |state, cx| {
                    state.set_transactions(visible_transactions.clone(), cx);
                });
                this.chart_state.update(cx, |state, cx| {
                    state.set_transactions(visible_transactions, cx);
                });
            })
        })
        .detach();

        Self {
            chart_state,
            accounts_tree,
            register_view,
            state,
        }
    }
}

impl Render for LedgerFile {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        h_resizable("ledger-register")
            .child(
                resizable_panel()
                    .size(px(250.))
                    .child(self.accounts_tree.clone()),
            )
            .child(
                resizable_panel().child(
                    v_flex()
                        .size_full()
                        .child(div().size_full().p_2().child(self.chart_state.clone()))
                        .child(self.register_view.clone()),
                ),
            )
    }
}
