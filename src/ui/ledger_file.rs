use gpui::*;
use gpui_component::{
    h_flex,
    resizable::{h_resizable, resizable_panel},
    v_flex,
};

use super::accounts_tree::AccountsTreeView;
use super::balance_chart::BalanceChart;
use super::components::commodity_selector::{commodity_selector, SelectCommodity};
use super::ledger_state::LedgerState;
use super::transactions_register::RegisterView;

pub struct LedgerFile {
    chart_state: Entity<BalanceChart>,
    register_view: Entity<RegisterView>,
    accounts_tree: Entity<AccountsTreeView>,
    state: Entity<LedgerState>,
}

impl LedgerFile {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| LedgerState::new(cx));
        let accounts_tree = cx.new(|cx| AccountsTreeView::new(state.clone(), cx));
        let register_view = cx.new(|cx| RegisterView::new(state.clone(), window, cx));
        let chart_state = cx.new(|cx| BalanceChart::new(state.clone(), cx));

        cx.observe(&accounts_tree, |this, accounts_tree, cx| {
            accounts_tree.update(cx, |accounts_tree, cx| {
                this.register_view.update(cx, |state, cx| {
                    state.refresh_data(accounts_tree.selected_accounts(), cx);
                });

                this.chart_state.update(cx, |state, cx| {
                    state.set_visible_accounts(accounts_tree.selected_accounts().clone(), cx);
                });
            })
        })
        .detach();

        cx.observe(&state, |this, _state, cx| {
            this.accounts_tree.update(cx, |accounts_tree, cx| {
                this.register_view.update(cx, |register_view, cx| {
                    register_view.refresh_data(accounts_tree.selected_accounts(), cx);
                });
            });
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let available_commodities = state.currency_converter.available_commodities();
        let selected_commodity = state.selected_commodity.clone();

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
                        .child(
                            h_flex()
                                .justify_end()
                                .p_2()
                                .child(commodity_selector(
                                    selected_commodity,
                                    available_commodities,
                                ))
                                .on_action(cx.listener(
                                    |this, commodity: &SelectCommodity, _window, cx| {
                                        this.state.update(cx, |state, cx| {
                                            state.set_selected_commodity(
                                                commodity.commodity.clone(),
                                                cx,
                                            );
                                        });
                                    },
                                )),
                        )
                        .child(div().size_full().p_2().child(self.chart_state.clone()))
                        .child(self.register_view.clone()),
                ),
            )
    }
}
