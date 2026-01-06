use std::collections::{BTreeMap, HashMap};

use fastnum::D128;

use crate::{CurrencyAmount, Price};

pub struct CurrencyConverter {
    // From commodity -> to commodity -> date -> price
    history: HashMap<String, HashMap<String, BTreeMap<chrono::NaiveDate, D128>>>,
}

impl CurrencyConverter {
    pub fn new() -> Self {
        Self {
            history: HashMap::new(),
        }
    }

    pub fn record(&mut self, price: Price) {
        {
            let to_map = self
                .history
                .entry(price.commodity.clone())
                .or_insert_with(HashMap::new);
            let date_map = to_map
                .entry(price.value.commodity.clone())
                .or_insert_with(BTreeMap::new);
            date_map.insert(price.date, price.value.value);
        }

        {
            let from_map = self
                .history
                .entry(price.value.commodity)
                .or_insert_with(HashMap::new);
            let date_map = from_map
                .entry(price.commodity)
                .or_insert_with(BTreeMap::new);
            date_map.insert(price.date, D128::ONE / price.value.value);
        }
    }

    pub fn convert(
        &self,
        amount: &CurrencyAmount,
        target_commodity: &str,
        at_date: chrono::NaiveDate,
    ) -> Option<CurrencyAmount> {
        if amount.commodity == target_commodity {
            return Some(amount.clone());
        }

        let to_map = self.history.get(&amount.commodity)?;
        let date_map = to_map.get(target_commodity)?;
        let (_, price) = date_map.range(..=at_date).next_back()?;

        Some(CurrencyAmount {
            value: amount.value * (*price),
            commodity: target_commodity.to_string(),
        })
    }

    pub fn available_commodities(&self) -> Vec<String> {
        let mut commodities: Vec<String> = self.history.keys().cloned().collect();
        commodities.sort();
        commodities
    }
}
