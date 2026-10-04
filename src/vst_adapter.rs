//! Adapter bridging `plugin_hostkit::VstInstance` to the `AudioProcessor` trait.
//!
//! This allows VST3 plugin instances to be used with the rendering pipeline
//! (`render_note`, batch processing, etc.).

use plugin_hostkit::VstInstance;

use crate::buffer::AudioBuffer;
use crate::midi::{MidiEvent, MidiEventKind};
use crate::render::AudioProcessor;

/// Wraps a [`VstInstance`] as an [`AudioProcessor`] for the rendering pipeline.
///
/// The VstInstance must already be in the Active state (loaded, initialized,
/// setup_processing called, and activated) before wrapping it.
pub struct VstProcessor {
    instance: VstInstance,
}

impl VstProcessor {
    /// Create a new VstProcessor from an already-activated VstInstance.
    pub fn new(instance: VstInstance) -> Self {
        Self { instance }
    }

    /// Get a reference to the underlying VstInstance.
    pub fn instance(&self) -> &VstInstance {
        &self.instance
    }

    /// Consume the adapter and return the inner VstInstance.
    pub fn into_inner(self) -> VstInstance {
        self.instance
    }

    /// Get a mutable reference to the underlying VstInstance.
    pub fn instance_mut(&mut self) -> &mut VstInstance {
        &mut self.instance
    }

    /// Set the tempo (BPM) communicated to the plugin via `ProcessContext` on each render block.
    /// Values are clamped to 20–300 BPM by the underlying `VstInstance`.
    pub fn set_tempo(&mut self, bpm: f64) {
        self.instance.set_tempo(bpm);
    }
}

impl AudioProcessor for VstProcessor {
    fn reset_transport(&mut self) {
        self.instance.reset_position();
    }

    fn process_block(
        &mut self,
        events: &[MidiEvent],
        output: &mut AudioBuffer,
    ) -> Result<(), String> {
        // Convert MidiEvents to the VstInstance tuple format:
        // (sample_offset, note, velocity, channel)
        // Convention: velocity > 0 = note-on, velocity == 0 = note-off
        let vst_events: Vec<(u32, u8, u8, u8)> = events
            .iter()
            .filter_map(|event| match event.kind {
                MidiEventKind::NoteOn {
                    note,
                    velocity,
                    channel,
                } => Some((event.sample_offset, note, velocity, channel)),
                MidiEventKind::NoteOff { note, channel, .. } => {
                    Some((event.sample_offset, note, 0, channel))
                }
                MidiEventKind::ControlChange { .. } => None,
            })
            .collect();

        // Get mutable channel slices from the output buffer
        let mut channels = output.channels_mut();

        self.instance
            .process(&vst_events, &mut channels)
            .map_err(|e| e.to_string())
    }

    fn set_state(&mut self, state_bytes: &[u8]) -> Result<(), String> {
        self.instance
            .set_state(state_bytes)
            .map_err(|e| e.to_string())
    }
}
