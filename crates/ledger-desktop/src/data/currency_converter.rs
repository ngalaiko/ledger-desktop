use std::collections::{BTreeMap, HashMap};

use fastnum::D128;
use gpui::{App, AppContext, Context, Entity, Global, Subscription};
use ledger::{Balance, CurrencyAmount, Price};

pub fn init(cx: &mut App) {
    CurrencyConverter::set_global(cx.new(CurrencyConverter::new), cx);
}

struct GlobalCurrencyConverter(Entity<CurrencyConverter>);

impl Global for GlobalCurrencyConverter {}

pub struct CurrencyConverter {
    // From commodity -> to commodity -> date -> price
    history: HashMap<String, HashMap<String, BTreeMap<chrono::NaiveDate, D128>>>,
    _subscriptions: Vec<Subscription>,
}

impl CurrencyConverter {
    pub fn global(cx: &App) -> Entity<CurrencyConverter> {
        cx.global::<GlobalCurrencyConverter>().0.clone()
    }

    pub(crate) fn set_global(currency_converter: Entity<CurrencyConverter>, cx: &mut App) {
        cx.set_global(GlobalCurrencyConverter(currency_converter));
    }

    fn new(cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![];
        let ledger_file = ledger::File::global(cx);

        subscriptions.push(cx.observe(&ledger_file, |this, ledger_file, cx| {
            this.history = match ledger_file.read(cx).state.as_ref() {
                Ok(state) => calculate(state),
                Err(_) => HashMap::new(),
            };
            cx.notify();
        }));

        Self {
            history: HashMap::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn convert_amount(
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

    pub fn convert_balance(
        &self,
        balance: &Balance,
        target_commodity: &str,
        at_date: chrono::NaiveDate,
    ) -> Balance {
        let mut converted_balance = Balance::new();
        for amount in balance.iter() {
            if let Some(converted_amount) = self.convert_amount(amount, target_commodity, at_date) {
                converted_balance.add_amount(converted_amount);
            } else {
                converted_balance.add_amount(amount.clone());
            }
        }
        converted_balance
    }

    pub fn available_commodities(&self) -> Vec<String> {
        let mut commodities: Vec<String> = self.history.keys().cloned().collect();
        commodities.sort();
        commodities
    }
}

fn calculate(
    state: &ledger::FileState,
) -> HashMap<String, HashMap<String, BTreeMap<chrono::NaiveDate, D128>>> {
    let mut history: HashMap<String, HashMap<String, BTreeMap<chrono::NaiveDate, D128>>> =
        HashMap::new();

    let mut record = |price: &Price| {
        {
            let to_map = history
                .entry(price.commodity.clone())
                .or_insert_with(HashMap::new);
            let date_map = to_map
                .entry(price.value.commodity.clone())
                .or_insert_with(BTreeMap::new);
            date_map.insert(price.date, price.value.value);
        }

        {
            let from_map = history
                .entry(price.value.commodity.clone())
                .or_insert_with(HashMap::new);
            let date_map = from_map
                .entry(price.commodity.clone())
                .or_insert_with(BTreeMap::new);
            date_map.insert(price.date, D128::ONE / price.value.value);
        }
    };

    // Record prices from price directives
    for price in &state.prices {
        record(price);
    }

    // Record prices from transaction costs
    for transaction in &state.transactions {
        for posting in &transaction.postings {
            if let Some(cost) = &posting.amount.cost {
                record(&Price {
                    date: transaction.date,
                    commodity: posting.amount.value.commodity.clone(),
                    value: cost.clone(),
                });
            }
        }
    }

    history
}
