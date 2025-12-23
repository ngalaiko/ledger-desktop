mod ledger;
mod sexpr;
mod transactions;

use std::rc::Rc;

use futures_lite::StreamExt;
#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::Root;
use gpui_component::{v_virtual_list, VirtualListScrollHandle};
use ledger::LedgerHandle;
use transactions::Transaction;

fn main() {
    Application::new().run(move |cx| {
        gpui_component::init(cx);

        cx.open_window(WindowOptions::default(), |window, cx| {
            let view = cx.new(|cx| LedgerFile::new(window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .ok();

        cx.activate(true);
    });
}

#[derive(Debug)]
enum StateUpdatedEvent {
    Loading,
    Loaded { transaction: Transaction },
    Error { message: String },
}

struct LedgerState {
    ledger_handle: LedgerHandle,
}

impl EventEmitter<StateUpdatedEvent> for LedgerState {}

impl LedgerState {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ledger_handle = LedgerHandle::spawn(cx, None);
        let mut ledger_state = Self { ledger_handle };
        ledger_state.reload_state(cx);
        ledger_state
    }

    fn reload_state(&mut self, cx: &mut Context<Self>) {
        let ledger = self.ledger_handle.clone();

        let account_filter = "".trim(); // todo: get from input
        let command = if account_filter.is_empty() {
            "lisp --lisp-date-format seconds".to_string()
        } else {
            // Escape forward slashes and backslashes to prevent injection
            let escaped = account_filter.replace('\\', "\\\\").replace('/', "\\/");
            format!("lisp --lisp-date-format seconds account =~ /{}/", escaped)
        };

        cx.emit(StateUpdatedEvent::Loading);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let Ok(stream) = ledger.stream(&command).await else {
                this.update(cx, |_this, cx| {
                    cx.emit(StateUpdatedEvent::Error {
                        message: "Failed to start ledger command".to_string(),
                    });
                    cx.notify();
                })
                .ok();
                return;
            };
            let mut stream = stream.sexpr().transactions();

            loop {
                match stream.next().await {
                    Some(Ok(transaction)) => {
                        this.update(cx, |_this, cx| {
                            cx.emit(StateUpdatedEvent::Loaded { transaction });
                            cx.notify();
                        })
                        .ok();
                    }
                    None => {
                        // Command completed successfully
                        break;
                    }
                    Some(Err(e)) => {
                        this.update(cx, |_this, cx| {
                            cx.emit(StateUpdatedEvent::Error {
                                message: format!("Error parsing transaction: {}", e),
                            });
                            cx.notify();
                        })
                        .ok();
                        break;
                    }
                }
            }
        })
        .detach();
    }
}

struct LedgerFile {
    register_view: Entity<RegisterView>,
    _ledger_state: Entity<LedgerState>,
}

impl LedgerFile {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ledger_state = cx.new(|cx| LedgerState::new(window, cx));
        Self {
            register_view: cx.new(|cx| RegisterView::new(ledger_state.clone(), window, cx)),
            _ledger_state: ledger_state,
        }
    }
}

impl Render for LedgerFile {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.register_view.clone()
    }
}

struct RegisterView {
    transaction_views: Vec<Entity<TransactionView>>,
    transaction_sizes: Rc<Vec<Size<Pixels>>>,

    scroll_handle: VirtualListScrollHandle,
}

impl RegisterView {
    fn new(
        ledger_state: Entity<LedgerState>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(&ledger_state, |this, _ledger_state, event, cx| {
            match event {
                StateUpdatedEvent::Loading => {
                    // Could show a loading indicator here
                }
                StateUpdatedEvent::Loaded { transaction } => {
                    let transaction_view =
                        cx.new(|cx| TransactionView::new(transaction.clone(), cx));
                    let size = transaction_view.read(cx).size();
                    this.transaction_views.push(transaction_view);
                    let sizes = Rc::make_mut(&mut this.transaction_sizes);
                    sizes.push(size);
                    cx.notify();
                }
                StateUpdatedEvent::Error { message } => {
                    // Could show an error message here
                    eprintln!("Error: {}", message);
                }
            }
        })
        .detach();
        Self {
            transaction_views: vec![],
            transaction_sizes: Rc::new(vec![]),
            scroll_handle: VirtualListScrollHandle::new(),
        }
    }
}

impl Render for RegisterView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_2()
            .bg(rgb(0x001a_1a1a))
            .child(
                div()
                    .flex_grow()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0x0033_3333))
                    .bg(rgb(0x000d_0d0d))
                    .font_family("monospace")
                    .text_sm()
                    .text_color(rgb(0x00e0_e0e0))
                    .child(
                        v_virtual_list(
                            view,
                            "transactions-list",
                            self.transaction_sizes.clone(),
                            move |this: &mut RegisterView, range, _window, _cx| {
                                this.transaction_views[range].to_vec()
                            },
                        )
                        .track_scroll(&self.scroll_handle),
                    ),
            )
    }
}

struct TransactionView {
    transaction: Transaction,
}

impl TransactionView {
    fn new(transaction: Transaction, _cx: &mut Context<Self>) -> Self {
        Self { transaction }
    }

    pub fn size(&self) -> Size<Pixels> {
        let height = 40.0 + 24.0 * self.transaction.postings.len() as f32;
        Size {
            height: px(height),
            width: px(0.0),
        }
    }
}

impl Render for TransactionView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let size = self.size();
        div()
            .h(size.height)
            .p_2()
            .mb_2()
            .rounded_md()
            .bg(rgb(0x001a_1a1a))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_color(rgb(0x00a0_a0ff))
                            .child(format!("{}", self.transaction.description)),
                    )
                    .child(div().text_xs().text_color(rgb(0x0080_8080)).child(format!(
                        "{} - {:?}",
                        self.transaction.file.display(),
                        self.transaction.time
                    )))
                    .children(self.transaction.postings.iter().map(|posting| {
                        div()
                            .pl_4()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .text_color(rgb(0x00ff_ff80))
                                    .child(posting.account.clone()),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x0080_ff80))
                                    .child(posting.amount.clone()),
                            )
                    })),
            )
    }
}
