//! Owned pipe workers keep native-child I/O off the supervising thread.
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::process::{ChildStdin, ChildStdout};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant};

pub(super) enum TransportError {
    Timeout,
    Closed,
    Io(String),
}

struct Outbound {
    line: Vec<u8>,
    completed: mpsc::Sender<Result<(), String>>,
}

pub(super) struct Transport {
    outbound: SyncSender<Outbound>,
    inbound: Receiver<Result<String, String>>,
}

impl Transport {
    pub(super) fn new(stdin: ChildStdin, stdout: ChildStdout) -> Self {
        let (outbound, requests) = mpsc::sync_channel::<Outbound>(1);
        std::thread::spawn(move || {
            let mut writer = BufWriter::with_capacity(64 * 1024, stdin);
            while let Ok(request) = requests.recv() {
                let result = writer
                    .write_all(&request.line)
                    .and_then(|()| writer.flush())
                    .map_err(|e| e.to_string());
                let failed = result.is_err();
                let _ = request.completed.send(result);
                if failed {
                    break;
                }
            }
        });
        let (sender, inbound) = mpsc::sync_channel(8);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                let message = match (&mut reader).take(65537).read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) if line.len() > 65536 => {
                        Err("renderer output line exceeds 64 KiB".into())
                    }
                    Ok(_) => Ok(line),
                    Err(e) => Err(e.to_string()),
                };
                let terminal = message.is_err();
                if sender.send(message).is_err() || terminal {
                    break;
                }
            }
        });
        Self { outbound, inbound }
    }

    pub(super) fn write(&self, line: Vec<u8>, deadline: Instant) -> Result<(), TransportError> {
        if Instant::now() >= deadline {
            return Err(TransportError::Timeout);
        }
        let (completed, result) = mpsc::channel();
        self.outbound
            .try_send(Outbound { line, completed })
            .map_err(|e| TransportError::Io(e.to_string()))?;
        match result.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(TransportError::Io(e)),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(TransportError::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(TransportError::Closed),
        }
    }

    pub(super) fn read(&self, deadline: Instant) -> Result<String, TransportError> {
        if Instant::now() >= deadline {
            return Err(TransportError::Timeout);
        }
        match self
            .inbound
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            Ok(Ok(line)) => Ok(line),
            Ok(Err(e)) => Err(TransportError::Io(e)),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(TransportError::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(TransportError::Closed),
        }
    }

    pub(super) fn quit(&self) {
        let (completed, _) = mpsc::channel();
        let _ = self.outbound.try_send(Outbound {
            line: b"{\"cmd\":\"quit\"}\n".to_vec(),
            completed,
        });
    }
}

/// State commands get 30 seconds; rendering also gets its requested audio budget.
pub(super) fn command_budget(command: &crate::bridge_protocol::Command) -> Duration {
    let base = Duration::from_secs(30);
    if let crate::bridge_protocol::Command::Render {
        duration_secs,
        tail_timeout_secs,
        ..
    } = command
        && let Ok(audio) = Duration::try_from_secs_f64(duration_secs + tail_timeout_secs)
    {
        return base.saturating_add(audio);
    }
    base
}
