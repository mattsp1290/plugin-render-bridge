mod common;
use plugin_render_bridge::bridge::{BridgeRenderer, find_renderer_bin};
use plugin_render_bridge::render::NoteRenderer;

fn spawn_vital_bridge() -> BridgeRenderer {
    let vital_path = common::find_vital_path().expect("Vital not found");
    let bundle = plugin_hostkit::Vst3Bundle::from_path(&vital_path).expect("invalid bundle");
    let binary_path = bundle.binary_path.expect("no binary in bundle");
    let renderer_bin = find_renderer_bin().expect("plugin-renderer not found");
    BridgeRenderer::spawn(&renderer_bin, &binary_path, 44100, 512, Some(120.0))
        .expect("failed to spawn bridge renderer")
}

#[test]
fn find_renderer_bin_from_test_context() {
    // This test verifies the parent-dir fallback in find_renderer_bin
    // works when running from target/debug/deps/
    let result = find_renderer_bin();
    assert!(
        result.is_ok(),
        "find_renderer_bin failed: {:?}",
        result.err()
    );
    let path = result.unwrap();
    assert!(
        path.exists(),
        "plugin-renderer binary not found at {}",
        path.display()
    );
}

#[test]
fn bridge_renderer_spawns_and_reports_name() {
    if !common::is_vital_installed() {
        eprintln!("SKIPPED: Vital not installed");
        return;
    }
    let renderer = spawn_vital_bridge();
    assert!(!renderer.plugin_name().is_empty());
    // Drop triggers graceful shutdown
}

#[test]
fn bridge_renders_single_note_to_file() {
    if !common::is_vital_installed() {
        eprintln!("SKIPPED: Vital not installed");
        return;
    }
    let mut renderer = spawn_vital_bridge();
    let dir = tempfile::tempdir().unwrap();
    let wav_path = dir.path().join("test_note.wav");
    let config = plugin_render_bridge::testing::fast_render_settings();

    let samples = renderer
        .render_to_file(60, 127, &wav_path, &config)
        .expect("render_to_file failed");

    assert!(samples > 0, "rendered 0 samples");
    assert!(wav_path.exists(), "WAV file not created");

    let audio = plugin_render_bridge::wav::read_wav(&wav_path).expect("failed to read WAV");
    assert!(audio.num_samples() > 0);
    plugin_render_bridge::testing::assert_not_silent(&audio);
}

#[test]
fn bridge_renders_different_pitches() {
    if !common::is_vital_installed() {
        eprintln!("SKIPPED: Vital not installed");
        return;
    }
    let mut renderer = spawn_vital_bridge();
    let dir = tempfile::tempdir().unwrap();
    let config = plugin_render_bridge::testing::fast_render_settings();

    let low_path = dir.path().join("low.wav");
    let high_path = dir.path().join("high.wav");

    renderer
        .render_to_file(48, 127, &low_path, &config)
        .expect("low render failed");
    renderer
        .render_to_file(72, 127, &high_path, &config)
        .expect("high render failed");

    let low_audio = plugin_render_bridge::wav::read_wav(&low_path).unwrap();
    let high_audio = plugin_render_bridge::wav::read_wav(&high_path).unwrap();

    // Both should have audio content
    plugin_render_bridge::testing::assert_not_silent(&low_audio);
    plugin_render_bridge::testing::assert_not_silent(&high_audio);

    // Different pitches should produce different audio
    // (check that the buffers aren't identical)
    let min_len = low_audio.num_samples().min(high_audio.num_samples());
    assert!(min_len > 0);
    let mut differs = false;
    for i in 0..min_len {
        if (low_audio.channel(0)[i] - high_audio.channel(0)[i]).abs() > 1e-6 {
            differs = true;
            break;
        }
    }
    assert!(differs, "low and high pitch rendered identical audio");
}

#[test]
fn bridge_render_note_via_trait() {
    if !common::is_vital_installed() {
        eprintln!("SKIPPED: Vital not installed");
        return;
    }
    let mut renderer = spawn_vital_bridge();
    let config = plugin_render_bridge::testing::fast_render_settings();

    // NoteRenderer::render_note goes through a temp file internally
    let audio = renderer
        .render_note(60, 127, &config)
        .expect("render_note via trait failed");

    assert!(audio.num_samples() > 0);
    plugin_render_bridge::testing::assert_not_silent(&audio);
}

#[test]
fn bridge_is_alive_after_spawn() {
    if !common::is_vital_installed() {
        eprintln!("SKIPPED: Vital not installed");
        return;
    }
    let mut renderer = spawn_vital_bridge();
    assert!(renderer.is_alive(), "renderer should be alive after spawn");
}

#[test]
fn bridge_respawn_then_render() {
    if !common::is_vital_installed() {
        eprintln!("SKIPPED: Vital not installed");
        return;
    }
    let mut renderer = spawn_vital_bridge();
    let dir = tempfile::tempdir().unwrap();
    let config = plugin_render_bridge::testing::fast_render_settings();

    // Render once before respawn
    let path1 = dir.path().join("before_respawn.wav");
    renderer
        .render_to_file(60, 127, &path1, &config)
        .expect("pre-respawn render failed");

    // Respawn
    renderer.respawn().expect("respawn failed");
    assert!(
        renderer.is_alive(),
        "renderer should be alive after respawn"
    );

    // Render again after respawn
    let path2 = dir.path().join("after_respawn.wav");
    renderer
        .render_to_file(60, 127, &path2, &config)
        .expect("post-respawn render failed");

    let audio = plugin_render_bridge::wav::read_wav(&path2).unwrap();
    plugin_render_bridge::testing::assert_not_silent(&audio);
}
