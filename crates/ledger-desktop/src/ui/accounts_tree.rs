#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::{
    h_flex,
    list::ListItem,
    tree::{tree, TreeItem, TreeState},
    IconName,
};

use ledger::{Account, TreeNode};
use state::AppState;

use super::components::checkbox::{Checkbox, CheckboxState};
use super::ledger_state::LedgerState;

pub struct AccountsTreeView {
    tree_state: Entity<TreeState>,
    state: Entity<LedgerState>,
}

impl AccountsTreeView {
    pub fn new(state: Entity<LedgerState>, cx: &mut Context<Self>) -> Self {
        let tree_state = cx.new(|cx| TreeState::new(cx));

        // Rebuild tree items with accounts when the state changes
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
            state: state.clone(),
        }
    }

    /// Calculate the checkbox state for a node based on its children
    fn calculate_state(
        &self,
        node: &TreeNode,
        account: &Account,
        cx: &mut Context<Self>,
    ) -> CheckboxState {
        // Find the node in the tree
        let target_node = node.find_node(account);
        let selected_accounts = AppState::get_selected_accounts(cx);

        if let Some(node) = target_node {
            if node.children.is_empty() {
                // Leaf node: just check if it's selected
                if selected_accounts.contains(&node.account) {
                    CheckboxState::Checked
                } else {
                    CheckboxState::Unchecked
                }
            } else {
                // Parent node: check children only (not the parent itself)
                let all_descendants: Vec<Account> = node
                    .children
                    .iter()
                    .flat_map(|child| child.collect_all_accounts())
                    .collect();
                let selected_count = all_descendants
                    .iter()
                    .filter(|a| selected_accounts.contains(a))
                    .count();

                if selected_count == 0 {
                    CheckboxState::Unchecked
                } else if selected_count == all_descendants.len() {
                    CheckboxState::Checked
                } else {
                    CheckboxState::Indeterminate
                }
            }
        } else {
            CheckboxState::Unchecked
        }
    }

    fn toggle_selection(&mut self, node: &TreeNode, account: Account, cx: &mut Context<Self>) {
        let state = self.calculate_state(node, &account, cx);

        // Get all descendants (including the account itself)
        let mut descendants = node.get_descendants(&account);
        if descendants.is_empty() {
            descendants = vec![account.clone()];
        }

        match state {
            CheckboxState::Unchecked => {
                let mut selected_accounts = AppState::get_selected_accounts(cx);
                // Check all descendants
                for descendant in descendants {
                    selected_accounts.insert(descendant);
                }
                AppState::update_selected_accounts(selected_accounts, cx);
            }
            CheckboxState::Checked | CheckboxState::Indeterminate => {
                let mut selected_accounts = AppState::get_selected_accounts(cx);
                for descendant in descendants {
                    selected_accounts.remove(&descendant);
                }
                AppState::update_selected_accounts(selected_accounts, cx);
            }
        }

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
            let state_entity = self.state.clone();
            move |ix, entry, _selected, _window, cx| {
                view.update(cx, |this, cx| {
                    let item = entry.item();
                    let account = Account::parse(&item.id);

                    // Get the tree node to calculate state
                    let tree_node = &state_entity.read(cx).accounts.clone();
                    let checkbox_state = this.calculate_state(tree_node, &account, cx);

                    let with_checkbox = div()
                        .flex()
                        .size_full()
                        .justify_between()
                        .items_center()
                        .child(item.label.clone())
                        .child(
                            div()
                                .child({
                                    let item_id = item.id.clone();
                                    let view = view.clone();
                                    let state_entity = state_entity.clone();
                                    Checkbox::new(item.id.clone())
                                        .state(checkbox_state)
                                        .on_click(move |_new_state, _window, cx| {
                                            let account = Account::parse(&item_id);
                                            view.update(cx, |this, cx| {
                                                let tree_node =
                                                    state_entity.read(cx).accounts.clone();
                                                this.toggle_selection(&tree_node, account, cx);
                                            });
                                        })
                                })
                                // note: this has to be here to prevent list item from
                                // toggling on click
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|_, _, _, cx| {
                                        cx.stop_propagation();
                                    }),
                                ),
                        );

                    let with_icon = if !entry.is_folder() {
                        h_flex().gap_2().pl(px(24.)).child(with_checkbox)
                    } else if entry.is_expanded() {
                        h_flex()
                            .gap_2()
                            .child(IconName::ChevronDown)
                            .child(with_checkbox)
                    } else {
                        h_flex()
                            .gap_2()
                            .child(IconName::ChevronRight)
                            .child(with_checkbox)
                    };

                    let is_any_selected = !matches!(checkbox_state, CheckboxState::Unchecked);
                    ListItem::new(ix)
                        .selected(is_any_selected)
                        .pl(px(16.) * entry.depth() + px(12.))
                        .child(with_icon)
                })
            }
        })
    }
}
