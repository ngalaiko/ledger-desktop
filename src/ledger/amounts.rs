use core::fmt;

use fastnum::D128;

#[derive(Debug, thiserror::Error)]
pub enum ParseAmountError {
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
    pub fn from_str(amount_str: &str) -> Result<Self, ParseAmountError> {
        let amount_str = amount_str.trim();
        let mut parts = amount_str.split_whitespace().collect::<Vec<_>>();
        if parts.is_empty() {
            return Err(ParseAmountError::InvalidFormat);
        }
        let value = parts.remove(0);
        let value = value.replace(",", ""); // Remove commas for thousands separators

        let value = value
            .parse::<D128>()
            .map_err(|e| ParseAmountError::InvalidDecimal(e.to_string()))?;
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
    pub fn parse(amount_str: &str) -> Result<Self, ParseAmountError> {
        let price_start = amount_str.find('{');
        let price = if let Some(price_start) = price_start {
            let price_end = amount_str.find('}').ok_or(ParseAmountError::InvalidFormat)?;
            let price_str = &amount_str[price_start + 1..price_end].trim();
            let price =
                CurrencyAmount::from_str(price_str).map_err(|_| ParseAmountError::InvalidFormat)?;
            Ok(Some(price))
        } else {
            Ok(None)
        }?;
        let date_start = amount_str.find('[');
        let date = if let Some(date_start) = date_start {
            let date_end = amount_str.find(']').ok_or(ParseAmountError::InvalidFormat)?;
            let date_str = &amount_str[date_start + 1..date_end].trim();
            let date = chrono::NaiveDate::parse_from_str(date_str, "%Y/%m/%d")
                .map_err(|_| ParseAmountError::InvalidFormat)?;
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
        let value = CurrencyAmount::from_str(amount_str)?;
        Ok(Amount { value, price, date })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_currency_amount_no_commodity() {
        let amount_str = "-1,020.48";
        let amount = Amount::parse(amount_str).expect("should parse amount");
        assert_eq!(amount.value.value, "-1020.48".parse::<D128>().unwrap());
        assert_eq!(amount.value.commodity, "");
        assert!(amount.price.is_none());
        assert!(amount.date.is_none());
    }

    #[test]
    fn test_parse_currency_amount_thousand() {
        let amount_str = "-1,020.48 GEL";
        let amount = Amount::parse(amount_str).expect("should parse amount");
        assert_eq!(amount.value.value, "-1020.48".parse::<D128>().unwrap());
        assert_eq!(amount.value.commodity, "GEL");
        assert!(amount.price.is_none());
        assert!(amount.date.is_none());
    }

    #[test]
    fn test_parse_currency_amount_simple() {
        let amount_str = "-20.48 GEL";
        let amount = Amount::parse(amount_str).expect("should parse amount");
        assert_eq!(amount.value.value, "-20.48".parse::<D128>().unwrap());
        assert_eq!(amount.value.commodity, "GEL");
        assert!(amount.price.is_none());
        assert!(amount.date.is_none());
    }

    #[test]
    fn test_parse_amount_priced() {
        let amount_str = "-20.48 GEL {3.6041025641 SEK} [2025/12/03]";
        let amount = Amount::parse(amount_str).expect("should parse amount");
        assert_eq!(amount.value.value, "-20.48".parse::<D128>().unwrap());
        assert_eq!(amount.value.commodity, "GEL");
        assert!(amount.price.is_some());
        let price = amount.price.as_ref().unwrap();
        assert_eq!(price.value, "3.6041025641".parse::<D128>().unwrap());
        assert_eq!(price.commodity, "SEK");
        assert!(amount.date.is_some());
        let date = amount.date.as_ref().unwrap();
        assert_eq!(*date, chrono::NaiveDate::from_ymd_opt(2025, 12, 3).unwrap());
    }

    #[test]
    fn test_parse_amount_long_price() {
        let amount_str = "194.21240000 USDT {9.525653356840242950501615756769 SEK} [2025/09/17]";
        let amount = Amount::parse(amount_str).expect("should parse amount");
        assert_eq!(amount.value.value, "194.21240000".parse::<D128>().unwrap());
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
