//! Typed IPC protocol for the `plugin-renderer` bridge.
//!
//! Shared between the host side ([`super::bridge::BridgeRenderer`]) and the
//! child process binary (`plugin-renderer`). Using typed structs instead of
//! `serde_json::Value` catches protocol mismatches at compile time.

use serde::{Deserialize, Serialize};

// ── Host → Child commands ──────────────────────────────────────────

/// Command sent from the host process to the `plugin-renderer` child.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum Command {
    /// Apply preset state to the plugin.
    #[serde(rename = "set_state")]
    SetState { state_base64: String },

    /// Render a single note and write the result as a WAV file.
    #[serde(rename = "render")]
    Render {
        note: u8,
        velocity: u8,
        wav_path: String,
        duration_secs: f64,
        silence_threshold: f64,
        tail_timeout_secs: f64,
    },

    /// Shut down the child process gracefully.
    #[serde(rename = "quit")]
    Quit,
}

// ── Child → Host responses ─────────────────────────────────────────

/// Signal emitted by the child on stdout once the plugin is loaded and ready.
#[derive(Debug, Serialize, Deserialize)]
pub struct ReadySignal {
    pub status: String,
    pub name: String,
}

impl ReadySignal {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            status: "ready".into(),
            name: name.into(),
        }
    }
}

/// Response from the child process to any command.
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub samples: Option<u64>,
}

impl Response {
    pub fn ok() -> Self {
        Self {
            ok: true,
            error: None,
            samples: None,
        }
    }

    pub fn ok_with_samples(samples: u64) -> Self {
        Self {
            ok: true,
            error: None,
            samples: Some(samples),
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            samples: None,
        }
    }
}
