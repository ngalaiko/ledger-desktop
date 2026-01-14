use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    f32::consts::{PI, TAU},
    hash::Hash,
    rc::Rc,
};

const HALF_PI: f32 = PI / 2.0;
/// Inner radius as a fraction of outer radius (0.5 = donut with hole half the size)
const INNER_RADIUS_RATIO: f32 = 0.8;

use gpui::prelude::*;
use gpui::{div, MouseMoveEvent};
use gpui::{px, App, Bounds, Context, Hsla, IntoElement, Pixels, Point, Render, Window};
use gpui_component::{
    h_flex,
    plot::{
        shape::{Arc, Pie},
        IntoPlot, Plot,
    },
    ActiveTheme, PixelsExt, StyledExt,
};

pub struct PieChart {
    plot_inner: PlotInner,
    mouse_position: Option<Point<Pixels>>,
    hovered_idx: Option<usize>,
    colors: Vec<Hsla>,
}

impl PieChart {
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

    pub fn refresh_data(&mut self, values: HashMap<String, f64>, _cx: &mut Context<Self>) {
        self.hovered_idx = None;
        self.plot_inner.set_data(values);
    }
}

impl Render for PieChart {
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
            .child(plot_inner)
            .when_some(tooltip_data, |this, (mouse_pos, bounds)| {
                let hovered_idx = self.plot_inner.get_hovered_index(mouse_pos, &bounds);

                let Some(hovered_idx) = hovered_idx else {
                    return this;
                };

                let Some((label, value)) = self.plot_inner.data.get(hovered_idx) else {
                    return this;
                };

                let color = get_color(&self.colors, label);
                let percentage = if self.plot_inner.total > 0.0 {
                    (*value / self.plot_inner.total) * 100.0
                } else {
                    0.0
                };

                this.child(
                    // full overlay div for centering
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
                                .flex()
                                .flex_col()
                                .items_center()
                                .child(
                                    h_flex()
                                        .gap_1()
                                        .items_center()
                                        .child(div().text_xs().text_color(color).child("●"))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_medium()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(label.to_string()),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_lg()
                                        .font_semibold()
                                        .child(format!("{:.2}", value)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("{:.1}%", percentage)),
                                ),
                        ),
                )
            })
    }
}

#[derive(IntoPlot, Clone)]
struct PlotInner {
    colors: Vec<Hsla>,
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
    pub fn new(colors: Vec<Hsla>) -> Self {
        Self {
            colors,
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
        let mut angle = dy.atan2(dx) + HALF_PI;
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
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, _cx: &mut App) {
        if self.data.is_empty() {
            return;
        }

        self.bounds.set(Some(bounds));

        let outer_radius = bounds.size.height.as_f32() * 0.4;
        let inner_radius = outer_radius * INNER_RADIUS_RATIO;

        let arc = Arc::new()
            .inner_radius(inner_radius)
            .outer_radius(outer_radius);

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
            let color = get_color(&self.colors, &a.data.0);
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

fn get_color(colors: &[Hsla], label: &String) -> Hsla {
    let hash = {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        label.hash(&mut hasher);
        hasher.finish()
    };
    let color_idx = (hash as usize) % colors.len();
    colors[color_idx]
}
