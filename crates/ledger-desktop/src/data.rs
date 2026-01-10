use gpui::App;

pub mod running_balance;
pub mod total_assets;

pub fn init(cx: &mut App) {
    running_balance::init(cx);
    total_assets::init(cx);
}
