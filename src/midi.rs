/// A MIDI event with sample-accurate timing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MidiEvent {
    /// Sample offset within the current buffer.
    pub sample_offset: u32,
    pub kind: MidiEventKind,
}

/// Types of MIDI events.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MidiEventKind {
    NoteOn {
        note: u8,
        velocity: u8,
        channel: u8,
    },
    NoteOff {
        note: u8,
        velocity: u8,
        channel: u8,
    },
    ControlChange {
        controller: u8,
        value: u8,
        channel: u8,
    },
}

/// A sequence of MIDI events for rendering one note sample.
#[derive(Debug, Clone)]
pub struct NoteSequence {
    pub events: Vec<MidiEvent>,
}

impl NoteSequence {
    /// Generate a note-on / note-off sequence for a single note.
    ///
    /// - `note_on_sample`: sample offset for note-on
    /// - `duration_samples`: how long to sustain (note-off = note_on_sample + duration_samples)
    pub fn single_note(
        note: u8,
        velocity: u8,
        channel: u8,
        note_on_sample: u32,
        duration_samples: u32,
    ) -> Self {
        let events = vec![
            MidiEvent {
                sample_offset: note_on_sample,
                kind: MidiEventKind::NoteOn {
                    note,
                    velocity,
                    channel,
                },
            },
            MidiEvent {
                sample_offset: note_on_sample + duration_samples,
                kind: MidiEventKind::NoteOff {
                    note,
                    velocity: 0,
                    channel,
                },
            },
        ];
        Self { events }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_note_sequence() {
        let seq = NoteSequence::single_note(60, 127, 0, 0, 44100);
        assert_eq!(seq.events.len(), 2);
        assert_eq!(seq.events[0].sample_offset, 0);
        assert_eq!(seq.events[1].sample_offset, 44100);
        assert!(matches!(
            seq.events[0].kind,
            MidiEventKind::NoteOn { note: 60, .. }
        ));
        assert!(matches!(
            seq.events[1].kind,
            MidiEventKind::NoteOff { note: 60, .. }
        ));
    }
}
