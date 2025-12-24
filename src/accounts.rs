use core::fmt;

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

    pub fn add_account(&mut self, account: Account) {
        self.add_account_recursive(&account.segments, 0, account.clone());
    }

    fn add_account_recursive(&mut self, segments: &[String], depth: usize, full_account: Account) {
        if depth >= segments.len() {
            return;
        }

        let current = Account::from_segments(segments[..=depth].to_vec());

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

        child.add_account_recursive(segments, depth + 1, full_account);
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
        tree.add_account(account);

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
        tree.add_account(Account::parse("assets:bank:checking"));
        tree.add_account(Account::parse("assets:bank:savings"));
        tree.add_account(Account::parse("assets:cash"));
        tree.add_account(Account::parse("expenses:groceries"));

        assert_eq!(tree.children.len(), 2); // assets and expenses

        let assets = &tree.children[0];
        assert_eq!(assets.account, Account::parse("assets"));
        assert_eq!(assets.children.len(), 2); // bank and cash

        let bank = &assets.children[0];
        assert_eq!(bank.account, Account::parse("assets:bank"));
        assert_eq!(bank.children.len(), 2); // checking and savings
    }
}
