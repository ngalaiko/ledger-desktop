mod ledger;
mod sexpr;
mod transactions;

use std::rc::Rc;

use futures_lite::StreamExt;
#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::Disableable;
use gpui_component::Root;
use gpui_component::{v_virtual_list, VirtualListScrollHandle};
use ledger::LedgerHandle;
use transactions::Transaction;

fn main() {
    Application::new().run(move |cx| {
        gpui_component::init(cx);

        let ledger = LedgerHandle::spawn(cx, None);

        cx.open_window(WindowOptions::default(), |window, cx| {
            let view = cx.new(|cx| RegisterView::new(ledger, window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .ok();

        cx.activate(true);
    });
}

struct RegisterView {
    input: Entity<InputState>,
    transactions: Vec<Transaction>,
    busy: bool,
    ledger: LedgerHandle,
    scroll_handle: VirtualListScrollHandle,
}

impl RegisterView {
    fn new(ledger: LedgerHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            input: cx.new(|cx| InputState::new(window, cx)),
            transactions: Vec::new(),
            busy: false,
            ledger,
            scroll_handle: VirtualListScrollHandle::new(),
        };
        view.run_command(window, cx);
        view
    }

    fn run_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }

        let account_filter = self.input.read(cx).text().to_string();
        let account_filter = account_filter.trim();
        let command = if account_filter.is_empty() {
            "lisp --lisp-date-format seconds".to_string()
        } else {
            // Escape forward slashes and backslashes to prevent injection
            let escaped = account_filter.replace('\\', "\\\\").replace('/', "\\/");
            format!("lisp --lisp-date-format seconds account =~ /{}/", escaped)
        };

        self.busy = true;
        self.transactions.clear();
        cx.notify();

        let ledger = self.ledger.clone();

        cx.spawn_in(window, async move |this, cx| {
            let Ok(stream) = ledger.stream(&command).await else {
                this.update(cx, |this, cx| {
                    this.busy = false;
                    cx.notify();
                })
                .ok();
                return;
            };
            let mut stream = stream.sexpr().transactions();

            loop {
                match stream.next().await {
                    Some(Ok(transaction)) => {
                        this.update(cx, |this, cx| {
                            this.transactions.push(transaction);
                            cx.notify();
                        })
                        .ok();
                    }
                    None => {
                        // Command completed successfully
                        break;
                    }
                    Some(Err(e)) => {
                        // Error occurred (including stderr)
                        eprintln!("Error: {}", e);
                        break;
                    }
                }
            }

            this.update(cx, |this, cx| {
                this.busy = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for RegisterView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();

        // Calculate item sizes - estimate height based on postings count
        // Each transaction has: header (40px) + each posting (24px)
        let item_sizes: Rc<Vec<Size<Pixels>>> = Rc::new(
            self.transactions
                .iter()
                .map(|tx| {
                    let height = px(40.0) + px(24.0 * tx.postings.len() as f32);
                    Size {
                        width: px(0.0), // Width is determined by container
                        height,
                    }
                })
                .collect(),
        );

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_2()
            .bg(rgb(0x001a_1a1a))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(Input::new(&self.input).w_full())
                    .child(
                        Button::new("run")
                            .with_variant(ButtonVariant::Primary)
                            .label(if self.busy { "..." } else { "Run" })
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.run_command(window, cx);
                            })),
                    ),
            )
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
                            item_sizes,
                            move |this: &mut RegisterView, range, _window, _cx| {
                                this.transactions[range]
                                    .iter()
                                    .map(|tx| {
                                        let height = px(40.0) + px(24.0 * tx.postings.len() as f32);
                                        div().h(height).p_2().mb_2().rounded_md().bg(rgb(0x001a_1a1a)).child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .text_color(rgb(0x00a0_a0ff))
                                                        .child(format!("{}", tx.description)),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(rgb(0x0080_8080))
                                                        .child(format!(
                                                            "{} - {:?}",
                                                            tx.file.display(),
                                                            tx.time
                                                        )),
                                                )
                                                .children(tx.postings.iter().map(|posting| {
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
                                    })
                                    .collect::<Vec<_>>()
                            },
                        )
                        .track_scroll(&self.scroll_handle),
                    ),
            )
    }
}
