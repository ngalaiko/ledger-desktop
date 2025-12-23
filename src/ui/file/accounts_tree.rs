#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::tree::TreeItem;

use crate::accounts::{self, TreeNode};
use crate::ui::dropdown_tree::{dropdown_tree, DropdownTreeState};

use super::state::{LedgerState, StateUpdatedEvent};

pub struct AccountsTree {
    tree: accounts::TreeNode,
    accounts_tree: Entity<DropdownTreeState>,
}

impl AccountsTree {
    pub fn new(ledger_state: Entity<LedgerState>, cx: &mut Context<Self>) -> Self {
        let accounts_tree = cx.new(|cx| DropdownTreeState::new(cx));

        cx.subscribe(
            &ledger_state,
            |this, _ledger_state, event, cx| match event {
                StateUpdatedEvent::NewTransaction { .. } => {
                    // No action needed for new transactions
                }
                StateUpdatedEvent::NewAccount { account } => {
                    this.tree.add_account(account.clone());
                    this.rebuild_tree(cx);
                }
                StateUpdatedEvent::Reset => {
                    this.tree.clear();
                    this.rebuild_tree(cx);
                }
                StateUpdatedEvent::Error { message: _message } => {
                    // Keep the current tree on error
                }
            },
        )
        .detach();

        Self {
            tree: accounts::TreeNode::new(),
            accounts_tree,
        }
    }

    fn rebuild_tree(&mut self, cx: &mut Context<Self>) {
        let tree_items = build_account_tree(&self.tree);
        self.accounts_tree.update(cx, |state, cx| {
            state.set_items(tree_items, cx);
        });
    }
}

fn build_account_tree(node: &TreeNode) -> Vec<TreeItem> {
    let mut items = Vec::new();

    for child in &node.children {
        let id = if let Some(ref account) = child.full_account {
            account.to_string()
        } else {
            child.name.clone()
        };

        let mut item = TreeItem::new(id, child.name.clone());

        if !child.children.is_empty() {
            item = item.expanded(false);
            for sub_child in build_account_tree(child) {
                item = item.child(sub_child);
            }
        }

        items.push(item);
    }

    items
}

impl Render for AccountsTree {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        dropdown_tree(&self.accounts_tree, |entry, _window, _cx| {
            div().child(entry.item().label.clone()).into_any_element()
        })
    }
}
