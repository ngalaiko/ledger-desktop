use super::amounts::CurrencyAmount;

#[derive(thiserror::Error, Debug)]
pub enum ParsePriceError {
    #[error(transparent)]
    ParseDateError(chrono::ParseError),
    #[error(transparent)]
    ParseAmountError(super::amounts::ParseAmounError),
    #[error("invalid price format")]
    InvalidFormat,
}

#[derive(Debug)]
pub struct Price {
    pub date: chrono::NaiveDate,
    pub commodity: String,
    pub value: CurrencyAmount,
}

impl Price {
    pub fn from_str(value: &str) -> Result<Self, ParsePriceError> {
        let Some(whitespace_pos) = value.find(' ') else {
            return Err(ParsePriceError::InvalidFormat);
        };
        let date_str = &value[..whitespace_pos];
        let date = chrono::NaiveDate::parse_from_str(date_str, "%Y/%m/%d")
            .map_err(ParsePriceError::ParseDateError)?;

        let rest = &value[whitespace_pos..].trim_start();
        let (commodity, rest) = if let Some(quote_pos) = rest.find('"') {
            let end_quote_pos = rest[quote_pos + 1..]
                .find('"')
                .ok_or(ParsePriceError::InvalidFormat)?
                + quote_pos
                + 1;
            let commodity = &rest[quote_pos + 1..end_quote_pos];
            let rest_after_commodity = &rest[end_quote_pos + 1..];
            Ok((commodity, rest_after_commodity))
        } else {
            let next_space_pos = rest.find(' ').ok_or(ParsePriceError::InvalidFormat)?;
            let commodity = &rest[..next_space_pos];
            let rest_after_commodity = &rest[next_space_pos..];
            Ok((commodity, rest_after_commodity))
        }?;

        let currency_amount =
            CurrencyAmount::from_str(rest.trim()).map_err(ParsePriceError::ParseAmountError)?;

        Ok(Price {
            date,
            commodity: commodity.to_string(),
            value: currency_amount,
        })
    }
}

#[cfg(test)]
mod tests {
    use fastnum::D128;

    use super::*;

    #[test]
    fn test_price_from_str_quoted_commodity() {
        let price_str = "2018/08/24 \"Nordea Nora Three\"   103.50 SEK";
        let price = Price::from_str(price_str).expect("should parse price");
        assert_eq!(
            price.date,
            chrono::NaiveDate::from_ymd_opt(2018, 8, 24).expect("valid date")
        );
        assert_eq!(price.commodity, "Nordea Nora Three");
        assert_eq!(price.value.value, "103.50".parse::<D128>().unwrap());
        assert_eq!(price.value.commodity, "SEK");
    }

    #[test]
    fn test_price_from_str_simple_commodity() {
        let price_str = "2023/08/31 EUR         12.05 SEK";
        let price = Price::from_str(price_str).expect("should parse price");
        assert_eq!(
            price.date,
            chrono::NaiveDate::from_ymd_opt(2023, 8, 31).expect("valid date")
        );
        assert_eq!(price.commodity, "EUR");
        assert_eq!(price.value.value, "12.05".parse::<D128>().unwrap());
        assert_eq!(price.value.commodity, "SEK");
    }

    #[test]
    fn test_price_from_str_utf8() {
        let price_str = "2022/05/23 \"Öhman Räntefond Kompass A\"   102.64 SEK";
        let price = Price::from_str(price_str).expect("should parse price");
        assert_eq!(
            price.date,
            chrono::NaiveDate::from_ymd_opt(2022, 5, 23).expect("valid date")
        );
        assert_eq!(price.commodity, "Öhman Räntefond Kompass A");
        assert_eq!(price.value.value, "102.64".parse::<D128>().unwrap());
        assert_eq!(price.value.commodity, "SEK");
    }
}
