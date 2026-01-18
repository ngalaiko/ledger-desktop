use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    f32::consts::{FRAC_PI_2, SQRT_2, TAU},
    rc::Rc,
};

/// Inner radius as a fraction of outer radius (0.5 = donut with hole half the size)
const INNER_RADIUS_RATIO: f32 = 0.8;

use gpui::prelude::*;
use gpui::{div, MouseMoveEvent};
use gpui::{px, App, Bounds, Context, IntoElement, Pixels, Point, Render, Window};
use gpui_component::{
    h_flex,
    plot::{
        shape::{Arc, Pie},
        tooltip::{Tooltip, TooltipPosition},
        IntoPlot, Plot,
    },
    ActiveTheme, PixelsExt, StyledExt,
};

use crate::view::colors::get_color;

pub struct PieChart {
    plot_inner: PlotInner,
    mouse_position: Option<Point<Pixels>>,
    hovered_idx: Option<usize>,
}

impl PieChart {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            plot_inner: PlotInner::new(),
            mouse_position: None,
            hovered_idx: None,
        }
    }

    pub fn refresh_data(&mut self, values: HashMap<String, f64>, _cx: &mut Context<Self>) {
        self.hovered_idx = None;
        self.plot_inner.set_data(values);
    }
}

impl Render for PieChart {
    #[allow(clippy::too_many_lines)]
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let plot_inner = self.plot_inner.clone();
        let tooltip_data = {
            match (self.mouse_position, plot_inner.bounds.get()) {
                (Some(mouse_position), Some(bounds)) if !plot_inner.data.is_empty() => {
                    Some((mouse_position, bounds))
                }
                _ => None,
            }
        };

        // Get hovered data if mouse is over a segment
        let hovered_data = tooltip_data.and_then(|(mouse_pos, bounds)| {
            let idx = self.plot_inner.get_hovered_index(mouse_pos, &bounds)?;
            let (label, value) = self.plot_inner.data.get(idx)?;
            let color = get_color(cx, &label);
            let percentage = if self.plot_inner.total > 0.0 {
                (*value / self.plot_inner.total) * 100.0
            } else {
                0.0
            };
            Some((label.clone(), *value, color, percentage))
        });

        div()
            .id("pie_chart")
            .size_full()
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                this.mouse_position = Some(event.position);

                if let (Some(mouse_pos), Some(bounds)) =
                    (this.mouse_position, this.plot_inner.bounds.get())
                {
                    if !this.plot_inner.data.is_empty() {
                        let new_hovered_idx = this.plot_inner.get_hovered_index(mouse_pos, &bounds);

                        if this.hovered_idx != new_hovered_idx {
                            this.hovered_idx = new_hovered_idx;
                            cx.notify();
                        }
                    }
                } else {
                    cx.notify();
                }
            }))
            .child(plot_inner.clone())
            // Center total display
            .when_some(plot_inner.bounds.get(), |this, bounds| {
                if plot_inner.data.is_empty() {
                    return this;
                }

                let inner_radius = bounds.size.height.as_f32() * 0.4 * INNER_RADIUS_RATIO;
                let square_side = px(inner_radius * SQRT_2);

                this.child(
                    div()
                        .absolute()
                        .w(bounds.size.width)
                        .h(bounds.size.height)
                        .left(px(0.0))
                        .top(px(0.0))
                        .flex()
                        .justify_center()
                        .items_center()
                        .child(
                            div()
                                .w(square_side)
                                .h(square_side)
                                .overflow_hidden()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .child(div().text_lg().child(format!("{:.2}", plot_inner.total))),
                        ),
                )
            })
            // Tooltip on hover
            .when_some(hovered_data, |this, (label, value, color, percentage)| {
                let position = if self.mouse_position.map_or(0.0, |p| p.x.as_f32())
                    < self
                        .plot_inner
                        .bounds
                        .get()
                        .map_or(0.0, |b| b.size.width.as_f32() / 2.0)
                {
                    TooltipPosition::Right
                } else {
                    TooltipPosition::Left
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
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(div().text_xs().text_color(color).child("●"))
                                .child(div().text_sm().font_semibold().child(label)),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .text_sm()
                                .child(format!("{value:.2}"))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("({percentage:.1}%)")),
                                ),
                        ),
                )
            })
    }
}

#[derive(IntoPlot, Clone)]
struct PlotInner {
    data: Vec<(String, f64)>,
    total: f64,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    cached_arcs: Rc<RefCell<Vec<CachedArc>>>,
}

#[derive(Clone)]
struct CachedArc {
    start_angle: f32,
    end_angle: f32,
}

impl PlotInner {
    pub fn new() -> Self {
        Self {
            data: vec![],
            total: 0.0,
            bounds: Rc::new(Cell::new(None)),
            cached_arcs: Rc::new(RefCell::new(vec![])),
        }
    }

    pub fn set_data(&mut self, values: HashMap<String, f64>) {
        // Convert to sorted vec for consistent ordering
        let mut data: Vec<(String, f64)> = values.into_iter().filter(|(_, v)| *v > 0.0).collect();
        data.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        self.total = data.iter().map(|(_, v)| v).sum();
        self.data = data;
        self.cached_arcs.replace(vec![]);
    }

    fn get_hovered_index(
        &self,
        mouse_pos: Point<Pixels>,
        bounds: &Bounds<Pixels>,
    ) -> Option<usize> {
        if self.data.is_empty() {
            return None;
        }

        let center_x = bounds.origin.x.as_f32() + bounds.size.width.as_f32() / 2.0;
        let center_y = bounds.origin.y.as_f32() + bounds.size.height.as_f32() / 2.0;
        let outer_radius = bounds.size.height.as_f32() * 0.4;
        let inner_radius = outer_radius * INNER_RADIUS_RATIO;

        let dx = mouse_pos.x.as_f32() - center_x;
        let dy = mouse_pos.y.as_f32() - center_y;

        // Check if mouse is within the pie ring
        let distance = (dx * dx + dy * dy).sqrt();
        if distance > outer_radius || distance < inner_radius {
            return None;
        }

        // Calculate angle from center (adjust to match Pie's coordinate system)
        let mut angle = dy.atan2(dx) + FRAC_PI_2;
        if angle < 0.0 {
            angle += TAU;
        }

        // Find which arc contains this angle
        let cached_arcs = self.cached_arcs.borrow();
        for (i, arc) in cached_arcs.iter().enumerate() {
            if angle >= arc.start_angle && angle < arc.end_angle {
                return Some(i);
            }
        }

        None
    }
}

impl Plot for PlotInner {
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        if self.data.is_empty() {
            return;
        }

        self.bounds.set(Some(bounds));

        let outer_radius = bounds.size.height.as_f32() * 0.4;
        let inner_radius = outer_radius * INNER_RADIUS_RATIO;

        let arc = Arc::new()
            .inner_radius(inner_radius)
            .outer_radius(outer_radius);

        #[allow(clippy::cast_possible_truncation)]
        let pie = Pie::<(String, f64)>::new().value(|(_, v)| Some(*v as f32));
        let arcs = pie.arcs(&self.data);

        // Cache arc angles for hover detection
        let cached: Vec<CachedArc> = arcs
            .iter()
            .map(|a| CachedArc {
                start_angle: a.start_angle,
                end_angle: a.end_angle,
            })
            .collect();
        self.cached_arcs.replace(cached);

        // Paint each arc
        for a in &arcs {
            let color = get_color(cx, &a.data.0);
            arc.paint(
                a,
                color,
                Some(inner_radius),
                Some(outer_radius),
                &bounds,
                window,
            );
        }
    }
}
