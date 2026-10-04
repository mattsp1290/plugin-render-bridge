//! Out-of-process plugin renderer via child process bridge.
//!
//! `BridgeRenderer` spawns a `plugin-renderer` child process and communicates
//! via JSON lines over stdin/stdout. Plugin crashes in the child process
//! do not affect the host.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use base64::Engine;

use crate::bridge_protocol::{Command as BridgeCommand, ReadySignal, Response};
use crate::buffer::AudioBuffer;
use crate::config::RenderSettings;
use crate::render::{NoteRenderer, RenderError};

mod transport;
use transport::{Transport, TransportError, command_budget};

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

    #[error("renderer command timed out")]
    CommandTimeout,

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
    transport: Option<Transport>,
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

        let transport = Transport::new(stdin, stdout);

        let deadline = Instant::now() + READY_TIMEOUT;
        loop {
            let result = transport.read(deadline);
            match result {
                Ok(line) => {
                    if let Ok(ready) = serde_json::from_str::<ReadySignal>(line.trim())
                        && ready.status == "ready"
                    {
                        return Ok(Self {
                            child,
                            transport: Some(transport),
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
                        Err(TransportError::Timeout) => BridgeError::ReadyTimeout,
                        Err(TransportError::Closed) => BridgeError::ProcessCrashed(
                            child.try_wait().ok().flatten().and_then(|s| s.code()),
                        ),
                        Err(TransportError::Io(e)) => BridgeError::Ipc(e),
                        _ => unreachable!(),
                    };
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error);
                }
            }
        }
    }

    /// Supervise both pipe writing and response receipt under one deadline.
    fn send_command(
        &mut self,
        cmd: &BridgeCommand,
        started: Instant,
    ) -> Result<Response, BridgeError> {
        if let Some(status) = self
            .child
            .try_wait()
            .map_err(|e| BridgeError::Ipc(e.to_string()))?
        {
            return Err(BridgeError::ProcessCrashed(status.code()));
        }
        let mut line = serde_json::to_vec(cmd).map_err(|e| BridgeError::Ipc(e.to_string()))?;
        line.push(b'\n');
        let deadline = started
            .checked_add(command_budget(cmd))
            .ok_or_else(|| BridgeError::Ipc("command duration is too large".into()))?;
        let result = self
            .transport
            .as_ref()
            .ok_or(BridgeError::ProcessCrashed(None))?
            .write(line, deadline)
            .and_then(|()| self.transport.as_ref().unwrap().read(deadline));
        match result {
            Ok(line) => serde_json::from_str(line.trim())
                .map_err(|e| BridgeError::Ipc(format!("invalid response JSON: {e}"))),
            Err(error) => {
                let result = match error {
                    TransportError::Timeout => BridgeError::CommandTimeout,
                    TransportError::Closed => BridgeError::ProcessCrashed(
                        self.child.try_wait().ok().flatten().and_then(|s| s.code()),
                    ),
                    TransportError::Io(e) => match self.child.try_wait().ok().flatten() {
                        Some(status) => BridgeError::ProcessCrashed(status.code()),
                        None => BridgeError::Ipc(e),
                    },
                };
                // Kill before dropping transport, unblocking pipe workers. Never reuse
                // this receiver: a late response cannot satisfy the next transaction.
                let _ = self.child.kill();
                let _ = self.child.wait();
                self.transport.take();
                Err(result)
            }
        }
    }

    /// Apply preset state to the plugin in the child process.
    pub fn bridge_set_state(&mut self, state_bytes: &[u8]) -> Result<(), BridgeError> {
        let started = Instant::now();
        let encoded = base64::engine::general_purpose::STANDARD.encode(state_bytes);
        let cmd = BridgeCommand::SetState {
            state_base64: encoded,
        };

        let resp = self.send_command(&cmd, started)?;
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
        let started = Instant::now();
        let cmd = BridgeCommand::Render {
            note,
            velocity,
            wav_path: wav_path.to_string_lossy().into(),
            duration_secs: config.note_duration_secs,
            silence_threshold: config.silence_threshold,
            tail_timeout_secs: config.tail_timeout.as_secs_f64(),
        };

        let resp = self.send_command(&cmd, started)?;
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
        // Queue a quit without ever writing a pipe on this thread.
        if let Some(transport) = &self.transport {
            transport.quit();
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
