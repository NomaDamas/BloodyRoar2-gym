# bloodyroar2-gym

Rust control harness for building Gymnasium-style RL and LLM agents around a
legally supplied Bloody Roar 2-compatible emulator backend.

This repository does not contain ROMs, BIOS files, Windows executables, DLLs, or
other proprietary game assets. Keep those outside Git and pass their paths to a
future backend adapter only if you have the legal right to use them.

## Supported platform

The primary supported development and execution platform is macOS on Apple
Silicon. Build, CLI, HTTP API, native emulator, and local MAME workflows are
validated first on arm64 macOS.

Other Unix-like hosts may work for asset-free Rust development, but they are
secondary and should not be assumed to have the same runtime coverage. Windows
ZiNc compatibility remains a legacy fallback path and is not the default target
for emulator-core work.

## macOS Apple Silicon setup

Required baseline for development on Apple Silicon:

```sh
xcode-select --install
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
brew install rustup-init mame
rustup-init
rustup default stable
rustup target add aarch64-apple-darwin
rustup component add rustfmt clippy
cargo test
```

Notes:

- The repository uses Rust edition 2024, so keep the stable toolchain current.
- `aarch64-apple-darwin` is the primary target; Rosetta/x86_64 builds are not
  the default validation path.
- Xcode Command Line Tools provide the macOS SDK and linker used by Cargo.
- Homebrew MAME is required for the local macOS play path and ROM-set checks.
- Python is optional and only needed for scripts that wrap the HTTP API in
  `examples/python`; the client itself uses the Python standard library.
- Wine is optional and only relevant to the legacy ZiNc compatibility path, not
  native emulator development.

Keep ROMs, BIOS files, ZiNc bundles, MAME samples, captures, save states, and
other proprietary runtime assets in ignored local directories such as `assets/`,
`roms/`, or `bios/`. The setup process must not download or commit proprietary
game data.

## Current status

- macOS-safe Rust project scaffold.
- Discrete action space for fighter controls.
- Observation contract compatible with RL loops.
- CLI and HTTP JSON API using a deterministic `NullBackend`.
- Apple Silicon-native Rust play window with GPU framebuffer presentation.
- Native scripted input validation for Gym/RL action wiring.
- macOS MAME launch path for local compatibility checks with legally supplied
  ROM assets.
- Asset policy and static security notes for the supplied Windows bundle.

The original ZiNc Windows bundle is not republished and is not treated as
portable Rust source. Human play on macOS can use the Rust-native window first;
MAME remains useful as an external compatibility reference.

## CLI

```sh
cargo run -- action-space
cargo run -- observation-space
cargo run -- reset
cargo run -- step 5 4
cargo run -- serve 127.0.0.1:8765
```

## macOS play path

Run the Rust-native Apple Silicon play window with legally supplied local assets:

```sh
cargo run -- native-input-check assets/roms 500000
cargo run -- native-health-check assets/roms 500000
cargo run -- native-autoplay assets/roms 500000 2
cargo run -- native-play assets/roms 500000 2
```

`native-autoplay` opens the same macOS window, runs the built-in coin/start and
fighter-control script until a playable candidate is observed, then falls back
to keyboard control. Use `native-play` when you want fully manual input from
boot.
`native-health-check` is stricter than the autoplay smoke path: it verifies the
CPU core, mapped controls, rendered frame statistics, and per-action branch
stability. It exits non-zero when the native renderer still has a full-scene
composition gap even if the macOS window and input path are working.

Window controls:

- Arrows: move.
- `Z`: punch.
- `X`: kick.
- `A`: beast.
- `S`: guard.
- `C`: coin.
- `Enter`: start.
- `Esc`: quit.

For automated smoke tests, pass an optional frame limit:

```sh
cargo run -- native-autoplay assets/roms 500000 2 1140
cargo run -- native-play assets/roms 500000 2 700
```

Install MAME if you also want an external compatibility reference:

```sh
brew install mame
cargo run -- prepare-assets "BloodRoar2 (2).zip" assets/roms
cargo run -- rom-ident assets/roms
cargo run -- mame-required assets/roms
cargo run -- mame-check assets/roms
cargo run -- doctor assets/roms
cargo run -- play assets/roms
```

`play` expects `mame-check` to pass first. If MAME reports missing files or
incorrect checksums, supply a ROM set that matches the installed MAME version;
the project will not download, patch, or commit proprietary ROM data.

Configuration:

- `BLOODYROAR2_MAME`: override the MAME executable path.
- `BLOODYROAR2_ROM_DIR`: override the local ROM directory.
- `BLOODYROAR2_MAME_GAME`: override the MAME game id, default `bldyror2`.

On Apple Silicon, the Rust-native path is the default development target. The
original ZiNc Windows executable is not native Apple Silicon software; running it
would require a separate Wine/Rosetta-style compatibility layer and is not the
default or recommended path.

## ZiNc compatibility path

The downloaded bundle is structured for ZiNc on Windows. On Apple Silicon this
is not native, but it can be attempted through Rosetta plus Wine:

```sh
cargo run -- prepare-zinc "BloodRoar2 (2).zip" assets/extracted
cargo run -- zinc-check assets/extracted/BloodRoar2
cargo run -- zinc-play assets/extracted/BloodRoar2
```

Configuration:

- `BLOODYROAR2_WINE`: override the Wine executable path.
- `BLOODYROAR2_ZINC_DIR`: override the extracted ZiNc bundle directory.
- `BLOODYROAR2_ZINC_RENDERER`: override renderer, default `renderer-sft.znc`.
- `BLOODYROAR2_ZINC_RENDERER_CFG`: override renderer config, default
  `zenith-renderer70.cfg`.

If Wine is missing, install a Wine distribution manually. Homebrew
`wine-stable` may require `sudo` for its GStreamer dependency and may not be
installable from non-interactive automation.

## Native Emulator Development

This repository also contains a from-scratch Apple Silicon-native emulator path.
It provides a ROM ZIP/directory inspector, boot ROM loader, MIPS R3000A/GTE
execution foundation, memory bus, DMA, GPU framebuffer renderer, input mapping,
Gym-style native backend, and a minifb-powered local play window:

```sh
cargo run -- native-inspect assets/roms/bldyror2.zip
cargo run -- native-rom-summary assets/roms/bldyror2.zip
cargo run -- native-step assets/roms/bldyror2.zip 16
cargo run -- native-step assets/roms/bldyror2.zip 1000000
cargo run -- native-env-step assets/roms/bldyror2.zip 5 1 10000
cargo run -- native-scripted-step assets/roms/bldyror2.zip 100000 /tmp/br2-script.png coin:30 noop:30 start:30 coin+start:60 noop:120
cargo run -- native-input-check assets/roms 500000
cargo run -- native-health-check assets/roms 500000
cargo run -- native-play assets/roms 500000 2
cargo run -- serve-native 127.0.0.1:8765 assets/roms/bldyror2.zip 10000
```

The native path is intentionally separated from MAME and ZiNc compatibility
commands so emulator-core work can proceed without republishing proprietary
Windows binaries or ROM data.
The current native core runs on macOS, opens a local framebuffer window, reads
the mapped controls, uploads visible texture data to VRAM, tracks presentation
quality, and exposes CPU/IO/GPU state for iterative validation. Full native
scene composition is still guarded by `native-health-check`; keep using that
command before claiming emulator parity.
`native-env-step` and `serve-native` connect the native core to the same
Gym-style action/observation contract used by the null backend.
`native-scripted-step` applies a sequence of Gym actions to the native core and
writes a PNG observation for repeatable boot/input debugging. `native-input-check`
verifies that the game reads mapped coin/start/fighter controls,
`native-health-check` fails on remaining full-scene rendering or branch-stability
gaps, and `native-play` opens the native macOS framebuffer window.

See [docs/NATIVE_WORKFLOW.md](docs/NATIVE_WORKFLOW.md) for the canonical macOS
Apple Silicon build, test, native-step, Gym API, and asset-compliance validation
path.

## HTTP API

```sh
curl -sS http://127.0.0.1:8765/action_space
curl -sS -X POST http://127.0.0.1:8765/reset
curl -sS -X POST http://127.0.0.1:8765/step -d '{"action":5,"frames":4}'
```

Endpoints:

- `GET /`
- `GET /action_space`
- `GET /observation_space`
- `POST /reset`
- `POST /step`

## Gymnasium mapping

Action space:

- Type: `Discrete(19)`
- Values: see `cargo run -- action-space`

Observation space:

- `frame`: integer frame counter
- `player_health`: `0.0..1.0`
- `opponent_health`: `0.0..1.0`
- `beast_meter`: `0.0..1.0`
- `round_time`: `0.0..99.0`
- `terminal`: boolean
- `screenshot_b64`: optional PNG frame for vision agents

## Legal asset workflow

1. Keep all ROM, BIOS, save-state, and emulator binary files outside this repo.
2. Store local paths in an untracked `.env` or user config file.
3. Verify assets with `cargo run -- asset-check <path>` before connecting a real
   backend.
4. Do not push proprietary dumps, archives, or extracted Windows bundles.

## Next backend step

For deterministic RL, connect a MAME debugger/Lua/input bridge to the `Backend`
trait so `step(action, frames)` drives the running emulator. The current `play`
command launches the real macOS emulator for human play, while `serve` exposes
the stable Gym-style API contract.
