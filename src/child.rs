//! Out-of-process VST3 renderer for crash-isolated plugin rendering.
//!
//! Loads a single VST3 plugin, accepts render commands via JSON on stdin,
//! writes WAV files directly, and responds with JSON on stdout.
//! Runs in isolation so a crashing plugin cannot corrupt the host process.
//!
//! Usage: `plugin-renderer <binary_path> <sample_rate> <buffer_size> [--tempo <bpm>]`
//!
//! Protocol (JSON lines on stdin/stdout):
//!   Ready:     {"status":"ready","name":"PluginName"}
//!   Commands:  {"cmd":"set_state","state_base64":"..."} | {"cmd":"render",...} | {"cmd":"quit"}
//!   Response:  {"ok":true,...} | {"ok":false,"error":"..."}

use std::io::{self, BufRead, Write};
use std::path::Path;
use std::time::Duration;

use crate::bridge_protocol::{Command, ReadySignal, Response};
use crate::config::RenderSettings;
use crate::render::{AudioProcessor, render_note};
use crate::vst_adapter::VstProcessor;
use crate::wav;
use base64::Engine;
use plugin_hostkit::{ProcessConfig, VstInstance};

fn respond(resp: &Response) {
    let json = serde_json::to_string(resp).expect("Response serialization cannot fail");
    println!("{json}");
    let _ = io::stdout().flush();
}

/// Run the disposable renderer process.
///
/// Arguments: plugin binary path, sample rate (Hz), buffer size, optional
/// `--tempo <bpm>`. Bit depth defaults to Int24 as in the original protocol.
/// Reads JSON-lines commands on stdin, writes ready/response JSON on stdout.
/// Exits nonzero on startup failure; never returns.
pub fn renderer_child_main() -> ! {
    // Native plugin faults terminate this disposable process.
    plugin_hostkit::mark_as_child_process();

    let args: Vec<String> = std::env::args().collect();

    if args.len() < 4 {
        eprintln!(
            "Usage: plugin-renderer <binary_path> <sample_rate> <buffer_size> [--tempo <bpm>]"
        );
        std::process::exit(1);
    }

    let binary_path = Path::new(&args[1]);
    let sample_rate: u32 = args[2].parse().unwrap_or_else(|_| {
        eprintln!("Invalid sample_rate: {}", args[2]);
        std::process::exit(1);
    });
    let buffer_size: u32 = args[3].parse().unwrap_or_else(|_| {
        eprintln!("Invalid buffer_size: {}", args[3]);
        std::process::exit(1);
    });

    if sample_rate == 0 || buffer_size == 0 || buffer_size > 1_048_576 {
        eprintln!("sample rate and buffer size must be positive; buffer size must be <= 1048576");
        std::process::exit(1);
    }
    let mut tempo_bpm: Option<f64> = None;
    let mut i = 4;
    while i < args.len() {
        if args[i] == "--tempo" && i + 1 < args.len() {
            tempo_bpm = args[i + 1].parse().ok();
            i += 2;
        } else {
            i += 1;
        }
    }

    let mut processor = match load_processor(binary_path, sample_rate, buffer_size) {
        Ok(processor) => processor,
        Err(error) => {
            respond(&Response::error(error));
            std::process::exit(1);
        }
    };
    if let Some(bpm) = tempo_bpm {
        processor.set_tempo(bpm);
    }

    let plugin_name = binary_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // Signal ready
    let ready = ReadySignal::new(plugin_name);
    let ready_json = serde_json::to_string(&ready).expect("ReadySignal serialization cannot fail");
    println!("{ready_json}");
    let _ = io::stdout().flush();

    // Command loop
    let stdin = io::stdin();
    let reader = stdin.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break, // stdin closed
        };

        if line.is_empty() {
            continue;
        }

        let cmd: Command = match serde_json::from_str(&line) {
            Ok(c) => c,
            Err(e) => {
                respond(&Response::error(format!("Invalid command: {e}")));
                continue;
            }
        };

        match cmd {
            Command::Quit => break,

            Command::SetState { state_base64 } => {
                respond(&handle_set_state(&state_base64, &mut processor));
            }

            Command::Render { .. } => {
                respond(&handle_render(
                    cmd,
                    &mut processor,
                    sample_rate,
                    buffer_size,
                ));
            }
        }
    }

    // VstInstance Drop runs here — if the plugin crashes during terminate,
    // only this child process dies, not the host.
    drop(processor);
    std::process::exit(0);
}

/// Keep native lifecycle ordering separate from the process response boundary.
fn load_processor(
    binary_path: &Path,
    sample_rate: u32,
    buffer_size: u32,
) -> Result<VstProcessor, String> {
    let mut instance =
        VstInstance::load(binary_path).map_err(|e| format!("Failed to load plugin: {e}"))?;
    instance
        .initialize()
        .map_err(|e| format!("Failed to initialize: {e}"))?;
    instance
        .setup_processing(ProcessConfig::new(sample_rate as f64, buffer_size))
        .map_err(|e| format!("Failed to setup processing: {e}"))?;
    instance
        .activate()
        .map_err(|e| format!("Failed to activate: {e}"))?;
    Ok(VstProcessor::new(instance))
}

fn handle_set_state(state_base64: &str, processor: &mut VstProcessor) -> Response {
    let state_bytes = match base64::engine::general_purpose::STANDARD.decode(state_base64) {
        Ok(b) => b,
        Err(e) => return Response::error(format!("base64 decode error: {e}")),
    };

    match processor.instance_mut().set_state(&state_bytes) {
        Ok(()) => Response::ok(),
        Err(e) => Response::error(format!("set_state failed: {e}")),
    }
}

fn handle_render(
    command: Command,
    processor: &mut VstProcessor,
    sample_rate: u32,
    buffer_size: u32,
) -> Response {
    let Command::Render {
        note,
        velocity,
        wav_path,
        duration_secs,
        silence_threshold,
        tail_timeout_secs,
    } = command
    else {
        return Response::error("expected render command");
    };
    let wav_path = wav_path.as_str();
    let plugin_tail_samples = processor.instance().tail_samples();
    // I2: Reject wav_path with '..' segments
    if Path::new(wav_path)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Response::error("wav_path must not contain '..' segments");
    }

    if note > 127
        || velocity > 127
        || !duration_secs.is_finite()
        || duration_secs < 0.0
        || !silence_threshold.is_finite()
        || silence_threshold < 0.0
    {
        return Response::error("invalid note, velocity, duration or silence threshold");
    }
    let Ok(tail_timeout) = Duration::try_from_secs_f64(tail_timeout_secs) else {
        return Response::error("invalid tail timeout");
    };
    let config = RenderSettings {
        sample_rate,
        buffer_size,
        note_duration_secs: duration_secs,
        silence_threshold,
        tail_timeout,
        ..Default::default()
    };

    processor.reset_transport();
    let latency = processor.instance().latency_samples();
    let audio = match render_note(
        processor,
        note,
        velocity,
        &config,
        plugin_tail_samples,
        latency,
    ) {
        Ok(buf) => buf,
        Err(e) => return Response::error(format!("render failed: {e}")),
    };

    let samples = audio.num_samples();

    // Create parent directory if needed
    if let Some(parent) = Path::new(wav_path).parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return Response::error(format!("failed to create directory: {e}"));
    }

    if let Err(e) = wav::write_wav(&audio, Path::new(wav_path), config.bit_depth) {
        return Response::error(format!("failed to write WAV: {e}"));
    }

    Response::ok_with_samples(samples as u64)
}
