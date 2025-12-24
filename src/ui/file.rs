use gpui::*;
use gpui_component::resizable::{h_resizable, resizable_panel};

use super::{
    accounts_tree::AccountsTreeView, state::LedgerState, transactions_register::RegisterView,
};

pub struct LedgerFile {
    register_view: Entity<RegisterView>,
    accounts_tree: Entity<AccountsTreeView>,

    _ledger_state: Entity<LedgerState>,
}

impl LedgerFile {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ledger_state = cx.new(|cx| LedgerState::new(window, cx));
        let accounts_tree = cx.new(|cx| AccountsTreeView::new(ledger_state.clone(), cx));
        let register_view = cx.new(|cx| RegisterView::new(ledger_state.clone(), cx));

        cx.subscribe(
            &accounts_tree,
            |this, _accounts_tree, event, cx| match event {
                super::accounts_tree::AccountsTreeEvent::Selected { account } => {
                    this.register_view.update(cx, |state, cx| {
                        state.set_account_filter(Some(account.clone()), cx);
                    });
                }
            },
        )
        .detach();

        Self {
            accounts_tree,
            register_view,
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
