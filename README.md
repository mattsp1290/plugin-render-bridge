# plugin-render-bridge

<img src="assets/compatible-logo.png" width="120" alt="VST Compatible logo"> Offline rendering for VST®3 instruments, with native hosting supplied by [plugin-hostkit](https://github.com/mattsp1290/plugin-hostkit).

MIT licensed. VST is a registered trademark of Steinberg Media Technologies GmbH.
This project is not affiliated with or endorsed by Steinberg.

Extracted from the render and bridge modules of multisamples at revision
`a2207788282cf943d9d12cfeab63b4897af808b2`, without application dependencies or history.

```text
Host application
  BridgeRenderer ── JSON lines over stdin/stdout ── plugin-renderer child
       │                                             │
       └── reads rendered WAV                 plugin-hostkit / instrument
                                                   └── writes WAV
```

The host remains alive when a native plugin crashes. It observes
`BridgeError::ProcessCrashed`, calls `respawn()`, reapplies preset state if needed,
and can render again. In-process rendering remains available for processors
implementing `AudioProcessor`; native failures there terminate the application.

## Ship the child

Cargo does not install binary targets belonging to a library dependency. Add a
binary target to your application with this body:

```rust,no_run
fn main() { plugin_render_bridge::renderer_child_main(); }
```

Name that target `plugin-renderer` and ship it beside your application, or pass
its path explicitly to `BridgeRenderer::spawn`. `examples/custom_renderer.rs`
shows the same pattern. The crate also builds its own `plugin-renderer` binary.
`find_renderer_bin()` checks beside the current executable and its parent (for
test executables under `deps/`); use an explicit path for other layouts.

```rust,no_run
use plugin_render_bridge::{bridge::BridgeRenderer, RenderSettings};
use std::path::Path;

let settings = RenderSettings::default();
let mut renderer = BridgeRenderer::spawn(
    Path::new("./plugin-renderer"), Path::new("/path/to/plugin-binary"),
    settings.sample_rate, settings.buffer_size, settings.tempo_bpm,
)?;
renderer.render_to_file(60, 127, Path::new("note.wav"), &settings)?;
# Ok::<(), plugin_render_bridge::bridge::BridgeError>(())
```

A bundle can be resolved to its native binary through the re-exported
`plugin_hostkit::Vst3Bundle`. The hostkit dependency is pinned to `v0.1.0`.

## Child protocol

Arguments: `<plugin-binary> <sample-rate-hz> <buffer-size> [--tempo <bpm>]`.
Startup emits `{"status":"ready","name":"..."}` or an error response followed
by nonzero exit. Startup is bounded to five seconds by `BridgeRenderer`.
State transactions have a 30-second deadline. Render transactions add the
requested note duration and tail budget to that deadline. The same deadline
covers pipe writing and response receipt. A timeout kills and reaps the child,
invalidates its transport and returns `BridgeError::CommandTimeout`; call
`respawn()` before retrying. Shutdown never blocks the caller on a pipe write.

Each command and response occupies one JSON line:

```json
{"cmd":"set_state","state_base64":"..."}
{"cmd":"render","note":60,"velocity":127,"wav_path":"note.wav","duration_secs":2.0,"silence_threshold":0.00001,"tail_timeout_secs":10.0}
{"cmd":"quit"}
```

Responses are `{"ok":true,"samples":12345}` for a rendered note,
`{"ok":true}` for state application, or `{"ok":false,"error":"..."}`.
The child uses the original protocol: sample rate, block size and tempo are set
at spawn, and WAV output uses Int24. `RenderSettings::bit_depth` applies to the
in-process WAV writer; it is not sent by this protocol. Keep render settings'
sample rate and block size equal to the spawn values. `respawn()` preserves the
spawn settings but does not preserve plugin state. Use trusted child executables
and plugins; the child has the same filesystem access as the application.

## Build and verification

Rust 1.93 or newer. Linux requires `libx11-dev` for the hosting dependency.
Synthetic IPC regression tests require Python 3 on Unix.

```sh
cargo build --locked --all-targets --features testing
cargo test --locked --features testing
cargo clippy --locked --all-targets --features testing -- -D warnings
cargo fmt --all -- --check
RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps --features testing
```

Feature `testing` exposes deterministic mock processors, five audio assertions,
four synthesized buffer generators and `fast_render_settings()`. Normal builds
have no testing support API. No plugin or rendered commercial audio is bundled.
Installed Vital tests print `SKIPPED:` if absent. The required SDK integration
workflow builds the pinned MIT SDK instrument, sets `REQUIRE_TEST_PLUGIN=1`,
and checks that `fixture_bridge_tests` executes without skipping. To use a local
fixture:

```sh
VST3_TEST_PLUGIN=/path/to/instrument.vst3 REQUIRE_TEST_PLUGIN=1 \
  cargo test --locked --features testing --test fixture_bridge_tests -- --nocapture
```

The fixture test renders audible audio, kills the child, checks the crash error,
respawns it and renders audible audio again. CI covers macOS and Linux;
Windows has not been validated. See [publication audit](docs/publication-audit.md)
and [third-party notices](THIRD-PARTY-NOTICES.md).
