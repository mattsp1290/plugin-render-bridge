//! Integration tests for audio buffer operations.

use plugin_render_bridge::buffer::AudioBuffer;
use plugin_render_bridge::testing::{assert_audio_eq, assert_not_silent, assert_silent};
use plugin_render_bridge::testing::{
    generate_noise_buffer, generate_silent_buffer, generate_sine_buffer,
};

#[test]
fn sine_buffer_roundtrip_interleaved() {
    let buf = generate_sine_buffer(440.0, 0.1, 44100, 2);
    let interleaved = buf.to_interleaved();
    let restored = AudioBuffer::from_interleaved(&interleaved, 2, 44100);

    assert_audio_eq(&buf, &restored, 0.0);
}

#[test]
fn buffer_slice_preserves_content() {
    let buf = generate_sine_buffer(440.0, 0.1, 44100, 1);
    let slice = buf.slice(100, 200);

    assert_eq!(slice.num_samples(), 200);
    assert_eq!(slice.channel(0)[0], buf.channel(0)[100]);
    assert_eq!(slice.channel(0)[199], buf.channel(0)[299]);
}

#[test]
fn buffer_append_concatenates() {
    let a = generate_sine_buffer(440.0, 0.05, 44100, 2);
    let b = generate_sine_buffer(880.0, 0.05, 44100, 2);
    let a_len = a.num_samples();
    let b_len = b.num_samples();

    let mut combined = a.clone();
    combined.append(&b);

    assert_eq!(combined.num_samples(), a_len + b_len);
    // First part matches a
    assert_eq!(combined.channel(0)[0], a.channel(0)[0]);
    // Second part matches b
    assert_eq!(combined.channel(0)[a_len], b.channel(0)[0]);
}

#[test]
fn buffer_clear_zeroes_all() {
    let mut buf = generate_sine_buffer(440.0, 0.05, 44100, 2);
    assert_not_silent(&buf);

    buf.clear();
    assert_silent(&buf);
}

#[test]
fn buffer_resize_extends() {
    let mut buf = AudioBuffer::new(2, 100, 44100);
    buf.channel_mut(0)[50] = 1.0;

    buf.resize(200);
    assert_eq!(buf.num_samples(), 200);
    assert_eq!(buf.channel(0)[50], 1.0);
    assert_eq!(buf.channel(0)[150], 0.0); // new samples are zero
}

#[test]
fn rms_of_known_signal() {
    // DC signal of 0.5 should have RMS of 0.5
    let mut buf = AudioBuffer::new(1, 1000, 44100);
    for s in buf.channel_mut(0).iter_mut() {
        *s = 0.5;
    }
    let rms = buf.rms();
    assert!((rms - 0.5).abs() < 1e-6);
}

#[test]
fn noise_buffer_has_content() {
    let buf = generate_noise_buffer(42, 0.1, 44100, 1);
    assert_not_silent(&buf);
}

#[test]
fn silent_buffer_is_detected() {
    let buf = generate_silent_buffer(0.1, 44100, 2);
    assert_silent(&buf);
}

#[test]
fn duration_calculation() {
    let buf = AudioBuffer::new(2, 44100, 44100);
    assert!((buf.duration_secs() - 1.0).abs() < 1e-6);

    let buf2 = AudioBuffer::new(2, 22050, 44100);
    assert!((buf2.duration_secs() - 0.5).abs() < 1e-6);
}

#[test]
fn zero_sample_rate_duration() {
    let buf = AudioBuffer::new(1, 100, 0);
    assert_eq!(buf.duration_secs(), 0.0);
}
