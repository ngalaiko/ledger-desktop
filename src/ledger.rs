use std::sync::Arc;

use async_channel::{bounded, Receiver, Sender};
use async_process::{Command, Stdio};
use futures_lite::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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
    Line(Vec<u8>),
    Done(Result<(), LedgerError>),
}

struct LedgerCommand {
    cmd: Vec<u8>,
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

    pub async fn send(&self, cmd: Vec<u8>) -> Result<Receiver<LedgerEvent>, ChannelClosed> {
        let (response_tx, response_rx) = bounded(64);
        self.cmd_tx
            .send(LedgerCommand { cmd, response_tx })
            .await
            .map_err(|_| ChannelClosed)?;
        Ok(response_rx)
    }

    pub async fn stream(&self, cmd: &[u8]) -> Result<ByteStream, ChannelClosed> {
        let event_rx = self.send(cmd.to_vec()).await?;
        Ok(ByteStream::from_events(event_rx))
    }
}

pub struct ByteStream {
    rx: Receiver<LedgerEvent>,
}

impl ByteStream {
    fn from_events(rx: Receiver<LedgerEvent>) -> Self {
        Self { rx }
    }

    pub async fn next(&mut self) -> Result<Option<Vec<u8>>, LedgerError> {
        loop {
            match self.rx.recv().await {
                Ok(LedgerEvent::Line(line)) => return Ok(Some(line)),
                Ok(LedgerEvent::Done(Ok(()))) => return Ok(None),
                Ok(LedgerEvent::Done(Err(e))) => return Err(e),
                Err(_) => {
                    return Err(LedgerError::Io(Arc::new(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "Channel closed",
                    ))))
                }
            }
        }
    }

    pub fn sexp(self) -> SexpStream {
        SexpStream::new(self)
    }
}

pub struct SexpStream {
    inner: ByteStream,
    buffer: String,
}

impl SexpStream {
    fn new(inner: ByteStream) -> Self {
        Self {
            inner,
            buffer: String::new(),
        }
    }

    pub async fn next(&mut self) -> Result<Option<lexpr::Value>, LedgerError> {
        loop {
            if let Some((value, consumed)) = try_parse_one(&self.buffer) {
                self.buffer.drain(..consumed);
                return Ok(Some(value));
            }

            match self.inner.next().await {
                Ok(Some(chunk)) => {
                    self.buffer.push_str(&String::from_utf8_lossy(&chunk));
                    self.buffer.push('\n');
                }
                Ok(None) => {
                    if self.buffer.trim().is_empty() {
                        return Ok(None);
                    }
                    match lexpr::from_str(&self.buffer) {
                        Ok(value) => {
                            self.buffer.clear();
                            return Ok(Some(value));
                        }
                        Err(e) => {
                            return Err(LedgerError::Stderr(format!(
                                "Incomplete S-expression at end of stream: {}",
                                e
                            )))
                        }
                    }
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }
}

fn try_parse_one(input: &str) -> Option<(lexpr::Value, usize)> {
    let trimmed = input.trim_start();
    let offset = input.len() - trimmed.len();

    if !trimmed.starts_with('(') {
        return None;
    }

    let mut depth = 0;
    let mut in_string = false;
    let mut escape = false;

    for (i, c) in trimmed.char_indices() {
        if escape {
            escape = false;
            continue;
        }

        match c {
            '\\' if in_string => escape = true,
            '"' => in_string = !in_string,
            '(' if !in_string => depth += 1,
            ')' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    let sexp_str = &trimmed[..=i];
                    if let Ok(value) = lexpr::from_str(sexp_str) {
                        return Some((value, offset + i + 1));
                    }
                    return None;
                }
            }
            _ => {}
        }
    }

    None
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
                        let combined: Vec<u8> = stderr_lines.into_iter().flatten().collect();
                        let error_msg = String::from_utf8_lossy(&combined).trim().to_string();
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
                    if !stderr_lines.is_empty() {
                        let combined: Vec<u8> = stderr_lines.into_iter().flatten().collect();
                        let error_msg = String::from_utf8_lossy(&combined).trim().to_string();
                        response_tx
                            .send(LedgerEvent::Done(Err(LedgerError::Stderr(error_msg))))
                            .await
                            .map_err(ActorError::Send)?;
                    } else {
                        response_tx
                            .send(LedgerEvent::Done(Err(LedgerError::Io(Arc::new(
                                std::io::Error::new(
                                    std::io::ErrorKind::UnexpectedEof,
                                    "Stderr closed",
                                ),
                            )))))
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
    Stdout(Option<Vec<u8>>),
    Stderr(Option<Vec<u8>>),
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

        let stdin = child.stdin.take().ok_or(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to open stdin of ledger process",
        ))?;
        let stdout = child.stdout.take().ok_or(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to open stdout of ledger process",
        ))?;
        let stderr = child.stderr.take().ok_or(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to open stderr of ledger process",
        ))?;

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

    async fn command(&mut self, cmd: &[u8]) -> std::io::Result<()> {
        if !cmd.is_empty() {
            self.stdin.write_all(cmd).await?;
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
                    Ok(ReadResult::Stdout(Some(buf)))
                }
            },
            async {
                let mut buf = Vec::new();
                let n = stderr_reader.read_until(b'\n', &mut buf).await?;
                if n == 0 {
                    Ok(ReadResult::Stderr(None))
                } else {
                    Ok(ReadResult::Stderr(Some(buf)))
                }
            },
        )
        .await
    }

    async fn read_line(&mut self) -> std::io::Result<Option<Vec<u8>>> {
        let mut buf = Vec::new();
        let n = self.stdout_reader.read_until(b'\n', &mut buf).await?;
        if n == 0 || buf.strip_suffix(b"\n").unwrap_or(&buf) == MARKER {
            return Ok(None);
        }
        Ok(Some(buf))
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
                .stream(b"balance")
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
                .stream(b"invalid")
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

            let stream = handle
                .stream(b"lisp")
                .await
                .expect("Failed to send command");
            let mut sexp_stream = stream.sexp();

            let mut transactions = 0;
            loop {
                match sexp_stream.next().await {
                    Ok(Some(value)) => {
                        assert!(value.is_list(), "Should be a list/s-expression");
                        transactions += 1;
                    }
                    Ok(None) => break,
                    Err(e) => panic!("Failed to parse s-expression: {:?}", e),
                }
            }

            assert!(transactions > 0, "Should have parsed one transaction");
        });
    }
}
