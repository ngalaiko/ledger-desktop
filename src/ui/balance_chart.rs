// todo:
// - add legend for commodities
// - configurable resolution (daily, weekly, monthly)

use core::fmt;
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    hash::Hash,
    rc::Rc,
};

use chrono::Datelike;
use fastnum::D128;
use gpui::prelude::FluentBuilder;
#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    menu::DropdownMenu,
    plot::{
        scale::{Scale, ScaleLinear, ScalePoint},
        shape::Line,
        tooltip::{CrossLine, Dot, Tooltip, TooltipPosition},
        AxisText, IntoPlot, Plot, PlotAxis, StrokeStyle,
    },
    StyledExt,
};
use gpui_component::{ActiveTheme, PixelsExt};

use crate::accounts::{Account, Balance};

use super::state::State;

// Constants for chart layout
/// Padding around the plot area in pixels
const PLOT_PADDING: f32 = 10.0;
/// Gap reserved for axis labels
const AXIS_GAP: f32 = 30.0;
/// Minimum number of data points before skipping ticks on X-axis
const MIN_TICK_SPACING: usize = 10;
/// Number of Y-axis value labels to display
const Y_AXIS_LABEL_COUNT: usize = 5;

pub struct BalanceChart {
    state: Entity<State>,
    plot_inner: PlotInner,
    mouse_position: Option<Point<Pixels>>,
    hovered_idx: Option<usize>,
    colors: Vec<Hsla>,
    visible_accounts: HashSet<Account>,
    period: Period,
}

impl BalanceChart {
    pub fn new(state: Entity<State>, cx: &mut Context<Self>) -> Self {
        let colors = vec![
            cx.theme().colors.red,
            cx.theme().colors.green,
            cx.theme().colors.blue,
            cx.theme().colors.yellow,
            cx.theme().colors.magenta,
            cx.theme().colors.cyan,
        ];

        Self {
            state,
            plot_inner: PlotInner::new(colors.clone()),
            mouse_position: None,
            hovered_idx: None,
            colors,
            visible_accounts: HashSet::new(),
            period: Period::D30,
        }
    }

    fn set_period(&mut self, period: Period, cx: &mut Context<Self>) {
        self.period = period;
        self.refresh_data(cx);
        cx.notify();
    }

    pub fn set_visible_accounts(&mut self, accounts: HashSet<Account>, cx: &mut Context<Self>) {
        self.visible_accounts = accounts;
        self.refresh_data(cx);
        cx.notify();
    }

    fn refresh_data(&mut self, cx: &mut Context<Self>) {
        let state = self.state.read(cx);

        // Collect all dates where any visible account has transactions
        let mut all_dates = std::collections::BTreeSet::new();
        for (account, date_balances) in state.running_balance.iter() {
            if self
                .visible_accounts
                .iter()
                .any(|visible_account| visible_account.is_parent_of(account))
            {
                for date in date_balances.keys() {
                    all_dates.insert(*date);
                }
            }
        }

        if all_dates.is_empty() {
            self.hovered_idx = None;
            self.plot_inner.set_data(vec![], vec![]);
            return;
        }

        let min_date = *all_dates.first().expect("at least one date exists");
        let max_date = *all_dates.last().expect("at least one date exists");

        // Calculate period start date based on max_date (latest transaction)
        let period_start = self.period.start_date(max_date).unwrap_or(min_date);

        // Apply period filter: use max of period_start and min_date
        let filtered_start = period_start.max(min_date);

        let mut plot_dates = Vec::new();
        let mut plot_balances = Vec::new();

        let mut current_date = filtered_start;
        while current_date <= max_date {
            // Aggregate balances from all visible accounts at this date
            let mut daily_balance = Balance::new();

            for visible_account in &self.visible_accounts {
                let balance = state
                    .running_balance
                    .get_balance(visible_account, current_date);
                daily_balance.add(&balance);
            }

            plot_dates.push(current_date);
            plot_balances.push(daily_balance);

            current_date += chrono::Duration::days(1);
        }

        self.plot_inner.set_data(plot_dates, plot_balances);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
enum Period {
    WTD,
    D7,
    MTD,
    D30,
    D90,
    YTD,
    Y1,
    Y3,
    All,
}

impl Period {
    pub fn start_date(&self, reference_date: chrono::NaiveDate) -> Option<chrono::NaiveDate> {
        match self {
            Period::WTD => {
                // Week-to-date: start of current week (Monday)
                let days_from_monday = reference_date.weekday().num_days_from_monday();
                Some(reference_date - chrono::Duration::days(days_from_monday as i64))
            }
            Period::D7 => {
                // Last 7 days
                Some(reference_date - chrono::Duration::days(7))
            }
            Period::MTD => {
                // Month-to-date: first day of current month
                reference_date.with_day(1)
            }
            Period::D30 => {
                // Last 30 days
                Some(reference_date - chrono::Duration::days(30))
            }
            Period::D90 => {
                // Last 90 days
                Some(reference_date - chrono::Duration::days(90))
            }
            Period::YTD => {
                // Year-to-date: January 1st of current year
                reference_date.with_month(1).and_then(|d| d.with_day(1))
            }
            Period::Y1 => {
                // Last 1 year
                Some(reference_date - chrono::Duration::days(365))
            }
            Period::Y3 => {
                // Last 3 years
                Some(reference_date - chrono::Duration::days(365 * 3))
            }
            Period::All => {
                // No filtering
                None
            }
        }
    }
}

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Period::WTD => "WTD",
            Period::D7 => "7D",
            Period::MTD => "MTD",
            Period::D30 => "30D",
            Period::D90 => "90D",
            Period::YTD => "YTD",
            Period::Y1 => "1Y",
            Period::Y3 => "3Y",
            Period::All => "All",
        };
        write!(f, "{s}")
    }
}

fn get_color(colors: &[Hsla], commodity: &String) -> Hsla {
    let hash = {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        commodity.hash(&mut hasher);
        hasher.finish()
    };
    let color_idx = (hash as usize) % colors.len();
    colors[color_idx]
}

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
#[action(namespace = balance_chart)]
struct SelectPeriod {
    period: Period,
}

impl Render for BalanceChart {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let plot_inner = self.plot_inner.clone();
        let tooltip_data = {
            match (self.mouse_position, plot_inner.bounds.get()) {
                (Some(mouse_position), Some(bounds)) if !plot_inner.dates.is_empty() => {
                    Some((mouse_position, bounds))
                }
                _ => None,
            }
        };
        div()
            .id("balance_chart")
            .size_full()
            .on_action(cx.listener(|this, period: &SelectPeriod, _window, cx| {
                this.set_period(period.period, cx);
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                // Update mouse position
                this.mouse_position = Some(event.position);

                // Only notify if the hovered data point changed
                if let (Some(mouse_pos), Some(bounds)) =
                    (this.mouse_position, this.plot_inner.bounds.get())
                {
                    if !this.plot_inner.dates.is_empty() {
                        let mouse_x = mouse_pos.x.as_f32() - bounds.origin.x.as_f32();
                        let (x_scale, _) = this.plot_inner.get_or_compute_scales(&bounds);
                        let new_hovered_idx = x_scale.least_index(mouse_x);

                        if this.hovered_idx != Some(new_hovered_idx) {
                            this.hovered_idx = Some(new_hovered_idx);
                            cx.notify();
                        }
                    }
                } else {
                    // No bounds yet, notify to trigger initial render
                    cx.notify();
                }
            }))
            .child(
                h_flex().justify_end().child(
                    Button::new("period-selector")
                        .label(self.period.to_string())
                        .ghost()
                        .dropdown_menu(|menu, _window, _cx| {
                            [
                                Period::WTD,
                                Period::D7,
                                Period::MTD,
                                Period::D30,
                                Period::D90,
                                Period::YTD,
                                Period::Y1,
                                Period::Y3,
                                Period::All,
                            ]
                            .iter()
                            .fold(menu, |menu, &period| {
                                menu.menu(period.to_string(), Box::new(SelectPeriod { period }))
                            })
                        }),
                ),
            )
            .child(plot_inner)
            .when_some(tooltip_data, |this, tooltip_data| {
                let mouse_x = tooltip_data.0.x.as_f32() - tooltip_data.1.origin.x.as_f32();
                let mouse_y = tooltip_data.0.y.as_f32() - tooltip_data.1.origin.y.as_f32();
                let width = calc_width(&tooltip_data.1);

                let (x_scale, y_scale) = self.plot_inner.get_or_compute_scales(&tooltip_data.1);

                let hovered_idx = x_scale.least_index(mouse_x);

                let date = self.plot_inner.dates[hovered_idx];
                let balances = &self.plot_inner.balances[hovered_idx];

                let x_pos = x_scale
                    .tick(&date)
                    .expect("hovered date should have x position");

                // Create CrossLine for the vertical crosshair
                let cross_line = CrossLine::new(point(px(x_pos), px(mouse_y)));

                // Create Dot components for each data point
                let dots: Vec<Dot> = balances
                    .iter()
                    .flat_map(|amount| {
                        let color = get_color(&self.colors, &amount.commodity);
                        y_scale.tick(&amount.value.to_f64()).map(|y_pos| {
                            // let color = self.colors[idx % self.colors.len()];
                            Dot::new(point(px(x_pos), px(y_pos)))
                                .size(px(10.0))
                                .fill(color)
                                .stroke(cx.theme().background)
                        })
                    })
                    .collect();

                // Determine tooltip position based on mouse location
                let position = if mouse_x < width / 2.0 {
                    TooltipPosition::Right
                } else {
                    TooltipPosition::Left
                };

                this.child(
                    Tooltip::new()
                        .position(position)
                        .gap(px(20.0))
                        .cross_line(cross_line)
                        .dots(dots)
                        .p_3()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .rounded_lg()
                        .shadow_lg()
                        .child(div().text_sm().font_semibold().child(date.to_string()))
                        .children(balances.iter().map(|amount| {
                            let color = get_color(&self.colors, &amount.commodity);

                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(div().text_xs().text_color(color).child("—"))
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .text_xs()
                                        .font_medium()
                                        .w_full()
                                        .justify_between()
                                        .child(amount.commodity.to_string())
                                        .child(amount.value.to_string()),
                                )
                        })),
                )
            })
    }
}

#[derive(IntoPlot, Clone)]
struct PlotInner {
    colors: Vec<Hsla>,

    dates: Vec<chrono::NaiveDate>,
    balances: Vec<Balance>,

    y_min: f64,
    y_max: f64,

    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    cached_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    cached_x_scale: Rc<RefCell<Option<ScalePoint<chrono::NaiveDate>>>>,
    cached_y_scale: Rc<RefCell<Option<ScaleLinear<f64>>>>,
}

impl PlotInner {
    pub fn new(colors: Vec<Hsla>) -> Self {
        Self {
            colors,
            dates: vec![],
            balances: vec![],
            y_min: 0.0,
            y_max: 0.0,
            bounds: Rc::new(Cell::new(None)),
            cached_bounds: Rc::new(Cell::new(None)),
            cached_x_scale: Rc::new(RefCell::new(None)),
            cached_y_scale: Rc::new(RefCell::new(None)),
        }
    }

    pub fn set_data(&mut self, dates: Vec<chrono::NaiveDate>, balances: Vec<Balance>) {
        self.dates = dates;
        self.balances = balances;

        let mut min_balance = D128::MAX;
        let mut max_balance = D128::MIN;

        for daily_balances in &self.balances {
            for amount in daily_balances.iter() {
                if amount.value < min_balance {
                    min_balance = amount.value;
                }
                if amount.value > max_balance {
                    max_balance = amount.value;
                }
            }
        }

        self.y_min = min_balance.to_f64();
        self.y_max = max_balance.to_f64();

        // Invalidate cached scales since data changed
        self.cached_bounds.set(None);
        self.cached_x_scale.replace(None);
        self.cached_y_scale.replace(None);
    }

    fn get_or_compute_scales(
        &self,
        bounds: &Bounds<Pixels>,
    ) -> (ScalePoint<chrono::NaiveDate>, ScaleLinear<f64>) {
        // Check if we need to recompute scales
        let needs_recompute = match self.cached_bounds.get() {
            Some(cached) => cached != *bounds,
            None => true,
        };

        if needs_recompute {
            let x_scale = calc_x_scale(bounds, &self.dates);
            let y_scale = calc_y_scale(bounds, &[self.y_min, self.y_max]);

            self.cached_bounds.set(Some(*bounds));
            self.cached_x_scale.replace(Some(x_scale.clone()));
            self.cached_y_scale.replace(Some(y_scale.clone()));

            (x_scale, y_scale)
        } else {
            (
                self.cached_x_scale
                    .borrow()
                    .as_ref()
                    .expect("X scale should be cached")
                    .clone(),
                self.cached_y_scale
                    .borrow()
                    .as_ref()
                    .expect("Y scale should be cached")
                    .clone(),
            )
        }
    }
}

fn calc_width(bounds: &Bounds<Pixels>) -> f32 {
    bounds.size.width.as_f32() - PLOT_PADDING
}

fn calc_height(bounds: &Bounds<Pixels>) -> f32 {
    bounds.size.height.as_f32() - PLOT_PADDING - AXIS_GAP
}

fn calc_x_scale(
    bounds: &Bounds<Pixels>,
    dates: &[chrono::NaiveDate],
) -> ScalePoint<chrono::NaiveDate> {
    let width = calc_width(bounds);
    ScalePoint::new(dates.to_vec(), vec![PLOT_PADDING, width])
}

fn calc_y_scale(bounds: &Bounds<Pixels>, domain: &[f64]) -> ScaleLinear<f64> {
    let height = calc_height(bounds);
    ScaleLinear::new(domain.to_vec(), vec![height, PLOT_PADDING])
}

impl Plot for PlotInner {
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        if self.dates.is_empty() {
            return;
        }

        self.bounds.set(Some(bounds));

        // Calculate drawing area with padding
        let height = calc_height(&bounds);

        // Get or compute cached scales
        let (x_scale, y_scale) = self.get_or_compute_scales(&bounds);

        // Create Y-axis labels
        let y_labels: Vec<AxisText> = (0..Y_AXIS_LABEL_COUNT)
            .filter_map(|i| {
                let value = self.y_min
                    + (self.y_max - self.y_min) * i as f64 / (Y_AXIS_LABEL_COUNT - 1) as f64;
                y_scale.tick(&value).map(|tick| {
                    AxisText::new(format!("{:.0}", value), tick, cx.theme().muted_foreground)
                })
            })
            .collect();

        // Create X-axis labels (show every Nth date to avoid crowding)
        let tick_margin = (self.dates.len() / MIN_TICK_SPACING).max(1);
        let x_labels: Vec<AxisText> = self
            .dates
            .iter()
            .enumerate()
            .filter_map(|(i, d)| {
                if i % tick_margin == 0 {
                    x_scale
                        .tick(&d)
                        .map(|tick| AxisText::new(d.to_string(), tick, cx.theme().muted_foreground))
                } else {
                    None
                }
            })
            .collect();

        // Draw axes
        PlotAxis::new()
            .x(height)
            .x_label(x_labels)
            .y_label(y_labels)
            .stroke(cx.theme().border)
            .paint(&bounds, window, cx);

        // Collect all unique commodities across all dates
        let mut all_commodities = HashSet::new();
        for balance in &self.balances {
            for amount in balance.iter() {
                all_commodities.insert(amount.commodity.clone());
            }
        }
        let mut all_commodities: Vec<String> = all_commodities.into_iter().collect();
        all_commodities.sort();

        // Draw a line for each commodity
        for commodity in all_commodities {
            let color = get_color(&self.colors, &commodity);
            let x_scale = x_scale.clone();
            let y_scale = y_scale.clone();

            Line::new()
                .data(self.dates.iter().zip(self.balances.iter()))
                .x(move |d: &(&chrono::NaiveDate, &Balance)| x_scale.tick(d.0))
                .y(move |d: &(&chrono::NaiveDate, &Balance)| {
                    d.1.get_amount(&commodity)
                        .and_then(|amt| y_scale.tick(&amt.value.to_f64()))
                })
                .stroke(color)
                .stroke_width(px(1.0))
                .stroke_style(StrokeStyle::Linear)
                .paint(&bounds, window);
        }
    }
}
