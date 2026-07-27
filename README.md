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
ZiNc compatibility remains a default-denied legacy path and is not the default
target for emulator-core work.

## macOS Apple Silicon setup

Required baseline for development on Apple Silicon:

```sh
xcode-select --install
brew install rustup-init mame
rustup-init
rustup default stable
rustup target add aarch64-apple-darwin
rustup component add rustfmt clippy
cargo test
```

Notes:

- The repository uses Rust edition 2024, so keep the stable toolchain current.
- Install Homebrew from the official https://brew.sh instructions if `brew` is
  not already available; this README intentionally does not provide a
  pipe-to-shell installer shortcut.
- `aarch64-apple-darwin` is the primary target; Rosetta/x86_64 builds are not
  the default validation path.
- Xcode Command Line Tools provide the macOS SDK and linker used by Cargo.
- Homebrew MAME is optional and used only for external compatibility checks;
  the Rust-native macOS play path does not launch MAME.
- Python is optional and only needed for scripts that wrap the HTTP API in
  `examples/python`; the client itself uses the Python standard library.
- Wine is optional and only relevant to the unsafe, default-denied legacy ZiNc
  compatibility path, not native emulator development.

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
cargo run -- native-cache-prepare assets/local-romset.zip
cargo run -- native-play
cargo run -- native-input-check
cargo run -- native-health-check
cargo run -- native-autoplay
cargo run --release -- native-startup-probe assets/roms 120000 optimized
```

ZIP inputs are not extracted by hand on every run. Native runtime startup
materializes recognized ROM entries once under ignored
`.runtime-cache/native-rom-cache` using a source-size/mtime fingerprint.
`native-cache-prepare` reports `cache_hit` and `materialized`, records the latest
cache directory, and `native-cache-path` prints the resolved ROM directory. For
repeat play sessions, omit the ROM argument: native commands use the latest cache
directory first, then ignored local asset paths configured by the runtime. That
keeps repeat runs off the source ZIP unless you intentionally rebuild the cache.
Set `BLOODYROAR2_NATIVE_ROM_CACHE_DIR=/path/to/cache` to keep the runtime cache
outside the repo.

`native-play` and `native-manual` use the cached ROM directory and open a macOS
keyboard window. `native-play` fast-forwards the built-in
warning/title/coin/start/select/match-entry assist before opening the window, so
the visible session starts near the manual play handoff instead of at warning or
intro screens. Its pre-window timeout scales with the scripted frame budget;
set `BR2_NATIVE_PLAY_FAST_FORWARD_WALL_TIMEOUT_SECS` only when an unusually slow
machine needs an explicit override. Use `native-manual` for a cold-boot window
with no entry script.
`native-startup-probe` executes the exact pre-window path without opening a GUI.
Use `optimized` for the production path and `tracked` for the legacy
diagnostic-capture path; its JSON reports startup timing and the final
render/playability gates.
Use `native-autoplay` when you intentionally want a visible scripted assist or
custom entry-assist diagnostics.
`native-window-snapshot` writes the normalized native display frame without
opening a window. `native-play-snapshot` writes the exact 640x480
aspect-corrected presentation used by the GUI, which is useful when checking
whether the visible window is cropped, doubled, black, or horizontally
distorted. `native-autoplay` also accepts an explicit script tail for smoke and
control-sweep validation.
`native-health-check` is stricter than the autoplay smoke path: it verifies the
CPU core, mapped controls, rendered frame statistics, and per-action branch
stability. It exits non-zero when the native renderer still has a full-scene
composition gap even if the macOS window and input path are working.

Bloody Roar 2 uses the MAME `znt2p` input map by default. When a ZiNc bundle
contains `cfg/bldyror2.cfg`, the native loader also uses it as an EEPROM
fallback, but keeps the MAME input map. Set
`BLOODYROAR2_NATIVE_LEGACY_ZINC_INPUT=1` only when explicitly testing legacy
ZiNc input mirroring for diagnostics.

Window controls:

- Arrows or `WASD`: move.
- `Z`, `Space`, `J`, or `F`: confirm/punch.
- `X`, `K`, or `H`: kick.
- `Q`, `L`, or `B`: beast.
- `E`, `I`, or `G`: guard.
- `C`: coin.
- `V`: service credit.
- `Enter`: coin+start for one-key title entry. `P`: start only.
- `Esc`: quit.

For automated smoke tests, pass an optional frame limit:

```sh
cargo run -- native-play 120000 fit 1000
cargo run -- native-autoplay 120000 fit 1000
cargo run -- native-manual 120000 2 700
cargo run -- native-window-snapshot 40505333 tmp/native-validation/manual-window.png
cargo run -- native-play-snapshot assets/local-romset.zip 120000 tmp/native-validation/smoke --match-script --fast-forward-frames 2400
cargo run --release -- native-play assets/roms 120000 1 150 \
  --gui-test-input coin:6 start:6 up:6 down:6 left:6 right:6 \
  punch:6 kick:6 beast:6 guard:6 noop:25
```

In `native-play assets/local-romset.zip 120000 1`, the final `1` is the
window scale, not a frame limit. Use a fourth argument for bounded smoke runs,
for example `native-play assets/local-romset.zip 120000 1 600`.
`--gui-test-input` is an opt-in QA path that traverses the same GUI input latch
and guest registers while keeping its result separate from physical-keyboard
verification. The final JSON reports `native_play_test_input_verified`.

`native-play-snapshot` exits non-zero unless the final rendered window frame
meets the stricter gameplay-scene criteria. Warning screens, black letterboxed
transitions, and red/salmon single-palette corruption are not counted as
playable native rendering.

Install MAME if you also want an external compatibility reference:

```sh
brew install mame
cargo run -- prepare-assets <local-archive.zip> assets/roms
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

The local legacy bundle is structured for ZiNc on Windows. On Apple Silicon this
is not native, and `zinc-play` is disabled by default because it launches a local
Windows executable through Wine. Only use it in an isolated environment after
you explicitly accept that host risk:

```sh
cargo run -- prepare-zinc <local-zinc-bundle-archive.zip> assets/extracted
cargo run -- zinc-check assets/extracted/<zinc-bundle-dir>
BLOODYROAR2_ALLOW_UNSAFE_ZINC_WINE=1 cargo run -- zinc-play assets/extracted/<zinc-bundle-dir>
```

Configuration:

- `BLOODYROAR2_WINE`: override the Wine executable path.
- `BLOODYROAR2_ZINC_DIR`: override the extracted ZiNc bundle directory.
- `BLOODYROAR2_ZINC_RENDERER`: override renderer, default `renderer-sft.znc`.
- `BLOODYROAR2_ZINC_RENDERER_CFG`: override renderer config, default
  `zenith-renderer70.cfg`.
- `BLOODYROAR2_ALLOW_UNSAFE_ZINC_WINE=1`: required unsafe opt-in for
  `zinc-play`; without it the command fails before launching Wine or the Windows
  executable.

If Wine is missing, install a Wine distribution manually. Homebrew
`wine-stable` may require `sudo` for its GStreamer dependency and may not be
installable from non-interactive automation.

## Native Emulator Development

This repository also contains a from-scratch Apple Silicon-native emulator path.
It provides a ROM ZIP/directory inspector, boot ROM loader, MIPS R3000A/GTE
execution foundation, memory bus, DMA, GPU framebuffer renderer, input mapping,
Gym-style native backend, and a minifb-powered local play window:

```sh
cargo run -- native-inspect assets/roms/<game-romset.zip>
cargo run -- native-rom-summary assets/roms/<game-romset.zip>
cargo run -- native-step assets/roms/<game-romset.zip> 16
cargo run -- native-step assets/roms/<game-romset.zip> 1000000
cargo run -- native-env-step assets/roms/<game-romset.zip> 5 1 10000
cargo run -- native-scripted-step assets/roms/<game-romset.zip> 100000 /tmp/br2-script.png coin:30 noop:30 start:30 coin+start:60 noop:120
cargo run -- native-input-check 120000
cargo run -- native-health-check 120000
cargo run -- native-play 120000 fit
cargo run -- serve-native 127.0.0.1:8765 assets/roms/<game-romset.zip> 500000
```

The native path is intentionally separated from MAME and ZiNc compatibility
commands so emulator-core work can proceed without republishing proprietary
Windows binaries or ROM data.
The current native core runs on macOS, opens a local framebuffer window, reads
the mapped controls, uploads visible texture data to VRAM, tracks presentation
quality, and exposes CPU/IO/GPU state for iterative validation.
`native-health-check` and `native-play-snapshot --match-script` remain the
release gates for full-scene composition, input coverage, and gameplay-scene
rendering.
`native-env-step` and `serve-native` connect the native core to the same
Gym-style action/observation contract used by the null backend. Native backend
creation performs the match-entry sequence once, stores a playable emulator
checkpoint, and clones that checkpoint for subsequent `/reset` calls instead
of cold-booting the game again.
`native-scripted-step` applies a sequence of Gym actions to the native core and
writes a normalized 512x480 diagnostic frame, with raw/window frame stats in
JSON for repeatable boot/input debugging. `native-play-snapshot` additionally
writes the exact 640x480 aspect-corrected GUI presentation. `native-input-check`
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
curl -sS -X POST http://127.0.0.1:8765/step \
  -d '{"action":5,"frames":1}'
curl -sS -X POST http://127.0.0.1:8765/step \
  -d '{"action":5,"frames":1,"screenshot":true}'
curl -sS -X POST http://127.0.0.1:8765/step \
  -d '{"buttons":{"up":true,"punch":true,"guard":true},"frames":2}'
```

`screenshot` defaults to `false` on both `/reset` and `/step` to keep
non-vision-agent responses small. Set it to `true` only when a base64 PNG is
needed. Native screenshots use the same aspect-corrected 640x480 buffer as the
macOS GUI. Each `/step` must provide exactly one of `action` or `buttons`.
`action` preserves the `Discrete(20)` contract; `buttons` permits any
simultaneous boolean control combination. `frames` must be between 1 and 600.
If the core cannot reach the next vblank within its safety budget, the request
fails instead of returning a partial frame count. The response `info` contains
the requested buttons, total and per-step guest input activity, playable-state
status, startup checkpoint details, and screenshot dimensions/source.

Endpoints:

- `GET /`
- `GET /action_space`
- `GET /observation_space`
- `POST /reset`
- `POST /step`

## Gymnasium mapping

Action space:

- Type: `Discrete(20)`
- Values: see `cargo run -- action-space`
- Extended control: a `buttons` object for arbitrary simultaneous controls

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

## Backend status

`serve-native` connects the Rust-native emulator directly to the `Backend`
trait, including deterministic bounded vblank stepping, mapped simultaneous
controller input, playable checkpoint reset, HUD observations, guest input
proof, and optional 640x480 PNG observations. `serve` remains a ROM-free null
backend for client development, while `play` is an optional MAME compatibility
reference.
