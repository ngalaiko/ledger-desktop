use std::sync::Arc;

use async_channel::{bounded, Receiver, Sender};
use async_process::{Command, Stdio};
use futures_lite::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::sexpr;

const MARKER: &[u8] = b"__END_OF_RESPONSE__";

#[derive(Debug, Clone, thiserror::Error)]
pub enum LedgerError {
    #[error(transparent)]
    Io(#[from] Arc<std::io::Error>),
    #[error("{0}")]
    Stderr(String),
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("Channel closed")]
pub struct ChannelClosed;

#[derive(Debug, Clone)]
pub enum LedgerEvent {
    Line(String),
    Done(Result<(), LedgerError>),
}

struct LedgerCommand {
    cmd: String,
    response_tx: Sender<LedgerEvent>,
}

#[derive(Clone)]
pub struct LedgerHandle {
    cmd_tx: Sender<LedgerCommand>,
}

impl LedgerHandle {
    pub fn spawn(cx: &mut gpui::App, file: Option<std::path::PathBuf>) -> Self {
        let (cmd_tx, cmd_rx) = bounded::<LedgerCommand>(16);

        cx.background_executor()
            .spawn(async move {
                run_actor(file, cmd_rx).await.expect("Ledger actor failed");
            })
            .detach();

        Self { cmd_tx }
    }

    pub async fn send(&self, cmd: String) -> Result<Receiver<LedgerEvent>, ChannelClosed> {
        let (response_tx, response_rx) = bounded(64);
        self.cmd_tx
            .send(LedgerCommand { cmd, response_tx })
            .await
            .map_err(|_| ChannelClosed)?;
        Ok(response_rx)
    }

    pub async fn stream(&self, cmd: &str) -> Result<LineStream, ChannelClosed> {
        let event_rx = self.send(cmd.to_string()).await?;
        Ok(LineStream::from_events(event_rx))
    }
}

pub struct LineStream {
    rx: Receiver<LedgerEvent>,
}

impl LineStream {
    fn from_events(rx: Receiver<LedgerEvent>) -> Self {
        Self { rx }
    }

    pub async fn next(&mut self) -> Result<Option<String>, LedgerError> {
        match self.rx.recv().await {
            Ok(LedgerEvent::Line(line)) => Ok(Some(line)),
            Ok(LedgerEvent::Done(Ok(()))) => Ok(None),
            Ok(LedgerEvent::Done(Err(e))) => Err(e),
            Err(_) => Err(LedgerError::Io(Arc::new(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "Channel closed",
            )))),
        }
    }

    pub fn sexp(self) -> SexpStream {
        SexpStream::new(self)
    }
}

pub struct SexpStream {
    inner: LineStream,
    parser: sexpr::Parser,
    pending: Vec<sexpr::Value>,
}

impl SexpStream {
    fn new(inner: LineStream) -> Self {
        Self {
            inner,
            parser: sexpr::Parser::new(),
            pending: Vec::new(),
        }
    }

    pub async fn next(&mut self) -> Result<Option<sexpr::Value>, LedgerError> {
        loop {
            // Return pending values first
            if let Some(value) = self.pending.pop() {
                return Ok(Some(value));
            }

            match self.inner.next().await {
                Ok(Some(line)) => {
                    self.parser.take(&line).map_err(|e| {
                        LedgerError::Stderr(format!("S-expression parse error: {e}"))
                    })?;

                    // Check if any complete s-expressions are ready
                    let mut completed = self.parser.drain_output();
                    if !completed.is_empty() {
                        // Reverse so we can pop from the end
                        completed.reverse();
                        self.pending = completed;
                    }
                }
                Ok(None) => {
                    // Stream ended - finish parsing
                    let parser = std::mem::replace(&mut self.parser, sexpr::Parser::new());
                    let mut values = parser.finish().map_err(|e| {
                        LedgerError::Stderr(format!("S-expression parse error: {e}"))
                    })?;

                    if values.is_empty() {
                        return Ok(None);
                    }

                    values.reverse();
                    self.pending = values;
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ActorError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Send(#[from] async_channel::SendError<LedgerEvent>),
}

async fn run_actor(
    file: Option<std::path::PathBuf>,
    cmd_rx: Receiver<LedgerCommand>,
) -> Result<(), ActorError> {
    let mut ledger = Ledger::spawn(file).await.map_err(ActorError::Io)?;

    while let Ok(command) = cmd_rx.recv().await {
        let LedgerCommand { cmd, response_tx } = command;

        if let Err(e) = ledger.command(&cmd).await {
            response_tx
                .send(LedgerEvent::Done(Err(LedgerError::Io(Arc::new(e)))))
                .await
                .map_err(ActorError::Send)?;
            continue;
        }

        // Accumulate stderr in case we see multiple lines before marker
        let mut stderr_lines = Vec::new();

        loop {
            match ledger.read_either().await {
                Ok(ReadResult::Stdout(Some(line))) => {
                    // Got stdout line
                    if response_tx.send(LedgerEvent::Line(line)).await.is_err() {
                        // Receiver dropped - drain remaining output
                        while let Ok(Some(_)) = ledger.read_line().await {}
                        break;
                    }
                }
                Ok(ReadResult::Stdout(None)) => {
                    // Marker reached
                    if stderr_lines.is_empty() {
                        // No stderr seen - success
                        response_tx
                            .send(LedgerEvent::Done(Ok(())))
                            .await
                            .map_err(ActorError::Send)?;
                    } else {
                        // Had stderr - return error
                        let error_msg = stderr_lines.join("").trim().to_string();
                        response_tx
                            .send(LedgerEvent::Done(Err(LedgerError::Stderr(error_msg))))
                            .await
                            .map_err(ActorError::Send)?;
                    }
                    break;
                }
                Ok(ReadResult::Stderr(Some(line))) => {
                    // Got stderr line - accumulate it
                    stderr_lines.push(line);
                }
                Ok(ReadResult::Stderr(None)) => {
                    // Stderr EOF - shouldn't happen normally, but treat as error if we have stderr
                    if stderr_lines.is_empty() {
                        response_tx
                            .send(LedgerEvent::Done(Err(LedgerError::Io(Arc::new(
                                std::io::Error::new(
                                    std::io::ErrorKind::UnexpectedEof,
                                    "Stderr closed",
                                ),
                            )))))
                            .await
                            .map_err(ActorError::Send)?;
                    } else {
                        let error_msg = stderr_lines.join("").trim().to_string();
                        response_tx
                            .send(LedgerEvent::Done(Err(LedgerError::Stderr(error_msg))))
                            .await
                            .map_err(ActorError::Send)?;
                    }
                    break;
                }
                Err(e) => {
                    response_tx
                        .send(LedgerEvent::Done(Err(LedgerError::Io(Arc::new(e)))))
                        .await
                        .map_err(ActorError::Send)?;
                    break;
                }
            }
        }
    }

    Ok(())
}

struct Ledger {
    stdin: async_process::ChildStdin,
    stdout_reader: BufReader<async_process::ChildStdout>,
    stderr_reader: BufReader<async_process::ChildStderr>,
    _child: async_process::Child,
}

enum ReadResult {
    Stdout(Option<String>),
    Stderr(Option<String>),
}

impl Ledger {
    async fn spawn(file: Option<std::path::PathBuf>) -> std::io::Result<Self> {
        let mut cmd = Command::new("ledger");

        if let Some(file_path) = file {
            cmd.arg("--file").arg(file_path);
        }

        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or(std::io::Error::other("Failed to open stdin of ledger process"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or(std::io::Error::other("Failed to open stdout of ledger process"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(std::io::Error::other("Failed to open stderr of ledger process"))?;

        let stdout_reader = BufReader::new(stdout);
        let stderr_reader = BufReader::new(stderr);

        let mut repl = Self {
            stdin,
            stdout_reader,
            stderr_reader,
            _child: child,
        };
        repl.drain().await?;

        Ok(repl)
    }

    async fn drain(&mut self) -> std::io::Result<()> {
        self.stdin.write_all(b"echo ").await?;
        self.stdin.write_all(MARKER).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;

        let mut buf = Vec::new();
        loop {
            buf.clear();
            let n = self.stdout_reader.read_until(b'\n', &mut buf).await?;
            if n == 0 || buf.strip_suffix(b"\n").unwrap_or(&buf) == MARKER {
                break;
            }
        }
        Ok(())
    }

    async fn command(&mut self, cmd: &str) -> std::io::Result<()> {
        if !cmd.is_empty() {
            self.stdin.write_all(cmd.as_bytes()).await?;
            self.stdin.write_all(b"\n").await?;
        }
        self.stdin.write_all(b"echo ").await?;
        self.stdin.write_all(MARKER).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await
    }

    /// Read from either stdout or stderr, whichever has data first
    async fn read_either(&mut self) -> std::io::Result<ReadResult> {
        let stdout_reader = &mut self.stdout_reader;
        let stderr_reader = &mut self.stderr_reader;

        futures_lite::future::race(
            async {
                let mut buf = Vec::new();
                let n = stdout_reader.read_until(b'\n', &mut buf).await?;
                if n == 0 || buf.strip_suffix(b"\n").unwrap_or(&buf) == MARKER {
                    Ok(ReadResult::Stdout(None))
                } else {
                    let line = String::from_utf8_lossy(&buf).into_owned();
                    Ok(ReadResult::Stdout(Some(line)))
                }
            },
            async {
                let mut buf = Vec::new();
                let n = stderr_reader.read_until(b'\n', &mut buf).await?;
                if n == 0 {
                    Ok(ReadResult::Stderr(None))
                } else {
                    let line = String::from_utf8_lossy(&buf).into_owned();
                    Ok(ReadResult::Stderr(Some(line)))
                }
            },
        )
        .await
    }

    async fn read_line(&mut self) -> std::io::Result<Option<String>> {
        let mut buf = Vec::new();
        let n = self.stdout_reader.read_until(b'\n', &mut buf).await?;
        if n == 0 || buf.strip_suffix(b"\n").unwrap_or(&buf) == MARKER {
            return Ok(None);
        }
        let line = String::from_utf8_lossy(&buf).into_owned();
        Ok(Some(line))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_command_no_stderr() {
        futures_lite::future::block_on(async {
            // Set up actor manually (without gpui)
            let (cmd_tx, cmd_rx) = bounded::<LedgerCommand>(16);

            // Spawn actor in background
            std::thread::spawn(move || futures_lite::future::block_on(run_actor(None, cmd_rx)));

            let handle = LedgerHandle { cmd_tx };

            // Send valid command
            let mut stream = handle
                .stream("balance")
                .await
                .expect("Failed to send command");

            // Read all events and ensure no errors
            loop {
                match stream.next().await {
                    Ok(Some(_line)) => {
                        // Got output, continue
                    }
                    Ok(None) => {
                        // Done - this is success
                        break;
                    }
                    Err(e) => {
                        panic!("Valid command should not produce error, got: {:?}", e);
                    }
                }
            }
        });
    }

    #[test]
    fn test_invalid_command_produces_stderr_error() {
        futures_lite::future::block_on(async {
            // Set up actor manually
            let (cmd_tx, cmd_rx) = bounded::<LedgerCommand>(16);

            std::thread::spawn(move || futures_lite::future::block_on(run_actor(None, cmd_rx)));

            let handle = LedgerHandle { cmd_tx };

            // Send invalid command
            let mut stream = handle
                .stream("invalid")
                .await
                .expect("Failed to send command");

            // Read events - should eventually get a stderr error
            let error = loop {
                match stream.next().await {
                    Ok(Some(_line)) => continue,
                    Ok(None) => panic!("Invalid command should produce error, not success"),
                    Err(e) => break e,
                }
            };

            // Verify it's a stderr error
            match error {
                LedgerError::Stderr(msg) => {
                    assert!(!msg.is_empty(), "Stderr message should not be empty");
                }
                _ => panic!("Expected LedgerError::Stderr, got: {:?}", error),
            }
        });
    }

    #[test]
    fn test_sexp_stream() {
        futures_lite::future::block_on(async {
            let manifest_dir = env!("CARGO_MANIFEST_DIR");
            let test_file =
                std::path::PathBuf::from(manifest_dir).join("src/fixtures/jornal.ledger");
            let (cmd_tx, cmd_rx) = bounded::<LedgerCommand>(16);

            std::thread::spawn(move || {
                futures_lite::future::block_on(run_actor(Some(test_file), cmd_rx))
            });

            let handle = LedgerHandle { cmd_tx };

            let stream = handle.stream("lisp").await.expect("Failed to send command");
            let mut sexp_stream = stream.sexp();

            let mut transactions = 0;
            loop {
                match sexp_stream.next().await {
                    Ok(Some(value)) => {
                        assert!(
                            matches!(value, sexpr::Value::List(_)),
                            "Should be a list/s-expression"
                        );
                        transactions += 1;
                    }
                    Ok(None) => break,
                    Err(e) => panic!("Failed to parse s-expression: {:?}", e),
                }
            }

            assert_eq!(transactions, 1, "Should have parsed one transaction");
        });
    }
}
