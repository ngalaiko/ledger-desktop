use std::collections::HashSet;

#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::{
    checkbox::Checkbox,
    h_flex,
    list::ListItem,
    tree::{tree, TreeItem, TreeState},
    IconName,
};

use crate::accounts::{Account, TreeNode};

use super::state::State;

pub struct AccountsTreeView {
    tree_state: Entity<TreeState>,
    selected_accounts: HashSet<Account>,
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
            selected_accounts: HashSet::new(),
        }
    }

    pub fn selected_accounts(&self) -> &HashSet<Account> {
        &self.selected_accounts
    }

    fn is_selected(&self, account: &Account) -> bool {
        self.selected_accounts.contains(account)
    }

    fn toggle_selection(&mut self, account: Account, cx: &mut Context<Self>) {
        if self.selected_accounts.contains(&account) {
            self.selected_accounts.remove(&account);
        } else {
            self.selected_accounts.insert(account);
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
            move |ix, entry, _selected, _window, cx| {
                view.update(cx, |this, cx| {
                    let item = entry.item();
                    let account = Account::parse(&item.id);
                    let is_multi_selected = this.is_selected(&account);

                    let with_checkbox = div()
                        .flex()
                        .size_full()
                        .justify_between()
                        .items_center()
                        .child(item.label.clone())
                        .child(
                            div()
                                .child(
                                    Checkbox::new(item.id.clone())
                                        .checked(is_multi_selected)
                                        .on_click(cx.listener({
                                            let item = entry.item().clone();
                                            move |this, _checked, _window, cx| {
                                                let account = Account::parse(&item.id);
                                                this.toggle_selection(account, cx);
                                            }
                                        })),
                                )
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

                    ListItem::new(ix)
                        .selected(is_multi_selected)
                        .pl(px(16.) * entry.depth() + px(12.))
                        .child(with_icon)
                })
            }
        })
    }
}
