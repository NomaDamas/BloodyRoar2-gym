# Native Emulator Workflow

This runbook is the canonical validation path for the Rust-native Sony
ZN/PS1-family emulator milestone on Apple Silicon macOS. It keeps proprietary
ROM, BIOS, save state, capture, and emulator binary assets outside Git while
validating the native CPU/IO/GPU/DMA/input/play-window foundations that are
currently implemented.

For milestone handoff on Apple Silicon, run these build and test commands in
order before native ROM diagnostics:

```sh
rustup target add aarch64-apple-darwin
rustup component add rustfmt clippy
cargo build --target aarch64-apple-darwin
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

On Apple Silicon hosts, `cargo build` may also be used for local iteration, but
the explicit `aarch64-apple-darwin` target above is the documented canonical
build command for production validation evidence.

## 1. Build

Install the Apple Silicon Rust target once:

```sh
rustup target add aarch64-apple-darwin
rustup component add rustfmt clippy
```

Build the native project:

```sh
cargo build --target aarch64-apple-darwin
```

For the default host target on Apple Silicon, this is equivalent to:

```sh
cargo build
```

## 2. Test and Static Checks

Run the required regression suite:

```sh
cargo test
```

Run formatting and lint checks before handing off milestone work:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Run cross-target compilation checks before merging native emulator changes:

```sh
rustup target add aarch64-apple-darwin
rustup target add x86_64-unknown-linux-gnu
cargo check --all-targets --target aarch64-apple-darwin
cargo test --target aarch64-apple-darwin
cargo check --all-targets --target x86_64-unknown-linux-gnu
cargo test --target x86_64-unknown-linux-gnu
```

The CI workflow in `.github/workflows/cross-target.yml` runs the same target
checks on an Apple Silicon macOS runner and an Ubuntu Linux runner. The Linux
target validates that the generic native platform path remains compilable and
testable without Apple-specific symbols.

If a local MAME install is available, verify the legally supplied ROM set before
using the external compatibility path:

```sh
cargo run -- mame-required assets/roms
cargo run -- rom-ident assets/roms
cargo run -- mame-check assets/roms
cargo run -- doctor assets/roms
```

## 3. Native ROM Diagnostics

Place legally obtained local assets in ignored paths such as `assets/roms/`.
The expected local game archive path for native commands is:

```sh
assets/roms/bldyror2.zip
```

Inspect the ROM ZIP without extracting or committing proprietary data:

```sh
cargo run -- native-inspect assets/roms/bldyror2.zip
```

The command emits JSON describing ZIP entries, boot ROM classification, and
compatibility diagnostics for the native loader.
For day-to-day diagnosis, prefer the compact compatibility summary:

```sh
cargo run -- native-rom-summary assets/roms/bldyror2.zip
```

`compatible:false` means the asset set does not exactly match the MAME
manifest. The native runtime may still identify a known local ZiNc-style
variant through `known_variants`, but do not patch, download, or commit
proprietary ROM data in this repository.

## 4. Deterministic Native Stepping

Run a small CPU/IO smoke step:

```sh
cargo run -- native-step assets/roms/bldyror2.zip 16
```

Run the milestone-scale deterministic state sample:

```sh
cargo run -- native-step assets/roms/bldyror2.zip 1000000
```

Demonstrate repeatability by capturing two outputs and comparing them:

```sh
cargo run -- native-step assets/roms/bldyror2.zip 1000000 > /tmp/br2-native-step-a.json
cargo run -- native-step assets/roms/bldyror2.zip 1000000 > /tmp/br2-native-step-b.json
diff -u /tmp/br2-native-step-a.json /tmp/br2-native-step-b.json
```

An empty `diff` means the same local assets and instruction budget produced the
same deterministic JSON CPU/IO state.

## 5. Gym-Style Native API

Exercise one native backend environment step from the CLI:

```sh
cargo run -- native-env-step assets/roms/bldyror2.zip 5 1 10000
```

Run a deterministic input script and write the selected PNG observation:

```sh
cargo run -- native-scripted-step assets/roms/bldyror2.zip 100000 /tmp/br2-script.png coin:30 noop:30 start:30 coin+start:60 noop:120
```

Each script segment is `<action:frames>` where `action` is either an action
index from `cargo run -- action-space` or an action name such as `coin`,
`start`, or `coin+start`.

Serve the native backend over the Gym-style HTTP API:

```sh
cargo run -- serve-native 127.0.0.1:8765 assets/roms/bldyror2.zip 10000
```

Probe the API from another shell:

```sh
curl -sS http://127.0.0.1:8765/action_space
curl -sS http://127.0.0.1:8765/observation_space
curl -sS -X POST http://127.0.0.1:8765/reset
curl -sS -X POST http://127.0.0.1:8765/step -d '{"action":5,"frames":1}'
```

The Python standard-library client can target the same server:

```sh
python3 examples/python/bloodyroar2_env.py
```

## 6. Native Play Validation

Verify that the native core reaches a textured gameplay candidate and that the
game reads the mapped controls:

```sh
cargo run --release -- native-input-check assets/roms 500000
```

Expected success evidence includes:

- Top-level `playable:true`.
- `native_playable_candidate:true`.
- `input_controls_active:true`.
- `state.native_playability.classification:"native_playable_candidate"`.
- `state.native_playability.has_actual_playfield:true`.
- `state.native_playability.actual_display.detail_edges` above the reported
  detail threshold implied by the current display size.

Open the native macOS play window:

```sh
cargo run --release -- native-autoplay assets/roms 500000 2
cargo run --release -- native-play assets/roms 500000 2
```

`native-autoplay` runs the built-in coin/start/control script first, then returns
control to the keyboard. Use `native-play` when you want manual input from boot.

Controls:

- Arrows: move.
- `Z`: punch.
- `X`: kick.
- `A`: beast.
- `S`: guard.
- `C`: coin.
- `Enter`: start.
- `Esc`: quit.

For non-interactive smoke validation, pass a frame limit:

```sh
cargo run --release -- native-autoplay assets/roms 500000 2 1140
cargo run --release -- native-play assets/roms 500000 2 700
```

Capture a deterministic visual artifact for review:

```sh
cargo run --release -- native-scripted-dump assets/roms 500000 tmp/native-validation/final-native-play-check noop:300 coin:30 noop:120 start:300 noop:120 punch:60
```

Inspect `tmp/native-validation/final-native-play-check.actual-display.png`.
`display.png` and `observation.png` may intentionally use cached fallback
frames for Gym-style observations, so they are not sufficient proof of native
gameplay. Keep `tmp/` untracked.

## 7. Asset Compliance

Check local asset paths before use:

```sh
cargo run -- asset-check assets/roms/bldyror2.zip
```

Verify no proprietary assets are tracked or staged:

```sh
git status --short
git ls-files | grep -Ei '(^|/)(assets|roms|bios|isos|samples|captures)/|\\.(zip|7z|rar|rom|bin|cue|iso|chd|img|ecm|ccd|sub|mdf|mds|pbp|exe|dll|dylib|so|znc)$'
```

The `git ls-files` command should print nothing. If it prints a proprietary
asset path, remove that asset from Git tracking before continuing.
