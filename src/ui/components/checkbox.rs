/// This is a copy of https://github.com/longbridge/gpui-component/blob/v0.5.0/crates/ui/src/checkbox.rs
/// with Indeterminate state support

use std::{rc::Rc, time::Duration};

use gpui::{
    div, prelude::FluentBuilder as _, px, relative, rems, svg, Animation, AnimationExt, AnyElement,
    App, Div, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, StyleRefinement, Styled, Window,
};
use gpui_component::{
    text::Text, v_flex, ActiveTheme, Disableable, IconName, IconNamed, Selectable, Sizable, Size,
    StyledExt as _,
};

/// The state of a checkbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckboxState {
    /// The checkbox is unchecked.
    Unchecked,
    /// The checkbox is in an indeterminate state (halfway between checked and unchecked).
    Indeterminate,
    /// The checkbox is checked.
    Checked,
}

impl CheckboxState {
    /// Returns true if the checkbox is checked.
    pub fn is_checked(&self) -> bool {
        matches!(self, CheckboxState::Checked)
    }

    /// Returns true if the checkbox is indeterminate.
    pub fn is_indeterminate(&self) -> bool {
        matches!(self, CheckboxState::Indeterminate)
    }

    /// Returns true if the checkbox is unchecked.
    pub fn is_unchecked(&self) -> bool {
        matches!(self, CheckboxState::Unchecked)
    }
}

impl Default for CheckboxState {
    fn default() -> Self {
        CheckboxState::Unchecked
    }
}

impl From<bool> for CheckboxState {
    fn from(checked: bool) -> Self {
        if checked {
            CheckboxState::Checked
        } else {
            CheckboxState::Unchecked
        }
    }
}

/// A Checkbox element.
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    base: Div,
    style: StyleRefinement,
    label: Option<Text>,
    children: Vec<AnyElement>,
    state: CheckboxState,
    disabled: bool,
    size: Size,
    tab_stop: bool,
    tab_index: isize,
    on_click: Option<Rc<dyn Fn(CheckboxState, &mut Window, &mut App) + 'static>>,
}

impl Checkbox {
    /// Create a new Checkbox with the given id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            base: div(),
            style: StyleRefinement::default(),
            label: None,
            children: Vec::new(),
            state: CheckboxState::default(),
            disabled: false,
            size: Size::default(),
            on_click: None,
            tab_stop: true,
            tab_index: 0,
        }
    }

    /// Set the label for the checkbox.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the state for the checkbox.
    pub fn state(mut self, state: CheckboxState) -> Self {
        self.state = state;
        self
    }

    /// Set the checked state for the checkbox (convenience method).
    pub fn checked(mut self, checked: bool) -> Self {
        self.state = checked.into();
        self
    }

    /// Set the checkbox to indeterminate state.
    pub fn indeterminate(mut self) -> Self {
        self.state = CheckboxState::Indeterminate;
        self
    }

    /// Set the click handler for the checkbox.
    ///
    /// The `CheckboxState` parameter indicates the new state after the click.
    pub fn on_click(mut self, handler: impl Fn(CheckboxState, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Set the tab stop for the checkbox, default is true.
    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }

    /// Set the tab index for the checkbox, default is 0.
    pub fn tab_index(mut self, tab_index: isize) -> Self {
        self.tab_index = tab_index;
        self
    }

    fn handle_click(
        on_click: &Option<Rc<dyn Fn(CheckboxState, &mut Window, &mut App) + 'static>>,
        state: CheckboxState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Toggle between unchecked and checked, skipping indeterminate
        // If indeterminate, clicking will make it checked
        let new_state = match state {
            CheckboxState::Unchecked => CheckboxState::Checked,
            CheckboxState::Indeterminate => CheckboxState::Checked,
            CheckboxState::Checked => CheckboxState::Unchecked,
        };
        if let Some(f) = on_click {
            (f)(new_state, window, cx);
        }
    }
}

impl InteractiveElement for Checkbox {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.base.interactivity()
    }
}
impl StatefulInteractiveElement for Checkbox {}

impl Styled for Checkbox {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        &mut self.style
    }
}

impl Disableable for Checkbox {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Selectable for Checkbox {
    fn selected(self, selected: bool) -> Self {
        self.checked(selected)
    }

    fn is_selected(&self) -> bool {
        self.state.is_checked()
    }
}

impl ParentElement for Checkbox {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Sizable for Checkbox {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

pub(crate) fn checkbox_check_icon(
    id: ElementId,
    size: Size,
    state: CheckboxState,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let toggle_state = window.use_keyed_state(id, cx, |_, _| state);
    let color = if disabled {
        cx.theme().primary_foreground.opacity(0.5)
    } else {
        cx.theme().primary_foreground
    };

    svg()
        .absolute()
        .top_px()
        .left_px()
        .map(|this| match size {
            Size::XSmall => this.size_2(),
            Size::Small => this.size_2p5(),
            Size::Medium => this.size_3(),
            Size::Large => this.size_3p5(),
            _ => this.size_3(),
        })
        .text_color(color)
        .map(|this| match state {
            CheckboxState::Checked => this.path(IconName::Check.path()),
            CheckboxState::Indeterminate => this.path(IconName::Dash.path()),
            CheckboxState::Unchecked => this,
        })
        .map(|this| {
            if !disabled && state != *toggle_state.read(cx) {
                let duration = Duration::from_secs_f64(0.25);
                cx.spawn({
                    let toggle_state = toggle_state.clone();
                    async move |cx| {
                        cx.background_executor().timer(duration).await;
                        _ = toggle_state.update(cx, |this, _| *this = state);
                    }
                })
                .detach();

                this.with_animation(
                    ElementId::NamedInteger("toggle".into(), state as u64),
                    Animation::new(Duration::from_secs_f64(0.25)),
                    move |this, delta| {
                        let show = !matches!(state, CheckboxState::Unchecked);
                        this.opacity(if show { 1.0 * delta } else { 1.0 - delta })
                    },
                )
                .into_any_element()
            } else {
                this.into_any_element()
            }
        })
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state;

        let focus_handle = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let is_focused = focus_handle.is_focused(window);

        let is_active = !matches!(state, CheckboxState::Unchecked);
        let border_color = if is_active {
            cx.theme().primary
        } else {
            cx.theme().input
        };
        let color = if self.disabled {
            border_color.opacity(0.5)
        } else {
            border_color
        };
        let radius = cx.theme().radius.min(px(4.));

        div().child(
            self.base
                .id(self.id.clone())
                .when(!self.disabled, |this| {
                    this.track_focus(
                        &focus_handle
                            .tab_stop(self.tab_stop)
                            .tab_index(self.tab_index),
                    )
                })
                .h_flex()
                .gap_2()
                .items_start()
                .line_height(relative(1.))
                .text_color(cx.theme().foreground)
                .map(|this| match self.size {
                    Size::XSmall => this.text_xs(),
                    Size::Small => this.text_sm(),
                    Size::Medium => this.text_base(),
                    Size::Large => this.text_lg(),
                    _ => this,
                })
                .when(self.disabled, |this| {
                    this.text_color(cx.theme().muted_foreground)
                })
                .rounded(cx.theme().radius * 0.5)
                .when(is_focused, |this| {
                    this.border_2().border_color(cx.theme().ring)
                })
                .refine_style(&self.style)
                .child(
                    div()
                        .relative()
                        .map(|this| match self.size {
                            Size::XSmall => this.size_3(),
                            Size::Small => this.size_3p5(),
                            Size::Medium => this.size_4(),
                            Size::Large => this.size(rems(1.125)),
                            _ => this.size_4(),
                        })
                        .flex_shrink_0()
                        .border_1()
                        .border_color(color)
                        .rounded(radius)
                        .when(cx.theme().shadow && !self.disabled, |this| this.shadow_xs())
                        .map(|this| {
                            if is_active {
                                this.bg(color)
                            } else {
                                this.bg(cx.theme().background)
                            }
                        })
                        .child(checkbox_check_icon(
                            self.id,
                            self.size,
                            state,
                            self.disabled,
                            window,
                            cx,
                        )),
                )
                .when(self.label.is_some() || !self.children.is_empty(), |this| {
                    this.child(
                        v_flex()
                            .w_full()
                            .line_height(relative(1.2))
                            .gap_1()
                            .map(|this| {
                                if let Some(label) = self.label {
                                    this.child(
                                        div()
                                            .size_full()
                                            .text_color(cx.theme().foreground)
                                            .when(self.disabled, |this| {
                                                this.text_color(cx.theme().muted_foreground)
                                            })
                                            .line_height(relative(1.))
                                            .child(label),
                                    )
                                } else {
                                    this
                                }
                            })
                            .children(self.children),
                    )
                })
                .on_mouse_down(gpui::MouseButton::Left, |_, window, _| {
                    // Avoid focus on mouse down.
                    window.prevent_default();
                })
                .when(!self.disabled, |this| {
                    this.on_click({
                        let on_click = self.on_click.clone();
                        move |_, window, cx| {
                            window.prevent_default();
                            Self::handle_click(&on_click, state, window, cx);
                        }
                    })
                }),
        )
    }
}
