use std::collections::HashSet;

#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::tree::TreeItem;

use crate::accounts::{Account, TreeNode};
use crate::ui::dropdown_tree::{dropdown_tree, DropdownTreeState};

use super::dropdown_tree::DropdownTreeEvent;
use super::state::State;

pub enum AccountsTreeEvent {
    Selected { accounts: HashSet<Account> },
}

impl EventEmitter<AccountsTreeEvent> for AccountsTreeView {}

pub struct AccountsTreeView {
    accounts_tree: Entity<DropdownTreeState>,
}

impl AccountsTreeView {
    pub fn new(state: Entity<State>, cx: &mut Context<Self>) -> Self {
        let accounts_tree = cx.new(|cx| DropdownTreeState::new(cx));

        cx.subscribe(
            &accounts_tree,
            |_this, _accounts_tree, event, cx| match event {
                DropdownTreeEvent::Selected { entries } => {
                    let accounts: HashSet<Account> = entries
                        .iter()
                        .map(|entry| Account::parse(entry.item().id.as_str()))
                        .collect();
                    cx.emit(AccountsTreeEvent::Selected { accounts });
                    cx.notify();
                }
            },
        )
        .detach();

        cx.observe(&state, |this, state, cx| {
            let tree_items = build_account_tree(&state.read(cx).accounts);
            this.accounts_tree.update(cx, |state, cx| {
                state.set_items(tree_items, cx);
                cx.notify();
            });
            cx.notify();
        })
        .detach();

        Self { accounts_tree }
    }
}

fn build_account_tree(node: &TreeNode) -> Vec<TreeItem> {
    let mut items = Vec::new();

    for child in &node.children {
        let label = format!("{} ({})", child.account.name(), child.balance.to_string());
        let mut item = TreeItem::new(child.account.to_string(), label);

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

impl Render for AccountsTreeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        dropdown_tree(&self.accounts_tree, |entry, _window, _cx| {
            div().child(entry.item().label.clone()).into_any_element()
        })
    }
}
