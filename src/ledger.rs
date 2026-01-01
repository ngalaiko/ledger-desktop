pub mod accounts;
pub mod amounts;
pub mod prices;
mod sexpr;
pub mod transactions;

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use async_channel::{bounded, Receiver, Sender};
use async_io::Timer;
use async_process::{Command, Stdio};
use futures_lite::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use futures_lite::{Future, Stream};

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

#[derive(Clone)]
pub struct LedgerHandle {
    pool: Arc<PoolManager>,
    executor: gpui::BackgroundExecutor,
}

impl LedgerHandle {
    pub fn spawn(cx: &mut gpui::App, file: Option<std::path::PathBuf>) -> Self {
        let executor = cx.background_executor();
        let pool = Arc::new(PoolManager::new(file, &executor));

        Self {
            pool,
            executor: executor.clone(),
        }
    }

    async fn send(&self, cmd: &str) -> Result<Receiver<LedgerEvent>, ChannelClosed> {
        let (response_tx, response_rx) = bounded(64);
        let cmd = cmd.to_string();
        let pool = self.pool.clone();

        // Spawn a task to execute this command
        self.executor
            .spawn(async move {
                if let Err(e) = execute_command(pool, cmd, response_tx).await {
                    // Log error - the task failed but we've already sent error events if possible
                    eprintln!("Command execution task failed: {:?}", e);
                }
            })
            .detach();

        Ok(response_rx)
    }

    pub async fn transactions(&self) -> Result<TransactionStream, ChannelClosed> {
        let event_rx = self.send("lisp --lisp-date-format %Y-%m-%d").await?;
        let line_stream = LineStream::from_events(event_rx);
        Ok(line_stream.sexpr().transactions())
    }

    pub async fn prices(&self) -> Result<PricesStream, ChannelClosed> {
        let event_rx = self.send("prices").await?;
        let line_stream = LineStream::from_events(event_rx);
        Ok(PricesStream::new(line_stream))
    }
}

struct IdleProcess {
    ledger: Ledger,
    idle_since: Instant,
}

struct PoolManager {
    file: Option<std::path::PathBuf>,
    idle_processes: Arc<Mutex<Vec<IdleProcess>>>,
}

impl PoolManager {
    fn new(file: Option<std::path::PathBuf>, executor: &gpui::BackgroundExecutor) -> Self {
        let idle_processes = Arc::new(Mutex::new(Vec::new()));

        // Spawn cleanup task to remove idle processes after timeout
        let idle_processes_clone = idle_processes.clone();
        executor
            .spawn(async move {
                loop {
                    Timer::after(Duration::from_secs(10)).await;
                    Self::cleanup_idle(&idle_processes_clone, Duration::from_secs(30));
                }
            })
            .detach();

        Self {
            file,
            idle_processes,
        }
    }

    async fn acquire(&self) -> std::io::Result<Ledger> {
        // Try to get an idle process
        {
            let mut processes = self.idle_processes.lock().unwrap();
            if let Some(idle) = processes.pop() {
                return Ok(idle.ledger);
            }
        }

        // No idle process available, spawn a new one
        Ledger::spawn(self.file.clone()).await
    }

    fn release(&self, ledger: Ledger) {
        let mut processes = self.idle_processes.lock().unwrap();
        processes.push(IdleProcess {
            ledger,
            idle_since: Instant::now(),
        });
    }

    fn cleanup_idle(idle_processes: &Mutex<Vec<IdleProcess>>, timeout: Duration) {
        let mut processes = idle_processes.lock().unwrap();
        let now = Instant::now();
        processes.retain(|idle| now.duration_since(idle.idle_since) < timeout);
        // Dropped processes will be cleaned up automatically
    }
}

#[derive(Clone)]
pub struct LedgerHandle {
    pool: Arc<PoolManager>,
    executor: gpui::BackgroundExecutor,
}

impl LedgerHandle {
    pub fn spawn(cx: &mut gpui::App, file: Option<std::path::PathBuf>) -> Self {
        let executor = cx.background_executor();
        let pool = Arc::new(PoolManager::new(file, &executor));

        Self {
            pool,
            executor: executor.clone(),
        }
    }

    async fn send(&self, cmd: &str) -> Result<Receiver<LedgerEvent>, ChannelClosed> {
        let (response_tx, response_rx) = bounded(64);
        let cmd = cmd.to_string();
        let pool = self.pool.clone();

        // Spawn a task to execute this command
        self.executor
            .spawn(async move {
                if let Err(e) = execute_command(pool, cmd, response_tx).await {
                    // Log error - the task failed but we've already sent error events if possible
                    eprintln!("Command execution task failed: {:?}", e);
                }
            })
            .detach();

        Ok(response_rx)
    }

    pub async fn transactions(&self) -> Result<TransactionStream, ChannelClosed> {
        let event_rx = self.send("lisp --lisp-date-format %Y-%m-%d").await?;
        let line_stream = LineStream::from_events(event_rx);
        Ok(line_stream.sexpr().transactions())
    }

    pub async fn prices(&self) -> Result<PricesStream, ChannelClosed> {
        let event_rx = self.send("prices").await?;
        let line_stream = LineStream::from_events(event_rx);
        Ok(PricesStream::new(line_stream))
    }
}

pin_project_lite::pin_project! {
    pub struct LineStream {
        rx: Receiver<LedgerEvent>,
        #[pin]
        pending: Option<Pin<Box<dyn std::future::Future<Output = Result<LedgerEvent, async_channel::RecvError>> + Send>>>,
    }
}

impl LineStream {
    fn from_events(rx: Receiver<LedgerEvent>) -> Self {
        Self { rx, pending: None }
    }

    pub fn sexpr(self) -> SexpStream {
        SexpStream::new(self)
    }
}

impl Stream for LineStream {
    type Item = Result<String, LedgerError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            // If we have a pending future, poll it
            if let Some(fut) = this.pending.as_mut().as_pin_mut() {
                match fut.poll(cx) {
                    Poll::Ready(result) => {
                        // Clear the pending future
                        this.pending.set(None);

                        return match result {
                            Ok(LedgerEvent::Line(line)) => Poll::Ready(Some(Ok(line))),
                            Ok(LedgerEvent::Done(Ok(()))) => Poll::Ready(None),
                            Ok(LedgerEvent::Done(Err(e))) => Poll::Ready(Some(Err(e))),
                            Err(_) => Poll::Ready(Some(Err(LedgerError::Io(Arc::new(
                                std::io::Error::new(
                                    std::io::ErrorKind::BrokenPipe,
                                    "Channel closed",
                                ),
                            ))))),
                        };
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }

            // No pending future, create a new one
            let rx = this.rx.clone();
            this.pending
                .set(Some(Box::pin(async move { rx.recv().await })));
        }
    }
}

pin_project_lite::pin_project! {
    pub struct SexpStream {
        #[pin]
        inner: LineStream,
        parser: sexpr::Parser,
        pending: Vec<sexpr::Value>,
        finished: bool,
    }
}

impl SexpStream {
    pub fn new(inner: LineStream) -> Self {
        Self {
            inner,
            parser: sexpr::Parser::new(),
            pending: Vec::new(),
            finished: false,
        }
    }

    pub fn transactions(self) -> TransactionStream {
        TransactionStream::new(self)
    }
}

impl Stream for SexpStream {
    type Item = Result<sexpr::Value, LedgerError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            // Return pending values first
            if let Some(value) = this.pending.pop() {
                return Poll::Ready(Some(Ok(value)));
            }

            // If we've already finished, return None
            if *this.finished {
                return Poll::Ready(None);
            }

            // Poll the inner stream
            match this.inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(line))) => {
                    // Got a line, parse it
                    if let Err(e) = this.parser.take(&line) {
                        *this.finished = true;
                        return Poll::Ready(Some(Err(LedgerError::Stderr(format!(
                            "S-expression parse error: {e}"
                        )))));
                    }

                    // Check if any complete s-expressions are ready
                    let mut completed = this.parser.drain_output();
                    if !completed.is_empty() {
                        // Reverse so we can pop from the end
                        completed.reverse();
                        *this.pending = completed;
                        // Continue loop to return the first pending value
                    }
                    // If no completed values yet, continue polling
                }
                Poll::Ready(Some(Err(e))) => {
                    *this.finished = true;
                    return Poll::Ready(Some(Err(e)));
                }
                Poll::Ready(None) => {
                    // Stream ended - finish parsing
                    *this.finished = true;
                    let parser = std::mem::replace(this.parser, sexpr::Parser::new());
                    match parser.finish() {
                        Ok(mut values) => {
                            if values.is_empty() {
                                return Poll::Ready(None);
                            }
                            values.reverse();
                            *this.pending = values;
                            // Continue loop to return the first pending value
                        }
                        Err(e) => {
                            return Poll::Ready(Some(Err(LedgerError::Stderr(format!(
                                "S-expression parse error: {e}"
                            )))));
                        }
                    }
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

pin_project_lite::pin_project! {
    pub struct PricesStream {
        #[pin]
        inner: LineStream,
    }
}

impl PricesStream {
    pub fn new(inner: LineStream) -> Self {
        Self { inner }
    }
}

impl Stream for PricesStream {
    type Item = Result<prices::Price, LedgerError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        match this.inner.poll_next(cx) {
            Poll::Ready(Some(Ok(line))) => match prices::Price::from_str(&line) {
                Ok(price) => Poll::Ready(Some(Ok(price))),
                Err(e) => Poll::Ready(Some(Err(LedgerError::Stderr(format!(
                    "Failed to parse transaction: {}",
                    e
                ))))),
            },
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

pin_project_lite::pin_project! {
    pub struct TransactionStream {
        #[pin]
        inner: SexpStream,
    }
}

impl TransactionStream {
    pub fn new(inner: SexpStream) -> Self {
        Self { inner }
    }
}

impl Stream for TransactionStream {
    type Item = Result<transactions::Transaction, LedgerError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        match this.inner.poll_next(cx) {
            Poll::Ready(Some(Ok(sexpr_value))) => {
                // Parse the sexpr value as a transaction
                let sexpr::Value::List(ref list) = sexpr_value else {
                    return Poll::Ready(Some(Err(LedgerError::Stderr(format!(
                        "Expected list for transaction, got: {:?}",
                        sexpr_value
                    )))));
                };

                match transactions::Transaction::from_sexpr(list) {
                    Ok(transaction) => Poll::Ready(Some(Ok(transaction))),
                    Err(e) => Poll::Ready(Some(Err(LedgerError::Stderr(format!(
                        "Failed to parse transaction: {}",
                        e
                    ))))),
                }
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
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

async fn execute_command(
    pool: Arc<PoolManager>,
    cmd: String,
    response_tx: Sender<LedgerEvent>,
) -> Result<(), ActorError> {
    // Acquire a process from the pool
    let mut ledger = pool.acquire().await.map_err(ActorError::Io)?;

    // Execute the command
    if let Err(e) = ledger.command(&cmd).await {
        response_tx
            .send(LedgerEvent::Done(Err(LedgerError::Io(Arc::new(e)))))
            .await
            .map_err(ActorError::Send)?;
        pool.release(ledger);
        return Ok(());
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
                            std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Stderr closed"),
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

    // Release the process back to the pool
    pool.release(ledger);

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

        let stdin = child.stdin.take().ok_or(std::io::Error::other(
            "Failed to open stdin of ledger process",
        ))?;
        let stdout = child.stdout.take().ok_or(std::io::Error::other(
            "Failed to open stdout of ledger process",
        ))?;
        let stderr = child.stderr.take().ok_or(std::io::Error::other(
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
    use futures_lite::StreamExt;

    // Test helper to create a handle without gpui
    struct TestHandle {
        pool: Arc<PoolManager>,
    }

    impl TestHandle {
        fn new(file: Option<std::path::PathBuf>) -> Self {
            Self {
                pool: Arc::new(PoolManager {
                    file,
                    idle_processes: Arc::new(Mutex::new(Vec::new())),
                }),
            }
        }

        async fn stream(&self, cmd: &str) -> Result<LineStream, ChannelClosed> {
            let (response_tx, response_rx) = bounded(64);
            let cmd = cmd.to_string();
            let pool = self.pool.clone();

            // Spawn task in a thread
            std::thread::spawn(move || {
                futures_lite::future::block_on(async move {
                    let _ = execute_command(pool, cmd, response_tx).await;
                });
            });

            Ok(LineStream::from_events(response_rx))
        }
    }

    #[test]
    fn test_valid_command_no_stderr() {
        futures_lite::future::block_on(async {
            let handle = TestHandle::new(None);

            // Send valid command
            let mut stream = handle
                .stream("balance")
                .await
                .expect("Failed to send command");

            // Read all events and ensure no errors
            loop {
                match stream.next().await {
                    Some(Ok(_line)) => {
                        // Got output, continue
                    }
                    None => {
                        // Done - this is success
                        break;
                    }
                    Some(Err(e)) => {
                        panic!("Valid command should not produce error, got: {:?}", e);
                    }
                }
            }
        });
    }

    #[test]
    fn test_invalid_command_produces_stderr_error() {
        futures_lite::future::block_on(async {
            let handle = TestHandle::new(None);

            // Send invalid command
            let mut stream = handle
                .stream("invalid")
                .await
                .expect("Failed to send command");

            // Read events - should eventually get a stderr error
            let error = loop {
                match stream.next().await {
                    Some(Ok(_line)) => continue,
                    None => panic!("Invalid command should produce error, not success"),
                    Some(Err(e)) => break e,
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
            let handle = TestHandle::new(Some(test_file));

            let stream = handle.stream("lisp").await.expect("Failed to send command");
            let mut sexp_stream = stream.sexpr();

            let mut transactions = 0;
            loop {
                match sexp_stream.next().await {
                    Some(Ok(value)) => {
                        assert!(
                            matches!(value, sexpr::Value::List(_)),
                            "Should be a list/s-expression"
                        );
                        transactions += 1;
                    }
                    None => break,
                    Some(Err(e)) => panic!("Failed to parse s-expression: {:?}", e),
                }
            }

            assert_eq!(transactions, 1, "Should have parsed one transaction");
        });
    }

    #[test]
    fn test_concurrent_commands() {
        futures_lite::future::block_on(async {
            let handle = TestHandle::new(None);

            // Spawn two concurrent commands
            let handle1 = handle.pool.clone();
            let handle2 = handle.pool.clone();

            let task1 = std::thread::spawn(move || {
                futures_lite::future::block_on(async move {
                    let mut count = 0;
                    let mut stream = {
                        let (response_tx, response_rx) = bounded(64);
                        std::thread::spawn(move || {
                            futures_lite::future::block_on(async move {
                                let _ =
                                    execute_command(handle1, "balance".to_string(), response_tx)
                                        .await;
                            });
                        });
                        LineStream::from_events(response_rx)
                    };

                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(_) => count += 1,
                            Err(_) => break,
                        }
                    }
                    count
                })
            });

            let task2 = std::thread::spawn(move || {
                futures_lite::future::block_on(async move {
                    let mut count = 0;
                    let mut stream = {
                        let (response_tx, response_rx) = bounded(64);
                        std::thread::spawn(move || {
                            futures_lite::future::block_on(async move {
                                let _ =
                                    execute_command(handle2, "accounts".to_string(), response_tx)
                                        .await;
                            });
                        });
                        LineStream::from_events(response_rx)
                    };

                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(_) => count += 1,
                            Err(_) => break,
                        }
                    }
                    count
                })
            });

            // Both tasks should complete successfully
            let count1 = task1.join().expect("Task 1 should complete");
            let count2 = task2.join().expect("Task 2 should complete");

            // Both commands should produce output
            assert!(
                count1 > 0 || count2 > 0,
                "At least one command should produce output"
            );
        });
    }
}
