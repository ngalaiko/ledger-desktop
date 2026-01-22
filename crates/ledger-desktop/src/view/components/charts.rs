use gpui::{App, Hsla};
use gpui_component::{ActiveTheme, Colorize, Theme};
use ledger::Account;

pub mod bar;
pub mod line;
pub mod pie;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Label {
    pub text: String,
    pub color: Hsla,
}

impl Label {
    pub fn for_account(cx: &App, account: &Account) -> Self {
        let text = account.to_string();
        let color = match account.type_of {
            ledger::AccountType::Assets => {
                pick_color(&text, &gradient_colors(cx.theme().colors.blue))
            }
            ledger::AccountType::Liabilities => {
                pick_color(&text, &gradient_colors(cx.theme().colors.red))
            }
            ledger::AccountType::Equity => {
                pick_color(&text, &gradient_colors(cx.theme().colors.cyan))
            }
            ledger::AccountType::Revenue => {
                pick_color(&text, &gradient_colors(cx.theme().colors.green))
            }
            ledger::AccountType::Expenses => {
                pick_color(&text, &gradient_colors(cx.theme().colors.yellow))
            }
            ledger::AccountType::Unknown => cx.theme().colors.danger_foreground,
        };
        Self { text, color }
    }

    pub fn for_commodity(cx: &App, commodity: &str) -> Self {
        let colors = base_colors(cx.theme())
            .iter()
            .flat_map(|color| gradient_colors(*color))
            .collect::<Vec<_>>();
        let color = pick_color(commodity, &colors);

        Self {
            text: commodity.to_string(),
            color,
        }
    }
}

fn base_colors(theme: &Theme) -> Vec<Hsla> {
    vec![
        theme.colors.red,
        theme.colors.green,
        theme.colors.blue,
        theme.colors.yellow,
        theme.colors.magenta,
        theme.colors.cyan,
    ]
}

fn gradient_colors(base_color: Hsla) -> Vec<Hsla> {
    vec![
        base_color.lighten(0.6),
        base_color.lighten(0.4),
        base_color.lighten(0.2),
        base_color,
        base_color.darken(0.2),
        base_color.darken(0.4),
        base_color.darken(0.6),
    ]
}

fn pick_color(text: &str, colors: &[Hsla]) -> Hsla {
    use std::hash::Hash;
    let hash = {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        hasher.finish()
    };
    #[allow(clippy::cast_possible_truncation)]
    let color_idx = (hash as usize) % colors.len();
    colors[color_idx]
}
