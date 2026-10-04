//! Out-of-process plugin renderer via child process bridge.
//!
//! `BridgeRenderer` spawns a `plugin-renderer` child process and communicates
//! via JSON lines over stdin/stdout. Plugin crashes in the child process
//! do not affect the host.

use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use base64::Engine;

use crate::bridge_protocol::{Command as BridgeCommand, ReadySignal, Response};
use crate::buffer::AudioBuffer;
use crate::config::RenderSettings;
use crate::render::{NoteRenderer, RenderError};

/// Timeout for the child process to become ready after spawning.
const READY_TIMEOUT: Duration = Duration::from_secs(5);

/// Error type for bridge operations.
#[derive(Debug, thiserror::Error)]
#[must_use]
pub enum BridgeError {
    #[error("failed to spawn renderer process: {0}")]
    SpawnFailed(String),

    #[error("renderer process not ready within timeout")]
    ReadyTimeout,

    #[error("renderer process crashed (exit code: {0:?})")]
    ProcessCrashed(Option<i32>),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("renderer returned error: {0}")]
    RendererError(String),

    #[error("renderer binary not found")]
    BinaryNotFound,
}

/// Out-of-process plugin renderer.
///
/// Spawns a `plugin-renderer` child process that loads a VST3 plugin
/// and serves render commands. If the child crashes (e.g., plugin SIGSEGV),
/// the host continues running and can respawn the renderer.
pub struct BridgeRenderer {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    responses: std::sync::mpsc::Receiver<Result<String, String>>,
    plugin_name: String,
    // Stored for respawn
    renderer_bin: PathBuf,
    binary_path: PathBuf,
    sample_rate: u32,
    buffer_size: u32,
    tempo_bpm: Option<f64>,
}

impl BridgeRenderer {
    /// Spawn a new renderer child process.
    ///
    /// Blocks until the child signals ready or the timeout expires.
    pub fn spawn(
        renderer_bin: &Path,
        binary_path: &Path,
        sample_rate: u32,
        buffer_size: u32,
        tempo_bpm: Option<f64>,
    ) -> Result<Self, BridgeError> {
        let mut cmd = Command::new(renderer_bin);
        cmd.arg(binary_path)
            .arg(sample_rate.to_string())
            .arg(buffer_size.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()); // let child stderr go to parent for debugging

        if let Some(bpm) = tempo_bpm {
            cmd.arg("--tempo").arg(bpm.to_string());
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| BridgeError::SpawnFailed(e.to_string()))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| BridgeError::SpawnFailed("failed to capture stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| BridgeError::SpawnFailed("failed to capture stdout".into()))?;

        // 64KB capacity handles large SetState payloads (base64-encoded preset
        // state) without chunking across multiple write syscalls.
        let stdin = BufWriter::with_capacity(64 * 1024, stdin);
        let (sender, responses) = std::sync::mpsc::sync_channel(8);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                // Bound unsolicited output without waiting for an entire huge line.
                let mut line = String::new();
                let result = (&mut reader).take(65537).read_line(&mut line);
                let message = match result {
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

        let deadline = Instant::now() + READY_TIMEOUT;
        loop {
            let result = responses.recv_timeout(deadline.saturating_duration_since(Instant::now()));
            match result {
                Ok(Ok(line)) => {
                    if let Ok(ready) = serde_json::from_str::<ReadySignal>(line.trim())
                        && ready.status == "ready"
                    {
                        return Ok(Self {
                            child,
                            stdin,
                            responses,
                            plugin_name: ready.name,
                            renderer_bin: renderer_bin.to_path_buf(),
                            binary_path: binary_path.to_path_buf(),
                            sample_rate,
                            buffer_size,
                            tempo_bpm,
                        });
                    }
                    if let Ok(response) = serde_json::from_str::<Response>(line.trim())
                        && !response.ok
                    {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(BridgeError::RendererError(
                            response.error.unwrap_or_default(),
                        ));
                    }
                }
                result => {
                    let error = match result {
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            BridgeError::ReadyTimeout
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            BridgeError::ProcessCrashed(
                                child.try_wait().ok().flatten().and_then(|s| s.code()),
                            )
                        }
                        Ok(Err(e)) => BridgeError::Ipc(e),
                        _ => unreachable!(),
                    };
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error);
                }
            }
        }
    }

    /// Send a JSON command and read the response.
    ///
    /// NOTE: Large payloads (>64KB after base64 encoding) risk blocking if the
    /// OS pipe buffer fills before the child reads. For very large preset state,
    /// a future improvement would write to a temp file and pass the path.
    fn send_command(&mut self, cmd: &BridgeCommand) -> Result<Response, BridgeError> {
        if let Some(status) = self
            .child
            .try_wait()
            .map_err(|e| BridgeError::Ipc(e.to_string()))?
        {
            return Err(BridgeError::ProcessCrashed(status.code()));
        }
        let line = serde_json::to_string(cmd)
            .map_err(|e| BridgeError::Ipc(format!("serialize error: {e}")))?;

        self.stdin
            .write_all(line.as_bytes())
            .map_err(|e| BridgeError::Ipc(format!("write error: {e}")))?;
        self.stdin
            .write_all(b"\n")
            .map_err(|e| BridgeError::Ipc(format!("write newline error: {e}")))?;
        self.stdin
            .flush()
            .map_err(|e| BridgeError::Ipc(format!("flush error: {e}")))?;

        match self.responses.recv() {
            Ok(Ok(line)) => serde_json::from_str(line.trim())
                .map_err(|e| BridgeError::Ipc(format!("invalid response JSON: {e}"))),
            Ok(Err(e)) => Err(BridgeError::Ipc(e)),
            Err(_) => Err(BridgeError::ProcessCrashed(
                self.child.try_wait().ok().flatten().and_then(|s| s.code()),
            )),
        }
    }

    /// Apply preset state to the plugin in the child process.
    pub fn bridge_set_state(&mut self, state_bytes: &[u8]) -> Result<(), BridgeError> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(state_bytes);
        let cmd = BridgeCommand::SetState {
            state_base64: encoded,
        };

        let resp = self.send_command(&cmd)?;
        if resp.ok {
            Ok(())
        } else {
            let err = resp.error.unwrap_or_else(|| "unknown error".into());
            Err(BridgeError::RendererError(err))
        }
    }

    /// Render a note and write the WAV file to the given path.
    ///
    /// Returns the number of samples rendered.
    pub fn render_to_file(
        &mut self,
        note: u8,
        velocity: u8,
        wav_path: &Path,
        config: &RenderSettings,
    ) -> Result<u64, BridgeError> {
        let cmd = BridgeCommand::Render {
            note,
            velocity,
            wav_path: wav_path.to_string_lossy().into(),
            duration_secs: config.note_duration_secs,
            silence_threshold: config.silence_threshold,
            tail_timeout_secs: config.tail_timeout.as_secs_f64(),
        };

        let resp = self.send_command(&cmd)?;
        if resp.ok {
            Ok(resp.samples.unwrap_or(0))
        } else {
            let err = resp.error.unwrap_or_else(|| "unknown error".into());
            Err(BridgeError::RendererError(err))
        }
    }

    /// Check if the child process is still running.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Kill the current child and spawn a new one with the same config.
    pub fn respawn(&mut self) -> Result<(), BridgeError> {
        let _ = self.child.kill();
        let _ = self.child.wait();

        let new = Self::spawn(
            &self.renderer_bin,
            &self.binary_path,
            self.sample_rate,
            self.buffer_size,
            self.tempo_bpm,
        )?;

        *self = new;
        Ok(())
    }

    /// Process identifier, for external supervision of the disposable child.
    pub fn child_id(&self) -> u32 {
        self.child.id()
    }

    /// Get the plugin name reported by the child process.
    pub fn plugin_name(&self) -> &str {
        &self.plugin_name
    }
}

impl NoteRenderer for BridgeRenderer {
    fn render_note(
        &mut self,
        note: u8,
        velocity: u8,
        config: &RenderSettings,
    ) -> Result<AudioBuffer, RenderError> {
        let tmp_file = tempfile::Builder::new()
            .prefix("plugin-bridge-")
            .suffix(".wav")
            .tempfile()
            .map_err(|e| RenderError::PluginError(format!("failed to create temp file: {e}")))?;
        let tmp_path = tmp_file.path().to_path_buf();

        self.render_to_file(note, velocity, &tmp_path, config)
            .map_err(|e| RenderError::PluginError(e.to_string()))?;

        let buffer = crate::wav::read_wav(&tmp_path)
            .map_err(|e| RenderError::PluginError(format!("failed to read rendered WAV: {e}")))?;

        // tmp_file drops here, auto-deleting the file
        Ok(buffer)
    }

    fn set_state(&mut self, state_bytes: &[u8]) -> Result<(), String> {
        self.bridge_set_state(state_bytes)
            .map_err(|e| e.to_string())
    }
}

impl Drop for BridgeRenderer {
    fn drop(&mut self) {
        // Try graceful shutdown
        if let Ok(line) = serde_json::to_string(&BridgeCommand::Quit) {
            let _ = self.stdin.write_all(line.as_bytes());
            let _ = self.stdin.write_all(b"\n");
            let _ = self.stdin.flush();
        }

        // Give the child a moment to exit
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(500) {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        // Force kill
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Find the `plugin-renderer` binary near the current executable.
///
/// Delegates to [`crate::find_sibling_binary`] which checks the same
/// directory first (production layout), then the parent directory (test
/// binaries in `target/debug/deps/`, renderer binary in `target/debug/`).
/// Result is cached for the process lifetime via `OnceLock`.
///
/// The renderer binary is cached unconditionally because it changes less
/// frequently during development and is spawned many times per batch job.
pub fn find_renderer_bin() -> Result<PathBuf, BridgeError> {
    static CACHED: OnceLock<Option<PathBuf>> = OnceLock::new();

    let result = CACHED.get_or_init(|| crate::find_sibling_binary("plugin-renderer"));

    result.clone().ok_or(BridgeError::BinaryNotFound)
}
