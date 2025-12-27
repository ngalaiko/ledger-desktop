use std::collections::HashSet;

use gpui::*;
use gpui_component::resizable::{h_resizable, resizable_panel};

use super::{
    accounts_tree::{self, AccountsTreeView},
    state::State,
    transactions_register::RegisterView,
};

pub struct LedgerFile {
    register_view: Entity<RegisterView>,
    accounts_tree: Entity<AccountsTreeView>,

    _state: Entity<State>,
}

impl LedgerFile {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| State::new(cx));
        let accounts_tree = cx.new(|cx| AccountsTreeView::new(state.clone(), cx));
        let register_view = cx.new(|cx| RegisterView::new(state.clone(), window, cx));

        cx.observe(&accounts_tree, |this, accounts_tree, cx| {
            accounts_tree.update(cx, |accounts_tree, cx| {
                this.register_view.update(cx, |state, cx| {
                    state.set_account_filter(accounts_tree.selected_accounts().clone(), cx);
                });
            })
        })
        .detach();

        Self {
            accounts_tree,
            register_view,
            _state: state,
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
            .child(resizable_panel().child(self.register_view.clone()))
    }
}
