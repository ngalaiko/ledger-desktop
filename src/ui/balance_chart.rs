// todo:
// - add tooltip on hover showing date and balances
// - add legend for commodities
// - configurable period (weekly, monthly)
// - configurable resolution (daily, weekly, monthly)

use core::fmt;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

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

use crate::transactions::Transaction;

// Constants for chart layout
/// Padding around the plot area in pixels
const PLOT_PADDING: f32 = 10.0;
/// Gap reserved for axis labels
const AXIS_GAP: f32 = 30.0;
/// Minimum number of data points before skipping ticks on X-axis
const MIN_TICK_SPACING: usize = 10;
/// Number of Y-axis value labels to display
const Y_AXIS_LABEL_COUNT: usize = 5;

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
    pub fn duration(&self) -> chrono::Duration {
        match self {
            Period::WTD => chrono::Duration::days(7),
            Period::D7 => chrono::Duration::days(7),
            Period::MTD => chrono::Duration::days(30),
            Period::D30 => chrono::Duration::days(30),
            Period::D90 => chrono::Duration::days(90),
            Period::YTD => chrono::Duration::days(365),
            Period::Y1 => chrono::Duration::days(365),
            Period::Y3 => chrono::Duration::days(365 * 3),
            Period::All => chrono::Duration::MAX,
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

pub struct BalanceChart {
    plot_inner: PlotInner,
    mouse_position: Option<Point<Pixels>>,
    hovered_idx: Option<usize>,
    colors: Vec<Hsla>,
}

impl BalanceChart {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let colors = vec![
            cx.theme().colors.red,
            cx.theme().colors.green,
            cx.theme().colors.blue,
            cx.theme().colors.yellow,
            cx.theme().colors.magenta,
            cx.theme().colors.cyan,
        ];

        Self {
            plot_inner: PlotInner::new(Period::D30, colors.clone()),
            mouse_position: None,
            hovered_idx: None,
            colors,
        }
    }

    pub fn set_transactions(&mut self, transactions: Vec<Transaction>, cx: &mut Context<Self>) {
        use std::collections::{HashMap, HashSet};

        if transactions.is_empty() {
            self.plot_inner.set_data(vec![], vec![], vec![]);
            self.mouse_position = None;
            self.hovered_idx = None;
            cx.notify();
            return;
        }

        // First pass: collect all unique commodities
        let mut all_commodities = HashSet::new();
        for transaction in transactions.iter() {
            for posting in &transaction.postings {
                all_commodities.insert(posting.amount.value.commodity.clone());
            }
        }

        // Sort commodities alphabetically for consistent ordering
        let mut commodities: Vec<String> = all_commodities.into_iter().collect();
        commodities.sort();

        let min_date = transactions
            .first()
            .map(|t| t.time)
            .expect("transactions are not empty");
        let max_date = transactions
            .last()
            .map(|t| t.time)
            .expect("transactions are not empty");

        let mut dates = Vec::new();
        let mut ordered_balances = Vec::new();
        let mut balances = HashMap::<String, fastnum::D128>::new();

        // Initialize all commodities with 0.0
        for commodity in &commodities {
            balances.insert(commodity.clone(), fastnum::D128::ZERO);
        }

        let mut transaction_idx = 0;

        // Iterate through each day
        let mut current_date = min_date;
        while current_date <= max_date {
            // Process all transactions on this date
            while transaction_idx < transactions.len()
                && transactions[transaction_idx].time == current_date
            {
                for posting in &transactions[transaction_idx].postings {
                    let commodity = posting.amount.value.commodity.clone();
                    let value = posting.amount.value.value;
                    *balances.entry(commodity).or_insert(fastnum::D128::ZERO) += value;
                }
                transaction_idx += 1;
            }

            // Create a data point with all commodities in consistent order
            let ordered: Vec<fastnum::D128> = commodities
                .iter()
                .map(|commodity| balances[commodity])
                .collect();

            dates.push(current_date);
            ordered_balances.push(ordered);

            current_date += chrono::Duration::days(1);
        }

        self.plot_inner
            .set_data(dates, commodities, ordered_balances);
        cx.notify();
    }
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
                (Some(mouse_position), Some(bounds)) => Some((mouse_position, bounds)),
                _ => None,
            }
        };
        div()
            .id("balance_chart")
            .size_full()
            .on_action(cx.listener(|this, period: &SelectPeriod, _window, cx| {
                this.plot_inner.set_period(period.period);
                cx.notify();
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
                        .label(plot_inner.period.to_string())
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
                let commodities = &self.plot_inner.commodities;

                let x_pos = x_scale.tick(&date).unwrap_or(0.0);

                // Create CrossLine for the vertical crosshair
                let cross_line = CrossLine::new(point(px(x_pos), px(mouse_y)));

                // Create Dot components for each data point
                let dots: Vec<Dot> = balances
                    .iter()
                    .enumerate()
                    .flat_map(|(idx, balance)| {
                        y_scale.tick(balance).map(|y_pos| {
                            let color = self.colors[idx % self.colors.len()];
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
                        .children(balances.iter().enumerate().map(|(idx, balance)| {
                            let commodity = &commodities[idx];
                            let color = self.colors[idx % self.colors.len()];

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
                                        .child(commodity.to_string())
                                        .child(balance.to_string()),
                                )
                        })),
                )
            })
    }
}

#[derive(IntoPlot, Clone)]
struct PlotInner {
    colors: Vec<Hsla>,

    period: Period,

    dates: Vec<chrono::NaiveDate>,
    balances: Vec<Vec<f64>>,
    commodities: Vec<String>,

    all_balances: Vec<f64>,
    y_min: f64,
    y_max: f64,

    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    cached_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    cached_x_scale: Rc<RefCell<Option<ScalePoint<chrono::NaiveDate>>>>,
    cached_y_scale: Rc<RefCell<Option<ScaleLinear<f64>>>>,
}

impl PlotInner {
    pub fn new(period: Period, colors: Vec<Hsla>) -> Self {
        Self {
            colors,
            period,
            dates: vec![],
            balances: vec![],
            all_balances: vec![],
            y_min: 0.0,
            y_max: 0.0,
            commodities: vec![],
            bounds: Rc::new(Cell::new(None)),
            cached_bounds: Rc::new(Cell::new(None)),
            cached_x_scale: Rc::new(RefCell::new(None)),
            cached_y_scale: Rc::new(RefCell::new(None)),
        }
    }

    pub fn set_period(&mut self, period: Period) {
        self.period = period;
    }

    pub fn set_data(
        &mut self,
        dates: Vec<chrono::NaiveDate>,
        commodities: Vec<String>,
        balances: Vec<Vec<fastnum::D128>>,
    ) {
        self.dates = dates;
        self.balances = balances
            .iter()
            .map(|balances| balances.iter().map(|b| b.to_f64()).collect())
            .collect();
        self.commodities = commodities;

        let mut min_balance = D128::MAX;
        let mut max_balance = D128::MIN;

        for daily_balances in &balances {
            for &balance in daily_balances {
                if balance < min_balance {
                    min_balance = balance;
                }
                if balance > max_balance {
                    max_balance = balance;
                }
            }
        }

        self.y_min = min_balance.to_f64();
        self.y_max = max_balance.to_f64();
        self.all_balances = vec![self.y_min, self.y_max];

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
            let y_scale = calc_y_scale(bounds, &self.all_balances);

            self.cached_bounds.set(Some(*bounds));
            self.cached_x_scale.replace(Some(x_scale.clone()));
            self.cached_y_scale.replace(Some(y_scale.clone()));

            (x_scale, y_scale)
        } else {
            // Safe to unwrap because we know cached values exist
            (
                self.cached_x_scale.borrow().as_ref().unwrap().clone(),
                self.cached_y_scale.borrow().as_ref().unwrap().clone(),
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

fn calc_y_scale(bounds: &Bounds<Pixels>, all_balances: &[f64]) -> ScaleLinear<f64> {
    let height = calc_height(bounds);
    ScaleLinear::new(all_balances.to_vec(), vec![height, PLOT_PADDING])
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

        let data = self
            .dates
            .iter()
            .zip(self.balances.iter())
            .collect::<Vec<_>>();

        // Draw a line for each commodity
        for idx in 0..self.commodities.len() {
            let color = self.colors[idx % self.colors.len()];
            let x_scale = x_scale.clone();
            let y_scale = y_scale.clone();

            Line::new()
                .data(data.clone())
                .x(move |d| x_scale.tick(&d.0))
                .y(move |d| d.1.get(idx).and_then(|v| y_scale.tick(v)))
                .stroke(color)
                .stroke_width(px(2.0))
                .stroke_style(StrokeStyle::Linear)
                .paint(&bounds, window);
        }
    }
}
