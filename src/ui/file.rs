mod accounts_tree;
mod state;
mod transactions_register;

use gpui::*;
use gpui_component::resizable::{h_resizable, resizable_panel};

use self::{accounts_tree::AccountsTree, state::LedgerState, transactions_register::RegisterView};

pub struct LedgerFile {
    register_view: Entity<RegisterView>,
    accounts_tree: Entity<AccountsTree>,

    _ledger_state: Entity<LedgerState>,
}

impl LedgerFile {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ledger_state = cx.new(|cx| LedgerState::new(window, cx));
        Self {
            accounts_tree: cx.new(|cx| AccountsTree::new(ledger_state.clone(), cx)),
            register_view: cx.new(|cx| RegisterView::new(ledger_state.clone(), window, cx)),
            _ledger_state: ledger_state,
        }
    }
}

impl Render for LedgerFile {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        h_resizable("ledger-register")
            .child(
                resizable_panel()
                    .size(px(100.))
                    .child(self.accounts_tree.clone()),
            )
            .child(
                resizable_panel()
                    .size(px(100.))
                    .child(self.register_view.clone()),
            )
    }
}
