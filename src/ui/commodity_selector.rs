#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    menu::DropdownMenu,
};

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
#[action(namespace = commodity_selector)]
pub struct SelectCommodity {
    pub commodity: Option<String>,
}

/// Renders a commodity selector dropdown button
pub fn commodity_selector(
    current_commodity: Option<String>,
    available_commodities: Vec<String>,
) -> impl IntoElement {
    let label = current_commodity.unwrap_or_else(|| "-".to_string());

    Button::new("commodity-selector")
        .label(label)
        .ghost()
        .dropdown_menu(move |menu, _window, _cx| {
            // First item: None (displayed as '-')
            let menu = menu
                .menu("-", Box::new(SelectCommodity { commodity: None }))
                .scrollable(true);

            // Add all available commodities
            available_commodities.iter().fold(menu, |menu, commodity| {
                menu.menu(
                    commodity.clone(),
                    Box::new(SelectCommodity {
                        commodity: Some(commodity.clone()),
                    }),
                )
            })
        })
}
