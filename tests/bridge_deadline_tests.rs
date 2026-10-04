#![cfg(unix)]
use plugin_render_bridge::bridge::{BridgeError, BridgeRenderer};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

fn script(dir: &Path, body: &str) -> std::path::PathBuf {
    let path = dir.join("fixture-child");
    std::fs::write(&path, format!("#!/usr/bin/env python3\n{body}")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}

#[test]
fn noisy_startup_cannot_accept_late_ready() {
    let dir = tempfile::tempdir().unwrap();
    let child = script(
        dir.path(),
        r#"import sys, time
end = time.monotonic() + 7
line = '{"noise":"' + 'x' * 60000 + '"}\n'
while time.monotonic() < end:
    sys.stdout.write(line)
    sys.stdout.flush()
print('{"status":"ready","name":"too-late"}', flush=True)
time.sleep(30)
"#,
    );
    let start = Instant::now();
    let result = BridgeRenderer::spawn(&child, Path::new("unused"), 44100, 512, None);
    assert!(matches!(result, Err(BridgeError::ReadyTimeout)));
    assert!(start.elapsed() < Duration::from_secs(6));
}

fn stalled_command(read_command: bool) {
    let dir = tempfile::tempdir().unwrap();
    let script_body = format!(
        r#"import sys, time, pathlib
marker = pathlib.Path(sys.argv[1])
first = not marker.exists()
marker.touch()
print('{{"status":"ready","name":"fixture"}}', flush=True)
if first:
    {read}
    time.sleep(90)
for line in sys.stdin:
    print('{{"ok":true}}', flush=True)
"#,
        read = if read_command {
            "sys.stdin.readline()"
        } else {
            "pass"
        }
    );
    let child = script(dir.path(), &script_body);
    let mut bridge =
        BridgeRenderer::spawn(&child, &dir.path().join("spawn-marker"), 44100, 512, None).unwrap();
    let data = vec![1; if read_command { 8 } else { 16 * 1024 * 1024 }];
    let start = Instant::now();
    assert!(matches!(
        bridge.bridge_set_state(&data),
        Err(BridgeError::CommandTimeout)
    ));
    assert!(start.elapsed() < Duration::from_secs(35));
    assert!(!bridge.is_alive());
    bridge.respawn().unwrap();
    bridge.bridge_set_state(&[1, 2, 3]).unwrap();
    let start = Instant::now();
    drop(bridge);
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn response_stall_is_bounded_and_respawnable() {
    stalled_command(true);
}

#[test]
fn blocked_large_state_write_is_bounded_and_respawnable() {
    stalled_command(false);
}

#[test]
fn stalled_child_drop_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let child = script(
        dir.path(),
        "import time\nprint('{\"status\":\"ready\",\"name\":\"fixture\"}', flush=True)\ntime.sleep(90)\n",
    );
    let bridge = BridgeRenderer::spawn(&child, Path::new("unused"), 44100, 512, None).unwrap();
    let start = Instant::now();
    drop(bridge);
    assert!(start.elapsed() < Duration::from_secs(2));
}
