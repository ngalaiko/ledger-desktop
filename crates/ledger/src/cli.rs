use std::ffi::OsStr;

use anyhow::Error;
use async_process::{ChildStderr, ChildStdout, Command, Stdio};
use futures_lite::{io::BufReader, AsyncBufReadExt, Stream, StreamExt};

use crate::{sexpr, Price, Transaction};

#[derive(Debug, Clone)]
pub struct Cli {
    file: Option<std::path::PathBuf>,
}

impl Cli {
    pub fn new<P: AsRef<std::path::Path>>(path: Option<P>) -> Self {
        Self {
            file: path.map(|p| p.as_ref().to_path_buf()),
        }
    }

    pub async fn files(
        &self,
    ) -> Result<impl Stream<Item = Result<std::path::PathBuf, Error>>, Error> {
        let stream = self.exec(["stats"]).await?;

        let stream = futures_lite::stream::unfold(
            (stream.boxed(), false),
            |(mut stream, mut in_files_section)| async move {
                loop {
                    match stream.next().await {
                        Some(Ok(line)) => {
                            if line.contains("Files these postings came from:") {
                                in_files_section = true;
                                continue;
                            }

                            let trimmed = line.trim();

                            if trimmed.is_empty() {
                                if in_files_section {
                                    return None; // End of files section
                                }
                                continue;
                            }

                            if in_files_section {
                                let path = std::path::PathBuf::from(trimmed);
                                return Some((Ok(path), (stream, in_files_section)));
                            }
                        }
                        Some(Err(e)) => {
                            return Some((Err(e), (stream, in_files_section)));
                        }
                        None => return None,
                    }
                }
            },
        );

        Ok(stream)
    }

    pub async fn prices(&self) -> Result<impl Stream<Item = Result<Price, Error>>, Error> {
        let stream = self.exec(["prices"]).await?;
        Ok(stream.map(|result| match result {
            Ok(line) => Price::from_str(&line).map_err(|e| Error::msg(e.to_string())),
            Err(e) => Err(e),
        }))
    }

    pub async fn transactions(
        &self,
    ) -> Result<impl Stream<Item = Result<Transaction, Error>>, Error> {
        let stream = self
            .exec_sexpr(["lisp", "--lisp-date-format", "%Y-%m-%d"])
            .await?;
        Ok(stream.map(|result| match result {
            Ok(sexpr::Value::List(list)) => {
                Transaction::from_sexpr(&list).map_err(|e| Error::msg(e.to_string()))
            }
            Ok(_) => Err(Error::msg("Expected list in transaction sexpr")),
            Err(e) => Err(e),
        }))
    }

    async fn exec_sexpr(
        &self,
        args: impl IntoIterator<Item = impl AsRef<OsStr> + 'static> + 'static,
    ) -> Result<impl Stream<Item = Result<sexpr::Value, Error>>, Error> {
        let stream = self.exec(args).await?;

        struct State<S> {
            stream: S,
            parser: sexpr::Parser,
            buffer: Vec<sexpr::Value>,
            done: bool,
        }

        let state = State {
            stream: stream.boxed(),
            parser: sexpr::Parser::new(),
            buffer: Vec::new(),
            done: false,
        };

        let stream = futures_lite::stream::unfold(state, |mut state| async move {
            loop {
                // Yield buffered values first
                if let Some(value) = state.buffer.pop() {
                    return Some((Ok(value), state));
                }

                if state.done {
                    return None;
                }

                // Get next line from stream
                match state.stream.next().await {
                    Some(Ok(line)) => {
                        if let Err(e) = state.parser.take(&line) {
                            state.done = true;
                            return Some((Err(Error::msg(e.to_string())), state));
                        }
                        // Drain parsed values into buffer
                        state.buffer = state.parser.drain_output();
                        state.buffer.reverse(); // So we can pop in order
                    }
                    Some(Err(e)) => {
                        return Some((Err(e), state));
                    }
                    None => {
                        // Stream ended, finish parsing
                        match state.parser.finish() {
                            Ok(remaining) => {
                                state.buffer = remaining;
                                state.buffer.reverse();
                                state.done = true;
                            }
                            Err(e) => {
                                state.done = true;
                                return Some((Err(Error::msg(e.to_string())), state));
                            }
                        }
                    }
                }
            }
        });

        Ok(stream)
    }

    async fn exec(
        &self,
        args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    ) -> Result<impl Stream<Item = Result<String, Error>>, Error> {
        let mut cmd = Command::new("ledger");
        cmd.args(args);
        if let Some(ref file) = self.file {
            cmd.arg(file);
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        let stdout = child
            .stdout
            .take()
            .ok_or(Error::msg("Failed to capture stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(Error::msg("Failed to capture stderr"))?;

        let stdout_reader = BufReader::new(stdout);
        let stderr_reader = BufReader::new(stderr);

        let stream = futures_lite::stream::unfold(
            (stdout_reader, stderr_reader, false, false), // (stdout, stderr, stdout_done, stderr_done)
            |(mut stdout, mut stderr, mut stdout_done, mut stderr_done)| async move {
                loop {
                    if stdout_done && stderr_done {
                        return None;
                    }

                    // Read from active streams only
                    let result = if !stdout_done && !stderr_done {
                        // Both streams active, read from either
                        read_either(&mut stdout, &mut stderr).await
                    } else if !stdout_done {
                        // Only stdout active
                        let mut buf = Vec::new();
                        match stdout.read_until(b'\n', &mut buf).await {
                            Ok(0) => Ok(ReadResult::Stdout(None)),
                            Ok(_) => String::from_utf8(buf)
                                .map(|line| ReadResult::Stdout(Some(line)))
                                .map_err(Error::from),
                            Err(e) => Err(Error::from(e)),
                        }
                    } else {
                        // Only stderr active
                        let mut buf = Vec::new();
                        match stderr.read_until(b'\n', &mut buf).await {
                            Ok(0) => Ok(ReadResult::Stderr(None)),
                            Ok(_) => String::from_utf8(buf)
                                .map(|line| ReadResult::Stderr(Some(line)))
                                .map_err(Error::from),
                            Err(e) => Err(Error::from(e)),
                        }
                    };

                    match result {
                        Ok(ReadResult::Stdout(Some(line))) => {
                            return Some((Ok(line), (stdout, stderr, stdout_done, stderr_done)));
                        }
                        Ok(ReadResult::Stdout(None)) => {
                            stdout_done = true;
                            // Continue loop to read from stderr if it's still active
                        }
                        Ok(ReadResult::Stderr(Some(line))) => {
                            return Some((
                                Err(Error::msg(line)),
                                (stdout, stderr, stdout_done, stderr_done),
                            ));
                        }
                        Ok(ReadResult::Stderr(None)) => {
                            stderr_done = true;
                            // Continue loop to read from stdout if it's still active
                        }
                        Err(e) => return Some((Err(e), (stdout, stderr, true, true))),
                    }
                }
            },
        );

        Ok(stream)
    }
}

enum ReadResult {
    Stdout(Option<String>),
    Stderr(Option<String>),
}

async fn read_either(
    stdout: &mut BufReader<ChildStdout>,
    stderr: &mut BufReader<ChildStderr>,
) -> Result<ReadResult, Error> {
    futures_lite::future::race(
        async {
            let mut buf = Vec::new();
            let n = stdout.read_until(b'\n', &mut buf).await?;
            if n == 0 {
                Ok(ReadResult::Stdout(None))
            } else {
                let line = String::from_utf8(buf)?;
                Ok(ReadResult::Stdout(Some(line)))
            }
        },
        async {
            let mut buf = Vec::new();
            let n = stderr.read_until(b'\n', &mut buf).await?;
            if n == 0 {
                Ok(ReadResult::Stderr(None))
            } else {
                let line = String::from_utf8(buf)?;
                Ok(ReadResult::Stderr(Some(line)))
            }
        },
    )
    .await
}
