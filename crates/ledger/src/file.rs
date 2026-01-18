use anyhow::Error;
use futures_lite::StreamExt;

use crate::{cli::Cli, Price, Transaction};

#[derive(Default)]
pub struct File {
    pub transactions: Vec<Transaction>,
    pub prices: Vec<Price>,
    pub files: Vec<std::path::PathBuf>,
}

impl File {
    pub async fn load(cli: &Cli) -> Result<Self, Error> {
        enum Item {
            Transaction(Transaction),
            Price(Price),
            File(std::path::PathBuf),
        }

        let cli = cli.clone();
        let mut transactions = Vec::new();
        let mut prices = Vec::new();
        let mut files = Vec::new();

        let transactions_stream = cli.transactions().await?.map(|r| r.map(Item::Transaction));
        let prices_stream = cli.prices().await?.map(|r| r.map(Item::Price));
        let files_stream = cli.files().await?.map(|r| r.map(Item::File));

        let mut combined =
            std::pin::pin!(transactions_stream.chain(prices_stream).chain(files_stream));

        while let Some(result) = combined.next().await {
            match result? {
                Item::Transaction(transaction) => {
                    transactions.push(transaction);
                }
                Item::Price(price) => {
                    prices.push(price);
                }
                Item::File(path) => {
                    files.push(path);
                }
            }
        }

        Ok(Self {
            transactions,
            prices,
            files,
        })
    }
}
