mod accounts_tree;
mod components;
mod total_assets_chart;
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
use self::components::commodity_selector::{commodity_selector, SelectCommodity};
use self::components::period_selector::{period_selector, SelectPeriod};
use self::total_assets_chart::TotalAssetsChart;
use self::transactions_register::RegisterView;

pub fn init(window: &mut gpui::Window, cx: &mut App) -> Entity<Window> {
    cx.new(|cx| Window::new(window, cx))
}

pub struct Window {
    total_assets_chart: Entity<TotalAssetsChart>,
    register_view: Entity<RegisterView>,
    accounts_tree: Entity<AccountsTreeView>,
    _subscriptions: Vec<Subscription>,
}

impl Window {
    fn new(window: &mut gpui::Window, cx: &mut Context<Self>) -> Self {
        let accounts_tree = accounts_tree::init(cx);
        let register_view = transactions_register::init(window, cx);
        let total_assets_chart = total_assets_chart::init(cx);

        let app_state = AppState::global(cx);

        let mut subscriptions = vec![];

        subscriptions.push(
            // observe state changes and update views accordingly
            cx.subscribe(
                &app_state,
                move |this, _app_state, event, _cx| match event {
                    state::StateEvent::CommodityChanged(_) => {
                        this.register_view.update(_cx, |this, cx| {
                            this.refresh_data(cx);
                        });
                        this.total_assets_chart.update(_cx, |this, cx| {
                            this.refresh_data(cx);
                        });
                    }
                    state::StateEvent::SelectedAccountsChanged(_) => {
                        this.register_view.update(_cx, |this, cx| {
                            this.refresh_data(cx);
                        });
                    }
                    state::StateEvent::PeriodChanged(_) => {
                        this.total_assets_chart.update(_cx, |this, cx| {
                            this.refresh_data(cx);
                        });
                    }
                    state::StateEvent::ExpandedAccountsChanged(_) => {}
                },
            ),
        );

        subscriptions.push(
            // observe ledger file changes and update views accordingly
            cx.observe(&ledger::File::global(cx), |this, _file, cx| {
                this.accounts_tree.update(cx, |this, cx| {
                    this.refresh_data(cx);
                });
                this.register_view.update(cx, |this, cx| {
                    this.refresh_data(cx);
                });
            }),
        );

        Self {
            total_assets_chart,
            accounts_tree,
            register_view,
            _subscriptions: subscriptions,
        }
    }
}

impl Render for Window {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let period = AppState::get_period(cx);
        let available_commodities = ledger::File::currency_converter(cx)
            .expect("todo")
            .available_commodities();
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
                                    .child(
                                        div()
                                            .size_full()
                                            .p_2()
                                            .child(self.total_assets_chart.clone()),
                                    )
                                    .child(self.register_view.clone()),
                            ),
                        ),
                ),
            )
    }
}
