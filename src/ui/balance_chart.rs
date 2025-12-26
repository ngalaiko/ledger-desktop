//! Balance chart component for visualizing account balances over time.
//!
//! This module provides an interactive chart that displays balance data for multiple
//! commodities across time. Features include:
//! - Multi-commodity line chart with color-coded lines
//! - Interactive hover tooltips showing exact values
//! - Automatic scaling and grid lines
//! - X and Y axis labels with smart tick spacing

use chrono::Datelike;
use gpui::prelude::FluentBuilder;
#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::plot::{
    scale::{Scale, ScaleLinear, ScalePoint},
    shape::Line,
    AxisText, Grid, IntoPlot, Plot, PlotAxis, StrokeStyle, AXIS_GAP,
};
use gpui_component::{h_flex, v_flex, ActiveTheme, PixelsExt, StyledExt};
use std::cell::Cell;
use std::rc::Rc;

// Constants for chart layout
/// Padding around the plot area in pixels
const PLOT_PADDING: f32 = 10.0;
/// Number of available chart colors for commodity lines
const CHART_COLORS_COUNT: usize = 5;
/// Minimum number of data points before skipping ticks on X-axis
const MIN_TICK_SPACING: usize = 10;
/// Number of horizontal grid lines to draw
const GRID_LINE_COUNT: usize = 4;
/// Number of Y-axis value labels to display
const Y_AXIS_LABEL_COUNT: usize = 5;

/// A single data point in the chart representing balances at a specific date.
#[derive(Clone)]
pub struct DataPoint {
    /// The date for this data point
    pub date: chrono::NaiveDate,
    /// List of (commodity_name, balance_value) pairs for this date
    pub balances: Vec<(String, f64)>,
}

/// Inner plot structure that implements the Plot trait for custom rendering.
///
/// This struct is wrapped by BalanceChart and handles the actual drawing
/// of lines, axes, and grid on the canvas.
#[derive(IntoPlot, Clone)]
struct PlotInner {
    /// Time series data points to plot
    data: Vec<DataPoint>,
    /// List of commodity names in the order they appear in each DataPoint
    commodities: Vec<String>,
    /// Shared bounds reference that persists across clones.
    /// Updated during paint and read by parent for hover detection.
    /// Uses Rc<Cell<>> for interior mutability.
    cached_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

/// Interactive balance chart component with hover tooltips.
///
/// Displays multi-commodity balance data over time with:
/// - Color-coded lines for each commodity
/// - Interactive tooltips on hover showing exact values
/// - Automatic axis scaling and labeling
pub struct BalanceChart {
    /// The inner plot component that renders the chart
    plot_inner: PlotInner,
    /// Index of the currently hovered data point, if any
    hovered_index: Option<usize>,
    /// Mouse position for tooltip placement
    mouse_position: Option<Point<Pixels>>,
}

impl BalanceChart {
    /// Creates a new empty balance chart.
    pub fn new() -> Self {
        Self {
            plot_inner: PlotInner {
                data: vec![],
                commodities: vec![],
                cached_bounds: Rc::new(Cell::new(None)),
            },
            hovered_index: None,
            mouse_position: None,
        }
    }

    /// Updates the chart with new data and commodity list.
    ///
    /// # Arguments
    /// * `data` - Vector of data points containing dates and balance values
    /// * `commodities` - List of commodity names in the order they appear in data points
    ///
    /// # Note
    /// Currently filters data to only show year 2025 for focused analysis.
    pub fn set_data(&mut self, data: Vec<DataPoint>, commodities: Vec<String>) {
        // Filter for year 2025
        self.plot_inner.data = data.into_iter().filter(|d| d.date.year() == 2025).collect();
        self.plot_inner.commodities = commodities;
    }

    /// Find the nearest data point to the given mouse position using proper scale calculations
    /// mouse_x should be in chart-div relative coordinates
    fn find_nearest_data_point(&self, mouse_x: f32, _bounds: &Bounds<Pixels>) -> Option<usize> {
        if self.plot_inner.data.is_empty() {
            return None;
        }

        // Create scale using chart-relative coordinates
        // The plot fills the chart div, so we use the full width
        let chart_width = _bounds.size.width.as_f32();

        let x_scale = ScalePoint::new(
            self.plot_inner
                .data
                .iter()
                .map(|d| d.date.to_string())
                .collect(),
            vec![PLOT_PADDING, chart_width - PLOT_PADDING],
        );

        // Find the closest data point to the mouse position
        let mut closest_index = 0;
        let mut closest_distance = f32::MAX;

        for (i, data_point) in self.plot_inner.data.iter().enumerate() {
            if let Some(x_pos) = x_scale.tick(&data_point.date.to_string()) {
                let distance = (x_pos - mouse_x).abs();
                if distance < closest_distance {
                    closest_distance = distance;
                    closest_index = i;
                }
            }
        }

        Some(closest_index)
    }
}

impl Render for BalanceChart {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let plot_inner = self.plot_inner.clone();
        let hovered_index = self.hovered_index;
        let cached_bounds = self.plot_inner.cached_bounds.clone();
        let mouse_position = self.mouse_position;

        div()
            .id("balance_chart")
            .size_full()
            .relative()
            .on_mouse_move(
                cx.listener(move |this, event: &MouseMoveEvent, _window, cx| {
                    // Store mouse position
                    this.mouse_position = Some(event.position);

                    // Get bounds from the shared cell
                    if let Some(bounds) = this.plot_inner.cached_bounds.get() {
                        // Mouse position is relative to chart div, which is what we need
                        let new_index =
                            this.find_nearest_data_point(event.position.x.as_f32(), &bounds);

                        // Only notify if the index actually changed to avoid unnecessary re-renders
                        if new_index != this.hovered_index {
                            this.hovered_index = new_index;
                            cx.notify();
                        }
                    }
                }),
            )
            .child(plot_inner.clone())
            .when_some(hovered_index, |this, idx| {
                // Only render hover elements if index is valid
                if idx < plot_inner.data.len() {
                    this.child(Self::render_hover_elements(
                        &plot_inner,
                        idx,
                        cached_bounds.get(),
                        mouse_position,
                        cx,
                    ))
                } else {
                    this
                }
            })
    }
}

impl BalanceChart {
    /// Renders hover elements including vertical line, markers, and tooltip
    fn render_hover_elements(
        plot_inner: &PlotInner,
        hovered_index: usize,
        cached_bounds: Option<Bounds<Pixels>>,
        mouse_position: Option<Point<Pixels>>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Bounds check
        if hovered_index >= plot_inner.data.len() || cached_bounds.is_none() {
            return div();
        }

        let bounds = cached_bounds.unwrap();
        let data_point = &plot_inner.data[hovered_index];
        let theme = cx.theme();

        let colors = vec![
            theme.chart_1,
            theme.chart_2,
            theme.chart_3,
            theme.chart_4,
            theme.chart_5,
        ];

        // Calculate scales using chart-relative coordinates
        let chart_width = bounds.size.width.as_f32();
        let chart_height = bounds.size.height.as_f32();

        let x_scale = ScalePoint::new(
            plot_inner.data.iter().map(|d| d.date.to_string()).collect(),
            vec![PLOT_PADDING, chart_width - PLOT_PADDING],
        );

        let all_values: Vec<f64> = plot_inner
            .data
            .iter()
            .flat_map(|d| d.balances.iter().map(|(_, v)| *v))
            .chain(std::iter::once(0.0))
            .collect();

        let y_scale = ScaleLinear::new(
            all_values,
            vec![chart_height - AXIS_GAP - PLOT_PADDING, PLOT_PADDING],
        );

        // Get the data point's X position in chart-relative coordinates
        let data_point_x = x_scale
            .tick(&data_point.date.to_string())
            .unwrap_or(PLOT_PADDING);

        // Use actual mouse position for hover line if available
        let hover_line_x = if let Some(mouse_pos) = mouse_position {
            mouse_pos.x.as_f32()
        } else {
            data_point_x
        };

        // Calculate tooltip position
        let tooltip_width_estimate = 220.0;
        let tooltip_x = if hover_line_x < chart_width / 2.0 {
            (hover_line_x + 20.0).min(chart_width - tooltip_width_estimate)
        } else {
            (hover_line_x - tooltip_width_estimate - 20.0).max(PLOT_PADDING)
        };

        div()
            .absolute()
            .left_0()
            .top_0()
            .size_full()
            .child(
                // Vertical hover line (follows mouse)
                div()
                    .absolute()
                    .left(px(hover_line_x - 0.5))
                    .top(px(PLOT_PADDING))
                    .w(px(1.0))
                    .h(px(chart_height - AXIS_GAP - PLOT_PADDING))
                    .bg(theme.border)
                    .opacity(0.6),
            )
            .children(
                // Circle markers on each line (at data point position)
                data_point
                    .balances
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, (_, balance))| {
                        y_scale.tick(balance).map(|y_pos| {
                            let color = colors[idx % CHART_COLORS_COUNT];
                            div()
                                .absolute()
                                .left(px(data_point_x - 5.0))
                                .top(px(y_pos - 5.0))
                                .h(px(10.0))
                                .rounded_full()
                                .bg(color)
                                .border_2()
                                .border_color(theme.background)
                        })
                    }),
            )
            .child(
                // Tooltip
                v_flex()
                    .gap_2()
                    .p_3()
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .shadow_lg()
                    .absolute()
                    .left(px(tooltip_x))
                    .top(px(PLOT_PADDING + 20.0))
                    .min_w(px(180.0))
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(theme.foreground)
                            .child(data_point.date.format("%B %d, %Y").to_string()),
                    )
                    .children(data_point.balances.iter().enumerate().map(
                        |(_idx, (commodity, balance))| {
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("—"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_medium()
                                        .text_color(theme.foreground)
                                        .child(format!("{} ${:.2}", commodity, balance)),
                                )
                        },
                    )),
            )
    }
}

impl Plot for PlotInner {
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        if self.data.is_empty() {
            return;
        }

        // Cache bounds for hover calculations using shared cell
        self.cached_bounds.set(Some(bounds));

        // Calculate drawing area with padding
        let width = bounds.size.width.as_f32() - PLOT_PADDING;
        let height = bounds.size.height.as_f32() - AXIS_GAP - PLOT_PADDING;

        // Create X scale for dates (categorical)
        let date_strings: Vec<String> = self.data.iter().map(|d| d.date.to_string()).collect();
        let x_scale = ScalePoint::new(date_strings.clone(), vec![PLOT_PADDING, width]);

        // Create Y scale for balances (continuous)
        // Include 0 in the domain for proper baseline
        let all_values: Vec<f64> = self
            .data
            .iter()
            .flat_map(|d| d.balances.iter().map(|(_, v)| *v))
            .chain(std::iter::once(0.0))
            .collect();

        // Calculate min/max for Y-axis labels
        let y_min = all_values
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min)
            .min(0.0);
        let y_max = all_values
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
            .max(0.0);

        let y_scale = ScaleLinear::new(all_values, vec![height, PLOT_PADDING]);

        // Create Y-axis labels
        let y_labels: Vec<AxisText> = (0..Y_AXIS_LABEL_COUNT)
            .filter_map(|i| {
                let value = y_min + (y_max - y_min) * i as f64 / (Y_AXIS_LABEL_COUNT - 1) as f64;
                y_scale.tick(&value).map(|tick| {
                    AxisText::new(format!("{:.0}", value), tick, cx.theme().muted_foreground)
                })
            })
            .collect();

        // Create X-axis labels (show every Nth date to avoid crowding)
        let tick_margin = (self.data.len() / MIN_TICK_SPACING).max(1);
        let x_labels: Vec<AxisText> = self
            .data
            .iter()
            .enumerate()
            .filter_map(|(i, d)| {
                if i % tick_margin == 0 {
                    x_scale.tick(&d.date.to_string()).map(|tick| {
                        AxisText::new(
                            d.date.format("%m-%d").to_string(),
                            tick,
                            cx.theme().muted_foreground,
                        )
                    })
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

        // Draw grid lines
        let grid_y_positions: Vec<f32> = (0..=GRID_LINE_COUNT)
            .map(|i| PLOT_PADDING + height * i as f32 / GRID_LINE_COUNT as f32)
            .collect();
        Grid::new()
            .y(grid_y_positions)
            .stroke(cx.theme().border)
            .paint(&bounds, window);

        // Define colors for different commodities
        let theme = cx.theme();
        let colors = vec![
            theme.chart_1,
            theme.chart_2,
            theme.chart_3,
            theme.chart_4,
            theme.chart_5,
        ];

        // Draw a line for each commodity
        for (commodity_idx, _commodity) in self.commodities.iter().enumerate() {
            let color = colors[commodity_idx % CHART_COLORS_COUNT];
            let x_scale_clone = x_scale.clone();
            let y_scale_clone = y_scale.clone();

            Line::new()
                .data(self.data.clone())
                .x(move |d| x_scale_clone.tick(&d.date.to_string()))
                .y(move |d| {
                    // Find the balance for this commodity
                    // Gracefully handle missing data by returning None
                    d.balances
                        .get(commodity_idx)
                        .and_then(|(_, value)| y_scale_clone.tick(value))
                })
                .stroke(color)
                .stroke_width(px(2.0))
                .stroke_style(StrokeStyle::Linear)
                .paint(&bounds, window);
        }
    }
}
