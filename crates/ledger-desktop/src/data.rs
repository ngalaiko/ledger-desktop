use gpui::App;

pub mod balance;
pub mod currency_converter;
mod ledger;
pub mod running_balance;
pub mod transactions;

pub fn init(cx: &mut App) {
    ledger::init::<&str>(None, cx);
    currency_converter::init(cx);
    transactions::init(cx);
    balance::init(cx);
    running_balance::init(cx);
}
