# BloodyRoar2-rust Project Handoff

Updated: 2026-08-07

## Repository State

- Repository: `/Users/jinminseong/Desktop/BloodyRoar2-rust`
- Branch: `main`
- Remote: `origin/main`
- Last implementation commit: `a6aa5a1 Fix native macOS gameplay rendering and input`
- Proprietary ROM, BIOS, ZIP, EXE, and DLL files are ignored and must not be
  committed.

## Runtime State

- Apple Silicon binary: `.runtime-cache/bin/bloodyroar2-gym`
- Active ROM cache pointer:
  `.runtime-cache/native-rom-cache/LATEST_ROM_DIR`
- Preserved binary SHA-256:
  `cbefd044535381a655fed047de4bf135323b5600d6cb5487148f1a3305095270`

Run without rebuilding `target`:

```bash
.runtime-cache/bin/bloodyroar2-gym native-play \
  .runtime-cache/native-rom-cache 600000 1
```

## Implemented

- Native Apple Silicon emulator execution path.
- Keyboard and P1/P2 input paths.
- Rendering and frame presentation fixes.
- Boot, character selection, combat, Beast transformation, effects, and
  recovery paths exercised by headless E2E tests.
- Gym-style action and observation interfaces.
- HTTP and MCP control surfaces.
- ROM materialization cache so the source archive is not extracted on every
  launch.

## Last Recorded Verification

- Rust library tests: 1715 passed, 2 ignored.
- Rust binary tests: 394 passed.
- Python QA: 60 passed, 6 skipped.
- Release clippy with `-D warnings`: passed.
- Headless real-game E2E covered boot, P1/P2 selection, combat, Beast,
  final-frame presentation, input, effect progression, and recovery.

## Remaining Physical Validation

- Fresh macOS GUI test using a physical keyboard.
- Fresh CoreAudio test using physical speakers or headphones.
- Long-running all-roster and all-stage soak test.

The previous GUI and CoreAudio automation attempt was blocked by execution
tenant GUI privileges. Headless verification does not replace these physical
checks.

## Resource Rules

- Keep total repository usage below 5 GiB.
- Keep project process RSS below 8 GiB.
- Preserve `.runtime-cache/bin` and the directory referenced by
  `.runtime-cache/native-rom-cache/LATEST_ROM_DIR`.
- `target`, `tmp`, test caches, and obsolete ROM caches are reproducible.
- Never delete or terminate sessions, files, or processes belonging to another
  project.
- Do not resume Codex thread `019ed623-4296-79f2-a751-ff3cec538edc`.

## Session Cleanup

The oversized conversation file associated with this project is:

```text
/Users/jinminseong/.codex/sessions/2026/06/18/rollout-2026-06-18T00-11-35-019ed623-4296-79f2-a751-ff3cec538edc.jsonl
```

It may be deleted only after wrapper PID `4714` and Codex PID `4720` have
terminated and `lsof` confirms that no process has the file open. No wildcard
or directory-wide session deletion is permitted.
