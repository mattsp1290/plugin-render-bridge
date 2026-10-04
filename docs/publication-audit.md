# Publication audit — plugin-render-bridge

<img src="../assets/compatible-logo.png" width="120" alt="VST Compatible logo">

VST is a registered trademark of Steinberg Media Technologies GmbH.
This project is not affiliated with or endorsed by Steinberg.

Plan: `plugin-render-bridge-plan-kf32`; work packages R1–R6.
Source: multisamples `a2207788282cf943d9d12cfeab63b4897af808b2`.
Source drift check `git log a220778..HEAD -- crates/ms-audio crates/ms-testing`
returned no commits. This is a fresh allowlisted import with no source history.
Native hosting dependency: plugin-hostkit tag `v0.1.0`, peeled revision
`27e5808a04eced5e49cb1acdaa3ef1a29a8daf32`.

## L3 — Import allowlist and proprietary content

Imported source files, individually selected from `crates/ms-audio/src`:
`buffer.rs`, `pool.rs`, `midi.rs`, `wav.rs`, `render.rs`, `effect_chain.rs`,
`vst_adapter.rs`, `bridge_protocol.rs`, `bridge.rs`, the sibling binary finder
from `lib.rs`, and the body of `bin/vst3_renderer.rs` (now `child.rs`).
The application orchestrator, hardware modules and sampling-plan code were excluded.

Testing support imports only the mock processors, five assertion functions,
four synthesized buffer generators, and fast render settings from
`crates/ms-testing/src/{mock_vst,assertions,fixtures}.rs`. They are organized into
private modules below `src/testing.rs`, gated by the `testing` feature.
Test-only `common/mod.rs` imports the five specified Vital helpers and their
path-search implementation, plus the newly written mandatory fixture resolver.
No product preset helper modules were imported into the library.

Imported tests from `crates/ms-audio/tests`: buffer, WAV roundtrip, render,
bridge protocol, bridge integration, and the MIDI cases for events/sequences.
The four batch SamplingPlan cases remain excluded. The Vital rendering target
from `crates/ms-vst3/tests` was adapted to the descriptor-returning scanner API;
its product preset scanner case was removed. The target now runs sequentially
on the main thread to obey the hosting library's native lifecycle contract.
The benchmark target imports `crates/ms-audio/benches/audio_benchmarks.rs`
with type/import substitutions only; it builds with `testing` enabled.
New tests cover nonexistent-plugin startup for both renderer binaries,
a silent child startup deadline, and actual fixture render/kill/respawn.

No plugin binary, preset, extracted resource, commercial audio, product scanner,
source application directory, investigation script or private documentation
was copied. Runtime fixtures are generated outside this repository.

Product-name search: `rg -ni 'serum|omnisphere|kontakt|vital|chipsynth|aria|spire|addictive|manis' src tests`.
There are zero product names in library source. Matches in tests are limited to
Vital fixture names, its standard installation paths, skip messages, test labels
and comments explaining default-state interoperability expectations. The literal
Vital in the protocol serialization test is a synthetic name string.
The unanchored expression also matches the word “variant” in protocol assertions;
these are false positives. No disassembly, internal plugin class names, resource
extraction details or private crash narratives remain.

Committed path-history check:
`git log --all --name-only --format=` must have no path matching
`serum2_uidesc|\.fxp$|\.nki$|\.vital$|\.vstpreset$|\.vst3/`.
The complete committed implementation history passed this prohibited-path check
before review. Repeat it after each review correction before publication.

## L4 — Trademark documentation

The unchanged official logo was obtained from the already verified hosting
repository asset and checked against
[Steinberg's immutable official asset](https://github.com/steinbergmedia/vst3_doc/blob/6d4737c9e70750056e731d88d49aa06eefc8a1a4/artwork/VST_Compatible_Logo_Steinberg_with_TM.png).
SHA-256: `fdf4f96b7a8bc1f53f0d2c167a27ac5fac00f29ff4a7bc3eaad851ad4be10f35`.
Its terms are the
[official usage guidelines](https://steinbergmedia.github.io/vst3_dev_portal/pages/VST%2B3%2BLicensing/Usage%2Bguidelines.html),
read again on 2026-10-03. The project's MIT license does not relicense the logo.
README places the logo beside the first format mention, with the registration
symbol. This page and third-party notices show it too. README/notices contain
trademark attribution and nonendorsement. Repository, crate and binary names do
not use the trademark; Rust API names are descriptive format names. No prohibited
stylized term appears outside audit descriptions. Hosting definitions and the SDK
fixture are covered by the upstream hostkit's release audit; this crate vendors
neither SDK code nor bindings.

## L5 — Package, dependencies and secrets

`cargo package --list --allow-dirty`: only Cargo metadata, manifest/lockfile,
license, README/notices, official logo, source modules, reviewed tests, downstream
renderer example, benchmark and audit. The manifest's explicit include list
excludes review artifacts, targets, workflow files, SDK material and runtime
plugin/audio data. Package listing is inspection; crates.io publication is outside
scope. The dependency is the exact git/tag specification required by decision K7.

`cargo tree -e normal`: no ms-*, lotel-*, Tauri or Specta package.
`cargo tree -d`: exactly one plugin-hostkit source/version in the graph.
`cargo license --json`: 97 packages across normal/dev/target lockfile dependencies.
Every expression has a permissive MIT, Apache-2.0, ISC or Unlicense choice.
Unicode-3.0 also applies to unicode-ident. The r-efi LGPL alternative is not
selected; MIT is available. No copyleft-only dependency appears.

Searches over source/tests/examples/benchmarks/workflows for personal paths,
application identifiers, service names, API keys, tokens, secrets and passwords
found no values or app coupling. Provenance references to multisamples in the
README, this audit and commit messages are intentional. No personal local path
is embedded. `ms_core|ms_vst3|ms_audio` has no source/test hits.

## Local verification and execution evidence

macOS arm64: build all targets with testing support, complete tests, strict
Clippy, formatting and rustdoc with warnings denied pass. Seventy-seven non-plugin
cases execute, plus six installed Vital bridge cases, six installed Vital
in-process rendering cases, and the separate required fixture case. Vital state
is the default instrument state; no product preset scanner or binary fixture is
published. The fixture test renders a WAV, verifies samples/audio and 44100 Hz,
kills the native child, observes ProcessCrashed, respawns and renders audio again.
The downstream renderer example and packaged binary reject a missing plugin in
under ten seconds; a silent child also reaches its actual five-second deadline.

Linux x86_64, Rust 1.93.1 in a local container: before the standard review fixes,
build all targets, complete tests and strict Clippy passed. An updated Linux
run is still live; it is not claimed as passing after the review corrections. Installed Vital cases explicitly skip because it is not
installed. The separate required SDK fixture executes exactly one passed test,
zero failures/ignored cases and no SKIPPED lines. Its instrument comes from the
MIT SDK root `9fad9770f2ae8542ab1a548a68c1ad1ac690abe0` and recursive submodules,
built as `note-expression-synth` with hosting examples, plugin links and validator
disabled, following hostkit's successful SDK fixture procedure.

Commands used:

```sh
cargo build --locked --all-targets --features testing
cargo test --locked --features testing
cargo clippy --locked --all-targets --features testing -- -D warnings
cargo fmt --all -- --check
RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps --features testing
REQUIRE_TEST_PLUGIN=1 VST3_TEST_PLUGIN=<fixture-bundle> \
  cargo test --locked --features testing --test fixture_bridge_tests -- --nocapture
cargo package --list --allow-dirty
cargo tree -e normal
cargo tree -d
cargo license --json
git diff --check
```

CI workflows reproduce macOS/Linux gates and the pinned Linux SDK fixture,
including executed-count/no-skip checks. Remote CI, review-gauntlet checkpoints,
user publication approval and release tag are pending. Local container evidence
is not a remote CI pass. R6 requires a green exact-revision CI/integration run
before the annotated v0.1.0 release. Windows is not validated.

## Approval and release state

This repository's imported code has not been pushed. The plan explicitly
requires the owner to approve this repository's completed audit before the first
public push. Approval has not yet been received. No release tag exists here.

## Standard review corrections

The two independent standard reviewers returned REQUEST_CHANGES on
`eec849f2eb2d45d22b711e50ebfac2f769121da4`. The canonical automatic fixer applied
three Important fixes and one Suggestion: explicit startup deadline checking even
under continuous non-ready output; supervised writes/responses/shutdown with a
single transaction deadline and invalidated transport after timeout; required
latency drain before tail silence detection; and sample-accurate note-off inside
a block. The mock processor now honors those event offsets as well.

New private `src/bridge/transport.rs` owns pipe workers and bounded channels.
New synthetic tests cover noisy startup, stalled responses, a 16 MiB state write
to an unread pipe, bounded Drop and healthy respawn after timeout. Three delay-line
cases verify audible short notes with latency longer than sustain/declared tail,
exact 441-sample sustain with 512-sample blocks, and defined zero-duration silence.
No additional source repository file, preset or plugin resource was imported.

Linux's first updated run exposed an end-to-end timing gap: state encoding and
JSON serialization happened before the command clock began. The deadline now
starts before encoding and covers both serialization and pipe I/O. The corrected
macOS stalled-write/response regressions, required Vital fixture, strict Clippy,
Rust 1.93 build and warning-free documentation pass. A reduced-concurrency Linux
retry is live; its result remains unverified until its process completes.

The empty remote also needs its initial main foundation. Local main contains
only the original scaffold plus a scaffold-specific L5 audit; implementation
commits remain on the feature branch. Both publication targets are prepared for
review, with no remote source write or release tag yet.
