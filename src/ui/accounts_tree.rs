#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::{
    h_flex,
    list::ListItem,
    tree::{tree, TreeItem, TreeState},
    IconName,
};

use crate::accounts::{Account, TreeNode};

use super::state::State;

pub struct AccountsTreeView {
    tree_state: Entity<TreeState>,
    pub selected_account: Option<Account>,
}

impl AccountsTreeView {
    pub fn new(state: Entity<State>, cx: &mut Context<Self>) -> Self {
        let tree_state = cx.new(|cx| TreeState::new(cx));

        cx.observe(&state, |this, state, cx| {
            let tree_items = build_items(&state.read(cx).accounts);
            this.tree_state.update(cx, |tree_state, cx| {
                tree_state.set_items(tree_items, cx);
                cx.notify();
            });
            cx.notify();
        })
        .detach();

        Self {
            tree_state,
            selected_account: None,
        }
    }

    fn handle_selection(&mut self, item: TreeItem, cx: &mut Context<Self>) {
        self.selected_account = Some(Account::parse(&item.id));
        cx.notify();
    }
}

fn build_items(node: &TreeNode) -> Vec<TreeItem> {
    let mut items = Vec::new();

    for child in &node.children {
        let mut item = TreeItem::new(child.account.to_string(), child.account.name().to_string());

        if !child.children.is_empty() {
            item = item.expanded(false);
            for sub_child in build_items(child) {
                item = item.child(sub_child);
            }
        }

        items.push(item);
    }

    items
}

impl Render for AccountsTreeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        tree(&self.tree_state, {
            let view = cx.entity();
            move |ix, entry, selected, _window, cx| {
                view.update(cx, |_this, cx| {
                    let item = entry.item();
                    let icon = if !entry.is_folder() {
                        None
                    } else if entry.is_expanded() {
                        Some(IconName::ChevronDown)
                    } else {
                        Some(IconName::ChevronRight)
                    };

                    let label = if let Some(icon) = icon {
                        h_flex().gap_2().child(icon).child(item.label.clone())
                    } else {
                        div().pl(px(24.)).child(item.label.clone())
                    };

                    let list_item = ListItem::new(ix)
                        .selected(selected)
                        .pl(px(16.) * entry.depth() + px(12.))
                        .child(label);

                    if entry.is_folder() {
                        list_item
                    } else {
                        list_item.on_click(cx.listener({
                            let item = entry.item().clone();
                            move |this, _, _, cx| {
                                this.handle_selection(item.clone(), cx);
                            }
                        }))
                    }
                })
            }
        })
    }
}
