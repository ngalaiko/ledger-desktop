use std::hash::Hash;

use gpui::{App, Hsla};
use gpui_component::ActiveTheme;

pub fn get_color(cx: &App, commodity: &str) -> Hsla {
    let colors = vec![
        cx.theme().colors.red,
        cx.theme().colors.green,
        cx.theme().colors.blue,
        cx.theme().colors.yellow,
        cx.theme().colors.magenta,
        cx.theme().colors.cyan,
    ];
    let hash = {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        commodity.hash(&mut hasher);
        hasher.finish()
    };
    #[allow(clippy::cast_possible_truncation)]
    let color_idx = (hash as usize) % colors.len();
    colors[color_idx]
}
