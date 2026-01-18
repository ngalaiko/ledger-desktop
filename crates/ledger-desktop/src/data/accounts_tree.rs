use gpui::{App, AppContext, Context, Entity, Global, Subscription};
use ledger::Account;

use super::transactions::Transactions;

pub fn init(cx: &mut App) {
    AccountsTree::set_global(cx.new(AccountsTree::new), cx);
}

struct GlobalAccountsTree(Entity<AccountsTree>);

impl Global for GlobalAccountsTree {}

pub struct AccountsTree {
    tree: TreeNode,
    _subscriptions: Vec<Subscription>,
}

impl AccountsTree {
    pub fn global(cx: &App) -> Entity<AccountsTree> {
        cx.global::<GlobalAccountsTree>().0.clone()
    }

    pub(crate) fn set_global(accounts_tree: Entity<AccountsTree>, cx: &mut App) {
        cx.set_global(GlobalAccountsTree(accounts_tree));
    }

    fn new(cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![];

        let transactions = Transactions::global(cx);
        subscriptions.push(cx.observe(&transactions, |this, transactions, cx| {
            this.tree = build_tree(transactions.read(cx).as_slice());
            cx.notify();
        }));

        Self {
            tree: TreeNode::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn tree(&self) -> &TreeNode {
        &self.tree
    }
}

fn build_tree(transactions: &[ledger::Transaction]) -> TreeNode {
    let mut tree = TreeNode::new();
    for transaction in transactions {
        for posting in &transaction.postings {
            tree.add_account(&posting.account);
        }
    }
    tree
}

#[derive(Clone)]
pub struct TreeNode {
    pub account: Account,
    pub children: Vec<TreeNode>,
}

impl Default for TreeNode {
    fn default() -> Self {
        Self {
            account: Account::empty(),
            children: Vec::new(),
        }
    }
}

impl TreeNode {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_account(&mut self, account: &Account) {
        self.add_account_recursive(account, 0);
    }

    pub fn find_node(&self, account: &Account) -> Option<&TreeNode> {
        if &self.account == account {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = Self::find_node(child, account) {
                return Some(found);
            }
        }
        None
    }

    pub fn get_descendants(&self, account: &Account) -> Vec<Account> {
        for child in &self.children {
            if &child.account == account {
                return child.collect_all_accounts();
            }
            let descendants = child.get_descendants(account);
            if !descendants.is_empty() {
                return descendants;
            }
        }
        Vec::new()
    }

    pub fn collect_all_accounts(&self) -> Vec<Account> {
        let mut accounts = vec![self.account.clone()];
        for child in &self.children {
            accounts.extend(child.collect_all_accounts());
        }
        accounts
    }

    fn add_account_recursive(&mut self, account: &Account, depth: usize) {
        if depth >= account.segments.len() {
            return;
        }

        let current = Account::from_segments(account.segments[..=depth].to_vec());

        // Find or create child node
        let child_index = self
            .children
            .iter()
            .position(|child| child.account.eq(&current));

        let child_index = if let Some(idx) = child_index {
            idx
        } else {
            self.children.push(TreeNode {
                account: current,
                children: Vec::new(),
            });
            self.children.len() - 1
        };

        let child = &mut self.children[child_index];

        child.add_account_recursive(account, depth + 1);
    }
}
