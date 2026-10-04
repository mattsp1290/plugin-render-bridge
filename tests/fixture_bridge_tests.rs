mod common;
use plugin_render_bridge::{
    RenderSettings,
    bridge::{BridgeError, BridgeRenderer},
};
use std::path::Path;
use std::time::{Duration, Instant};

#[test]
fn fixture_render_crash_and_respawn() {
    let Some(path) = common::fixture_plugin_path() else {
        return;
    };
    let bundle = plugin_hostkit::Vst3Bundle::from_path(&path).expect("invalid fixture bundle");
    let binary = bundle.binary_path.expect("fixture binary missing");
    let renderer_bin = Path::new(env!("CARGO_BIN_EXE_plugin-renderer"));
    let mut renderer =
        BridgeRenderer::spawn(renderer_bin, &binary, 44100, 512, Some(120.0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let settings = RenderSettings {
        note_duration_secs: 0.1,
        tail_timeout: Duration::from_secs(3),
        ..Default::default()
    };
    let before = dir.path().join("before.wav");
    assert!(
        renderer
            .render_to_file(60, 127, &before, &settings)
            .unwrap()
            > 0
    );
    common::assert_wav_has_audio(&before);
    common::assert_wav_valid(&before, 44100);
    #[cfg(unix)]
    unsafe {
        assert_eq!(libc::kill(renderer.child_id() as i32, libc::SIGKILL), 0);
    }
    #[cfg(not(unix))]
    panic!("crash fixture requires a supported Unix platform");
    let deadline = Instant::now() + Duration::from_secs(5);
    while renderer.is_alive() {
        assert!(Instant::now() < deadline, "killed child did not exit");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(matches!(
        renderer.render_to_file(60, 127, &before, &settings),
        Err(BridgeError::ProcessCrashed(_))
    ));
    renderer.respawn().unwrap();
    let after = dir.path().join("after.wav");
    assert!(renderer.render_to_file(60, 127, &after, &settings).unwrap() > 0);
    common::assert_wav_has_audio(&after);
    assert!(renderer.is_alive());
}
