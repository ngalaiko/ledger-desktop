use gpui::App;

pub mod currency_converter;
pub mod running_balance;
pub mod total_assets;
pub mod transactions;

pub fn init(cx: &mut App) {
    currency_converter::init(cx);
    transactions::init(cx);
    running_balance::init(cx);
    total_assets::init(cx);
}
