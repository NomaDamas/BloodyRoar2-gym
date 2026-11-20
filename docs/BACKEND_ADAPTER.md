# Backend Adapter Contract

`bloodyroar2-gym` separates agent APIs from emulator implementation details.

Backends implement:

```rust
pub trait Backend {
    fn set_observation_screenshot(&mut self, _enabled: bool) {}
    fn reset(&mut self) -> Result<Observation, BackendError>;
    fn step(&mut self, buttons: ActionButtons, frames: u32) -> Result<Observation, BackendError>;
}
```

## Required emulator capabilities

- Load a legally supplied game image or arcade ROM set from a local path.
- Reset to a deterministic state.
- Advance exactly `frames` frames after applying controller input.
- Return health, meter, timer, terminal state, and optionally a screenshot.
- Avoid writing generated captures or save states into Git-tracked paths.

## Portable host notes

The Rust backend is intended to build and run on macOS, Linux, and Windows.
Backend work should keep deterministic stepping and the headless HTTP/MCP
surfaces independent of display and audio-device APIs. macOS CoreAudio is an
optional speaker sink; generated PCM and diagnostics remain available on every
host. The native play window is a separate desktop convenience path.

The supplied ZiNc bundle is Windows binary content, not Rust source. A portable
path is therefore an adapter around a compatible emulator core or a separate
process that exposes deterministic frame stepping and input injection.

The `NullBackend` is intentionally deterministic so CI, API clients, and agent
code can be developed without ROMs or proprietary binaries.

## MAME runtime

The repository provides an optional MAME runtime helper:

```sh
cargo run -- prepare-assets <local-rom-archive.zip> assets/roms
cargo run -- mame-check assets/roms
cargo run -- play assets/roms
```

This launches the host's MAME emulator for human play. It intentionally stores
ROM zip files under ignored local paths and never commits them.

The Rust-native backend is the RL-grade path: it maps actions directly into the
emulated controller and advances `step(action, frames)` by emulated vblanks.
MAME remains an optional external compatibility reference.
