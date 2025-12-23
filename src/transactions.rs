use std::path;
use std::time;

use crate::accounts::Account;
use crate::accounts::ParseAccountError;
use crate::sexpr;

#[derive(Debug, thiserror::Error)]
pub enum ParseTransactionError {
    #[error("expected a list of {0}, got {1}")]
    UnexpectedLength(usize, usize),
    #[error("expected type {1} at position {0}")]
    UnexpectedType(usize, sexpr::Value),
    #[error("error parsing posting at index {0}: {1}")]
    PostingError(usize, ParsePostingError),
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub file: path::PathBuf,
    pub line: i64,
    pub time: time::SystemTime,
    pub description: String,
    pub postings: Vec<Posting>,
}

impl Transaction {
    pub fn from_sexpr(value: &[sexpr::Value]) -> Result<Self, ParseTransactionError> {
        if value.len() < 5 {
            return Err(ParseTransactionError::UnexpectedLength(5, value.len()));
        }
        let sexpr::Value::String(file) = value[0].to_owned() else {
            return Err(ParseTransactionError::UnexpectedType(1, value[1].clone()));
        };
        let sexpr::Value::I64(line) = value[1].to_owned() else {
            return Err(ParseTransactionError::UnexpectedType(1, value[1].clone()));
        };
        let sexpr::Value::I64(epoch_seconds) = value[2].to_owned() else {
            return Err(ParseTransactionError::UnexpectedType(2, value[2].clone()));
        };
        let sexpr::Value::String(description) = value[4].to_owned() else {
            return Err(ParseTransactionError::UnexpectedType(4, value[4].clone()));
        };
        let postings = value[5..]
            .iter()
            .enumerate()
            .map(|(i, posting_value)| {
                let sexpr::Value::List(posting_list) = posting_value else {
                    return Err(ParseTransactionError::UnexpectedType(
                        i + 5,
                        posting_value.clone(),
                    ));
                };
                Posting::from_sexpr(posting_list)
                    .map_err(|e| ParseTransactionError::PostingError(i, e))
            })
            .collect::<Result<Vec<Posting>, ParseTransactionError>>()?;
        Ok(Transaction {
            file: path::PathBuf::from(file),
            line,
            time: time::UNIX_EPOCH + time::Duration::from_secs(epoch_seconds as u64),
            description,
            postings,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParsePostingError {
    #[error("expected a list of {0}, got {1}")]
    UnexpectedLength(usize, usize),
    #[error("expected type {1} at position {0}")]
    UnexpectedType(usize, sexpr::Value),
    #[error("invalid account name: {0}")]
    InvalidAccountName(ParseAccountError),
}

#[derive(Debug, Clone)]
pub struct Posting {
    pub account: Account,
    pub amount: String,
    pub note: Option<String>,
}

impl Posting {
    pub fn from_sexpr(value: &[sexpr::Value]) -> Result<Self, ParsePostingError> {
        if value.len() < 4 {
            return Err(ParsePostingError::UnexpectedLength(4, value.len()));
        }
        let sexpr::Value::String(account) = value[1].to_owned() else {
            return Err(ParsePostingError::UnexpectedType(1, value[1].clone()));
        };
        let account = Account::parse(&account).map_err(ParsePostingError::InvalidAccountName)?;
        let sexpr::Value::String(amount) = value[2].to_owned() else {
            return Err(ParsePostingError::UnexpectedType(2, value[2].clone()));
        };
        if value.len() == 5 {
            let sexpr::Value::String(note) = value[4].to_owned() else {
                return Err(ParsePostingError::UnexpectedType(4, value[4].clone()));
            };
            Ok(Posting {
                account,
                amount,
                note: Some(note),
            })
        } else {
            Ok(Posting {
                account,
                amount,
                note: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_posting() {
        let sexpr_str = "(8562 \"expenses:Pending\" \"148.95 SEK\" pending \" shared:: 35%\")";
        let sexpr_value = sexpr::parse_sexpr(sexpr_str).expect("should sexpr");
        let posting = Posting::from_sexpr(&sexpr_value).expect("should parse posting");
        assert_eq!(posting.account.to_string(), "expenses:Pending");
        assert_eq!(posting.amount, "148.95 SEK");
        assert!(posting.note.is_some());
        assert_eq!(posting.note.unwrap(), " shared:: 35%");
    }

    #[test]
    fn test_parse_transaction() {
        let sexpr_str  = "(\"/Users/nikita.galaiko/Developer/finance/transactions/2025.ledger\" 8561 1765666800 nil \"Kop\"
  (8562 \"expenses:Pending\" \"148.95 SEK\" pending \" shared:: 35%\"))";
        let sexpr_value = sexpr::parse_sexpr(sexpr_str).expect("should sexpr");
        let transaction = Transaction::from_sexpr(&sexpr_value).expect("should parse transaction");
        assert_eq!(
            transaction.file,
            path::PathBuf::from("/Users/nikita.galaiko/Developer/finance/transactions/2025.ledger")
        );
        assert_eq!(transaction.line, 8561);
        assert_eq!(transaction.description, "Kop");
        assert_eq!(
            transaction.time,
            time::UNIX_EPOCH + time::Duration::from_secs(1765666800),
        );
        assert_eq!(transaction.postings.len(), 1);
        let posting = &transaction.postings[0];
        assert_eq!(posting.account.to_string(), "expenses:Pending");
        assert_eq!(posting.amount, "148.95 SEK");
        assert!(posting.note.is_some());
        assert_eq!(posting.note.as_ref().unwrap(), " shared:: 35%");
    }
}
