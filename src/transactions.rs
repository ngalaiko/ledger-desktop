use core::fmt;
use std::path;
use std::time;

use rust_decimal::Decimal;

use crate::accounts::Account;
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
    #[error("invalid amount: {0}")]
    InvalidAmount(ParseAmounError),
}

#[derive(Debug, Clone)]
pub struct Posting {
    pub account: Account,
    pub amount: Amount,
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
        let account = Account::parse(&account);
        let sexpr::Value::String(amount) = value[2].to_owned() else {
            return Err(ParsePostingError::UnexpectedType(2, value[2].clone()));
        };
        let amount = Amount::parse(&amount).map_err(ParsePostingError::InvalidAmount)?;
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

#[derive(Debug, thiserror::Error)]
pub enum ParseAmounError {
    #[error(transparent)]
    InvalidDecimal(rust_decimal::Error),
    #[error("invalid amount format")]
    InvalidFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Amount {
    pub value: Decimal,
    pub commodity: String,
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.value, self.commodity)
    }
}

impl Amount {
    pub fn parse(amount_str: &str) -> Result<Self, ParseAmounError> {
        let mut parts = amount_str.split_whitespace().collect::<Vec<_>>();
        if parts.is_empty() {
            return Err(ParseAmounError::InvalidFormat);
        }
        let value = parts.remove(0);
        let value = value.replace(",", ""); // Remove commas for thousands separators
        let value = Decimal::from_str_exact(&value).map_err(ParseAmounError::InvalidDecimal)?;
        if parts.is_empty() {
            return Ok(Amount {
                value,
                commodity: "".to_string(),
            });
        }
        let commodity = parts.remove(0).to_string();
        Ok(Amount { value, commodity })
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
        assert_eq!(
            posting.amount,
            Amount::parse("148.95 SEK").expect("should parse amount")
        );
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
        assert_eq!(
            posting.amount,
            Amount::parse("148.95 SEK").expect("should parse amount")
        );
        assert!(posting.note.is_some());
        assert_eq!(posting.note.as_ref().unwrap(), " shared:: 35%");
    }

    #[test]
    fn test_parse_amount_no_commodity() {
        let amount_str = "-1,020.48";
        let amount = Amount::parse(amount_str).expect("should parse amount");
        assert_eq!(amount.value, Decimal::from_str_exact("-1020.48").unwrap());
        assert_eq!(amount.commodity, "");
    }

    #[test]
    fn test_parse_amount_thousand() {
        let amount_str = "-1,020.48 GEL";
        let amount = Amount::parse(amount_str).expect("should parse amount");
        assert_eq!(amount.value, Decimal::from_str_exact("-1020.48").unwrap());
        assert_eq!(amount.commodity, "GEL");
    }

    #[test]
    fn test_parse_amount_simple() {
        let amount_str = "-20.48 GEL";
        let amount = Amount::parse(amount_str).expect("should parse amount");
        assert_eq!(amount.value, Decimal::from_str_exact("-20.48").unwrap());
        assert_eq!(amount.commodity, "GEL");
    }

    #[test]
    fn test_parse_amount_priced() {
        let amount_str = "-20.48 GEL {3.6041025641 SEK} [2025/12/03]";
        let amount = Amount::parse(amount_str).expect("should parse amount");
        assert_eq!(amount.value, Decimal::from_str_exact("-20.48").unwrap());
        assert_eq!(amount.commodity, "GEL");
    }
}
