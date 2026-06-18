# bloodyroar2-gym

Rust control harness for building Gymnasium-style RL and LLM agents around a
legally supplied Bloody Roar 2-compatible emulator backend.

This repository does not contain ROMs, BIOS files, Windows executables, DLLs, or
other proprietary game assets. Keep those outside Git and pass their paths to a
future backend adapter only if you have the legal right to use them.

## Current status

- macOS-safe Rust project scaffold.
- Discrete action space for fighter controls.
- Observation contract compatible with RL loops.
- CLI and HTTP JSON API using a deterministic `NullBackend`.
- macOS MAME launch path for local human play with legally supplied ROM assets.
- Asset policy and static security notes for the supplied Windows bundle.

The original ZiNc Windows bundle is not republished and is not treated as
portable Rust source. Human play on macOS is handled through MAME.

## CLI

```sh
cargo run -- action-space
cargo run -- observation-space
cargo run -- reset
cargo run -- step 5 4
cargo run -- serve 127.0.0.1:8765
```

## macOS play path

Install MAME and prepare local assets from your legally owned archive:

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

On Apple Silicon, Homebrew MAME is the native emulator path. The original ZiNc
Windows executable is not native Apple Silicon software; running it would
require a separate Wine/Rosetta-style compatibility layer and is not the default
or recommended path.

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

This repository also contains the start of a from-scratch Apple Silicon-native
emulator path. It is not playable yet; it currently provides a ROM ZIP inspector,
boot ROM loader, memory bus, and a small MIPS R3000A interpreter foundation:

```sh
cargo run -- native-inspect assets/roms/bldyror2.zip
cargo run -- native-step assets/roms/bldyror2.zip 16
cargo run -- native-step assets/roms/bldyror2.zip 1000000
```

The native path is intentionally separated from MAME and ZiNc compatibility
commands so long-term emulator work can proceed without pretending incomplete
CPU/GPU/SPU/protection-chip implementation is already game-playable.
The current native core can execute the bundled COH-1002E boot ROM instruction
stream and exposes CPU/IO state for iterative development.

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

- Type: `Discrete(16)`
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
