mod accounts_tree;
mod charts;
mod components;
mod transactions_register;

#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::{h_flex, v_flex, ActiveTheme, StyledExt, TitleBar};
use state::period::Period;

use self::components::period_toggle;
use self::components::{commodity_selector, period_selector};

pub fn init(window: &mut gpui::Window, cx: &mut App) -> Entity<Window> {
    cx.new(|cx| Window::new(window, cx))
}

pub struct Window {
    revenue: Entity<charts::revenue::Revenue>,
    expenses: Entity<charts::expenses::Expenses>,
    total_assets: Entity<charts::total_assets::TotalAssets>,
    period_selector: Entity<period_selector::PeriodSelector>,
    period_toggle: Entity<period_toggle::PeriodToggle>,
    commodity_selector: Entity<commodity_selector::CommoditySelector>,
}

impl Window {
    fn new(_window: &mut gpui::Window, cx: &mut Context<Self>) -> Self {
        let total_assets = charts::total_assets::init(cx);
        let revenue = charts::revenue::init(cx);
        let expenses = charts::expenses::init(cx);

        Self {
            revenue,
            expenses,
            total_assets,
            period_selector: period_selector::init(cx),
            period_toggle: period_toggle::init(cx),
            commodity_selector: commodity_selector::init(cx),
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
                    v_flex()
                        .size_full()
                        .child(
                            h_flex()
                                .w_full()
                                .justify_between()
                                .child(div())
                                .child(Entity::clone(&self.period_toggle))
                                .child(Entity::clone(&self.commodity_selector)),
                        )
                        .child(
                            h_flex()
                                .w_full()
                                .justify_between()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(div().font_semibold().text_lg().child(
                                            match period {
                                                Period::Week | Period::Month => {
                                                    to.format("%B %Y").to_string()
                                                }
                                                Period::Year => to.format("%Y").to_string(),
                                            },
                                        ))
                                        .child(
                                            div()
                                                .text_lg()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!(
                                                    "{} - {}",
                                                    from.format("%d %b %Y"),
                                                    to.format("%d %b %Y")
                                                )),
                                        ),
                                )
                                .child(Entity::clone(&self.period_selector)),
                        )
                        .child(
                            h_flex()
                                .size_full()
                                .child(Entity::clone(&self.revenue))
                                .child(Entity::clone(&self.expenses)),
                        )
                        .child(Entity::clone(&self.total_assets)),
                ),
            )
    }
}
