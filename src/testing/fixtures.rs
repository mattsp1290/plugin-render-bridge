use crate::{RenderSettings, buffer::AudioBuffer};
use std::f32::consts::PI;
/// Generate a test buffer filled with a sine wave at the given frequency.
pub fn generate_sine_buffer(
    frequency: f32,
    duration_secs: f32,
    sample_rate: u32,
    num_channels: usize,
) -> AudioBuffer {
    let num_samples = (duration_secs * sample_rate as f32) as usize;
    let mut buf = AudioBuffer::new(num_channels, num_samples, sample_rate);

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let val = (2.0 * PI * frequency * t).sin();
        for ch in 0..num_channels {
            buf.channel_mut(ch)[i] = val;
        }
    }

    buf
}

/// Generate a test buffer filled with white noise (deterministic via seed).
pub fn generate_noise_buffer(
    seed: u64,
    duration_secs: f32,
    sample_rate: u32,
    num_channels: usize,
) -> AudioBuffer {
    let num_samples = (duration_secs * sample_rate as f32) as usize;
    let mut buf = AudioBuffer::new(num_channels, num_samples, sample_rate);

    // Simple LCG for deterministic "noise"
    let mut state = seed;
    for i in 0..num_samples {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let val = (state >> 33) as f32 / (u32::MAX >> 1) as f32 - 1.0;
        for ch in 0..num_channels {
            buf.channel_mut(ch)[i] = val;
        }
    }

    buf
}

/// Generate a silent buffer.
pub fn generate_silent_buffer(
    duration_secs: f32,
    sample_rate: u32,
    num_channels: usize,
) -> AudioBuffer {
    let num_samples = (duration_secs * sample_rate as f32) as usize;
    AudioBuffer::new(num_channels, num_samples, sample_rate)
}

/// Generate a buffer with audio followed by silence (for trim testing).
pub fn generate_buffer_with_tail(
    audio_secs: f32,
    silence_secs: f32,
    sample_rate: u32,
) -> AudioBuffer {
    let audio_samples = (audio_secs * sample_rate as f32) as usize;
    let silence_samples = (silence_secs * sample_rate as f32) as usize;
    let total = audio_samples + silence_samples;
    let mut buf = AudioBuffer::new(2, total, sample_rate);

    for i in 0..audio_samples {
        let t = i as f32 / sample_rate as f32;
        let val = (2.0 * PI * 440.0 * t).sin() * 0.5;
        buf.channel_mut(0)[i] = val;
        buf.channel_mut(1)[i] = val;
    }

    buf
}

/// Create a fast RenderSettings suitable for unit tests.
pub fn fast_render_settings() -> RenderSettings {
    RenderSettings {
        note_duration_secs: 0.05,
        silence_threshold: 0.01,
        buffer_size: 256,
        ..Default::default()
    }
}
