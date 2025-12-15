use async_channel::{bounded, Receiver, Sender};
use async_process::{Command, Stdio};
use futures_lite::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const MARKER: &[u8] = b"__END_OF_RESPONSE__";

#[derive(Debug, Clone)]
pub enum LedgerEvent {
    Line(Vec<u8>),
    Done,
    Error(String),
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
    pub fn spawn(cx: &mut gpui::App) -> Self {
        let (cmd_tx, cmd_rx) = bounded::<LedgerCommand>(16);

        cx.background_executor()
            .spawn(async move {
                if let Err(e) = run_actor(cmd_rx).await {
                    eprintln!("Ledger actor failed: {}", e);
                }
            })
            .detach();

        Self { cmd_tx }
    }

    pub async fn send(&self, cmd: Vec<u8>) -> Option<Receiver<LedgerEvent>> {
        let (response_tx, response_rx) = bounded(64);
        self.cmd_tx
            .send(LedgerCommand { cmd, response_tx })
            .await
            .ok()?;
        Some(response_rx)
    }

    pub async fn stream(&self, cmd: &[u8]) -> Option<ByteStream> {
        let event_rx = self.send(cmd.to_vec()).await?;
        Some(ByteStream::from_events(event_rx))
    }
}

pub struct ByteStream {
    rx: Receiver<LedgerEvent>,
}

impl ByteStream {
    fn from_events(rx: Receiver<LedgerEvent>) -> Self {
        Self { rx }
    }

    pub async fn next(&mut self) -> Option<Vec<u8>> {
        loop {
            match self.rx.recv().await {
                Ok(LedgerEvent::Line(line)) => return Some(line),
                Ok(LedgerEvent::Done) | Err(_) => return None,
                Ok(LedgerEvent::Error(_)) => return None,
            }
        }
    }
}

async fn run_actor(cmd_rx: Receiver<LedgerCommand>) -> std::io::Result<()> {
    let mut ledger = Ledger::spawn().await?;

    while let Ok(command) = cmd_rx.recv().await {
        let LedgerCommand { cmd, response_tx } = command;

        if let Err(e) = ledger.command(&cmd).await {
            response_tx
                .send(LedgerEvent::Error(e.to_string()))
                .await
                .ok();
            continue;
        }

        loop {
            match ledger.read_line().await {
                Ok(Some(line)) => {
                    if response_tx.send(LedgerEvent::Line(line)).await.is_err() {
                        // Receiver dropped - drain remaining output
                        while let Ok(Some(_)) = ledger.read_line().await {}
                        break;
                    }
                }
                Ok(None) => {
                    response_tx.send(LedgerEvent::Done).await.ok();
                    break;
                }
                Err(e) => {
                    response_tx
                        .send(LedgerEvent::Error(e.to_string()))
                        .await
                        .ok();
                    break;
                }
            }
        }
    }

    Ok(())
}

struct Ledger {
    stdin: async_process::ChildStdin,
    reader: BufReader<async_process::ChildStdout>,
    _child: async_process::Child,
}

impl Ledger {
    async fn spawn() -> std::io::Result<Self> {
        let mut child = Command::new("ledger")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let reader = BufReader::new(stdout);

        let mut repl = Self {
            stdin,
            reader,
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
            let n = self.reader.read_until(b'\n', &mut buf).await?;
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

    async fn read_line(&mut self) -> std::io::Result<Option<Vec<u8>>> {
        let mut buf = Vec::new();
        let n = self.reader.read_until(b'\n', &mut buf).await?;
        if n == 0 || buf.strip_suffix(b"\n").unwrap_or(&buf) == MARKER {
            return Ok(None);
        }
        Ok(Some(buf))
    }
}
