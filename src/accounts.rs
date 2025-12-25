use core::fmt;
use std::collections::HashMap;

use rust_decimal::Decimal;

use crate::transactions::Amount;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Account {
    pub segments: Vec<String>,
}

impl fmt::Display for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.segments.join(":"))
    }
}

impl Account {
    pub fn is_parent_of(&self, other: &Account) -> bool {
        if self.segments.len() >= other.segments.len() {
            return false;
        }
        for (a, b) in self.segments.iter().zip(other.segments.iter()) {
            if a != b {
                return false;
            }
        }
        true
    }

    pub fn from_segments(segments: Vec<String>) -> Self {
        Account { segments }
    }

    pub fn empty() -> Self {
        Account {
            segments: Vec::new(),
        }
    }

    pub fn parse(name: &str) -> Self {
        let segments: Vec<String> = name
            .split(':')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        Account { segments }
    }

    pub fn name(&self) -> &str {
        self.segments.last().unwrap()
    }

    #[cfg(test)]
    pub fn parent(&self) -> Option<Account> {
        if self.segments.len() > 1 {
            Some(Account {
                segments: self.segments[..self.segments.len() - 1].to_vec(),
            })
        } else {
            None
        }
    }

    #[cfg(test)]
    pub fn depth(&self) -> usize {
        self.segments.len()
    }
}

#[derive(Debug)]
pub struct Balance {
    by_commodity: HashMap<String, Amount>,
}

impl fmt::Display for Balance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        for amount in self.by_commodity.values() {
            parts.push(format!("{}", amount));
        }
        write!(f, "{}", parts.join(", "))
    }
}

impl Balance {
    pub fn new() -> Self {
        Self {
            by_commodity: HashMap::new(),
        }
    }

    pub fn add_amount(&mut self, amount: Amount) {
        let entry = self
            .by_commodity
            .entry(amount.commodity.clone())
            .or_insert(Amount {
                value: Decimal::new(0, 0),
                commodity: amount.commodity.clone(),
            });
        entry.value += amount.value;
    }
}

pub struct TreeNode {
    pub account: Account,
    pub balance: Balance,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn new() -> Self {
        Self {
            account: Account::empty(),
            balance: Balance::new(),
            children: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.children = Vec::new();
        self.account = Account::empty();
        self.balance = Balance::new();
    }

    pub fn add_account(&mut self, account: &Account) {
        self.add_account_recursive(&account, 0)
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

        let child_index = match child_index {
            Some(idx) => idx,
            None => {
                self.children.push(TreeNode {
                    account: current,
                    balance: Balance::new(),
                    children: Vec::new(),
                });
                self.children.len() - 1
            }
        };

        let child = &mut self.children[child_index];

        child.add_account_recursive(account, depth + 1)
    }

    pub fn add_amount_to_account(&mut self, account: &Account, amount: &Amount) {
        self.add_amount_recursive(account, amount, 0);
    }

    fn add_amount_recursive(&mut self, account: &Account, amount: &Amount, depth: usize) -> bool {
        if depth >= account.segments.len() {
            return false;
        }

        let current = Account::from_segments(account.segments[..=depth].to_vec());

        // Find the child node
        if let Some(child) = self
            .children
            .iter_mut()
            .find(|child| child.account.eq(&current))
        {
            // If this is the target account, add the amount
            if child.account.eq(account) {
                child.balance.add_amount(amount.clone());
                return true;
            }

            // Otherwise, recurse to children and if found, add to this node's balance too
            if child.add_amount_recursive(account, amount, depth + 1) {
                child.balance.add_amount(amount.clone());
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_parse() {
        let account = Account::parse("assets:bank:checking");
        assert_eq!(account.name(), "checking");
        assert_eq!(account.segments, vec!["assets", "bank", "checking"]);
        assert_eq!(account.to_string(), "assets:bank:checking");

        let parent = account.parent().expect("should have parent");
        assert_eq!(parent.name(), "bank");
        assert_eq!(parent.segments, vec!["assets", "bank"]);

        let grandparent = parent.parent().expect("should have grandparent");
        assert_eq!(grandparent.name(), "assets");
        assert_eq!(grandparent.segments, vec!["assets"]);
        assert!(grandparent.parent().is_none());
    }

    #[test]
    fn test_account_depth() {
        let account = Account::parse("assets:bank:checking");
        assert_eq!(account.depth(), 3);

        let account = Account::parse("assets");
        assert_eq!(account.depth(), 1);
    }

    #[test]
    fn test_tree_single_account() {
        let mut tree = TreeNode::new();
        let account = Account::parse("assets:bank:checking");
        tree.add_account(&account);

        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].account, Account::parse("assets"));
        assert_eq!(tree.children[0].children.len(), 1);
        assert_eq!(
            tree.children[0].children[0].account,
            Account::parse("assets:bank")
        );
        assert_eq!(tree.children[0].children[0].children.len(), 1);
        assert_eq!(
            tree.children[0].children[0].children[0].account,
            Account::parse("assets:bank:checking")
        );
    }

    #[test]
    fn test_tree_multiple_accounts() {
        let mut tree = TreeNode::new();
        tree.add_account(&Account::parse("assets:bank:checking"));
        tree.add_account(&Account::parse("assets:bank:savings"));
        tree.add_account(&Account::parse("assets:cash"));
        tree.add_account(&Account::parse("expenses:groceries"));

        assert_eq!(tree.children.len(), 2); // assets and expenses

        let assets = &tree.children[0];
        assert_eq!(assets.account, Account::parse("assets"));
        assert_eq!(assets.children.len(), 2); // bank and cash

        let bank = &assets.children[0];
        assert_eq!(bank.account, Account::parse("assets:bank"));
        assert_eq!(bank.children.len(), 2); // checking and savings
    }

    #[test]
    fn test_subtree_balance_single_account() {
        use crate::transactions::Amount;
        use rust_decimal::Decimal;

        let mut tree = TreeNode::new();
        tree.add_account(&Account::parse("assets:bank:checking"));

        let amount = Amount {
            value: Decimal::new(10000, 2), // 100.00
            commodity: "USD".to_string(),
        };

        tree.add_amount_to_account(&Account::parse("assets:bank:checking"), &amount);

        // Check that the leaf account has the balance
        let assets = &tree.children[0];
        let bank = &assets.children[0];
        let checking = &bank.children[0];
        assert_eq!(checking.balance.to_string(), "100.00 USD");

        // Check that all parent accounts have the same balance (subtree total)
        assert_eq!(bank.balance.to_string(), "100.00 USD");
        assert_eq!(assets.balance.to_string(), "100.00 USD");
    }

    #[test]
    fn test_subtree_balance_multiple_accounts() {
        use crate::transactions::Amount;
        use rust_decimal::Decimal;

        let mut tree = TreeNode::new();
        tree.add_account(&Account::parse("assets:bank:checking"));
        tree.add_account(&Account::parse("assets:bank:savings"));
        tree.add_account(&Account::parse("assets:cash"));

        // Add amounts to different accounts
        tree.add_amount_to_account(
            &Account::parse("assets:bank:checking"),
            &Amount {
                value: Decimal::new(10000, 2), // 100.00
                commodity: "USD".to_string(),
            },
        );

        tree.add_amount_to_account(
            &Account::parse("assets:bank:savings"),
            &Amount {
                value: Decimal::new(20000, 2), // 200.00
                commodity: "USD".to_string(),
            },
        );

        tree.add_amount_to_account(
            &Account::parse("assets:cash"),
            &Amount {
                value: Decimal::new(5000, 2), // 50.00
                commodity: "USD".to_string(),
            },
        );

        let assets = &tree.children[0];
        let bank = &assets.children[0];
        let checking = &bank.children[0];
        let savings = &bank.children[1];
        let cash = &assets.children[1];

        // Check individual account balances
        assert_eq!(checking.balance.to_string(), "100.00 USD");
        assert_eq!(savings.balance.to_string(), "200.00 USD");
        assert_eq!(cash.balance.to_string(), "50.00 USD");

        // Check that bank account has the sum of checking and savings
        assert_eq!(bank.balance.to_string(), "300.00 USD");

        // Check that assets account has the sum of all children
        assert_eq!(assets.balance.to_string(), "350.00 USD");
    }

    #[test]
    fn test_subtree_balance_multiple_commodities() {
        use crate::transactions::Amount;
        use rust_decimal::Decimal;

        let mut tree = TreeNode::new();
        tree.add_account(&Account::parse("assets:bank:checking"));
        tree.add_account(&Account::parse("assets:cash"));

        // Add USD to checking
        tree.add_amount_to_account(
            &Account::parse("assets:bank:checking"),
            &Amount {
                value: Decimal::new(10000, 2), // 100.00
                commodity: "USD".to_string(),
            },
        );

        // Add EUR to cash
        tree.add_amount_to_account(
            &Account::parse("assets:cash"),
            &Amount {
                value: Decimal::new(5000, 2), // 50.00
                commodity: "EUR".to_string(),
            },
        );

        let assets = &tree.children[0];
        let bank = &assets.children[0];
        let checking = &bank.children[0];
        let cash = &assets.children[1];

        // Check individual account balances
        assert_eq!(checking.balance.to_string(), "100.00 USD");
        assert_eq!(cash.balance.to_string(), "50.00 EUR");

        // Check that parent accounts track both commodities
        assert_eq!(bank.balance.to_string(), "100.00 USD");
        let assets_balance = assets.balance.to_string();
        assert!(assets_balance.contains("100.00 USD") && assets_balance.contains("50.00 EUR"));
    }
}
