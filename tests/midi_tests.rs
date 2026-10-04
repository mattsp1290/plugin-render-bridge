//! Integration tests for MIDI generation and sampling plans.

use plugin_render_bridge::midi::{MidiEvent, MidiEventKind, NoteSequence};

#[test]
fn note_sequence_has_on_and_off() {
    let seq = NoteSequence::single_note(60, 100, 0, 0, 22050);

    assert_eq!(seq.events.len(), 2);

    match seq.events[0].kind {
        MidiEventKind::NoteOn {
            note,
            velocity,
            channel,
        } => {
            assert_eq!(note, 60);
            assert_eq!(velocity, 100);
            assert_eq!(channel, 0);
        }
        _ => panic!("expected NoteOn"),
    }

    match seq.events[1].kind {
        MidiEventKind::NoteOff { note, channel, .. } => {
            assert_eq!(note, 60);
            assert_eq!(channel, 0);
        }
        _ => panic!("expected NoteOff"),
    }

    assert_eq!(seq.events[0].sample_offset, 0);
    assert_eq!(seq.events[1].sample_offset, 22050);
}

#[test]
fn note_sequence_with_offset() {
    let seq = NoteSequence::single_note(69, 127, 0, 1000, 44100);
    assert_eq!(seq.events[0].sample_offset, 1000);
    assert_eq!(seq.events[1].sample_offset, 45100);
}

#[test]
fn midi_event_equality() {
    let a = MidiEvent {
        sample_offset: 0,
        kind: MidiEventKind::NoteOn {
            note: 60,
            velocity: 127,
            channel: 0,
        },
    };
    let b = MidiEvent {
        sample_offset: 0,
        kind: MidiEventKind::NoteOn {
            note: 60,
            velocity: 127,
            channel: 0,
        },
    };
    assert_eq!(a, b);

    let c = MidiEvent {
        sample_offset: 0,
        kind: MidiEventKind::NoteOn {
            note: 61,
            velocity: 127,
            channel: 0,
        },
    };
    assert_ne!(a, c);
}
