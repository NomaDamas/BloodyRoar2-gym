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

## BloodyRoar2Recomp process adapter

The upstream [BloodyRoar2Recomp](https://github.com/novapowers0/BloodyRoar2Recomp)
release is a separate, caller-built process. It contains generated game code,
but it still needs the caller's own PSX disc image at launch. The Rust adapter
does not pretend that a PlayStation image is an arcade ROM set and does not
copy either input into this repository.

Build the upstream project in its own checkout with
[`scripts/recomp-macos.sh`](../scripts/recomp-macos.sh):

```sh
scripts/recomp-macos.sh \
  --project-root /Users/me/src/BloodyRoar2Recomp \
  --disc "/Users/me/discs/Bloody Roar 2 - Bringer of the New Age (Europe).cue" \
  --build-dir /Users/me/src/BloodyRoar2Recomp/build-release
```

The script runs the upstream submodule, emitter, `verify-disc`, `generate`, and
`rebuild` stages, forcing `PSX_DEBUG_TOOLS=ON` for the MCP debug server.
`--dry-run` prints the exact command sequence without needing
the disc. Generated C, the executable, and the disc remain in the caller-owned
checkout. The same command is suitable on Linux; the name documents the
macOS-first path in this project.

The isolated Japan target adds `-DBR2_BUILD_JAPAN=ON` automatically. The
universal target needs every regional generated set, so supply all three legal
images and let the helper run each generation step before linking:

```sh
scripts/recomp-macos.sh --target psx-runtime-universal \
  --project-root /Users/me/src/BloodyRoar2Recomp \
  --disc /Users/me/discs/europe.cue \
  --disc-us /Users/me/discs/usa.cue \
  --disc-japan /Users/me/discs/japan.cue \
  --build-dir /Users/me/src/BloodyRoar2Recomp/build-universal
```

Drive the resulting executable through MCP:

```sh
cargo run --release -- recomp-mcp \
  /Users/me/src/BloodyRoar2Recomp/build-release/BloodyRoar2_Recompiled \
  "/Users/me/discs/Bloody Roar 2 - Bringer of the New Age (Europe).cue" 4370
```

`RecompBackend` starts the executable with `--headless --no-launcher
--disc ... --debug-port ...`, waits for the local JSON debug server, maps P1
Gym controls to the active-low PS1 pad word, polls `frame` for bounded progress,
and captures PNGs through `screenshot` when requested. The upstream protocol
currently exposes only pad1 through `set_input`; P2 requests return a clear
error instead of being silently sent to the wrong controller. Arcade coin and
service inputs are also outside the PS1 pad word and are rejected. Recomp does not
publish health/meter RAM addresses through this stable protocol, so those
observation fields remain conservative defaults and the raw debug connection
details are included in `info`.

The adapter follows the upstream one-request-per-TCP-connection rule. It is
therefore compatible with the native `psx-runtime` debug server and with a
small fake server in tests, without requiring a ROM or a proprietary binary in
CI. The upstream process must be a debug-tools build; a release with its debug
server disabled produces an actionable startup timeout.

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
