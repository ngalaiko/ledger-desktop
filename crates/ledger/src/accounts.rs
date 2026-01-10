use core::fmt;
use std::collections::BTreeMap;

use fastnum::D128;

use super::amounts::CurrencyAmount;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Account {
    pub segments: Vec<String>,
    pub type_of: AccountType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AccountType {
    Unknown,
    Assets,
    Liabilities,
}

impl serde::Serialize for Account {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for Account {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Account::parse(&s))
    }
}

impl fmt::Display for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.segments.join(":"))
    }
}

impl Account {
    pub fn is_parent_of(&self, other: &Account) -> bool {
        if self.segments.len() > other.segments.len() {
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
        let type_of = if segments.is_empty() {
            AccountType::Unknown
        } else {
            match segments[0].to_lowercase().as_str() {
                "assets" => AccountType::Assets,
                "liabilities" => AccountType::Liabilities,
                _ => AccountType::Unknown,
            }
        };
        Account { segments, type_of }
    }

    pub fn empty() -> Self {
        Account {
            segments: Vec::new(),
            type_of: AccountType::Unknown,
        }
    }

    pub fn parse(name: &str) -> Self {
        let segments: Vec<String> = name
            .split(':')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        Self::from_segments(segments)
    }

    pub fn name(&self) -> &str {
        self.segments.last().unwrap()
    }

    #[cfg(test)]
    pub fn parent(&self) -> Option<Account> {
        if self.segments.len() > 1 {
            Some(Account {
                segments: self.segments[..self.segments.len() - 1].to_vec(),
                type_of: self.type_of.clone(),
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

#[derive(Debug, Clone)]
pub struct Balance {
    by_commodity: BTreeMap<String, CurrencyAmount>,
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
            by_commodity: BTreeMap::new(),
        }
    }

    pub fn get_amount(&self, commodity: &str) -> Option<&CurrencyAmount> {
        self.by_commodity.get(commodity)
    }

    pub fn add(&mut self, other: &Balance) {
        for amount in other.iter() {
            self.add_amount(amount.clone());
        }
    }

    pub fn subtract(&mut self, other: &Balance) {
        for amount in other.iter() {
            self.subtract_amount(amount.clone());
        }
    }

    pub fn add_amount(&mut self, amount: CurrencyAmount) {
        let entry = self
            .by_commodity
            .entry(amount.commodity.clone())
            .or_insert(CurrencyAmount {
                value: D128::ZERO,
                commodity: amount.commodity.clone(),
            });
        entry.value += amount.value;
    }

    pub fn subtract_amount(&mut self, amount: CurrencyAmount) {
        let entry = self
            .by_commodity
            .entry(amount.commodity.clone())
            .or_insert(CurrencyAmount {
                value: D128::ZERO,
                commodity: amount.commodity.clone(),
            });
        entry.value -= amount.value;
    }

    pub fn iter(&self) -> impl Iterator<Item = &CurrencyAmount> + '_ {
        self.by_commodity.values()
    }
}

#[derive(Clone)]
pub struct TreeNode {
    pub account: Account,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn new() -> Self {
        Self {
            account: Account::empty(),
            children: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.children = Vec::new();
        self.account = Account::empty();
    }

    pub fn add_account(&mut self, account: &Account) {
        self.add_account_recursive(&account, 0)
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

        let child_index = match child_index {
            Some(idx) => idx,
            None => {
                self.children.push(TreeNode {
                    account: current,
                    children: Vec::new(),
                });
                self.children.len() - 1
            }
        };

        let child = &mut self.children[child_index];

        child.add_account_recursive(account, depth + 1)
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
}
