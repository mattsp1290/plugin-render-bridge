use plugin_render_bridge::bridge_protocol::{Command, ReadySignal, Response};

#[test]
fn command_set_state_serializes_with_tag() {
    let cmd = Command::SetState {
        state_base64: "AQID".into(),
    };
    let json = serde_json::to_value(&cmd).unwrap();
    assert_eq!(json["cmd"], "set_state");
    assert_eq!(json["state_base64"], "AQID");
}

#[test]
fn command_render_serializes_with_tag() {
    let cmd = Command::Render {
        note: 60,
        velocity: 127,
        wav_path: "/tmp/out.wav".into(),
        duration_secs: 2.0,
        silence_threshold: 1e-5,
        tail_timeout_secs: 10.0,
    };
    let json = serde_json::to_value(&cmd).unwrap();
    assert_eq!(json["cmd"], "render");
    assert_eq!(json["note"], 60);
    assert_eq!(json["velocity"], 127);
    assert_eq!(json["wav_path"], "/tmp/out.wav");
}

#[test]
fn command_quit_serializes() {
    let cmd = Command::Quit;
    let json = serde_json::to_value(&cmd).unwrap();
    assert_eq!(json["cmd"], "quit");
    // Quit has no other fields
    assert_eq!(json.as_object().unwrap().len(), 1);
}

#[test]
fn response_ok_serializes() {
    let resp = Response::ok();
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["ok"], true);
    assert!(json.get("error").is_none());
    assert!(json.get("samples").is_none());
}

#[test]
fn response_error_serializes() {
    let resp = Response::error("something broke");
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"], "something broke");
}

#[test]
fn response_ok_with_samples_serializes() {
    let resp = Response::ok_with_samples(48000);
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["samples"], 48000);
    assert!(json.get("error").is_none());
}

#[test]
fn ready_signal_serializes() {
    let sig = ReadySignal::new("Vital");
    let json = serde_json::to_value(&sig).unwrap();
    assert_eq!(json["status"], "ready");
    assert_eq!(json["name"], "Vital");
}

#[test]
fn command_set_state_roundtrip() {
    let cmd = Command::SetState {
        state_base64: "dGVzdA==".into(),
    };
    let json_str = serde_json::to_string(&cmd).unwrap();
    let parsed: Command = serde_json::from_str(&json_str).unwrap();
    match parsed {
        Command::SetState { state_base64 } => assert_eq!(state_base64, "dGVzdA=="),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn command_render_roundtrip() {
    let cmd = Command::Render {
        note: 72,
        velocity: 64,
        wav_path: "/out/test.wav".into(),
        duration_secs: 1.5,
        silence_threshold: 0.001,
        tail_timeout_secs: 5.0,
    };
    let json_str = serde_json::to_string(&cmd).unwrap();
    let parsed: Command = serde_json::from_str(&json_str).unwrap();
    match parsed {
        Command::Render {
            note,
            velocity,
            wav_path,
            ..
        } => {
            assert_eq!(note, 72);
            assert_eq!(velocity, 64);
            assert_eq!(wav_path, "/out/test.wav");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn command_quit_roundtrip() {
    let cmd = Command::Quit;
    let json_str = serde_json::to_string(&cmd).unwrap();
    let parsed: Command = serde_json::from_str(&json_str).unwrap();
    assert!(matches!(parsed, Command::Quit));
}

#[test]
fn response_roundtrip() {
    let resp = Response::ok_with_samples(44100);
    let json_str = serde_json::to_string(&resp).unwrap();
    let parsed: Response = serde_json::from_str(&json_str).unwrap();
    assert!(parsed.ok);
    assert_eq!(parsed.samples, Some(44100));
    assert!(parsed.error.is_none());
}
