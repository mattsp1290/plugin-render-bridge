//! Integration tests for the rendering pipeline using the mock VST.

use plugin_render_bridge::render::{AudioProcessor, blocks_for_duration, render_note};
use plugin_render_bridge::testing::MockVstPlugin;
use plugin_render_bridge::testing::fast_render_settings;
use plugin_render_bridge::testing::{assert_audio_eq, assert_louder_than, assert_not_silent};

#[test]
fn render_note_produces_audio() {
    let mut plugin = MockVstPlugin::new(44100);
    let config = fast_render_settings();

    let result = render_note(&mut plugin, 60, 127, &config, None, 0).unwrap();

    assert!(result.num_samples() > 0);
    assert_not_silent(&result);
}

#[test]
fn render_different_notes_produce_different_output() {
    let config = fast_render_settings();

    let mut plugin_c4 = MockVstPlugin::new(44100);
    let buf_c4 = render_note(&mut plugin_c4, 60, 127, &config, None, 0).unwrap();

    let mut plugin_a4 = MockVstPlugin::new(44100);
    let buf_a4 = render_note(&mut plugin_a4, 69, 127, &config, None, 0).unwrap();

    // Both should have audio
    assert_not_silent(&buf_c4);
    assert_not_silent(&buf_a4);

    // But they should differ (different frequencies)
    let min_len = buf_c4.num_samples().min(buf_a4.num_samples());
    let mut any_diff = false;
    for i in 0..min_len {
        if (buf_c4.channel(0)[i] - buf_a4.channel(0)[i]).abs() > 1e-6 {
            any_diff = true;
            break;
        }
    }
    assert!(any_diff, "different notes should produce different audio");
}

#[test]
fn render_louder_velocity_is_louder() {
    let config = fast_render_settings();

    let mut loud = MockVstPlugin::new(44100);
    let buf_loud = render_note(&mut loud, 60, 127, &config, None, 0).unwrap();

    let mut quiet = MockVstPlugin::new(44100);
    let buf_quiet = render_note(&mut quiet, 60, 32, &config, None, 0).unwrap();

    assert_louder_than(&buf_loud, &buf_quiet);
}

#[test]
fn render_with_preset_change() {
    let config = fast_render_settings();

    let mut full = MockVstPlugin::new(44100);
    full.set_preset(0); // gain = 1.0
    let buf_full = render_note(&mut full, 60, 127, &config, None, 0).unwrap();

    let mut half = MockVstPlugin::new(44100);
    half.set_preset(2); // gain = 0.5
    let buf_half = render_note(&mut half, 60, 127, &config, None, 0).unwrap();

    assert_louder_than(&buf_full, &buf_half);
}

#[test]
fn render_note_stops_after_silence() {
    let mut plugin = MockVstPlugin::new(44100);
    let config = fast_render_settings();

    let result = render_note(&mut plugin, 60, 127, &config, None, 0).unwrap();

    // The render should stop shortly after note-off (when silence detected)
    // With 50ms note + silence detection, should be reasonable length
    let max_expected_secs = config.note_duration_secs + 1.0; // generous margin
    assert!(result.duration_secs() < max_expected_secs);
}

#[test]
fn blocks_for_duration_various_rates() {
    // 1 second at 44100 Hz, 512 block size -> ceil(44100/512) = 87
    assert_eq!(blocks_for_duration(1.0, 44100, 512), 87);

    // 0.5 seconds at 48000 Hz, 256 block size -> ceil(24000/256) = 94
    assert_eq!(blocks_for_duration(0.5, 48000, 256), 94);

    // Very short duration
    assert_eq!(blocks_for_duration(0.001, 44100, 512), 1);
}

#[test]
fn render_note_is_stereo() {
    let mut plugin = MockVstPlugin::new(44100);
    let config = fast_render_settings();

    let result = render_note(&mut plugin, 60, 127, &config, None, 0).unwrap();

    assert_eq!(result.num_channels(), 2);
    // Both channels should have the same content (mock outputs identical stereo)
    let min_nonzero = result.channel(0).iter().position(|&s| s.abs() > 1e-6);
    assert!(min_nonzero.is_some());
    let idx = min_nonzero.unwrap();
    assert_eq!(result.channel(0)[idx], result.channel(1)[idx]);
}

#[test]
fn render_with_state_roundtrip() {
    let config = fast_render_settings();

    // Render with preset 2 (gain=0.5), then capture state bytes.
    let mut plugin1 = MockVstPlugin::new(44100);
    plugin1.set_preset(2);
    let state = plugin1.get_state();
    let buf1 = render_note(&mut plugin1, 60, 127, &config, None, 0).unwrap();

    // Fresh plugin (default preset 0, gain=1.0), restore state, render again.
    let mut plugin2 = MockVstPlugin::new(44100);
    plugin2.set_state(&state).unwrap();
    let buf2 = render_note(&mut plugin2, 60, 127, &config, None, 0).unwrap();

    // Same state should produce identical audio.
    assert_audio_eq(&buf1, &buf2, 1e-6);
}

#[test]
fn render_with_state_differs_from_default() {
    let config = fast_render_settings();

    // Default state (preset 0, gain=1.0)
    let mut default_plugin = MockVstPlugin::new(44100);
    let buf_default = render_note(&mut default_plugin, 60, 127, &config, None, 0).unwrap();

    // Restored state (preset 2, gain=0.5)
    let mut source = MockVstPlugin::new(44100);
    source.set_preset(2);
    let state = source.get_state();
    let mut restored = MockVstPlugin::new(44100);
    restored.set_state(&state).unwrap();
    let buf_restored = render_note(&mut restored, 60, 127, &config, None, 0).unwrap();

    // Default (louder) should be louder than restored (quieter)
    assert_louder_than(&buf_default, &buf_restored);
}
