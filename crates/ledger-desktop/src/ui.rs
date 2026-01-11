mod accounts_tree;
mod charts;
mod components;
mod transactions_register;

#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::{
    h_flex,
    resizable::{h_resizable, resizable_panel},
    v_flex, StyledExt, TitleBar,
};
use state::{period::Period, AppState};

use self::components::{commodity_selector, period_selector};
use self::transactions_register::RegisterView;
use self::{accounts_tree::AccountsTreeView, components::period_toggle};

pub fn init(window: &mut gpui::Window, cx: &mut App) -> Entity<Window> {
    cx.new(|cx| Window::new(window, cx))
}

pub struct Window {
    total_assets: Entity<charts::total_assets::TotalAssets>,
    register_view: Entity<RegisterView>,
    accounts_tree: Entity<AccountsTreeView>,
    period_selector: Entity<period_selector::PeriodSelector>,
    period_toggle: Entity<period_toggle::PeriodToggle>,
    commodity_selector: Entity<commodity_selector::CommoditySelector>,
    _subscriptions: Vec<Subscription>,
}

impl Window {
    fn new(window: &mut gpui::Window, cx: &mut Context<Self>) -> Self {
        let accounts_tree = accounts_tree::init(cx);
        let register_view = transactions_register::init(window, cx);
        let total_assets = charts::total_assets::init(cx);

        let mut subscriptions = vec![];
        subscriptions.push(
            // observe state changes and update views accordingly
            cx.observe(&AppState::global(cx), move |this, _app_state, _cx| {
                this.register_view.update(_cx, |this, cx| {
                    this.refresh_data(cx);
                });
            }),
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
            total_assets,
            accounts_tree,
            register_view,
            period_selector: period_selector::init(cx),
            period_toggle: period_toggle::init(cx),
            commodity_selector: commodity_selector::init(cx),
            _subscriptions: subscriptions,
        }
    }
}

impl Render for Window {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (from, to) = state::AppState::get_period_interval(cx);
        let period = state::AppState::get_period(cx);
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
                                            .w_full()
                                            .justify_between()
                                            .child(div())
                                            .child(self.period_toggle.clone())
                                            .child(self.commodity_selector.clone()),
                                    )
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .justify_between()
                                            .child(div().flex().font_semibold().text_lg().child(
                                                match period {
                                                    Period::Week | Period::Month => {
                                                        to.format("%B %Y").to_string()
                                                    }
                                                    Period::Year => to.format("%Y").to_string(),
                                                },
                                            ))
                                            .child(self.period_selector.clone()),
                                    )
                                    .child(self.total_assets.clone())
                                    .child(self.register_view.clone()),
                            ),
                        ),
                ),
            )
    }
}
