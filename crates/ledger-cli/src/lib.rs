use anyhow::{Context, Result};
use futures::stream::Stream;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context as TaskContext, Poll};
use tokio::sync::mpsc;

pub struct Ledger {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    reader: Arc<Mutex<Box<dyn Read + Send>>>,
}

impl Ledger {
    pub fn new() -> Result<Self> {
        let pty_system = native_pty_system();

        // Create a pseudo-terminal
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to open PTY")?;

        // Spawn ledger in REPL mode within the PTY
        let mut cmd = CommandBuilder::new("ledger");
        cmd.cwd(std::env::current_dir()?);

        let _child = pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn ledger")?;

        // Get reader/writer for the master side
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        let ledger = Ledger {
            writer: Arc::new(Mutex::new(writer)),
            reader: Arc::new(Mutex::new(reader)),
        };

        // Read initial banner and prompt synchronously
        ledger.read_until_prompt_sync()?;

        Ok(ledger)
    }

    /// Read until we see the '] ' prompt (synchronous, used for initialization)
    fn read_until_prompt_sync(&self) -> Result<String> {
        let mut output = Vec::new();
        let mut buf = [0u8; 8192];
        let mut reader = self.reader.lock().unwrap();

        loop {
            let bytes_read = reader.read(&mut buf)?;

            if bytes_read == 0 {
                break;
            }

            output.extend_from_slice(&buf[..bytes_read]);

            let len = output.len();
            if len >= 2 && output[len - 2] == b']' && output[len - 1] == b' ' {
                output.truncate(len - 2);
                break;
            }
        }

        Ok(String::from_utf8_lossy(&output).to_string())
    }

    pub fn execute(&self, command: &str) -> impl Stream<Item = Result<String>> {
        let reader = self.reader.clone();
        let writer = self.writer.clone();
        let command = command.to_string();

        let (tx, rx) = mpsc::unbounded_channel();

        std::thread::spawn(move || {
            // Send command
            let send_result = {
                let mut writer = writer.lock().unwrap();
                writeln!(writer, "--no-pager --no-color {}", command)
                    .and_then(|_| writer.flush())
                    .context("Failed to send command")
            };

            if let Err(e) = send_result {
                let _ = tx.send(Err(e));
                return;
            }

            // Read output line by line until we see the prompt
            let mut accumulated = Vec::new();
            let mut buf = [0u8; 1024];
            let mut line_buffer = String::new();

            let mut line_count = 0;
            loop {
                let bytes_read = {
                    let mut reader = reader.lock().unwrap();
                    match reader.read(&mut buf) {
                        Ok(n) => n,
                        Err(e) => {
                            let _ = tx.send(Err(anyhow::Error::from(e)));
                            return;
                        }
                    }
                };

                if bytes_read == 0 {
                    break;
                }

                accumulated.extend_from_slice(&buf[..bytes_read]);

                // Check for prompt '] '
                let len = accumulated.len();
                let has_prompt =
                    len >= 2 && accumulated[len - 2] == b']' && accumulated[len - 1] == b' ';

                if has_prompt {
                    accumulated.truncate(len - 2);
                }

                // Convert to string and process lines
                let text = String::from_utf8_lossy(&accumulated);
                line_buffer.push_str(&text);
                accumulated.clear();

                // Split by newlines
                let mut lines: Vec<&str> = line_buffer.split('\n').collect();

                if has_prompt {
                    // Send all lines including the last one
                    for line in lines {
                        if tx.send(Ok(line.to_string())).is_err() {
                            return;
                        }
                        line_count += 1;
                    }
                    line_buffer.clear();
                    break;
                } else if lines.len() > 1 {
                    // Keep the last incomplete line in buffer
                    let incomplete = lines.pop().unwrap();
                    for line in lines {
                        line_count += 1;
                        if line_count == 1 {
                            // first line is the prompt, skip it
                            continue;
                        }
                        if tx.send(Ok(line.to_string())).is_err() {
                            return;
                        }
                    }
                    line_buffer = incomplete.to_string();
                }
            }

            // Send remaining line if any
            let _ = tx.send(Ok(line_buffer));
        });

        OutputStream { rx }
    }
}

struct OutputStream {
    rx: mpsc::UnboundedReceiver<Result<String>>,
}

impl Stream for OutputStream {
    type Item = Result<String>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}
