use core::f64;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use gpui::prelude::*;
use gpui::{div, point, MouseMoveEvent};
use gpui::{px, App, Bounds, Context, IntoElement, Pixels, Point, Render, Window};
use gpui_component::{
    h_flex,
    plot::{
        scale::{Scale, ScaleLinear, ScalePoint},
        shape::Line,
        tooltip::{CrossLine, Dot, Tooltip, TooltipPosition},
        AxisText, IntoPlot, Plot, PlotAxis, StrokeStyle,
    },
    ActiveTheme, PixelsExt, StyledExt,
};

use super::Label;

/// Data for the previous period comparison
#[derive(Debug, Clone, Default)]
pub struct PreviousPeriodData {
    /// Original dates from the previous period (for tooltip display)
    pub dates: Vec<chrono::NaiveDate>,
    /// Values aligned to current period indices (by day-of-period)
    pub values: HashMap<String, Vec<Option<f64>>>,
}

// Constants for chart layout
/// Padding around the plot area in pixels
const PLOT_PADDING: f32 = 10.0;
/// Gap reserved for X-axis labels below chart
const X_AXIS_GAP: f32 = 10.0;
/// Space reserved for Y-axis labels on the right
const Y_AXIS_LABEL_WIDTH: f32 = 35.0;
/// Estimated width of date label "YYYY-MM-DD" in pixels (10 chars * ~6px + padding)
const ESTIMATED_DATE_LABEL_WIDTH: f32 = 70.0;
/// Estimated height of Y-axis label in pixels (font size + padding)
const ESTIMATED_Y_LABEL_HEIGHT: f32 = 20.0;
/// Minimum number of Y-axis labels to display
const MIN_Y_LABELS: usize = 2;
/// Maximum number of Y-axis labels to display
const MAX_Y_LABELS: usize = 10;

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
        dates: &[chrono::NaiveDate],
        values: HashMap<Label, Vec<Option<f64>>>,
        previous_period: Option<PreviousPeriodData>,
        higher_is_better: bool,
        _cx: &mut Context<Self>,
    ) {
        self.hovered_idx = None;
        self.higher_is_better = higher_is_better;
        self.plot_inner
            .set_data(dates.to_vec(), values, previous_period);
    }
}

impl Render for Chart {
    #[allow(clippy::too_many_lines)]
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
                let mouse_y = tooltip_data.0.y.as_f32() - tooltip_data.1.origin.y.as_f32();
                let width = calc_width(&tooltip_data.1);

                let (x_scale, y_scale) = self.plot_inner.get_or_compute_scales(&tooltip_data.1);

                let hovered_idx = x_scale.least_index(mouse_x);

                let date = self.plot_inner.dates[hovered_idx];

                let x_pos = x_scale
                    .tick(&date)
                    .expect("hovered date should have x position");

                // Create CrossLine for the vertical crosshair (limited to chart height)
                let chart_height = calc_height(&tooltip_data.1);
                let cross_line = CrossLine::new(point(px(x_pos), px(mouse_y))).height(chart_height);

                // Get current period values
                let current_values: Vec<_> = self
                    .plot_inner
                    .values
                    .iter()
                    .filter_map(|(label, values_vec)| {
                        values_vec[hovered_idx].map(|v| (label.text.clone(), label.color, v))
                    })
                    .collect();

                // Get previous period values and date if available
                let previous_data: Option<(chrono::NaiveDate, HashMap<String, f64>)> =
                    self.plot_inner.previous_period.as_ref().and_then(|prev| {
                        let prev_date = prev.dates.get(hovered_idx).copied()?;
                        let prev_values: HashMap<String, f64> = prev
                            .values
                            .iter()
                            .filter_map(|(commodity, values)| {
                                values
                                    .get(hovered_idx)
                                    .and_then(|v| v.map(|val| (commodity.clone(), val)))
                            })
                            .collect();
                        Some((prev_date, prev_values))
                    });

                // Create Dot components for each data point (current period only)
                let dots: Vec<Dot> = current_values
                    .iter()
                    .filter_map(|(_, color, value)| {
                        y_scale.tick(value).map(|y_pos| {
                            Dot::new(point(px(x_pos), px(y_pos)))
                                .size(px(10.0))
                                .fill(*color)
                                .stroke(cx.theme().background)
                        })
                    })
                    .collect();

                if dots.is_empty() {
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
                        .children(current_values.iter().map(
                            |(commodity, color, current_value)| {
                                let prev_value = previous_data
                                    .as_ref()
                                    .and_then(|(_, vals)| vals.get(commodity).copied());

                                // Calculate difference if previous value exists
                                let diff_element = prev_value.map(|prev| {
                                    let diff = current_value - prev;
                                    let (indicator, diff_color) = if diff > 0.0 {
                                        // More than previous period
                                        ("▲", if self.higher_is_better { green } else { red })
                                    } else if diff < 0.0 {
                                        // Less than previous period
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
                                    // Header row: commodity name + difference
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
                                                            .child("—"),
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
                                    // Current period row
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .text_xs()
                                            .justify_between()
                                            .child(date.format("%b %-d").to_string())
                                            .child(format_y_value(*current_value)),
                                    )
                                    // Previous period row (if available)
                                    .when_some(
                                        previous_data.as_ref(),
                                        |el, (prev_date, prev_vals)| {
                                            if let Some(prev_val) = prev_vals.get(commodity) {
                                                el.child(
                                                    h_flex()
                                                        .gap_2()
                                                        .text_xs()
                                                        .text_color(muted)
                                                        .justify_between()
                                                        .child(
                                                            prev_date.format("%b %-d, %Y").to_string(),
                                                        )
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
    dates: Vec<chrono::NaiveDate>,
    values: HashMap<Label, Vec<Option<f64>>>,

    /// Previous period data for comparison (optional)
    previous_period: Option<PreviousPeriodData>,

    y_min: f64,
    y_max: f64,

    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    cached_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    cached_x_scale: Rc<RefCell<Option<ScalePoint<chrono::NaiveDate>>>>,
    cached_y_scale: Rc<RefCell<Option<ScaleLinear<f64>>>>,
}

impl PlotInner {
    pub fn new() -> Self {
        Self {
            dates: vec![],
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
        dates: Vec<chrono::NaiveDate>,
        values: HashMap<Label, Vec<Option<f64>>>,
        previous_period: Option<PreviousPeriodData>,
    ) {
        self.dates = dates;
        self.values = values;
        self.previous_period = previous_period;

        let mut min_balance = f64::MAX;
        let mut max_balance = f64::MIN;

        // Consider current period values for Y scale
        for values in self.values.values() {
            debug_assert!(
                values.len() == self.dates.len(),
                "Values length must match dates length"
            );
            for v in values.iter().flatten() {
                min_balance = v.min(min_balance);
                max_balance = v.max(max_balance);
            }
        }

        // Also consider previous period values for Y scale
        if let Some(prev) = &self.previous_period {
            for values in prev.values.values() {
                for v in values.iter().flatten() {
                    min_balance = v.min(min_balance);
                    max_balance = v.max(max_balance);
                }
            }
        }

        self.y_min = min_balance;
        self.y_max = max_balance;

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
    bounds.size.width.as_f32() - PLOT_PADDING - Y_AXIS_LABEL_WIDTH
}

fn calc_height(bounds: &Bounds<Pixels>) -> f32 {
    bounds.size.height.as_f32() - PLOT_PADDING - X_AXIS_GAP
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

/// Format a number for display on the Y-axis (1.2K, 1.5M, etc.)
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

/// Calculate the number of Y-axis labels based on available height
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn calc_y_label_count(bounds: &Bounds<Pixels>) -> usize {
    let height = calc_height(bounds);
    let max_labels = (height / ESTIMATED_Y_LABEL_HEIGHT).floor() as usize;
    max_labels.clamp(MIN_Y_LABELS, MAX_Y_LABELS)
}

impl Plot for PlotInner {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        if self.dates.is_empty() {
            return;
        }

        self.bounds.set(Some(bounds));

        // Calculate drawing area with padding
        let height = calc_height(&bounds);

        // Get or compute cached scales
        let (x_scale, y_scale) = self.get_or_compute_scales(&bounds);

        // Create Y-axis labels with adaptive spacing based on available height
        // Skip the first label (i=0, at y_min) to avoid overlap with X-axis labels in the corner
        let y_label_count = calc_y_label_count(&bounds);
        #[allow(clippy::cast_precision_loss)]
        let y_labels: Vec<AxisText> = (1..y_label_count)
            .filter_map(|i| {
                let value =
                    self.y_min + (self.y_max - self.y_min) * i as f64 / (y_label_count - 1) as f64;
                y_scale.tick(&value).map(|tick| {
                    AxisText::new(format_y_value(value), tick, cx.theme().muted_foreground)
                })
            })
            .collect();

        // Create X-axis labels with width-based spacing to prevent overlap
        let available_width = calc_width(&bounds);
        let max_labels_that_fit = (available_width / ESTIMATED_DATE_LABEL_WIDTH).floor() as usize;
        let tick_margin = if max_labels_that_fit > 0 && max_labels_that_fit < self.dates.len() {
            ((self.dates.len() as f32) / (max_labels_that_fit as f32)).ceil() as usize
        } else {
            1
        };
        let x_labels: Vec<AxisText> = self
            .dates
            .iter()
            .enumerate()
            .filter_map(|(i, d)| {
                // Show evenly spaced labels, skip the last to avoid overlap with Y-axis area
                if i % tick_margin == 0 {
                    x_scale
                        .tick(d)
                        .map(|tick| AxisText::new(d.to_string(), tick, cx.theme().muted_foreground))
                } else {
                    None
                }
            })
            .collect();

        // Draw axes (no axis lines, just labels - lines would extend into label areas)
        let width = calc_width(&bounds);
        PlotAxis::new()
            .x(height)
            .hide_x_axis()
            .x_label(x_labels)
            .y(width)
            .hide_y_axis()
            .y_label(y_labels)
            .stroke(cx.theme().border)
            .paint(&bounds, window, cx);

        // Draw previous period lines first (so they appear behind current period)
        if let Some(prev) = &self.previous_period {
            let prev_color = cx.theme().muted_foreground;
            for values in prev.values.values() {
                let x_scale = x_scale.clone();
                let y_scale = y_scale.clone();

                Line::new()
                    .data(self.dates.iter().zip(values.iter()))
                    .x(move |d| x_scale.tick(d.0))
                    .y(move |d| d.1.and_then(|value| y_scale.tick(&value)))
                    .stroke(prev_color)
                    .stroke_width(px(1.0))
                    .stroke_style(StrokeStyle::Linear)
                    .paint(&bounds, window);
            }
        }

        // Draw current period lines (on top)
        for (label, values) in &self.values {
            let x_scale = x_scale.clone();
            let y_scale = y_scale.clone();

            Line::new()
                .data(self.dates.iter().zip(values.iter()))
                .x(move |d| x_scale.tick(d.0))
                .y(move |d| d.1.and_then(|value| y_scale.tick(&value)))
                .stroke(label.color)
                .stroke_width(px(1.0))
                .stroke_style(StrokeStyle::Linear)
                .paint(&bounds, window);
        }
    }
}
