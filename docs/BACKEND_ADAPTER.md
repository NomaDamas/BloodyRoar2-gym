# Backend Adapter Contract

`bloodyroar2-gym` separates agent APIs from emulator implementation details.

Backends implement:

```rust
pub trait Backend {
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

## macOS porting notes

The supplied ZiNc bundle is Windows binary content, not Rust source. A practical
macOS path is therefore an adapter around a macOS-compatible emulator core or a
separate process that exposes deterministic frame stepping and input injection.

The `NullBackend` is intentionally deterministic so CI, API clients, and agent
code can be developed without ROMs or proprietary binaries.

## MAME runtime

The repository provides a MAME runtime helper:

```sh
cargo run -- prepare-assets "BloodRoar2 (2).zip" assets/roms
cargo run -- mame-check assets/roms
cargo run -- play assets/roms
```

This launches the macOS emulator for human play. It intentionally stores ROM zip
files under ignored local paths and never commits them.

For RL-grade control, the next backend should connect MAME's debugger/Lua/input
surfaces to the `Backend` trait so `step(action, frames)` performs deterministic
frame advance instead of only launching the emulator window.
