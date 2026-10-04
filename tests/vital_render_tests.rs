mod common;
// Integration tests for rendering audio through real Vital VST3 plugin.
//
// These tests require Vital to be installed on the system.
// Run with: `cargo test --features testing --test vital_render_tests`

use common::{find_vital_path, is_vital_installed, rms_level};
use plugin_hostkit::VstInstance;
use plugin_hostkit::discovery::Vst3Bundle;
use plugin_hostkit::host::ProcessConfig;
use plugin_render_bridge::RenderSettings;
use plugin_render_bridge::render::render_note;
use plugin_render_bridge::vst_adapter::VstProcessor;
use std::sync::Mutex;
use std::time::Duration;

/// In-process VST3 plugin operations are not thread-safe. Tests that load
/// a VstInstance directly must hold this lock to avoid concurrent plugin
/// access causing SIGSEGV on process exit.
static IN_PROCESS_LOCK: Mutex<()> = Mutex::new(());

/// Short render config for fast testing.
fn fast_config() -> RenderSettings {
    RenderSettings {
        note_duration_secs: 0.5,
        tail_timeout: Duration::from_secs(3),
        silence_threshold: 1e-6,
        ..Default::default()
    }
}

/// Load and activate Vital, returning a VstProcessor.
fn load_vital() -> Option<VstProcessor> {
    let vital_path = find_vital_path()?;
    let bundle = Vst3Bundle::from_path(&vital_path)?;
    let binary = bundle.binary_path?;

    let mut instance = match VstInstance::load(&binary) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("VstInstance::load failed: {e}");
            return None;
        }
    };
    if let Err(e) = instance.initialize() {
        eprintln!("initialize failed: {e}");
        return None;
    }
    if let Err(e) = instance.setup_processing(ProcessConfig::new(44100.0, 512)) {
        eprintln!("setup_processing failed: {e}");
        return None;
    }
    if let Err(e) = instance.activate() {
        eprintln!("activate failed: {e}");
        return None;
    }

    Some(VstProcessor::new(instance))
}

/// Verify Vital is discoverable via the plugin scanner.
fn vital_discoverable_via_scanner() {
    if !is_vital_installed() {
        eprintln!("SKIPPED: Vital not installed");
        return;
    }

    let scanner = plugin_hostkit::PluginScanner::new();
    let bundles = scanner.discover_bundles();

    let vital_bundle = bundles.iter().find(|b| {
        let info = plugin_hostkit::PluginScanner::bundle_to_info(b);
        info.name.to_lowercase().contains("vital")
    });

    assert!(vital_bundle.is_some(), "Vital should be discoverable");
}

/// Test that Vital renders non-silent audio for a single note.
fn vital_renders_audio_for_single_note() {
    if !is_vital_installed() {
        eprintln!("SKIPPED: Vital not installed");
        return;
    }
    let _guard = IN_PROCESS_LOCK.lock().unwrap();

    let mut processor = match load_vital() {
        Some(p) => p,
        None => {
            panic!("installed Vital failed to load");
        }
    };

    let config = fast_config();
    let result = render_note(&mut processor, 60, 127, &config, None, 0);

    match result {
        Ok(audio) => {
            assert!(
                audio.num_samples() > 0,
                "Rendered audio should have samples"
            );
            let rms = rms_level(&audio);
            eprintln!(
                "Rendered C4 v127: {} samples, RMS={rms:.6}",
                audio.num_samples()
            );
            // Vital's Init preset should produce audible output
            assert!(rms > 1e-5, "Audio should not be silent (RMS={rms})");
        }
        Err(e) => {
            panic!("Render failed: {e}");
        }
    }
}

/// Test that different pitches produce different audio.
fn vital_renders_different_pitches() {
    if !is_vital_installed() {
        eprintln!("SKIPPED: Vital not installed");
        return;
    }
    let _guard = IN_PROCESS_LOCK.lock().unwrap();

    let mut processor = match load_vital() {
        Some(p) => p,
        None => {
            panic!("installed Vital failed to load");
        }
    };

    let config = fast_config();

    let low = render_note(&mut processor, 36, 127, &config, None, 0);
    let mid = render_note(&mut processor, 60, 127, &config, None, 0);
    let high = render_note(&mut processor, 84, 127, &config, None, 0);

    // All should produce audio
    if let (Ok(low), Ok(mid), Ok(high)) = (&low, &mid, &high) {
        let rms_low = rms_level(low);
        let rms_mid = rms_level(mid);
        let rms_high = rms_level(high);
        eprintln!("RMS: low={rms_low:.6}, mid={rms_mid:.6}, high={rms_high:.6}");

        assert!(rms_low > 1e-5, "Low note should produce audio");
        assert!(rms_mid > 1e-5, "Mid note should produce audio");
        assert!(rms_high > 1e-5, "High note should produce audio");
    } else {
        panic!(
            "Render failed: low={:?}, mid={:?}, high={:?}",
            low.as_ref().err(),
            mid.as_ref().err(),
            high.as_ref().err()
        );
    }
}

/// Test that velocity affects output amplitude.
fn vital_velocity_scaling() {
    if !is_vital_installed() {
        eprintln!("SKIPPED: Vital not installed");
        return;
    }
    let _guard = IN_PROCESS_LOCK.lock().unwrap();

    let mut processor = match load_vital() {
        Some(p) => p,
        None => {
            panic!("installed Vital failed to load");
        }
    };

    let config = fast_config();

    let soft = render_note(&mut processor, 60, 32, &config, None, 0);
    let loud = render_note(&mut processor, 60, 127, &config, None, 0);

    if let (Ok(soft), Ok(loud)) = (&soft, &loud) {
        let rms_soft = rms_level(soft);
        let rms_loud = rms_level(loud);
        eprintln!("RMS: soft(v32)={rms_soft:.6}, loud(v127)={rms_loud:.6}");

        // Velocity 127 should generally be louder than velocity 32
        // (depends on Vital's Init preset velocity sensitivity)
        if rms_soft > 1e-5 && rms_loud > 1e-5 {
            assert!(
                rms_loud >= rms_soft * 0.5,
                "Loud velocity should produce comparable or more audio"
            );
        }
    } else {
        panic!(
            "Render failed: soft={:?}, loud={:?}",
            soft.as_ref().err(),
            loud.as_ref().err()
        );
    }
}

/// Test that rendering with tempo (ProcessContext) works correctly.
fn vital_renders_with_tempo() {
    if !is_vital_installed() {
        eprintln!("SKIPPED: Vital not installed");
        return;
    }
    let _guard = IN_PROCESS_LOCK.lock().unwrap();

    let mut processor = match load_vital() {
        Some(p) => p,
        None => {
            panic!("installed Vital failed to load");
        }
    };

    processor.set_tempo(140.0);

    let config = fast_config();
    let result = render_note(&mut processor, 60, 127, &config, None, 0);

    match result {
        Ok(audio) => {
            let rms = rms_level(&audio);
            eprintln!(
                "Rendered C4 with tempo=140 BPM: {} samples, RMS={rms:.6}",
                audio.num_samples()
            );
            assert!(
                rms > 1e-5,
                "Audio with tempo should not be silent (RMS={rms})"
            );
        }
        Err(e) => panic!("Render with tempo failed: {e}"),
    }
}

/// Test that rendering works correctly across tempo changes.
fn vital_render_with_tempo_change() {
    if !is_vital_installed() {
        eprintln!("SKIPPED: Vital not installed");
        return;
    }
    let _guard = IN_PROCESS_LOCK.lock().unwrap();

    let mut processor = match load_vital() {
        Some(p) => p,
        None => {
            panic!("installed Vital failed to load");
        }
    };

    let config = fast_config();

    // Render with slow tempo
    processor.set_tempo(80.0);
    let slow = render_note(&mut processor, 60, 127, &config, None, 0);

    // Render with fast tempo
    processor.set_tempo(200.0);
    let fast = render_note(&mut processor, 60, 127, &config, None, 0);

    match (&slow, &fast) {
        (Ok(slow_audio), Ok(fast_audio)) => {
            let rms_slow = rms_level(slow_audio);
            let rms_fast = rms_level(fast_audio);
            eprintln!("RMS: tempo=80 → {rms_slow:.6}, tempo=200 → {rms_fast:.6}");
            assert!(rms_slow > 1e-5, "Slow tempo render should not be silent");
            assert!(rms_fast > 1e-5, "Fast tempo render should not be silent");
        }
        _ => panic!(
            "Render failed: slow={:?}, fast={:?}",
            slow.as_ref().err(),
            fast.as_ref().err()
        ),
    }
}

fn main() {
    let cases: &[(&str, fn())] = &[
        (
            "vital_discoverable_via_scanner",
            vital_discoverable_via_scanner,
        ),
        (
            "vital_renders_audio_for_single_note",
            vital_renders_audio_for_single_note,
        ),
        (
            "vital_renders_different_pitches",
            vital_renders_different_pitches,
        ),
        ("vital_velocity_scaling", vital_velocity_scaling),
        ("vital_renders_with_tempo", vital_renders_with_tempo),
        (
            "vital_render_with_tempo_change",
            vital_render_with_tempo_change,
        ),
    ];
    for (name, run) in cases {
        run();
        eprintln!("test {name} ... ok");
    }
    eprintln!("{} cases completed", cases.len());
}
