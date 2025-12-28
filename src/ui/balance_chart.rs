// todo:
// - add tooltip on hover showing date and balances
// - add legend for commodities
// - configurable period (weekly, monthly)
// - configurable resolution (daily, weekly, monthly)

use std::{cell::Cell, collections::HashSet, rc::Rc};

use chrono::Datelike;
use fastnum::D128;
use gpui::prelude::FluentBuilder;
#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::{
    h_flex,
    plot::{
        scale::{Scale, ScaleLinear, ScalePoint},
        shape::Line,
        AxisText, IntoPlot, Plot, PlotAxis, StrokeStyle, AXIS_GAP,
    },
    v_flex, StyledExt,
};
use gpui_component::{ActiveTheme, PixelsExt};

use crate::transactions::Transaction;

// Constants for chart layout
/// Padding around the plot area in pixels
const PLOT_PADDING: f32 = 10.0;
/// Minimum number of data points before skipping ticks on X-axis
const MIN_TICK_SPACING: usize = 10;
/// Number of Y-axis value labels to display
const Y_AXIS_LABEL_COUNT: usize = 5;

pub struct BalanceChart {
    plot_inner: PlotInner,
    mouse_position: Option<Point<Pixels>>,
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
            plot_inner: PlotInner::new(colors.clone()),
            mouse_position: None,
            colors,
        }
    }

    pub fn set_data(&mut self, transactions: &[Transaction]) {
        let (dates, commodities, balances) = build_chart_data_points(transactions);
        self.plot_inner.set_data(dates, commodities, balances);
    }
}

fn build_chart_data_points(
    transactions: &[Transaction],
) -> (Vec<chrono::NaiveDate>, Vec<String>, Vec<Vec<fastnum::D128>>) {
    use std::collections::{HashMap, HashSet};

    if transactions.is_empty() {
        return (vec![], vec![], vec![]);
    }

    let transactions = transactions
        .iter()
        .filter(|t| t.time.year() == 2025)
        .collect::<Vec<_>>();

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

    (dates, commodities, ordered_balances)
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
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                // Update mouse position
                this.mouse_position = Some(event.position);
                cx.notify();
            }))
            .child(plot_inner)
            .when_some(tooltip_data, |this, tooltip_data| {
                let mouse_x = tooltip_data.0.x.as_f32() - tooltip_data.1.origin.x.as_f32();
                let mouse_y = tooltip_data.0.y.as_f32() - tooltip_data.1.origin.y.as_f32();
                let width = calc_width(&tooltip_data.1);
                let height = calc_height(&tooltip_data.1);

                let x_scale = calc_x_scale(&tooltip_data.1, &self.plot_inner.dates);
                let y_scale = calc_y_scale(&tooltip_data.1, &self.plot_inner.all_balances);

                let hovered_idx = x_scale.least_index(mouse_x);

                let date = self.plot_inner.dates[hovered_idx];
                let balances = &self.plot_inner.balances[hovered_idx];
                let commodities = &self.plot_inner.commodities;

                let tooltip_width_estimate = 220.0;
                let tooltip_x = if mouse_x < width / 2.0 {
                    (mouse_x + 20.0).min(width - tooltip_width_estimate)
                } else {
                    (mouse_x - tooltip_width_estimate - 20.0).max(0.0)
                };
                let tooltip_y = mouse_y;

                let x_pos = x_scale.tick(&date).unwrap_or(0.0);

                let dots = balances.iter().enumerate().flat_map(|(idx, balance)| {
                    y_scale.tick(balance).map(|y_pos| {
                        let color = self.colors[idx % self.colors.len()];
                        div()
                            .absolute()
                            .left(px(x_pos - 5.0))
                            .top(px(y_pos - 5.0))
                            .w(px(10.0))
                            .h(px(10.0))
                            .bg(color)
                            .rounded_full()
                            .border_2()
                            .border_color(cx.theme().background)
                    })
                });

                let crosshair_vertical = div()
                    .id("crosshair-vertical")
                    .absolute()
                    .left(tooltip_data.0.x - tooltip_data.1.origin.x)
                    .top(px(0.0))
                    .w(px(1.0))
                    .h(px(height))
                    .bg(cx.theme().muted_foreground);

                let tooltip = v_flex()
                    .id("tooltip")
                    .absolute()
                    .left(px(tooltip_x))
                    .top(px(tooltip_y))
                    .gap_2()
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
                                div()
                                    .text_sm()
                                    .font_medium()
                                    .text_color(cx.theme().foreground)
                                    .child(format!("{} ${:.2}", commodity, balance)),
                            )
                    }));

                this.child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .size_full()
                        .child(crosshair_vertical)
                        .children(dots)
                        .child(tooltip),
                )
            })
    }
}

#[derive(IntoPlot, Clone)]
struct PlotInner {
    colors: Vec<Hsla>,

    dates: Vec<chrono::NaiveDate>,
    balances: Vec<Vec<f64>>,
    commodities: Vec<String>,

    all_balances: Vec<f64>,
    y_min: f64,
    y_max: f64,

    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl PlotInner {
    pub fn new(colors: Vec<Hsla>) -> Self {
        Self {
            colors,
            dates: vec![],
            balances: vec![],
            all_balances: vec![],
            y_min: 0.0,
            y_max: 0.0,
            commodities: vec![],
            bounds: Rc::new(Cell::new(None)),
        }
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

        let mut all_unique_balances: HashSet<D128> = HashSet::new();
        let mut min_balance = D128::MAX;
        let mut max_balance = D128::MIN;

        for daily_balances in &balances {
            for &balance in daily_balances {
                all_unique_balances.insert(balance);
                if balance < min_balance {
                    min_balance = balance;
                }
                if balance > max_balance {
                    max_balance = balance;
                }
            }
        }

        self.all_balances = all_unique_balances.iter().map(|b| b.to_f64()).collect();
        self.y_min = min_balance.to_f64();
        self.y_max = max_balance.to_f64();
    }
}

fn calc_width(bounds: &Bounds<Pixels>) -> f32 {
    bounds.size.width.as_f32() - PLOT_PADDING
}

fn calc_height(bounds: &Bounds<Pixels>) -> f32 {
    bounds.size.height.as_f32() - AXIS_GAP - PLOT_PADDING
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

        // Create X scale for dates (categorical)
        let x_scale = calc_x_scale(&bounds, &self.dates);
        // Create Y scale for balances (linear)
        let y_scale = calc_y_scale(&bounds, &self.all_balances);

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
        for (commodity_idx, _commodity) in self.commodities.iter().enumerate() {
            let color = self.colors[commodity_idx % self.colors.len()];
            let x_scale = x_scale.clone();
            let y_scale = y_scale.clone();

            Line::new()
                .data(data.clone())
                .x(move |d| x_scale.tick(&d.0))
                .y(move |d| d.1.get(commodity_idx).and_then(|v| y_scale.tick(v)))
                .stroke(color)
                .stroke_width(px(2.0))
                .stroke_style(StrokeStyle::Linear)
                .paint(&bounds, window);
        }
    }
}
