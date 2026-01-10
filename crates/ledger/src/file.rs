use anyhow::Error;
use futures_lite::StreamExt;
use gpui::{App, AppContext, Context, Entity, Global, Subscription, Task};

use crate::{cli::Cli, converter::CurrencyConverter, Price, Transaction, TreeNode};

pub fn init<P: AsRef<std::path::Path>>(path: Option<P>, cx: &mut App) {
    File::set_global(cx.new(|cx| File::new(path, cx)), cx);
}

struct GlobalFile(Entity<File>);

impl Global for GlobalFile {}

struct FileState {
    accounts: TreeNode,
    transactions: Vec<Transaction>,
    files: Vec<std::path::PathBuf>,
    currency_converter: CurrencyConverter,
}

impl Default for FileState {
    fn default() -> Self {
        Self {
            accounts: TreeNode::new(),
            transactions: Vec::new(),
            files: Vec::new(),
            currency_converter: CurrencyConverter::new(),
        }
    }
}

impl FileState {
    pub fn load(cli: &Cli, cx: &App) -> Task<Result<Self, Error>> {
        enum Item {
            Transaction(Transaction),
            Price(Price),
            File(std::path::PathBuf),
        }

        cx.background_spawn({
            let cli = cli.clone();
            async move {
                let mut transactions = Vec::new();
                let mut currency_converter = CurrencyConverter::new();
                let mut accounts = TreeNode::new();
                let mut files = Vec::new();

                let transactions_stream =
                    cli.transactions().await?.map(|r| r.map(Item::Transaction));
                let prices_stream = cli.prices().await?.map(|r| r.map(Item::Price));
                let files_stream = cli.files().await?.map(|r| r.map(Item::File));

                let mut combined =
                    std::pin::pin!(transactions_stream.chain(prices_stream).chain(files_stream));

                while let Some(result) = combined.next().await {
                    match result? {
                        Item::Transaction(transaction) => {
                            for posting in transaction.postings.iter() {
                                accounts.add_account(&posting.account);
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
                        Item::File(path) => {
                            files.push(path);
                        }
                    }
                }

                Ok(Self {
                    accounts,
                    transactions,
                    files,
                    currency_converter,
                })
            }
        })
    }
}

pub struct File {
    state: Entity<Result<FileState, Error>>,
    _tasks: Vec<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl File {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalFile>().0.clone()
    }

    pub(crate) fn set_global(file: Entity<Self>, cx: &mut App) {
        cx.set_global(GlobalFile(file));
    }

    pub fn currency_converter(cx: &App) -> Result<&CurrencyConverter, Error> {
        let state = Self::global(cx).read(cx).state.read(cx);
        match state {
            Ok(state) => Ok(&state.currency_converter),
            Err(e) => Err(Error::msg(e.to_string())),
        }
    }

    pub fn transactions(cx: &App) -> Result<Vec<Transaction>, Error> {
        let state = Self::global(cx).read(cx).state.read(cx);
        match state {
            Ok(state) => Ok(state.transactions.clone()),
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
        let mut subscriptions = vec![];

        let load_state = FileState::load(&cli, cx);

        subscriptions.push(
            // watch for changes to the loaded files
            cx.observe_self(|this, cx| {
                if let Ok(state) = this.state.read(cx) {
                    // todo: watch for changes
                    state
                        .files
                        .iter()
                        .for_each(|path| println!("Loaded file: {:?}", path));
                }
            }),
        );

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
            _subscriptions: subscriptions,
            _tasks: tasks,
        }
    }
}
