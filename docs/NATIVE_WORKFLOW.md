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
manifest. `known_variants` reports supported non-program asset variants.
`known_bad_dumps` reports program images that differ from the current archival
manifest. In particular, `flash1.024` CRC `4866dce3` is the historical image
shipped with the Windows ZiNc bundle. The native runtime accepts it through an
explicit ZiNc compatibility path while still reporting its provenance. The
current archival image is CRC `03465a69`. Do not patch, download, or commit
proprietary ROM data in this repository.

If the legally obtained good `flash1.024` is available as a separate file,
place it beside the existing archive and rebuild the cache without repacking
the ZIP:

```sh
cp /legal/path/flash1.024 assets/roms/flash1.024
cargo run --release -- native-cache-prepare assets/roms
```

The cache loader prefers an exact manifest-matching sidecar over a same-named
bad entry inside the legacy ZIP.

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
cargo run -- native-env-step assets/roms/<game-romset.zip> 5 1 500000
```

Run a deterministic input script and write a normalized 512x480 diagnostic
frame:

```sh
cargo run -- native-scripted-step assets/roms/<game-romset.zip> 100000 /tmp/br2-script.png coin:30 noop:30 start:30 coin+start:60 noop:120
```

Each script segment is `<action:frames>` where `action` is either an action
index from `cargo run -- action-space` or an action name such as `coin`,
`start`, or `coin+start`.

The JSON includes both `raw_frame` and `window_frame` statistics. Use
`window_frame` and the written PNG when checking the macOS GUI crop/deinterlace
path; use `native-scripted-dump` when raw display, observation, and VRAM images
are all needed for GPU debugging.

For the exact 640x480 aspect-corrected buffer presented by `native-play`, use
`native-play-snapshot`.

Serve the native backend over the Gym-style HTTP API:

```sh
cargo run -- serve-native 127.0.0.1:8765 assets/roms/<game-romset.zip> 500000
```

Probe the API from another shell:

```sh
curl -sS http://127.0.0.1:8765/action_space
curl -sS http://127.0.0.1:8765/observation_space
curl -sS -X POST http://127.0.0.1:8765/reset
curl -sS -X POST http://127.0.0.1:8765/step -d '{"action":5,"frames":1}'
curl -sS -X POST http://127.0.0.1:8765/step \
  -d '{"action":5,"frames":1,"screenshot":true}'
curl -sS -X POST http://127.0.0.1:8765/step \
  -d '{"buttons":{"left":true,"punch":true,"guard":true},"frames":2}'
```

`screenshot` defaults to `false` for compact RL/LLM responses. Native steps
advance 1 through 600 emulated vblanks and report an error rather than silently
returning a partial frame count. Provide exactly one of the backward-compatible
discrete `action` index or an arbitrary simultaneous `buttons` object. Native
screenshots are the same 640x480 aspect-corrected buffer presented by the GUI.
The response `info` records requested controls, total and per-step guest input
activity, playable state, checkpoint startup metadata, and screenshot
dimensions/source.

The native backend performs the warning/title/coin/start/select/match-entry
sequence once when it is created. `/reset` then clones that verified playable
checkpoint, avoiding a repeated cold boot for every RL episode.

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

This check is an input/progress smoke test. It is not by itself proof that the
full 3D scene is being composed correctly on the final display.

Run the stricter health gate when validating native emulator completeness:

```sh
cargo run --release -- native-health-check assets/roms 500000
```

`native-health-check` branches from the autoplay checkpoint through up, down,
left, right, punch, kick, beast, and guard. It reports whether every action was
read by the game I/O, whether rendered frames contain visible/detail content,
and whether each branch remains a native playable candidate. A non-zero exit
with `overall_status:"partial"` means the macOS core and controls are running
but the native renderer or branch stability is not yet complete.

Open the native macOS play window:

```sh
cargo run --release -- native-cache-prepare <local-rom-archive.zip>
cargo run --release -- native-cache-path <local-rom-archive.zip>
cargo run --release -- native-play <local-rom-archive.zip> 500000 fit
cargo run --release -- native-autoplay assets/roms 500000 fit
cargo run --release -- native-play assets/roms 500000 fit
```

When a ZIP is passed directly, the native runtime does not shell out to
`unzip` on every launch. It materializes the recognized ROM assets once into
ignored `.runtime-cache/native-rom-cache/<source-fingerprint>/roms`, and all subsequent
native emulator starts reuse that cache until the source archive changes or
that cache is removed. Set `BLOODYROAR2_NATIVE_ROM_CACHE_DIR=/path/to/cache` to
store the runtime cache outside the repository.

`native-play` fast-forwards only the warning sequence, opens the native macOS
keyboard window at the title screen, and waits for the player. One `Enter` press
retries guest-vblank-timed coin pulses until credit is accepted, then retries
Start pulses until the game accepts the player session. This feedback-driven
sequence reaches character selection even when the title polls controls
sparsely. `native-manual` opens a cold-boot keyboard window with no entry script.
Use `native-autoplay` when you need the full scripted
coin/start/select/match-entry assist, bounded pre-window fast-forward
diagnostics, or custom scripted smoke runs. `native-autoplay` performs a bounded
fast-forward of the supplied entry script first, continues any remaining
scripted input in the visible window until manual takeover, then returns control
to the keyboard. The bounded phase uses a full vblank-sized instruction budget
so it does not mistake partial warning, transition, or stale frames for progress.
`native-play` scales the pre-window wall timeout from that frame budget; set
`BR2_NATIVE_PLAY_FAST_FORWARD_WALL_TIMEOUT_SECS` to override it on unusually
slow systems.

Input defaults to the MAME `znt2p` map for Bloody Roar 2. A ZiNc
`cfg/bldyror2.cfg` file is treated as an EEPROM fallback, but native input
stays on the MAME map. Set `BLOODYROAR2_NATIVE_LEGACY_ZINC_INPUT=1` only when
explicitly testing legacy ZiNc input mirroring for diagnostics.

Controls:

- P1: arrows move; `Z` or `Space` punch/confirm; `X` kick; `Q` beast; `E` guard.
- P2: `WASD` move; `J` punch/confirm; `K` kick; `U` beast; `I` guard.
- P1 `C`: coin. P1 `P`: start.
- P2 `N`: coin. P2 `O`: start.
- `V`: service credit.
- `Enter`: feedback-driven P1 title entry. Press once to insert credit, then
  press again after credit is accepted to start.
- `Esc`: quit.

For non-interactive smoke validation, use autoplay, a headless snapshot, or
explicit GUI test input. A bounded `native-play` without input intentionally
waits at the title screen:

```sh
cargo run --release -- native-autoplay assets/roms 500000 fit 1000
cargo run --release -- native-play-snapshot assets/roms 500000 tmp/native-validation/smoke --match-script --fast-forward-frames 2400
cargo run --release -- native-enter-probe assets/roms 500000 720
cargo run --release -- native-play assets/roms 500000 fit 150 \
  --gui-test-input coin:6 start:6 up:6 down:6 left:6 right:6 \
  punch:6 kick:6 beast:6 guard:6 noop:25
```

The third `native-play` argument is the window scale. For example,
`native-play assets/BloodRoar2-combined.zip 240000 1` uses scale `1` and has no
frame limit; add a fourth argument such as `600` for bounded smoke validation.
For repeated runs, materialize an archive once and reuse the resulting ROM
directory:

```sh
cargo run --release -- native-cache-prepare assets/BloodRoar2-combined.zip
cargo run --release -- native-cache-path
```

`native-play` also supports frame-gated draw capture without changing the input
or renderer path:

```sh
cargo run --release -- native-play assets/roms 240000 1 900 \
  --gui-test-input coin:18 noop:18 start:24 noop:840 \
  --draw-capture-predicate texture_page=0x0039,clut=0x785a,min_area=1000 \
  --draw-capture-arm-gui-frame 600 \
  --draw-capture-output tmp/native-validation/beast-effect/draw
```

The generated manifest records each captured GP0 command, bounds, texture
origin, transparency statistics, VRAM transfer provenance, and associated
display/texture/palette image paths.
`native-enter-probe` uses the real title boot state and the same physical Enter
coin/release/start sequencer as the GUI, then requires both guest acceptance
counters and a clean transition away from the title.

`native-play-snapshot` is intentionally stricter than the quick GUI path: it
fails unless the final window frame satisfies gameplay-scene heuristics. A
character transition, black letterbox, or red/salmon corrupted frame is a
rendering gap, not a native playable pass.

Capture a deterministic visual artifact for review:

```sh
cargo run --release -- native-scripted-dump assets/roms 500000 tmp/native-validation/final-native-play-check noop:300 coin:30 noop:120 start:300 noop:120 punch:30 noop:60 punch:30 noop:300 start:30 noop:300
```

Inspect `tmp/native-validation/final-native-play-check.actual-display.png`.
`display.png` and `observation.png` may intentionally use cached fallback
frames for Gym-style observations, so they are not sufficient proof of complete
native gameplay. Keep `tmp/` untracked.

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
