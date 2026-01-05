mod accounts_tree;
mod balance_chart;
mod components;
mod ledger_state;
mod transactions_register;

#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::{
    h_flex,
    resizable::{h_resizable, resizable_panel},
    v_flex, TitleBar,
};
use state::AppState;

use self::accounts_tree::AccountsTreeView;
use self::balance_chart::BalanceChart;
use self::components::commodity_selector::{commodity_selector, SelectCommodity};
use self::ledger_state::LedgerState;
use self::transactions_register::RegisterView;

pub struct Window {
    chart_state: Entity<BalanceChart>,
    register_view: Entity<RegisterView>,
    accounts_tree: Entity<AccountsTreeView>,
    state: Entity<LedgerState>,
}

impl Window {
    pub fn new(window: &mut gpui::Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| LedgerState::new(cx));
        let accounts_tree = cx.new(|cx| AccountsTreeView::new(state.clone(), cx));
        let register_view = cx.new(|cx| RegisterView::new(state.clone(), window, cx));
        let chart_state = cx.new(|cx| BalanceChart::new(state.clone(), cx));

        cx.observe(&accounts_tree, |this, accounts_tree, cx| {
            accounts_tree.update(cx, |_this, cx| {
                this.register_view.update(cx, |this, cx| {
                    this.refresh_data(cx);
                });

                this.chart_state.update(cx, |this, cx| {
                    this.refresh_data(cx);
                });
            })
        })
        .detach();

        cx.observe(&state, |this, _state, cx| {
            this.register_view.update(cx, |this, cx| {
                this.refresh_data(cx);
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

impl Render for Window {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let available_commodities = state.currency_converter.available_commodities();
        let selected_commodity = AppState::get_commodity(cx);

        v_flex()
            .size_full()
            .child(TitleBar::new().child(div().text_center().flex_1().child("ledger-desktop")))
            .child(
                div().size_full().child(
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
                                            .on_action(
                                                |commodity: &SelectCommodity, _window, cx| {
                                                    AppState::update_commodity(
                                                        commodity.commodity.clone(),
                                                        cx,
                                                    );
                                                },
                                            ),
                                    )
                                    .child(div().size_full().p_2().child(self.chart_state.clone()))
                                    .child(self.register_view.clone()),
                            ),
                        ),
                ),
            )
    }
}
