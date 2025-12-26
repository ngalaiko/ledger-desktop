use core::fmt;
use std::fmt::Debug;
use std::path;

use fastnum::D128;

use crate::accounts::Account;
use crate::sexpr;

#[derive(Debug, thiserror::Error)]
pub enum ParseTransactionError {
    #[error(transparent)]
    ParseDateError(chrono::ParseError),
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
    pub time: chrono::NaiveDate,
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
        let sexpr::Value::String(date) = value[2].to_owned() else {
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
            time: chrono::NaiveDate::parse_from_str(date.as_str(), "%Y-%m-%d")
                .map_err(ParseTransactionError::ParseDateError)?,
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
    #[error("invalid decimal: {0}")]
    InvalidDecimal(String),
    #[error("invalid amount format")]
    InvalidFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrencyAmount {
    pub value: D128,
    pub commodity: String,
}

impl fmt::Display for CurrencyAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.value, self.commodity)
    }
}

impl CurrencyAmount {
    pub fn parse(amount_str: &str) -> Result<Self, ParseAmounError> {
        let amount_str = amount_str.trim();
        let mut parts = amount_str.split_whitespace().collect::<Vec<_>>();
        if parts.is_empty() {
            return Err(ParseAmounError::InvalidFormat);
        }
        let value = parts.remove(0);
        let value = value.replace(",", ""); // Remove commas for thousands separators

        let value = value.parse::<D128>().map_err(|e| ParseAmounError::InvalidDecimal(e.to_string()))?;
        if parts.is_empty() {
            return Ok(CurrencyAmount {
                value,
                commodity: "".to_string(),
            });
        }
        let commodity = parts.join(" ").trim_matches(|c| c == '"').to_string();
        Ok(CurrencyAmount { value, commodity })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Amount {
    pub value: CurrencyAmount,
    pub price: Option<CurrencyAmount>,
    pub date: Option<chrono::NaiveDate>,
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)?;
        if let Some(price) = &self.price {
            write!(f, " {{{}}}", price)?;
        }
        if let Some(date) = &self.date {
            write!(f, " [{}]", date.format("%Y/%m/%d"))?;
        }
        Ok(())
    }
}

impl Amount {
    pub fn parse(amount_str: &str) -> Result<Self, ParseAmounError> {
        let price_start = amount_str.find('{');
        let price = if let Some(price_start) = price_start {
            let price_end = amount_str.find('}').ok_or(ParseAmounError::InvalidFormat)?;
            let price_str = &amount_str[price_start + 1..price_end].trim();
            let price =
                CurrencyAmount::parse(price_str).map_err(|_| ParseAmounError::InvalidFormat)?;
            Ok(Some(price))
        } else {
            Ok(None)
        }?;
        let date_start = amount_str.find('[');
        let date = if let Some(date_start) = date_start {
            let date_end = amount_str.find(']').ok_or(ParseAmounError::InvalidFormat)?;
            let date_str = &amount_str[date_start + 1..date_end].trim();
            let date = chrono::NaiveDate::parse_from_str(date_str, "%Y/%m/%d")
                .map_err(|_| ParseAmounError::InvalidFormat)?;
            Ok(Some(date))
        } else {
            Ok(None)
        }?;
        let amount_str = if let Some(price_start) = price_start {
            &amount_str[..price_start]
        } else if let Some(date_start) = date_start {
            &amount_str[..date_start]
        } else {
            amount_str
        };
        let value = CurrencyAmount::parse(amount_str)?;
        Ok(Amount { value, price, date })
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
        let sexpr_str  = "(\"/Users/nikita.galaiko/Developer/finance/transactions/2025.ledger\" 8561 \"2025-12-13\" nil \"Kop\"
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
            chrono::NaiveDate::from_ymd_opt(2025, 12, 13).unwrap()
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
    fn test_parse_currency_amount_no_commodity() {
        let amount_str = "-1,020.48";
        let amount = Amount::parse(amount_str).expect("should parse amount");
        assert_eq!(
            amount.value.value,
            "-1020.48".parse::<D128>().unwrap()
        );
        assert_eq!(amount.value.commodity, "");
        assert!(amount.price.is_none());
        assert!(amount.date.is_none());
    }

    #[test]
    fn test_parse_currency_amount_thousand() {
        let amount_str = "-1,020.48 GEL";
        let amount = Amount::parse(amount_str).expect("should parse amount");
        assert_eq!(
            amount.value.value,
            "-1020.48".parse::<D128>().unwrap()
        );
        assert_eq!(amount.value.commodity, "GEL");
        assert!(amount.price.is_none());
        assert!(amount.date.is_none());
    }

    #[test]
    fn test_parse_currency_amount_simple() {
        let amount_str = "-20.48 GEL";
        let amount = Amount::parse(amount_str).expect("should parse amount");
        assert_eq!(
            amount.value.value,
            "-20.48".parse::<D128>().unwrap()
        );
        assert_eq!(amount.value.commodity, "GEL");
        assert!(amount.price.is_none());
        assert!(amount.date.is_none());
    }

    #[test]
    fn test_parse_amount_priced() {
        let amount_str = "-20.48 GEL {3.6041025641 SEK} [2025/12/03]";
        let amount = Amount::parse(amount_str).expect("should parse amount");
        assert_eq!(
            amount.value.value,
            "-20.48".parse::<D128>().unwrap()
        );
        assert_eq!(amount.value.commodity, "GEL");
        assert!(amount.price.is_some());
        let price = amount.price.as_ref().unwrap();
        assert_eq!(
            price.value,
            "3.6041025641".parse::<D128>().unwrap()
        );
        assert_eq!(price.commodity, "SEK");
        assert!(amount.date.is_some());
        let date = amount.date.as_ref().unwrap();
        assert_eq!(*date, chrono::NaiveDate::from_ymd_opt(2025, 12, 3).unwrap());
    }

    #[test]
    fn test_parse_amount_long_price() {
        let amount_str = "194.21240000 USDT {9.525653356840242950501615756769 SEK} [2025/09/17]";
        let amount = Amount::parse(amount_str).expect("should parse amount");
        assert_eq!(
            amount.value.value,
            "194.21240000".parse::<D128>().unwrap()
        );
        assert_eq!(amount.value.commodity, "USDT");
        assert!(amount.price.is_some());
        let price = amount.price.as_ref().unwrap();
        // D128 supports up to ~38 decimal digits, so the full 30-digit precision is preserved
        assert_eq!(
            price.value,
            "9.525653356840242950501615756769".parse::<D128>().unwrap()
        );
        assert_eq!(price.commodity, "SEK");
        assert!(amount.date.is_some());
        let date = amount.date.as_ref().unwrap();
        assert_eq!(*date, chrono::NaiveDate::from_ymd_opt(2025, 9, 17).unwrap());
    }
}
