use core::fmt;

#[derive(Debug, thiserror::Error)]
pub enum ParseAccountError {
    #[error("invalid account name: {0}")]
    InvalidAccountName(String),
}

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
    pub fn parse(name: &str) -> Result<Self, ParseAccountError> {
        let segments: Vec<String> = name
            .split(':')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if segments.is_empty() {
            return Err(ParseAccountError::InvalidAccountName(name.to_string()));
        }

        Ok(Account { segments })
    }

    #[cfg(test)]
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
    pub name: String,
    pub full_account: Option<Account>,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            full_account: None,
            children: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.children = Vec::new();
        self.name = String::new();
        self.full_account = None;
    }

    pub fn with_name(name: String) -> Self {
        Self {
            name,
            full_account: None,
            children: Vec::new(),
        }
    }

    pub fn add_account(&mut self, account: Account) {
        self.add_account_recursive(&account.segments, 0, account.clone());
    }

    fn add_account_recursive(&mut self, segments: &[String], depth: usize, full_account: Account) {
        if depth >= segments.len() {
            return;
        }

        let segment = &segments[depth];

        // Find or create child node
        let child_index = self
            .children
            .iter()
            .position(|child| &child.name == segment);

        let child_index = match child_index {
            Some(idx) => idx,
            None => {
                self.children.push(TreeNode::with_name(segment.clone()));
                self.children.len() - 1
            }
        };

        let child = &mut self.children[child_index];

        // If this is the last segment, store the full account
        if depth == segments.len() - 1 {
            child.full_account = Some(full_account);
        } else {
            child.add_account_recursive(segments, depth + 1, full_account);
        }
    }

    #[cfg(test)]
    pub fn find(&self, segments: &[String]) -> Option<&TreeNode> {
        self.find_recursive(segments, 0)
    }

    #[cfg(test)]
    fn find_recursive(&self, segments: &[String], depth: usize) -> Option<&TreeNode> {
        if depth >= segments.len() {
            return Some(self);
        }

        let segment = &segments[depth];
        let child = self.children.iter().find(|child| &child.name == segment)?;

        if depth == segments.len() - 1 {
            Some(child)
        } else {
            child.find_recursive(segments, depth + 1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_parse() {
        let account = Account::parse("assets:bank:checking").expect("should parse account");
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
        let account = Account::parse("assets:bank:checking").unwrap();
        assert_eq!(account.depth(), 3);

        let account = Account::parse("assets").unwrap();
        assert_eq!(account.depth(), 1);
    }

    #[test]
    fn test_tree_single_account() {
        let mut tree = TreeNode::new();
        let account = Account::parse("assets:bank:checking").unwrap();
        tree.add_account(account);

        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].name, "assets");
        assert_eq!(tree.children[0].children.len(), 1);
        assert_eq!(tree.children[0].children[0].name, "bank");
        assert_eq!(tree.children[0].children[0].children.len(), 1);
        assert_eq!(tree.children[0].children[0].children[0].name, "checking");
    }

    #[test]
    fn test_tree_multiple_accounts() {
        let mut tree = TreeNode::new();
        tree.add_account(Account::parse("assets:bank:checking").unwrap());
        tree.add_account(Account::parse("assets:bank:savings").unwrap());
        tree.add_account(Account::parse("assets:cash").unwrap());
        tree.add_account(Account::parse("expenses:groceries").unwrap());

        assert_eq!(tree.children.len(), 2); // assets and expenses

        let assets = &tree.children[0];
        assert_eq!(assets.name, "assets");
        assert_eq!(assets.children.len(), 2); // bank and cash

        let bank = &assets.children[0];
        assert_eq!(bank.name, "bank");
        assert_eq!(bank.children.len(), 2); // checking and savings
    }

    #[test]
    fn test_tree_find() {
        let mut tree = TreeNode::new();
        tree.add_account(Account::parse("assets:bank:checking").unwrap());
        tree.add_account(Account::parse("assets:bank:savings").unwrap());

        let node = tree.find(&vec!["assets".to_string(), "bank".to_string()]);
        assert!(node.is_some());
        assert_eq!(node.unwrap().name, "bank");

        let node = tree.find(&vec![
            "assets".to_string(),
            "bank".to_string(),
            "checking".to_string(),
        ]);
        assert!(node.is_some());
        assert_eq!(node.unwrap().name, "checking");

        let node = tree.find(&vec!["nonexistent".to_string()]);
        assert!(node.is_none());
    }
}
