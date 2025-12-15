mod ledger;

use gpui::*;
use gpui_component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::Disableable;
use gpui_component::Root;
use ledger::LedgerHandle;

fn main() {
    Application::new().run(move |cx| {
        gpui_component::init(cx);

        let ledger = LedgerHandle::spawn(cx);

        cx.open_window(WindowOptions::default(), |window, cx| {
            let view = cx.new(|cx| ReplView::new(ledger, window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .ok();

        cx.activate(true);
    });
}

struct ReplView {
    input: Entity<InputState>,
    lines: Vec<SharedString>,
    busy: bool,
    ledger: LedgerHandle,
}

impl ReplView {
    fn new(ledger: LedgerHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            input: cx.new(|cx| InputState::new(window, cx)),
            lines: Vec::new(),
            busy: false,
            ledger,
        }
    }

    fn run_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }

        let command = self.input.read(cx).text().to_string();
        if command.is_empty() {
            return;
        }

        self.busy = true;
        self.lines.clear();
        cx.notify();

        let ledger = self.ledger.clone();
        let cmd = command.into_bytes();

        cx.spawn_in(window, async move |this, cx| {
            let Some(mut stream) = ledger.stream(&cmd).await else {
                this.update(cx, |this, cx| {
                    this.lines.push("Ledger not available".into());
                    this.busy = false;
                    cx.notify();
                })
                .ok();
                return;
            };

            while let Some(line) = stream.next().await {
                let s = String::from_utf8_lossy(&line).trim_end().to_string();
                this.update(cx, |this, cx| {
                    this.lines.push(s.into());
                    cx.notify();
                })
                .ok();
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

impl Render for ReplView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_2()
            .bg(rgb(0x1a1a1a))
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
                    .border_color(rgb(0x333333))
                    .bg(rgb(0x0d0d0d))
                    .font_family("monospace")
                    .text_sm()
                    .text_color(rgb(0xe0e0e0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .children(self.lines.iter().map(|line| div().child(line.clone()))),
                    ),
            )
    }
}
