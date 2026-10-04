use std::time::{Duration, Instant};

use crate::config::RenderSettings;

use crate::buffer::AudioBuffer;
use crate::midi::{MidiEvent, NoteSequence};

/// Error type for rendering operations.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("render timed out after {0:?}")]
    Timeout(Duration),

    #[error("plugin error: {0}")]
    PluginError(String),
}

/// Trait for anything that can process audio blocks (VST3 instance, mock, etc.)
pub trait AudioProcessor {
    fn process_block(
        &mut self,
        events: &[MidiEvent],
        output: &mut AudioBuffer,
    ) -> Result<(), String>;

    /// Reset the transport position to zero. Called before each note render
    /// so tempo-synced effects start from a consistent position.
    fn reset_transport(&mut self) {}

    /// Apply raw preset/component state bytes to the processor.
    /// Called when switching presets during batch rendering.
    /// Default implementation is a no-op (for mocks/stubs).
    fn set_state(&mut self, _state_bytes: &[u8]) -> Result<(), String> {
        Ok(())
    }
}

/// Render a single note using the given audio processor.
///
/// Stages:
/// 1. Pre-roll: process silence to warm up the plugin
/// 2. Note-on: send MIDI note-on, process blocks
/// 3. Sustain: continue processing for the specified duration
/// 4. Note-off: send MIDI note-off
/// 5. Tail: continue until silence detected or timeout
#[tracing::instrument(name = "render_note", skip_all, fields(note, velocity))]
pub fn render_note(
    processor: &mut dyn AudioProcessor,
    note: u8,
    velocity: u8,
    config: &RenderSettings,
    plugin_tail_samples: Option<u32>,
    plugin_latency_samples: u32,
) -> Result<AudioBuffer, RenderError> {
    if config.sample_rate == 0
        || config.buffer_size == 0
        || !config.note_duration_secs.is_finite()
        || config.note_duration_secs < 0.0
        || config.note_duration_secs * config.sample_rate as f64 > u32::MAX as f64
        || !config.silence_threshold.is_finite()
        || config.silence_threshold < 0.0
        || note > 127
        || velocity > 127
    {
        return Err(RenderError::PluginError(
            "invalid render settings or MIDI values".into(),
        ));
    }
    processor.reset_transport();

    let sample_rate = config.sample_rate;
    let block_size = config.buffer_size as usize;
    let num_channels = 2; // stereo

    let note_duration_samples = (config.note_duration_secs * sample_rate as f64) as usize;
    let pre_roll_blocks = 4; // warm up with a few silent blocks

    let mut output = AudioBuffer::new(num_channels, 0, sample_rate);
    let mut block_buf = AudioBuffer::new(num_channels, block_size, sample_rate);

    // Create MIDI sequence
    let seq = NoteSequence::single_note(note, velocity, 0, 0, note_duration_samples as u32);

    let start = Instant::now();
    let mut total_samples: usize = 0;

    // Stage 1: Pre-roll (silence, no events)
    for _ in 0..pre_roll_blocks {
        block_buf.clear();
        processor
            .process_block(&[], &mut block_buf)
            .map_err(RenderError::PluginError)?;
        // Don't accumulate pre-roll output
    }

    // Stage 2 + 3: Note-on through sustain
    let note_on_event = seq.events[0];
    let note_off_event = seq.events[1];
    let mut note_off_sent = false;

    // Use plugin-declared tail as a tighter upper bound when available.
    // Falls back to config.tail_timeout for infinite tail or when not provided.
    let max_tail_samples = match plugin_tail_samples {
        Some(declared) => {
            let config_tail = (config.tail_timeout.as_secs_f64() * sample_rate as f64) as usize;
            (declared as usize + block_size).min(config_tail)
        }
        None => (config.tail_timeout.as_secs_f64() * sample_rate as f64) as usize,
    };

    loop {
        // Check timeout
        if start.elapsed() > config.tail_timeout {
            return Err(RenderError::Timeout(config.tail_timeout));
        }

        // Determine events for this block
        let mut block_events = Vec::new();

        // Note-on at the start
        if total_samples == 0 {
            block_events.push(note_on_event);
        }

        // Zero duration sends note-on then note-off at sample zero.
        if !note_off_sent && note_duration_samples < total_samples.saturating_add(block_size) {
            block_events.push(MidiEvent {
                sample_offset: note_duration_samples.saturating_sub(total_samples) as u32,
                kind: note_off_event.kind,
            });
            note_off_sent = true;
        }

        // Process one block
        block_buf.clear();
        processor
            .process_block(&block_events, &mut block_buf)
            .map_err(RenderError::PluginError)?;

        output.append(&block_buf);
        total_samples += block_size;

        // Drain mandatory output latency before measuring the audible tail.
        if note_off_sent {
            let samples_since_note_off = total_samples.saturating_sub(note_duration_samples);
            let latency = plugin_latency_samples as usize;
            if samples_since_note_off >= latency {
                let audible_tail_samples = samples_since_note_off - latency;
                if is_block_silent(&block_buf, config.silence_threshold)
                    || audible_tail_samples >= max_tail_samples
                {
                    break;
                }
            }
        }
    }

    // Trim plugin latency from the start of the output.
    // Plugins report their internal delay via getLatencySamples; the first
    // N samples of output are delayed silence that should be removed.
    if plugin_latency_samples > 0 {
        output.trim_start(plugin_latency_samples as usize);
    }

    Ok(output)
}

/// Check if a block is below the silence threshold.
fn is_block_silent(block: &AudioBuffer, threshold: f64) -> bool {
    block.rms() < threshold
}

/// Calculate the number of blocks needed for a given duration.
pub fn blocks_for_duration(duration_secs: f64, sample_rate: u32, block_size: u32) -> usize {
    let total_samples = (duration_secs * sample_rate as f64) as usize;
    total_samples.div_ceil(block_size as usize)
}

/// Backend for rendering complete notes. Implementations may run in-process
/// (for unit tests with mock processors) or out-of-process (BridgeRenderer
/// for crash-isolated plugin rendering).
pub trait NoteRenderer: Send {
    /// Render a single note and return the resulting audio buffer.
    fn render_note(
        &mut self,
        note: u8,
        velocity: u8,
        config: &RenderSettings,
    ) -> Result<AudioBuffer, RenderError>;

    /// Apply raw preset/component state bytes to the underlying plugin.
    fn set_state(&mut self, state_bytes: &[u8]) -> Result<(), String>;
}

/// Wraps an [`AudioProcessor`] to implement [`NoteRenderer`] for in-process rendering.
///
/// Used in unit tests with mock processors. Production rendering uses
/// `BridgeRenderer` (out-of-process) instead.
pub struct InProcessRenderer<P: AudioProcessor> {
    processor: P,
}

impl<P: AudioProcessor> InProcessRenderer<P> {
    pub fn new(processor: P) -> Self {
        Self { processor }
    }

    /// Get a mutable reference to the underlying processor.
    pub fn processor_mut(&mut self) -> &mut P {
        &mut self.processor
    }
}

impl<P: AudioProcessor + Send> NoteRenderer for InProcessRenderer<P> {
    fn render_note(
        &mut self,
        note: u8,
        velocity: u8,
        config: &RenderSettings,
    ) -> Result<AudioBuffer, RenderError> {
        self.processor.reset_transport();
        render_note(&mut self.processor, note, velocity, config, None, 0)
    }

    fn set_state(&mut self, state_bytes: &[u8]) -> Result<(), String> {
        self.processor.set_state(state_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::MidiEventKind;

    /// A mock processor that outputs silence.
    struct SilentProcessor;

    impl AudioProcessor for SilentProcessor {
        fn process_block(
            &mut self,
            _events: &[MidiEvent],
            _output: &mut AudioBuffer,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    /// A mock processor that outputs a tone on note-on and silence after note-off.
    struct ToneProcessor {
        active: bool,
    }

    impl AudioProcessor for ToneProcessor {
        fn process_block(
            &mut self,
            events: &[MidiEvent],
            output: &mut AudioBuffer,
        ) -> Result<(), String> {
            for event in events {
                match event.kind {
                    MidiEventKind::NoteOn { .. } => self.active = true,
                    MidiEventKind::NoteOff { .. } => self.active = false,
                    _ => {}
                }
            }
            if self.active {
                for i in 0..output.num_samples() {
                    let val = (i as f32 * 0.01).sin() * 0.5;
                    for ch in 0..output.num_channels() {
                        output.channel_mut(ch)[i] = val;
                    }
                }
            }
            Ok(())
        }
    }

    #[test]
    fn render_silent_plugin() {
        let config = RenderSettings {
            note_duration_secs: 0.1,
            ..Default::default()
        };
        let mut proc = SilentProcessor;
        let result = render_note(&mut proc, 60, 127, &config, None, 0).unwrap();
        assert!(result.num_samples() > 0);
    }

    #[test]
    fn render_tone_plugin() {
        let config = RenderSettings {
            note_duration_secs: 0.1,
            silence_threshold: 0.01,
            ..Default::default()
        };
        let mut proc = ToneProcessor { active: false };
        let result = render_note(&mut proc, 60, 127, &config, None, 0).unwrap();
        assert!(result.num_samples() > 0);
        // Should have audio content during the note
        assert!(!result.is_silent());
    }

    #[test]
    fn render_note_respects_declared_tail() {
        let config = RenderSettings {
            note_duration_secs: 0.05,
            tail_timeout: Duration::from_secs(10),
            silence_threshold: 1e-10, // very low so silence detection doesn't trigger first
            ..Default::default()
        };
        let mut proc = ToneProcessor { active: false };
        let with_tail = render_note(&mut proc, 60, 127, &config, Some(1000), 0).unwrap();

        let mut proc2 = ToneProcessor { active: false };
        let without_tail = render_note(&mut proc2, 60, 127, &config, None, 0).unwrap();

        // With a declared tail, the render should stop no later than without one.
        // Equality is expected when silence detection triggers before the tail limit.
        assert!(with_tail.num_samples() <= without_tail.num_samples());
    }

    #[test]
    fn blocks_for_duration_calculation() {
        let blocks = blocks_for_duration(1.0, 44100, 512);
        assert_eq!(blocks, 87); // ceil(44100 / 512)
    }
}
