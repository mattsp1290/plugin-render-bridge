//! Deterministic mock VST3 plugin for testing.
//!
//! Generates a known sine waveform for each note, scaled by velocity.
//! This enables fully reproducible, CI-friendly tests with no real plugins.

use std::f32::consts::PI;

use crate::buffer::AudioBuffer;
use crate::midi::{MidiEvent, MidiEventKind};
use crate::render::AudioProcessor;

/// Deterministic mock VST plugin that generates a sine wave per active note.
///
/// The frequency is derived from the MIDI note number (A4 = 440 Hz),
/// and amplitude is scaled by velocity (0..127 -> 0.0..1.0).
///
/// Supports multiple presets that apply a simple gain multiplier.
pub struct MockVstPlugin {
    sample_rate: u32,
    phase: f64,
    active_note: Option<ActiveNote>,
    preset_index: usize,
    preset_gains: Vec<f32>,
}

struct ActiveNote {
    frequency: f64,
    amplitude: f32,
}

impl MockVstPlugin {
    /// Create a new mock plugin with default preset list.
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            phase: 0.0,
            active_note: None,
            preset_index: 0,
            // 4 built-in "presets" with different gain levels
            preset_gains: vec![1.0, 0.75, 0.5, 0.25],
        }
    }

    /// Set the active preset by index.
    pub fn set_preset(&mut self, index: usize) {
        self.preset_index = index.min(self.preset_gains.len() - 1);
    }

    /// Get the number of available presets.
    pub fn num_presets(&self) -> usize {
        self.preset_gains.len()
    }

    /// Get preset names.
    pub fn preset_names(&self) -> Vec<String> {
        vec![
            "Full".into(),
            "Three Quarter".into(),
            "Half".into(),
            "Quarter".into(),
        ]
    }

    /// Convert MIDI note number to frequency (A4 = 440 Hz).
    fn note_to_freq(note: u8) -> f64 {
        440.0 * 2.0_f64.powf((note as f64 - 69.0) / 12.0)
    }

    fn current_gain(&self) -> f32 {
        self.preset_gains
            .get(self.preset_index)
            .copied()
            .unwrap_or(1.0)
    }

    /// Capture the current preset state as raw bytes (4-byte u32 LE preset index).
    pub fn get_state(&self) -> Vec<u8> {
        (self.preset_index as u32).to_le_bytes().to_vec()
    }
}

impl AudioProcessor for MockVstPlugin {
    fn process_block(
        &mut self,
        events: &[MidiEvent],
        output: &mut AudioBuffer,
    ) -> Result<(), String> {
        // Process MIDI events
        for event in events {
            match event.kind {
                MidiEventKind::NoteOn { note, velocity, .. } => {
                    self.active_note = Some(ActiveNote {
                        frequency: Self::note_to_freq(note),
                        amplitude: velocity as f32 / 127.0,
                    });
                }
                MidiEventKind::NoteOff { .. } => {
                    self.active_note = None;
                }
                MidiEventKind::ControlChange { .. } => {}
            }
        }

        // Generate audio
        if let Some(ref note) = self.active_note {
            let gain = self.current_gain();
            let phase_inc = note.frequency / self.sample_rate as f64;

            for i in 0..output.num_samples() {
                let sample = (self.phase * 2.0 * PI as f64).sin() as f32 * note.amplitude * gain;
                for ch in 0..output.num_channels() {
                    output.channel_mut(ch)[i] = sample;
                }
                self.phase += phase_inc;
                // Keep phase in [0, 1) to avoid precision loss
                if self.phase >= 1.0 {
                    self.phase -= 1.0;
                }
            }
        }
        // If no active note, output remains zeroed (silence)

        Ok(())
    }

    fn set_state(&mut self, state_bytes: &[u8]) -> Result<(), String> {
        if state_bytes.len() < 4 {
            return Err("state too short: expected at least 4 bytes".into());
        }
        let idx = u32::from_le_bytes(state_bytes[..4].try_into().unwrap()) as usize;
        self.set_preset(idx);
        Ok(())
    }
}

/// A mock processor that always returns an error after N blocks.
pub struct FailingProcessor {
    blocks_until_fail: usize,
    blocks_processed: usize,
}

impl FailingProcessor {
    pub fn new(blocks_until_fail: usize) -> Self {
        Self {
            blocks_until_fail,
            blocks_processed: 0,
        }
    }
}

impl AudioProcessor for FailingProcessor {
    fn process_block(
        &mut self,
        _events: &[MidiEvent],
        _output: &mut AudioBuffer,
    ) -> Result<(), String> {
        self.blocks_processed += 1;
        if self.blocks_processed > self.blocks_until_fail {
            Err("simulated plugin crash".into())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::MidiEventKind;

    #[test]
    fn generates_sine_for_a4() {
        let mut plugin = MockVstPlugin::new(44100);
        let mut buf = AudioBuffer::new(2, 512, 44100);

        // Send note-on for A4 (MIDI 69)
        let events = vec![MidiEvent {
            sample_offset: 0,
            kind: MidiEventKind::NoteOn {
                note: 69,
                velocity: 127,
                channel: 0,
            },
        }];

        plugin.process_block(&events, &mut buf).unwrap();

        // Output should not be silent
        assert!(!buf.is_silent());
        // First sample should be near 0 (sine starts at 0)
        assert!(buf.channel(0)[0].abs() < 0.1);
        // Check that both channels have the same content (stereo)
        assert_eq!(buf.channel(0)[100], buf.channel(1)[100]);
    }

    #[test]
    fn silence_when_no_note() {
        let mut plugin = MockVstPlugin::new(44100);
        let mut buf = AudioBuffer::new(2, 512, 44100);

        plugin.process_block(&[], &mut buf).unwrap();
        assert!(buf.is_silent());
    }

    #[test]
    fn note_off_produces_silence() {
        let mut plugin = MockVstPlugin::new(44100);
        let mut buf = AudioBuffer::new(2, 512, 44100);

        // Note on
        plugin
            .process_block(
                &[MidiEvent {
                    sample_offset: 0,
                    kind: MidiEventKind::NoteOn {
                        note: 60,
                        velocity: 100,
                        channel: 0,
                    },
                }],
                &mut buf,
            )
            .unwrap();
        assert!(!buf.is_silent());

        // Note off
        buf.clear();
        plugin
            .process_block(
                &[MidiEvent {
                    sample_offset: 0,
                    kind: MidiEventKind::NoteOff {
                        note: 60,
                        velocity: 0,
                        channel: 0,
                    },
                }],
                &mut buf,
            )
            .unwrap();
        assert!(buf.is_silent());
    }

    #[test]
    fn velocity_scales_amplitude() {
        let mut loud = MockVstPlugin::new(44100);
        let mut quiet = MockVstPlugin::new(44100);

        let mut loud_buf = AudioBuffer::new(1, 512, 44100);
        let mut quiet_buf = AudioBuffer::new(1, 512, 44100);

        let loud_event = MidiEvent {
            sample_offset: 0,
            kind: MidiEventKind::NoteOn {
                note: 60,
                velocity: 127,
                channel: 0,
            },
        };
        let quiet_event = MidiEvent {
            sample_offset: 0,
            kind: MidiEventKind::NoteOn {
                note: 60,
                velocity: 32,
                channel: 0,
            },
        };

        loud.process_block(&[loud_event], &mut loud_buf).unwrap();
        quiet.process_block(&[quiet_event], &mut quiet_buf).unwrap();

        assert!(loud_buf.rms() > quiet_buf.rms());
    }

    #[test]
    fn preset_changes_gain() {
        let mut plugin = MockVstPlugin::new(44100);
        let event = MidiEvent {
            sample_offset: 0,
            kind: MidiEventKind::NoteOn {
                note: 60,
                velocity: 127,
                channel: 0,
            },
        };

        // Full preset (gain=1.0)
        plugin.set_preset(0);
        let mut full_buf = AudioBuffer::new(1, 512, 44100);
        plugin.process_block(&[event], &mut full_buf).unwrap();
        let full_rms = full_buf.rms();

        // Reset phase
        plugin.phase = 0.0;
        plugin.active_note = None;

        // Half preset (gain=0.5)
        plugin.set_preset(2);
        let mut half_buf = AudioBuffer::new(1, 512, 44100);
        plugin.process_block(&[event], &mut half_buf).unwrap();
        let half_rms = half_buf.rms();

        // Half gain should produce half RMS (approximately)
        let ratio = half_rms / full_rms;
        assert!((ratio - 0.5).abs() < 0.01);
    }

    #[test]
    fn different_notes_different_frequencies() {
        let mut plugin_low = MockVstPlugin::new(44100);
        let mut plugin_high = MockVstPlugin::new(44100);

        let mut buf_low = AudioBuffer::new(1, 4410, 44100); // 100ms
        let mut buf_high = AudioBuffer::new(1, 4410, 44100);

        plugin_low
            .process_block(
                &[MidiEvent {
                    sample_offset: 0,
                    kind: MidiEventKind::NoteOn {
                        note: 48, // C3
                        velocity: 127,
                        channel: 0,
                    },
                }],
                &mut buf_low,
            )
            .unwrap();

        plugin_high
            .process_block(
                &[MidiEvent {
                    sample_offset: 0,
                    kind: MidiEventKind::NoteOn {
                        note: 72, // C5
                        velocity: 127,
                        channel: 0,
                    },
                }],
                &mut buf_high,
            )
            .unwrap();

        // Count zero crossings - higher note should have more
        let crossings_low = count_zero_crossings(buf_low.channel(0));
        let crossings_high = count_zero_crossings(buf_high.channel(0));
        assert!(crossings_high > crossings_low);
    }

    fn count_zero_crossings(samples: &[f32]) -> usize {
        samples
            .windows(2)
            .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
            .count()
    }
}
