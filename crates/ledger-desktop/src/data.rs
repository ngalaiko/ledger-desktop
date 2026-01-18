use gpui::App;

pub mod accounts_tree;
pub mod balance;
pub mod currency_converter;
pub mod running_balance;
pub mod transactions;

pub fn init(cx: &mut App) {
    currency_converter::init(cx);
    transactions::init(cx);
    balance::init(cx);
    running_balance::init(cx);
    accounts_tree::init(cx);
}
