use gpui::prelude::*;
use gpui::App;
use gpui::Entity;
use gpui::SharedString;
use gpui_component::button::Button;
use gpui_component::button::ButtonVariants;
use gpui_component::menu::DropdownMenu;
use gpui_component::menu::PopupMenuItem;

use crate::data::currency_converter::CurrencyConverter;

pub fn init(cx: &mut App) -> Entity<CommoditySelector> {
    cx.new(|cx| CommoditySelector::new(cx))
}

pub struct CommoditySelector {}

impl CommoditySelector {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl Render for CommoditySelector {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_commodity = state::AppState::get_commodity(cx);
        let converter = CurrencyConverter::global(cx);
        let available_commodities = converter.read(cx).available_commodities();

        let label = current_commodity.unwrap_or_else(|| "-".to_string());

        Button::new("commodity-selector")
            .label(label)
            .ghost()
            .dropdown_menu(move |menu, _window, _cx| {
                // First item: None (displayed as '-')
                let menu = menu
                    .item(PopupMenuItem::new("-").on_click(|_event, _window, cx| {
                        state::AppState::update_commodity(None, cx);
                    }))
                    .scrollable(true);

                // Add all available commodities
                available_commodities.iter().fold(menu, |menu, commodity| {
                    menu.item(PopupMenuItem::new(SharedString::from(commodity)).on_click({
                        let commodity = commodity.clone();
                        move |_event, _window, cx| {
                            state::AppState::update_commodity(Some(commodity.clone()), cx);
                        }
                    }))
                })
            })
    }
}
