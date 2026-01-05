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
use self::components::period_selector::{period_selector, SelectPeriod};
use self::ledger_state::LedgerState;
use self::transactions_register::RegisterView;

pub fn init(window: &mut gpui::Window, cx: &mut App) -> Entity<Window> {
    cx.new(|cx| Window::new(window, cx))
}

pub struct Window {
    chart_state: Entity<BalanceChart>,
    register_view: Entity<RegisterView>,
    accounts_tree: Entity<AccountsTreeView>,
    state: Entity<LedgerState>,
    _subscriptions: Vec<Subscription>,
}

impl Window {
    fn new(window: &mut gpui::Window, cx: &mut Context<Self>) -> Self {
        let state = ledger_state::init(cx);
        let accounts_tree = accounts_tree::init(state.clone(), cx);
        let register_view = transactions_register::init(state.clone(), window, cx);
        let chart_state = balance_chart::init(state.clone(), cx);

        let app_state = AppState::global(cx);

        let mut subscriptions = vec![];

        subscriptions.push(
            // observe state changes and update views accordingly
            {
                let register_view = register_view.clone();
                let chart_state = chart_state.clone();
                cx.subscribe(
                    &app_state,
                    move |_window, _app_state, event, _cx| match event {
                        state::StateEvent::CommodityChanged(_) => {
                            register_view.update(_cx, |this, cx| {
                                this.refresh_data(cx);
                            });
                            chart_state.update(_cx, |this, cx| {
                                this.refresh_data(cx);
                            });
                        }
                        state::StateEvent::SelectedAccountsChanged(_) => {
                            register_view.update(_cx, |this, cx| {
                                this.refresh_data(cx);
                            });
                            chart_state.update(_cx, |this, cx| {
                                this.refresh_data(cx);
                            });
                        }
                        state::StateEvent::PeriodChanged(_) => {
                            chart_state.update(_cx, |this, cx| {
                                this.refresh_data(cx);
                            });
                        }
                    },
                )
            },
        );

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
            _subscriptions: subscriptions,
        }
    }
}

impl Render for Window {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let period = AppState::get_period(cx);
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
                                            .child(div().child(period_selector(period)).on_action(
                                                |action: &SelectPeriod, _window, cx| {
                                                    AppState::update_period(action.period, cx);
                                                },
                                            ))
                                            .child(
                                                div()
                                                    .child(commodity_selector(
                                                        selected_commodity,
                                                        available_commodities,
                                                    ))
                                                    .on_action(
                                                        |action: &SelectCommodity, _window, cx| {
                                                            AppState::update_commodity(
                                                                action.commodity.clone(),
                                                                cx,
                                                            );
                                                        },
                                                    ),
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
