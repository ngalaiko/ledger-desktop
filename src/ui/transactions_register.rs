use std::rc::Rc;

#[allow(clippy::wildcard_imports)]
use gpui::*;
use gpui_component::v_virtual_list;

use crate::{transactions::Transaction, ui::state::StateUpdatedEvent};

use super::state::LedgerState;

pub struct RegisterView {
    transaction_views: Vec<Entity<TransactionView>>,
    transaction_sizes: Rc<Vec<Size<Pixels>>>,
}

impl RegisterView {
    pub fn new(ledger_state: Entity<LedgerState>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(
            &ledger_state,
            |this, _ledger_state, event, cx| match event {
                StateUpdatedEvent::Reset => {
                    this.transaction_views.clear();
                    Rc::make_mut(&mut this.transaction_sizes).clear();
                    cx.notify();
                }
                StateUpdatedEvent::NewAccount { .. } => {
                    // No action needed for new accounts
                }
                StateUpdatedEvent::NewTransaction { transaction } => {
                    let transaction_view =
                        cx.new(|cx| TransactionView::new(transaction.clone(), cx));
                    let size = transaction_view.read(cx).size();
                    this.transaction_views.push(transaction_view);
                    let sizes = Rc::make_mut(&mut this.transaction_sizes);
                    sizes.push(size);
                    cx.notify();
                }
                StateUpdatedEvent::Error { message: _message } => {
                    todo!();
                }
            },
        )
        .detach();
        Self {
            transaction_views: vec![],
            transaction_sizes: Rc::new(vec![]),
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
                    .child(v_virtual_list(
                        view,
                        "transactions-list",
                        self.transaction_sizes.clone(),
                        move |this: &mut RegisterView, range, _window, _cx| {
                            this.transaction_views[range].to_vec()
                        },
                    )),
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
                                    .child(posting.account.to_string()),
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
