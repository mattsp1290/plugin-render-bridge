//! Audio buffer management, MIDI generation, and WAV writing.

use std::path::PathBuf;

pub mod bridge;
pub mod bridge_protocol;
/// Find a sibling binary (same directory as current exe, or parent directory
/// for test binaries in `target/debug/deps/`). Returns `None` if not found.
pub fn find_sibling_binary(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    // Same directory as current exe (production layout)
    let candidate = exe_dir.join(name);
    if candidate.exists() {
        return Some(candidate);
    }

    // Parent directory (test binaries in target/debug/deps/,
    // sibling binary in target/debug/)
    if let Some(parent) = exe_dir.parent() {
        let candidate = parent.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    #[cfg(target_os = "windows")]
    {
        let with_ext = exe_dir.join(format!("{name}.exe"));
        if with_ext.exists() {
            return Some(with_ext);
        }
        if let Some(parent) = exe_dir.parent() {
            let with_ext = parent.join(format!("{name}.exe"));
            if with_ext.exists() {
                return Some(with_ext);
            }
        }
    }

    None
}
pub mod buffer;
pub mod child;
pub mod config;
pub mod effect_chain;
pub mod midi;
pub mod pool;
pub mod render;
pub mod vst_adapter;
pub mod wav;
pub use child::renderer_child_main;
pub use config::{BitDepth, RenderSettings};
pub use plugin_hostkit;
#[cfg(feature = "testing")]
pub mod testing;
