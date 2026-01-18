use gpui::{App, AppContext, Context, Entity, Global, Subscription};
use ledger::{Amount, Posting, Transaction};
use state::AppState;

use super::currency_converter::CurrencyConverter;
use crate::util::observe_multiple;

pub fn init(cx: &mut App) {
    Transactions::set_global(cx.new(Transactions::new), cx);
}

struct GlobalTransactions(Entity<Transactions>);

impl Global for GlobalTransactions {}

pub struct Transactions {
    data: Vec<Transaction>,
    _subscriptions: Vec<Subscription>,
}

impl Transactions {
    pub fn global(cx: &App) -> Entity<Transactions> {
        cx.global::<GlobalTransactions>().0.clone()
    }

    pub(crate) fn set_global(transactions: Entity<Transactions>, cx: &mut App) {
        cx.set_global(GlobalTransactions(transactions));
    }

    fn new(cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![];

        subscriptions.push(observe_multiple(
            cx,
            (
                &ledger::File::global(cx),
                &AppState::global(cx),
                &CurrencyConverter::global(cx),
            ),
            |this, cx| {
                this.recalculate(cx);
                cx.notify();
            },
        ));

        Self {
            data: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    fn recalculate(&mut self, cx: &App) {
        self.data = match ledger::File::transactions(cx) {
            Ok(transactions) => {
                let converter = CurrencyConverter::global(cx).read(cx);
                let target_commodity = AppState::get_commodity(cx);

                transactions
                    .iter()
                    .filter_map(|tx| convert_transaction(converter, tx, target_commodity.clone()))
                    .collect()
            }
            Err(_) => Vec::new(),
        };
    }

    pub fn as_slice(&self) -> &[Transaction] {
        &self.data
    }
}

fn convert_transaction(
    converter: &CurrencyConverter,
    transaction: &Transaction,
    target_commodity: Option<String>,
) -> Option<Transaction> {
    let Some(target_commodity) = target_commodity else {
        return Some(transaction.clone());
    };

    let postings = transaction
        .postings
        .iter()
        .filter_map(|p| {
            converter
                .convert_amount(&p.amount.value, target_commodity.as_str(), transaction.date)
                .map(|converted_amount| Posting {
                    amount: Amount {
                        value: converted_amount,
                        cost: None,
                        cost_date: None,
                    },
                    account: p.account.clone(),
                    ..p.clone()
                })
        })
        .collect::<Vec<_>>();

    if postings.is_empty() {
        return None;
    }

    Some(Transaction {
        postings,
        ..transaction.clone()
    })
}
