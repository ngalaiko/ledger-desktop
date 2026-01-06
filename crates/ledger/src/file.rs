use anyhow::Error;
use futures_lite::StreamExt;
use gpui::{App, AppContext, Context, Entity, Global, Task};

use crate::{
    balance::RunningBalance, cli::Cli, converter::CurrencyConverter, Price, Transaction, TreeNode,
};

pub fn init<P: AsRef<std::path::Path>>(path: Option<P>, cx: &mut App) {
    File::set_global(cx.new(|cx| File::new(path, cx)), cx);
}

struct GlobalFile(Entity<File>);

impl Global for GlobalFile {}

struct FileState {
    accounts: TreeNode,
    transactions: Vec<Transaction>,
    running_balance: RunningBalance,
    currency_converter: CurrencyConverter,
}

impl Default for FileState {
    fn default() -> Self {
        Self {
            accounts: TreeNode::new(),
            transactions: Vec::new(),
            running_balance: RunningBalance::new(),
            currency_converter: CurrencyConverter::new(),
        }
    }
}

impl FileState {
    pub fn load(cli: &Cli, cx: &App) -> Task<Result<Self, Error>> {
        enum Item {
            Transaction(Transaction),
            Price(Price),
        }

        cx.background_spawn({
            let cli = cli.clone();
            async move {
                let mut transactions = Vec::new();
                let mut running_balance = RunningBalance::new();
                let mut currency_converter = CurrencyConverter::new();
                let mut accounts = TreeNode::new();

                let transactions_stream =
                    cli.transactions().await?.map(|r| r.map(Item::Transaction));

                let prices_stream = cli.prices().await?.map(|r| r.map(Item::Price));

                let mut combined = std::pin::pin!(transactions_stream.or(prices_stream));

                while let Some(result) = combined.next().await {
                    match result? {
                        Item::Transaction(transaction) => {
                            for posting in transaction.postings.iter() {
                                accounts.add_account(&posting.account);
                                running_balance.record_diff(
                                    transaction.date,
                                    &posting.account,
                                    &posting.amount.value,
                                );
                                if let Some(cost) = &posting.amount.cost {
                                    currency_converter.record(Price {
                                        date: transaction.date,
                                        commodity: posting.amount.value.commodity.clone(),
                                        value: cost.clone(),
                                    });
                                }
                            }
                            transactions.push(transaction);
                        }
                        Item::Price(price) => {
                            currency_converter.record(price);
                        }
                    }
                }

                Ok(Self {
                    accounts,
                    transactions,
                    running_balance,
                    currency_converter,
                })
            }
        })
    }
}

impl FileState {}

pub struct File {
    state: Entity<Result<FileState, Error>>,
    _tasks: Vec<Task<()>>,
}

impl File {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalFile>().0.clone()
    }

    pub(crate) fn set_global(file: Entity<Self>, cx: &mut App) {
        cx.set_global(GlobalFile(file));
    }

    pub fn running_balance(cx: &App) -> Result<RunningBalance, Error> {
        let state = Self::global(cx).read(cx).state.read(cx);
        match state {
            Ok(state) => Ok(state.running_balance.clone()),
            Err(e) => Err(Error::msg(e.to_string())),
        }
    }

    pub fn currency_converter(cx: &App) -> Result<&CurrencyConverter, Error> {
        let state = Self::global(cx).read(cx).state.read(cx);
        match state {
            Ok(state) => Ok(&state.currency_converter),
            Err(e) => Err(Error::msg(e.to_string())),
        }
    }

    pub fn transactions(cx: &App) -> Result<Vec<&Transaction>, Error> {
        let state = Self::global(cx).read(cx).state.read(cx);
        match state {
            Ok(state) => Ok(state.transactions.iter().collect()),
            Err(e) => Err(Error::msg(e.to_string())),
        }
    }

    pub fn accounts(cx: &App) -> Result<&TreeNode, Error> {
        let state = Self::global(cx).read(cx).state.read(cx);
        match state {
            Ok(state) => Ok(&state.accounts),
            Err(e) => Err(Error::msg(e.to_string())),
        }
    }

    fn new<P: AsRef<std::path::Path>>(path: Option<P>, cx: &mut Context<Self>) -> Self {
        let cli = Cli::new(path);

        let mut tasks = vec![];

        let load_state = FileState::load(&cli, cx);

        tasks.push(
            // load the initial settings
            cx.spawn(async move |this, cx| {
                let state = load_state.await;
                this.update(cx, |this, cx| {
                    this.state = cx.new(|_cx| state);
                    cx.notify();
                })
                .ok();
            }),
        );

        Self {
            state: cx.new(|_cx| Ok(FileState::default())),
            _tasks: tasks,
        }
    }
}
