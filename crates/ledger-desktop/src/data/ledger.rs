use anyhow::Error;
use gpui::{App, AppContext, Context, Entity, Global, Subscription, Task};

use ledger::{self, cli::Cli, Transaction};

pub fn init<P: AsRef<std::path::Path>>(path: Option<P>, cx: &mut App) {
    Ledger::set_global(cx.new(|cx| Ledger::new(path, cx)), cx);
}

struct GlobalLedger(Entity<Ledger>);

impl Global for GlobalLedger {}

pub struct Ledger {
    pub file: Result<ledger::File, Error>,
    _tasks: Vec<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Ledger {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalLedger>().0.clone()
    }

    pub(crate) fn set_global(file: Entity<Self>, cx: &mut App) {
        cx.set_global(GlobalLedger(file));
    }

    pub fn transactions(cx: &App) -> Result<&[Transaction], Error> {
        let state = &Self::global(cx).read(cx).file;
        match state {
            Ok(state) => Ok(&state.transactions),
            Err(e) => Err(Error::msg(e.to_string())),
        }
    }

    fn new<P: AsRef<std::path::Path>>(path: Option<P>, cx: &mut Context<Self>) -> Self {
        let cli = Cli::new(path);

        let mut tasks = vec![];
        let mut subscriptions = vec![];

        subscriptions.push(
            // watch for changes to the loaded files
            cx.observe_self(|this, _cx| {
                if let Ok(state) = &this.file {
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
                let state = ledger::File::load(&cli).await;
                this.update(cx, |this, cx| {
                    this.file = state;
                    cx.notify();
                })
                .ok();
            }),
        );

        Self {
            file: Ok(ledger::File::default()),
            _subscriptions: subscriptions,
            _tasks: tasks,
        }
    }
}
