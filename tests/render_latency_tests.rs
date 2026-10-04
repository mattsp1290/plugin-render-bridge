use plugin_render_bridge::{
    RenderSettings,
    buffer::AudioBuffer,
    midi::{MidiEvent, MidiEventKind},
    render::{AudioProcessor, render_note},
};
use std::collections::VecDeque;

struct DelayedTone {
    delay: VecDeque<f32>,
    active: bool,
}
impl DelayedTone {
    fn new(latency: usize) -> Self {
        Self {
            delay: vec![0.0; latency].into(),
            active: false,
        }
    }
}
impl AudioProcessor for DelayedTone {
    fn process_block(
        &mut self,
        events: &[MidiEvent],
        output: &mut AudioBuffer,
    ) -> Result<(), String> {
        for i in 0..output.num_samples() {
            for event in events
                .iter()
                .filter(|event| event.sample_offset as usize == i)
            {
                match event.kind {
                    MidiEventKind::NoteOn { .. } => self.active = true,
                    MidiEventKind::NoteOff { .. } => self.active = false,
                    _ => {}
                }
            }
            self.delay.push_back(if self.active { 0.5 } else { 0.0 });
            let value = self.delay.pop_front().unwrap();
            for ch in 0..output.num_channels() {
                output.channel_mut(ch)[i] = value;
            }
        }
        Ok(())
    }
}

fn render(latency: u32, tail: Option<u32>, duration: f64) -> AudioBuffer {
    let settings = RenderSettings {
        note_duration_secs: duration,
        ..Default::default()
    };
    render_note(
        &mut DelayedTone::new(latency as usize),
        60,
        127,
        &settings,
        tail,
        latency,
    )
    .unwrap()
}

#[test]
fn latency_longer_than_note_retains_audio() {
    let plain = render(0, None, 0.01);
    let delayed = render(4096, None, 0.01);
    assert_eq!(&plain.channel(0)[..441], &[0.5; 441]);
    assert_eq!(&delayed.channel(0)[..441], &plain.channel(0)[..441]);
    assert!(delayed.channel(0)[441..].iter().all(|s| *s == 0.0));
}

#[test]
fn latency_is_drained_even_when_declared_tail_is_shorter() {
    let delayed = render(4096, Some(0), 0.01);
    assert_eq!(&delayed.channel(0)[..441], &[0.5; 441]);
}

#[test]
fn note_off_is_sample_accurate_inside_block() {
    let audio = render(0, None, 0.01);
    assert_eq!(&audio.channel(0)[..441], &[0.5; 441]);
    assert!(audio.channel(0)[441..].iter().all(|sample| *sample == 0.0));
    assert!(render(0, None, 0.0).is_silent());
}
