use core::f64;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    hash::Hash,
    rc::Rc,
};

use gpui::prelude::*;
use gpui::{div, point, MouseMoveEvent};
use gpui::{px, App, Bounds, Context, Hsla, IntoElement, Pixels, Point, Render, Window};
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

// Constants for chart layout
/// Padding around the plot area in pixels
const PLOT_PADDING: f32 = 10.0;
/// Gap reserved for axis labels
const AXIS_GAP: f32 = 30.0;
/// Minimum number of data points before skipping ticks on X-axis
const MIN_TICK_SPACING: usize = 10;
/// Number of Y-axis value labels to display
const Y_AXIS_LABEL_COUNT: usize = 5;

pub struct LineChart {
    plot_inner: PlotInner,
    mouse_position: Option<Point<Pixels>>,
    hovered_idx: Option<usize>,
    colors: Vec<Hsla>,
}

impl LineChart {
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
            hovered_idx: None,
            colors,
        }
    }

    pub fn refresh_data(
        &mut self,
        dates: &[chrono::NaiveDate],
        values: HashMap<String, Vec<Option<f64>>>,
        _cx: &mut Context<Self>,
    ) {
        self.hovered_idx = None;
        self.plot_inner.set_data(dates.to_vec(), values);
    }
}

impl Render for LineChart {
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

                // Create CrossLine for the vertical crosshair
                let cross_line = CrossLine::new(point(px(x_pos), px(mouse_y)));

                let values = self
                    .plot_inner
                    .values
                    .iter()
                    .filter_map(|(commodity, values_vec)| {
                        values_vec[hovered_idx].map(|v| (commodity, v))
                    })
                    .collect::<Vec<_>>();

                // Create Dot components for each data point
                let dots: Vec<Dot> = values
                    .iter()
                    .filter_map(|(commodity, value)| {
                        let color = get_color(&self.colors, commodity);
                        y_scale.tick(value).map(|y_pos| {
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
                        .children(values.iter().map(|(commodity, value)| {
                            let color = get_color(&self.colors, commodity);

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
                                        .child((*commodity).clone())
                                        .child(value.to_string()),
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
    values: HashMap<String, Vec<Option<f64>>>,

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
            values: HashMap::new(),
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
        values: HashMap<String, Vec<Option<f64>>>,
    ) {
        self.dates = dates;
        self.values = values;

        let mut min_balance = f64::MAX;
        let mut max_balance = f64::MIN;

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
        #[allow(clippy::cast_precision_loss)]
        let y_labels: Vec<AxisText> = (0..Y_AXIS_LABEL_COUNT)
            .filter_map(|i| {
                let value = self.y_min
                    + (self.y_max - self.y_min) * i as f64 / (Y_AXIS_LABEL_COUNT - 1) as f64;
                y_scale.tick(&value).map(|tick| {
                    AxisText::new(format!("{value:.0}"), tick, cx.theme().muted_foreground)
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
                        .tick(d)
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

        // Draw a line for each label
        for (label, values) in &self.values {
            let color = get_color(&self.colors, label);
            let x_scale = x_scale.clone();
            let y_scale = y_scale.clone();

            Line::new()
                .data(self.dates.iter().zip(values.iter()))
                .x(move |d| x_scale.tick(d.0))
                .y(move |d| d.1.and_then(|value| y_scale.tick(&value)))
                .stroke(color)
                .stroke_width(px(1.0))
                .stroke_style(StrokeStyle::Linear)
                .paint(&bounds, window);
        }
    }
}

fn get_color(colors: &[Hsla], commodity: &str) -> Hsla {
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
