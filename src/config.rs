//! Settings owned by the rendering crate.
use std::time::Duration;

/// WAV output encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BitDepth {
    Int16,
    #[default]
    Int24,
    Float32,
}

/// Configuration for rendering a single note.
#[derive(Debug, Clone)]
pub struct RenderSettings {
    pub sample_rate: u32,
    pub bit_depth: BitDepth,
    pub buffer_size: u32,
    pub note_duration_secs: f64,
    pub tail_timeout: Duration,
    pub silence_threshold: f64,
    pub tempo_bpm: Option<f64>,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            bit_depth: BitDepth::Int24,
            buffer_size: 512,
            note_duration_secs: 2.0,
            tail_timeout: Duration::from_secs(10),
            silence_threshold: 1e-5,
            tempo_bpm: Some(120.0),
        }
    }
}
