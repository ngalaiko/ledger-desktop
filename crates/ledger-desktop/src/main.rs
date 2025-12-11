use futures::StreamExt;
use gpui::*;
use gpui_component::label::Label;
use gpui_component::list::{ListDelegate, ListItem, ListState};
use gpui_component::{IndexPath, Root};
use ledger_cli::Ledger;
use std::sync::Arc;
use std::vec;

struct TransactionListDelegate {
    items: Vec<SharedString>,
    selected_index: Option<IndexPath>,
}

impl ListDelegate for TransactionListDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.items.len()
    }

    fn render_item(
        &self,
        ix: IndexPath,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<Self::Item> {
        self.items.get(ix.row).map(|item| {
            ListItem::new(ix)
                .child(Label::new(item.clone()))
                .selected(Some(ix) == self.selected_index)
        })
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }
}

struct LedgerDesktop {
    state: Entity<ListState<TransactionListDelegate>>,
}

impl LedgerDesktop {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Create the list with initial empty items
        let delegate = TransactionListDelegate {
            items: vec!["Loading...".into()],
            selected_index: None,
        };
        let state = cx.new(|cx| ListState::new(delegate, window, cx));

        // Clone state for the spawned task
        let state_clone = state.clone();

        // Spawn a task to initialize ledger and stream output
        cx.spawn_in(window, async move |_, cx| {
            // Initialize ledger
            let ledger = match Ledger::new() {
                Ok(l) => Arc::new(l),
                Err(e) => {
                    state_clone.update(cx, |this, cx| {
                        this.delegate_mut().items =
                            vec![format!("Error initializing ledger: {}", e).into()];
                        cx.notify();
                    })?;
                    return Ok(());
                }
            };

            // Execute a command and stream the results
            let mut stream = ledger.execute("register");

            // Clear the loading message
            state_clone.update(cx, |this, cx| {
                this.delegate_mut().items.clear();
                cx.notify();
            })?;

            // Stream lines as they arrive
            while let Some(result) = stream.next().await {
                match result {
                    Ok(line) => {
                        state_clone.update(cx, |this, cx| {
                            this.delegate_mut().items.push(SharedString::from(line));
                            cx.notify();
                        })?;
                    }
                    Err(e) => {
                        state_clone.update(cx, |this, cx| {
                            this.delegate_mut()
                                .items
                                .push(SharedString::from(format!("Error reading line: {}", e)));
                            cx.notify();
                        })?;
                        break;
                    }
                }
            }

            Ok::<_, anyhow::Error>(())
        })
        .detach();

        Self { state }
    }
}

impl Render for LedgerDesktop {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.state.clone()
    }
}

fn main() {
    let app = Application::new();

    app.run(move |cx| {
        // This must be called before using any GPUI Component features.
        gpui_component::init(cx);

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|cx| LedgerDesktop::new(window, cx));
                // This first level on the window, should be a Root.
                cx.new(|cx| Root::new(view, window, cx))
            })?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();

        cx.activate(true);
    });
}
