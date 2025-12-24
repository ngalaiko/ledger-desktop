use std::collections::HashMap;
use std::ops::Range;

#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::list::ListItem;
use gpui_component::tree::TreeItem;
use gpui_component::{h_flex, Icon, IconName, Sizable};

/// Events emitted by DropdownTreeState
#[derive(Clone)]
pub enum DropdownTreeEvent {
    /// An item was selected
    Selected { entry: DropdownTreeEntry },
}

/// A tree entry with depth information
#[derive(Clone)]
pub struct DropdownTreeEntry {
    item: TreeItem,
    depth: usize,
}

impl DropdownTreeEntry {
    pub fn item(&self) -> &TreeItem {
        &self.item
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    pub fn is_folder(&self) -> bool {
        !self.item.children.is_empty()
    }
}

/// State for managing dropdown tree items with separate toggle and selection actions
///
/// # Events
/// This component emits `DropdownTreeEvent::Selected` when an item is selected.
///
/// # Example
/// ```ignore
/// let tree_state = cx.new(|cx| DropdownTreeState::new(cx));
///
/// cx.subscribe(&tree_state, |this, _tree_state, event, cx| {
///     match event {
///         DropdownTreeEvent::Selected{ entry } => {
///             println!("Selected: {}", entry.item().label);
///         }
///     }
/// });
/// ```
pub struct DropdownTreeState {
    focus_handle: FocusHandle,
    root_items: Vec<TreeItem>,
    entries: Vec<DropdownTreeEntry>,
    expanded: HashMap<SharedString, bool>,
    scroll_handle: UniformListScrollHandle,
    selected_ix: Option<usize>,
}

impl DropdownTreeState {
    pub fn new(cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            root_items: Vec::new(),
            entries: Vec::new(),
            expanded: HashMap::new(),
            scroll_handle: UniformListScrollHandle::default(),
            selected_ix: None,
        }
    }

    /// Set the tree items
    pub fn set_items(&mut self, items: impl Into<Vec<TreeItem>>, cx: &mut Context<Self>) {
        let items = items.into();

        // Initialize expanded state from TreeItem's initial expanded state
        for item in &items {
            self.init_expanded_state(item);
        }

        self.root_items = items;
        self.rebuild_entries();
        self.selected_ix = None;
        cx.notify();
    }

    fn init_expanded_state(&mut self, item: &TreeItem) {
        // Only set if not already in the map (preserve user's toggle state)
        if !self.expanded.contains_key(&item.id) {
            self.expanded.insert(item.id.clone(), item.is_expanded());
        }
        // Recursively init children
        for child in &item.children {
            self.init_expanded_state(child);
        }
    }

    /// Get the currently selected index
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_ix
    }

    /// Set the selected index and emit a Selected event
    pub fn set_selected_index(&mut self, ix: Option<usize>, cx: &mut Context<Self>) {
        self.selected_ix = ix;
        if let Some(ix) = ix {
            self.scroll_handle
                .scroll_to_item(ix, ScrollStrategy::Center);

            // Emit the Selected event
            if let Some(entry) = self.entries.get(ix) {
                cx.emit(DropdownTreeEvent::Selected {
                    entry: entry.clone(),
                });
            }
        }
        cx.notify();
    }

    /// Get the currently selected entry
    pub fn selected_entry(&self) -> Option<&DropdownTreeEntry> {
        self.selected_ix.and_then(|ix| self.entries.get(ix))
    }

    /// Check if an item is expanded
    pub fn is_expanded(&self, item_id: &SharedString) -> bool {
        self.expanded.get(item_id).copied().unwrap_or(false)
    }

    /// Toggle the expanded state of an item at the given index
    pub fn toggle_expanded(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.get(ix) else {
            return;
        };
        if !entry.is_folder() {
            return;
        }

        let item_id = &entry.item.id;
        let current = self.is_expanded(item_id);
        self.expanded.insert(item_id.clone(), !current);

        self.rebuild_entries();
        cx.notify();
    }

    fn rebuild_entries(&mut self) {
        self.entries.clear();
        let root_items = self.root_items.clone();
        for item in root_items {
            self.add_entry(item, 0);
        }
    }

    fn add_entry(&mut self, item: TreeItem, depth: usize) {
        self.entries.push(DropdownTreeEntry {
            item: item.clone(),
            depth,
        });

        // Add children if this item is expanded
        if self.is_expanded(&item.id) && !item.children.is_empty() {
            for child in &item.children {
                self.add_entry(child.clone(), depth + 1);
            }
        }
    }
}

impl EventEmitter<DropdownTreeEvent> for DropdownTreeState {}

impl Render for DropdownTreeState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("dropdown-tree-state").size_full()
    }
}

/// Renders a dropdown tree with expandable/collapsible nodes using chevron icons.
///
/// This component provides:
/// - Chevron icons (→ collapsed, ↓ expanded) that can be clicked to toggle expansion
/// - Labels that can be clicked to select the item (without toggling)
/// - Proper indentation based on tree depth
///
/// # Arguments
/// * `tree_state` - The dropdown tree state entity managing the tree structure
/// * `render_label` - Function to render the label content for each entry
pub fn dropdown_tree(
    tree_state: &Entity<DropdownTreeState>,
    render_label: impl Fn(&DropdownTreeEntry, &Window, &mut App) -> AnyElement + 'static,
) -> DropdownTree {
    DropdownTree::new(tree_state, render_label)
}

#[derive(IntoElement)]
pub struct DropdownTree {
    tree_state: Entity<DropdownTreeState>,
    render_label: std::rc::Rc<dyn Fn(&DropdownTreeEntry, &Window, &mut App) -> AnyElement>,
}

impl DropdownTree {
    pub fn new(
        tree_state: &Entity<DropdownTreeState>,
        render_label: impl Fn(&DropdownTreeEntry, &Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self {
            tree_state: tree_state.clone(),
            render_label: std::rc::Rc::new(render_label),
        }
    }
}

impl RenderOnce for DropdownTree {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let tree_state_clone = self.tree_state.clone();
        let render_label = self.render_label.clone();

        let entry_count = self.tree_state.read(cx).entries.len();
        let scroll_handle = self.tree_state.read(cx).scroll_handle.clone();

        div().id("dropdown-tree").size_full().child(
            uniform_list(
                "dropdown-tree-entries",
                entry_count,
                move |visible_range: Range<usize>, window, cx| {
                    // Collect all data we need from state first
                    let entries_data: Vec<_> = {
                        let state = tree_state_clone.read(cx);
                        visible_range
                            .clone()
                            .filter_map(|ix| {
                                let entry = state.entries.get(ix)?;
                                let depth = entry.depth();
                                let is_folder = entry.is_folder();
                                let is_expanded = state.is_expanded(&entry.item.id);
                                let selected = Some(ix) == state.selected_ix;
                                Some((ix, entry.clone(), depth, is_folder, is_expanded, selected))
                            })
                            .collect()
                    };

                    let mut items = Vec::with_capacity(entries_data.len());

                    for (ix, entry_clone, depth, is_folder, is_expanded, selected) in entries_data {
                        let tree_state_for_chevron = tree_state_clone.clone();
                        let tree_state_for_label = tree_state_clone.clone();
                        let render_label = render_label.clone();

                        // Build the content with indentation, chevron, and label
                        let mut content = h_flex().gap_2().items_center().w_full();

                        // Add indentation based on depth
                        if depth > 0 {
                            content = content.pl(px(depth as f32 * 16.0));
                        }

                        // Add chevron icon for folders with separate click handler
                        if is_folder {
                            let chevron_icon = if is_expanded {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            };

                            let chevron = div()
                                .id(("chevron", ix))
                                .cursor_pointer()
                                .child(Icon::new(chevron_icon).small().text_color(gpui::white()))
                                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                    tree_state_for_chevron.update(cx, |state, cx| {
                                        state.toggle_expanded(ix, cx);
                                    });
                                    cx.stop_propagation();
                                });

                            content = content.child(chevron);
                        } else {
                            // Add spacing for non-folder items to align with folders
                            content = content.child(div().w(px(20.0)));
                        }

                        // Add the label with separate click handler for selection
                        let label_element = render_label(&entry_clone, window, cx);
                        let label = div()
                            .id(("label", ix))
                            .flex_grow()
                            .cursor_pointer()
                            .child(label_element)
                            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                tree_state_for_label.update(cx, |state, cx| {
                                    state.set_selected_index(Some(ix), cx);
                                });
                                cx.stop_propagation();
                            });

                        content = content.child(label);

                        let item = ListItem::new(ix).selected(selected).child(content);

                        items.push(div().id(ix).child(item));
                    }

                    items
                },
            )
            .flex_grow()
            .size_full()
            .track_scroll(scroll_handle)
            .with_sizing_behavior(ListSizingBehavior::Auto),
        )
    }
}
