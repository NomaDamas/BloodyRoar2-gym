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
- Asset policy and static security notes for the supplied Windows bundle.

The actual game runtime is intentionally a pluggable backend. The original ZiNc
Windows bundle cannot be safely or legally republished here, and it is not a
portable macOS Rust codebase.

## CLI

```sh
cargo run -- action-space
cargo run -- observation-space
cargo run -- reset
cargo run -- step 5 4
cargo run -- serve 127.0.0.1:8765
```

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

Implement a `Backend` for a macOS-compatible emulator that can expose frame
advance, input injection, memory observation, and screenshot capture. The public
Rust trait is already in `src/backend.rs`.
