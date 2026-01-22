use core::f64;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use gpui::prelude::*;
use gpui::{div, MouseMoveEvent};
use gpui::{px, App, Bounds, Context, IntoElement, Pixels, Point, Render, Window};
use gpui_component::{
    h_flex,
    plot::{
        scale::{Scale, ScaleBand, ScaleLinear},
        shape::Bar,
        tooltip::{Tooltip, TooltipPosition},
        AxisText, IntoPlot, Plot, PlotAxis,
    },
    ActiveTheme, PixelsExt, StyledExt,
};

use super::Label;

/// Data for the previous period comparison
#[derive(Debug, Clone, Default)]
pub struct PreviousPeriodData {
    /// Original labels from the previous period (for tooltip display)
    pub labels: Vec<String>,
    /// Values aligned to current period indices
    pub values: HashMap<String, Vec<Option<f64>>>,
}

/// Data for a single bar in the chart
#[derive(Debug, Clone)]
pub struct BarData {
    /// Label for the X-axis (e.g., "Jan", "1-7", "Mon")
    pub label: String,
    /// Date range this bar represents (for tooltip)
    pub start_date: chrono::NaiveDate,
    pub end_date: chrono::NaiveDate,
}

// Constants for chart layout
const PLOT_PADDING: f32 = 10.0;
const X_AXIS_GAP: f32 = 20.0;
const Y_AXIS_LABEL_WIDTH: f32 = 35.0;
const ESTIMATED_Y_LABEL_HEIGHT: f32 = 20.0;
const MIN_Y_LABELS: usize = 2;
const MAX_Y_LABELS: usize = 10;
const BAR_WIDTH_RATIO: f32 = 0.7;

pub struct Chart {
    plot_inner: PlotInner,
    mouse_position: Option<Point<Pixels>>,
    hovered_idx: Option<usize>,
    higher_is_better: bool,
}

impl Chart {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            plot_inner: PlotInner::new(),
            mouse_position: None,
            hovered_idx: None,
            higher_is_better: false,
        }
    }

    pub fn refresh_data(
        &mut self,
        bars: &[BarData],
        values: HashMap<Label, Vec<Option<f64>>>,
        previous_period: Option<PreviousPeriodData>,
        higher_is_better: bool,
        _cx: &mut Context<Self>,
    ) {
        self.hovered_idx = None;
        self.higher_is_better = higher_is_better;
        self.plot_inner
            .set_data(bars.to_vec(), values, previous_period);
    }
}

impl Render for Chart {
    #[allow(clippy::too_many_lines)]
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let plot_inner = self.plot_inner.clone();
        let tooltip_data = {
            match (self.mouse_position, plot_inner.bounds.get()) {
                (Some(mouse_position), Some(bounds)) if !plot_inner.bars.is_empty() => {
                    Some((mouse_position, bounds))
                }
                _ => None,
            }
        };
        div()
            .id("bar_chart")
            .size_full()
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                this.mouse_position = Some(event.position);

                if let (Some(mouse_pos), Some(bounds)) =
                    (this.mouse_position, this.plot_inner.bounds.get())
                {
                    if !this.plot_inner.bars.is_empty() {
                        let mouse_x = mouse_pos.x.as_f32() - bounds.origin.x.as_f32();
                        let labels: Vec<String> =
                            this.plot_inner.bars.iter().map(|b| b.label.clone()).collect();
                        let (x_scale, _) = this.plot_inner.get_or_compute_scales(&bounds);
                        let new_hovered_idx = find_hovered_bar_index(&x_scale, &labels, mouse_x);

                        if this.hovered_idx != new_hovered_idx {
                            this.hovered_idx = new_hovered_idx;
                            cx.notify();
                        }
                    }
                } else {
                    cx.notify();
                }
            }))
            .on_hover(cx.listener(|this, hovered, _window, cx| {
                if !hovered {
                    this.mouse_position = None;
                    this.hovered_idx = None;
                    cx.notify();
                }
            }))
            .child(plot_inner)
            .when_some(tooltip_data, |this, tooltip_data| {
                let mouse_x = tooltip_data.0.x.as_f32() - tooltip_data.1.origin.x.as_f32();
                let width = calc_width(&tooltip_data.1);

                let labels: Vec<String> =
                    self.plot_inner.bars.iter().map(|b| b.label.clone()).collect();
                let (x_scale, _y_scale) = self.plot_inner.get_or_compute_scales(&tooltip_data.1);

                let hovered_idx = match find_hovered_bar_index(&x_scale, &labels, mouse_x) {
                    Some(idx) => idx,
                    None => return this.child(div()),
                };

                let bar_data = &self.plot_inner.bars[hovered_idx];

                // Get current period values
                let current_values: Vec<_> = self
                    .plot_inner
                    .values
                    .iter()
                    .filter_map(|(label, values_vec)| {
                        values_vec
                            .get(hovered_idx)
                            .and_then(|v| v.map(|val| (label.text.clone(), label.color, val)))
                    })
                    .collect();

                // Get previous period values if available
                let previous_data: Option<(String, HashMap<String, f64>)> =
                    self.plot_inner.previous_period.as_ref().and_then(|prev| {
                        let prev_label = prev.labels.get(hovered_idx)?.clone();
                        let prev_values: HashMap<String, f64> = prev
                            .values
                            .iter()
                            .filter_map(|(commodity, values)| {
                                values
                                    .get(hovered_idx)
                                    .and_then(|v| v.map(|val| (commodity.clone(), val)))
                            })
                            .collect();
                        Some((prev_label, prev_values))
                    });

                if current_values.is_empty() {
                    return this.child(div());
                }

                // Determine tooltip position based on mouse location
                let position = if mouse_x < width / 2.0 {
                    TooltipPosition::Right
                } else {
                    TooltipPosition::Left
                };

                let green = cx.theme().colors.green;
                let red = cx.theme().colors.red;
                let muted = cx.theme().muted_foreground;

                // Format date range for tooltip
                let date_range_str = if bar_data.start_date == bar_data.end_date {
                    bar_data.start_date.format("%b %-d").to_string()
                } else {
                    format!(
                        "{} - {}",
                        bar_data.start_date.format("%b %-d"),
                        bar_data.end_date.format("%b %-d")
                    )
                };

                this.child(
                    Tooltip::new()
                        .position(position)
                        .gap(px(20.0))
                        .p_3()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .rounded_lg()
                        .shadow_lg()
                        .children(current_values.iter().map(
                            |(commodity, color, current_value)| {
                                let prev_value = previous_data
                                    .as_ref()
                                    .and_then(|(_, vals)| vals.get(commodity).copied());

                                let diff_element = prev_value.map(|prev| {
                                    let diff = current_value - prev;
                                    let (indicator, diff_color) = if diff > 0.0 {
                                        ("▲", if self.higher_is_better { green } else { red })
                                    } else if diff < 0.0 {
                                        ("▼", if self.higher_is_better { red } else { green })
                                    } else {
                                        ("", muted)
                                    };
                                    (indicator, diff_color, diff.abs())
                                });

                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                h_flex()
                                                    .gap_1()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(*color)
                                                            .child("■"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_semibold()
                                                            .child(commodity.clone()),
                                                    ),
                                            )
                                            .when_some(
                                                diff_element,
                                                |el, (indicator, diff_color, diff_val)| {
                                                    el.child(
                                                        h_flex()
                                                            .gap_1()
                                                            .text_sm()
                                                            .text_color(diff_color)
                                                            .child(indicator)
                                                            .child(format_y_value(diff_val)),
                                                    )
                                                },
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .text_xs()
                                            .justify_between()
                                            .child(date_range_str.clone())
                                            .child(format_y_value(*current_value)),
                                    )
                                    .when_some(
                                        previous_data.as_ref(),
                                        |el, (prev_label, prev_vals)| {
                                            if let Some(prev_val) = prev_vals.get(commodity) {
                                                el.child(
                                                    h_flex()
                                                        .gap_2()
                                                        .text_xs()
                                                        .text_color(muted)
                                                        .justify_between()
                                                        .child(prev_label.clone())
                                                        .child(format_y_value(*prev_val)),
                                                )
                                            } else {
                                                el
                                            }
                                        },
                                    )
                            },
                        )),
                )
            })
    }
}

#[derive(IntoPlot, Clone)]
struct PlotInner {
    bars: Vec<BarData>,
    values: HashMap<Label, Vec<Option<f64>>>,
    previous_period: Option<PreviousPeriodData>,

    y_min: f64,
    y_max: f64,

    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    cached_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    cached_x_scale: Rc<RefCell<Option<ScaleBand<String>>>>,
    cached_y_scale: Rc<RefCell<Option<ScaleLinear<f64>>>>,
}

impl PlotInner {
    pub fn new() -> Self {
        Self {
            bars: vec![],
            values: HashMap::new(),
            previous_period: None,
            y_min: 0.0,
            y_max: 0.0,
            bounds: Rc::new(Cell::new(None)),
            cached_bounds: Rc::new(Cell::new(None)),
            cached_x_scale: Rc::new(RefCell::new(None)),
            cached_y_scale: Rc::new(RefCell::new(None)),
        }
    }

    pub fn set_data(
        &mut self,
        bars: Vec<BarData>,
        values: HashMap<Label, Vec<Option<f64>>>,
        previous_period: Option<PreviousPeriodData>,
    ) {
        self.bars = bars;
        self.values = values;
        self.previous_period = previous_period;

        let mut min_balance = 0.0_f64; // Start at 0 for bar charts
        let mut max_balance = f64::MIN;

        for values in self.values.values() {
            for v in values.iter().flatten() {
                min_balance = v.min(min_balance);
                max_balance = v.max(max_balance);
            }
        }

        if let Some(prev) = &self.previous_period {
            for values in prev.values.values() {
                for v in values.iter().flatten() {
                    min_balance = v.min(min_balance);
                    max_balance = v.max(max_balance);
                }
            }
        }

        // Handle case where no values exist
        if max_balance == f64::MIN {
            max_balance = 1.0;
        }

        // Ensure some range even if all values are the same
        if (max_balance - min_balance).abs() < f64::EPSILON {
            max_balance = min_balance + 1.0;
        }

        self.y_min = min_balance;
        self.y_max = max_balance;

        self.cached_bounds.set(None);
        self.cached_x_scale.replace(None);
        self.cached_y_scale.replace(None);
    }

    fn get_or_compute_scales(
        &self,
        bounds: &Bounds<Pixels>,
    ) -> (ScaleBand<String>, ScaleLinear<f64>) {
        let needs_recompute = match self.cached_bounds.get() {
            Some(cached) => cached != *bounds,
            None => true,
        };

        if needs_recompute {
            let labels: Vec<String> = self.bars.iter().map(|b| b.label.clone()).collect();
            let x_scale = calc_x_scale(bounds, &labels);
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
    bounds.size.width.as_f32() - PLOT_PADDING - Y_AXIS_LABEL_WIDTH
}

fn calc_height(bounds: &Bounds<Pixels>) -> f32 {
    bounds.size.height.as_f32() - PLOT_PADDING - X_AXIS_GAP
}

fn calc_x_scale(bounds: &Bounds<Pixels>, labels: &[String]) -> ScaleBand<String> {
    let width = calc_width(bounds);
    ScaleBand::new(labels.to_vec(), vec![PLOT_PADDING, width])
}

fn calc_y_scale(bounds: &Bounds<Pixels>, domain: &[f64]) -> ScaleLinear<f64> {
    let height = calc_height(bounds);
    ScaleLinear::new(domain.to_vec(), vec![height, PLOT_PADDING])
}

fn format_y_value(value: f64) -> String {
    let abs_value = value.abs();
    if abs_value >= 1_000_000_000.0 {
        format!("{:.1}B", value / 1_000_000_000.0)
    } else if abs_value >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if abs_value >= 1_000.0 {
        format!("{:.1}K", value / 1_000.0)
    } else if abs_value >= 1.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn calc_y_label_count(bounds: &Bounds<Pixels>) -> usize {
    let height = calc_height(bounds);
    let max_labels = (height / ESTIMATED_Y_LABEL_HEIGHT).floor() as usize;
    max_labels.clamp(MIN_Y_LABELS, MAX_Y_LABELS)
}

/// Find the index of the bar that the mouse is hovering over
fn find_hovered_bar_index(
    x_scale: &ScaleBand<String>,
    labels: &[String],
    mouse_x: f32,
) -> Option<usize> {
    let band_width = x_scale.band_width();
    for (idx, label) in labels.iter().enumerate() {
        if let Some(bar_x) = x_scale.tick(label) {
            if mouse_x >= bar_x && mouse_x < bar_x + band_width {
                return Some(idx);
            }
        }
    }
    None
}

impl Plot for PlotInner {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        self.bounds.set(Some(bounds));

        if self.bars.is_empty() {
            // Draw empty axes
            let height = calc_height(&bounds);
            let width = calc_width(&bounds);
            PlotAxis::new()
                .x(height)
                .hide_x_axis()
                .y(width)
                .hide_y_axis()
                .stroke(cx.theme().border)
                .paint(&bounds, window, cx);
            return;
        }

        let height = calc_height(&bounds);
        let width = calc_width(&bounds);

        let (x_scale, y_scale) = self.get_or_compute_scales(&bounds);

        // Y-axis labels
        let y_label_count = calc_y_label_count(&bounds);
        let y_labels: Vec<AxisText> = (0..y_label_count)
            .filter_map(|i| {
                let value =
                    self.y_min + (self.y_max - self.y_min) * i as f64 / (y_label_count - 1) as f64;
                y_scale.tick(&value).map(|tick| {
                    AxisText::new(format_y_value(value), tick, cx.theme().muted_foreground)
                })
            })
            .collect();

        // X-axis labels
        let x_labels: Vec<AxisText> = self
            .bars
            .iter()
            .filter_map(|bar| {
                x_scale.tick(&bar.label).map(|tick| {
                    // Center the label under the bar
                    let centered_tick = tick + x_scale.band_width() / 2.0;
                    AxisText::new(bar.label.clone(), centered_tick, cx.theme().muted_foreground)
                })
            })
            .collect();

        // Draw axes
        PlotAxis::new()
            .x(height)
            .hide_x_axis()
            .x_label(x_labels)
            .y(width)
            .hide_y_axis()
            .y_label(y_labels)
            .stroke(cx.theme().border)
            .paint(&bounds, window, cx);

        let band_width = x_scale.band_width();
        let bar_width = band_width * BAR_WIDTH_RATIO;
        let bar_offset = (band_width - bar_width) / 2.0;

        // Draw previous period bars first (behind current)
        if let Some(prev) = &self.previous_period {
            let prev_color = cx.theme().muted_foreground.opacity(0.3);
            for values in prev.values.values() {
                let x_scale = x_scale.clone();
                let y_scale = y_scale.clone();

                let data: Vec<(&BarData, &Option<f64>)> =
                    self.bars.iter().zip(values.iter()).collect();

                let y_baseline = y_scale.tick(&0.0).unwrap_or(height);

                Bar::new()
                    .data(data)
                    .band_width(bar_width)
                    .x(move |d| x_scale.tick(&d.0.label).map(|x| x + bar_offset))
                    .y0(move |_| y_baseline)
                    .y1(move |d| d.1.and_then(|v| y_scale.tick(&v)))
                    .fill(move |_| prev_color)
                    .paint(&bounds, window, cx);
            }
        }

        // Draw current period bars
        let y_baseline = y_scale.tick(&0.0).unwrap_or(height);

        // Collect values to avoid borrowing issues
        let values_with_labels: Vec<_> = self
            .values
            .iter()
            .map(|(label, values)| (label.color, values.clone()))
            .collect();

        for (color, values) in values_with_labels {
            let x_scale = x_scale.clone();
            let y_scale = y_scale.clone();

            let data: Vec<(&BarData, Option<f64>)> = self
                .bars
                .iter()
                .zip(values.iter())
                .map(|(bar, val)| (bar, *val))
                .collect();

            Bar::new()
                .data(data)
                .band_width(bar_width)
                .x(move |d| x_scale.tick(&d.0.label).map(|x| x + bar_offset))
                .y0(move |_| y_baseline)
                .y1(move |d| d.1.and_then(|v| y_scale.tick(&v)))
                .fill(move |_| color)
                .paint(&bounds, window, cx);
        }
    }
}
