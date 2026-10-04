use plugin_render_bridge::bridge::{BridgeError, BridgeRenderer};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn rejects_missing_plugin(binary: &Path) {
    let start = Instant::now();
    let missing = tempfile::tempdir().unwrap();
    let result = BridgeRenderer::spawn(
        binary,
        &missing.path().join("missing-plugin"),
        44100,
        512,
        None,
    );
    assert!(matches!(
        result,
        Err(BridgeError::RendererError(_)) | Err(BridgeError::ProcessCrashed(_))
    ));
    assert!(start.elapsed() < Duration::from_secs(10));
}

#[test]
fn packaged_child_rejects_missing_plugin() {
    rejects_missing_plugin(Path::new(env!("CARGO_BIN_EXE_plugin-renderer")));
}

#[test]
fn downstream_child_rejects_missing_plugin() {
    let root = PathBuf::from(env!("CARGO_BIN_EXE_plugin-renderer"));
    let profile_dir = root.parent().unwrap();
    let mut build = std::process::Command::new(env!("CARGO"));
    build
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "build",
            "--locked",
            "--offline",
            "--example",
            "custom_renderer",
        ])
        .arg("--target-dir")
        .arg(profile_dir.parent().unwrap());
    if profile_dir.file_name().unwrap() == "release" {
        build.arg("--release");
    }
    assert!(
        build.status().unwrap().success(),
        "custom renderer example build failed"
    );
    let binary = root
        .parent()
        .unwrap()
        .join("examples")
        .join(format!("custom_renderer{}", std::env::consts::EXE_SUFFIX));
    assert!(
        binary.exists(),
        "build the example with cargo build --all-targets"
    );
    rejects_missing_plugin(&binary);
}

#[cfg(unix)]
#[test]
fn silent_child_startup_is_bounded() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("silent-child");
    std::fs::write(&script, "#!/bin/sh\nexec sleep 30\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let start = Instant::now();
    let result = BridgeRenderer::spawn(&script, Path::new("unused"), 44100, 512, None);
    assert!(matches!(result, Err(BridgeError::ReadyTimeout)));
    assert!(start.elapsed() < Duration::from_secs(10));
}
