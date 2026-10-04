//! Integration tests for WAV I/O with generated test buffers.

use plugin_render_bridge::BitDepth;
use plugin_render_bridge::testing::assert_audio_eq;
use plugin_render_bridge::testing::{generate_noise_buffer, generate_sine_buffer};
use plugin_render_bridge::wav::{read_wav, write_wav};

#[test]
fn sine_wav_roundtrip_float32() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("sine_f32.wav");

    let buf = generate_sine_buffer(440.0, 0.1, 44100, 2);
    write_wav(&buf, &path, BitDepth::Float32).unwrap();

    let read_back = read_wav(&path).unwrap();
    assert_audio_eq(&buf, &read_back, 1e-6);
}

#[test]
fn sine_wav_roundtrip_int24() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("sine_24.wav");

    let buf = generate_sine_buffer(440.0, 0.1, 44100, 2);
    write_wav(&buf, &path, BitDepth::Int24).unwrap();

    let read_back = read_wav(&path).unwrap();
    // 24-bit has some quantization error
    assert_audio_eq(&buf, &read_back, 0.0002);
}

#[test]
fn sine_wav_roundtrip_int16() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("sine_16.wav");

    let buf = generate_sine_buffer(440.0, 0.1, 44100, 2);
    write_wav(&buf, &path, BitDepth::Int16).unwrap();

    let read_back = read_wav(&path).unwrap();
    // 16-bit has more quantization error
    assert_audio_eq(&buf, &read_back, 0.001);
}

#[test]
fn noise_wav_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("noise.wav");

    let buf = generate_noise_buffer(42, 0.05, 44100, 1);
    write_wav(&buf, &path, BitDepth::Float32).unwrap();

    let read_back = read_wav(&path).unwrap();
    assert_audio_eq(&buf, &read_back, 1e-6);
}

#[test]
fn different_sample_rates() {
    for &rate in &[44100u32, 48000, 88200, 96000] {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(format!("test_{rate}.wav"));

        let buf = generate_sine_buffer(440.0, 0.05, rate, 2);
        write_wav(&buf, &path, BitDepth::Float32).unwrap();

        let read_back = read_wav(&path).unwrap();
        assert_eq!(read_back.sample_rate(), rate);
        assert_audio_eq(&buf, &read_back, 1e-6);
    }
}

#[test]
fn mono_wav() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("mono.wav");

    let buf = generate_sine_buffer(440.0, 0.05, 44100, 1);
    write_wav(&buf, &path, BitDepth::Float32).unwrap();

    let read_back = read_wav(&path).unwrap();
    assert_eq!(read_back.num_channels(), 1);
    assert_audio_eq(&buf, &read_back, 1e-6);
}
