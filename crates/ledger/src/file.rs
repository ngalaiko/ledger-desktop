use anyhow::Error;
use futures_lite::StreamExt;
use gpui::{App, AppContext, Context, Entity, Global, Subscription, Task};

use crate::{cli::Cli, Price, Transaction};

pub fn init<P: AsRef<std::path::Path>>(path: Option<P>, cx: &mut App) {
    File::set_global(cx.new(|cx| File::new(path, cx)), cx);
}

struct GlobalFile(Entity<File>);

impl Global for GlobalFile {}

#[derive(Default)]
pub struct FileState {
    pub transactions: Vec<Transaction>,
    pub prices: Vec<Price>,
    pub files: Vec<std::path::PathBuf>,
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
                let mut prices = Vec::new();
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
        })
    }
}

pub struct File {
    pub state: Result<FileState, Error>,
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

    pub fn state(cx: &App) -> &Result<FileState, Error> {
        &Self::global(cx).read(cx).state
    }

    pub fn prices(cx: &App) -> Result<&[Price], Error> {
        let state = &Self::global(cx).read(cx).state;
        match state {
            Ok(state) => Ok(&state.prices),
            Err(e) => Err(Error::msg(e.to_string())),
        }
    }

    pub fn transactions(cx: &App) -> Result<&[Transaction], Error> {
        let state = &Self::global(cx).read(cx).state;
        match state {
            Ok(state) => Ok(&state.transactions),
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
            cx.observe_self(|this, _cx| {
                if let Ok(state) = &this.state {
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
                    this.state = state;
                    cx.notify();
                })
                .ok();
            }),
        );

        Self {
            state: Ok(FileState::default()),
            _subscriptions: subscriptions,
            _tasks: tasks,
        }
    }
}
