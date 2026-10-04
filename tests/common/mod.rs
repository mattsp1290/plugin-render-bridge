#![allow(dead_code)]
//! Helpers for integration tests that require real Vital installation.
//!
//! Provides detection, skip logic, and WAV assertion utilities.

use std::path::{Path, PathBuf};

use plugin_render_bridge::buffer::AudioBuffer;

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

/// Platform-specific search paths for the Vital VST3 bundle.
fn vital_search_paths() -> Vec<Option<PathBuf>> {
    let mut paths = Vec::new();

    #[cfg(target_os = "linux")]
    {
        paths.push(home_dir().map(|h| h.join(".vst3/Vital.vst3")));
        paths.push(Some(PathBuf::from("/usr/lib/vst3/Vital.vst3")));
        paths.push(Some(PathBuf::from("/usr/local/lib/vst3/Vital.vst3")));
    }

    #[cfg(target_os = "macos")]
    {
        paths.push(home_dir().map(|h| h.join("Library/Audio/Plug-Ins/VST3/Vital.vst3")));
        paths.push(Some(PathBuf::from(
            "/Library/Audio/Plug-Ins/VST3/Vital.vst3",
        )));
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(pf) = std::env::var("PROGRAMFILES") {
            paths.push(Some(
                PathBuf::from(pf)
                    .join("Common Files")
                    .join("VST3")
                    .join("Vital.vst3"),
            ));
        }
        if let Ok(pf) = std::env::var("PROGRAMFILES(X86)") {
            paths.push(Some(
                PathBuf::from(pf)
                    .join("Common Files")
                    .join("VST3")
                    .join("Vital.vst3"),
            ));
        }
    }

    paths
}

/// Check if Vital is installed at any common path.
pub fn is_vital_installed() -> bool {
    vital_search_paths()
        .into_iter()
        .flatten()
        .any(|p| p.exists())
}

/// Find the Vital bundle path, if installed.
pub fn find_vital_path() -> Option<PathBuf> {
    vital_search_paths()
        .into_iter()
        .flatten()
        .find(|p| p.exists())
}

/// Assert that a WAV file exists and contains non-silent audio.
pub fn assert_wav_has_audio(path: &Path) {
    assert!(path.exists(), "WAV file should exist: {}", path.display());
    let audio = plugin_render_bridge::wav::read_wav(path)
        .unwrap_or_else(|e| panic!("Failed to read WAV at {}: {e}", path.display()));
    assert!(
        audio.num_samples() > 0,
        "WAV should have samples: {}",
        path.display()
    );
    assert!(
        !audio.is_silent(),
        "WAV should not be silent: {}",
        path.display()
    );
}

/// Assert that a WAV file exists and has the expected sample rate.
pub fn assert_wav_valid(path: &Path, expected_sample_rate: u32) {
    assert!(path.exists(), "WAV file should exist: {}", path.display());
    let audio = plugin_render_bridge::wav::read_wav(path)
        .unwrap_or_else(|e| panic!("Failed to read WAV at {}: {e}", path.display()));
    assert_eq!(
        audio.sample_rate(),
        expected_sample_rate,
        "Sample rate mismatch for {}",
        path.display()
    );
    assert!(
        audio.num_channels() >= 1,
        "WAV should have at least 1 channel: {}",
        path.display()
    );
}

/// Calculate the RMS level of an audio buffer.
pub fn rms_level(buffer: &AudioBuffer) -> f64 {
    buffer.rms()
}

/// Resolve an explicitly requested fixture; required fixtures never skip.
pub fn fixture_plugin_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("VST3_TEST_PLUGIN") {
        let path = PathBuf::from(path);
        assert!(
            path.exists(),
            "VST3_TEST_PLUGIN does not exist: {}",
            path.display()
        );
        return Some(path);
    }
    assert!(
        std::env::var("REQUIRE_TEST_PLUGIN").as_deref() != Ok("1"),
        "REQUIRE_TEST_PLUGIN=1 requires VST3_TEST_PLUGIN"
    );
    eprintln!("SKIPPED: VST3_TEST_PLUGIN not set");
    None
}
