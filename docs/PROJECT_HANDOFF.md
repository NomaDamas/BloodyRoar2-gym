# BloodyRoar2-rust Project Handoff

Updated: 2026-08-08

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
  `1f9955883abd6c2e9894cb09ab9d41b839109212c4e58d3c1a304dd2d5a200c9`

Run without rebuilding `target`:

```bash
BIN=.runtime-cache/bin/bloodyroar2-gym
ROM_CACHE="$("$BIN" native-cache-path)"
BR2_NATIVE_DISABLE_AUDIO=1 "$BIN" native-play "$ROM_CACHE" 600000 1
```

Resolve `ROM_CACHE` immediately before each run. Passing the cache root can
select an obsolete materialization when multiple ignored cache generations are
present.
Audio is disabled in the current manual-QA command because the latest live
CoreAudio run generated nonzero PCM but failed the realtime queue-underflow
gate.

## Implemented

- Native Apple Silicon emulator execution path.
- Keyboard and P1/P2 input paths.
- Rendering and frame presentation fixes.
- Boot, character selection, combat, Beast transformation, effects, and
  recovery paths exercised by headless E2E tests.
- Dedicated headless QA commands for all 11 character slots and all nine
  verified stage states.
- Gym-style action and observation interfaces.
- HTTP and MCP control surfaces.
- ROM materialization cache so the source archive is not extracted on every
  launch.

## Committed Baseline Verification

- Rust library tests: 1715 passed, 2 ignored.
- Rust binary tests: 394 passed.
- Python QA: 60 passed, 6 skipped.
- Release clippy with `-D warnings`: passed.
- Headless real-game E2E covered boot, P1/P2 selection, combat, Beast,
  final-frame presentation, input, effect progression, and recovery.

## Post-Session Continuation

The archived Codex source ends at `2026-08-07 23:08:10 KST`. The current
worktree has later uncommitted rendering, stage QA, input, and sound changes in
12 existing files. Do not revert those changes while following this handoff.

- `qa-summary-20260808-224406.json`: build passed and the 5,136-frame headless
  smoke path passed.
- Its live GUI path stopped at frame 291 with
  `gui_stop_reason=window_closed_or_escape`, so full live input, gameplay,
  Beast, final rendering, and performance were not verified.
- CoreAudio started and PCM was nonzero, but realtime health failed with
  `coreaudio_queue_underflow_ratio_exceeded`.
- The current release binary supports `BR2_NATIVE_DISABLE_AUDIO=1` for manual
  visual/input QA without broken audio.

## Remaining Validation

- Fresh macOS GUI test using a physical keyboard.
- Complete bounded live GUI run without early window closure.
- CoreAudio pacing fix followed by a physical speakers/headphones test.

Headless verification does not replace these physical checks.

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

The oversized conversation file associated only with this project is:

```text
/Users/jinminseong/.codex/sessions/2026/06/18/rollout-2026-06-18T00-11-35-019ed623-4296-79f2-a751-ff3cec538edc.jsonl
```

Its byte-exact archive, searchable semantic archive, full chronology, hashes,
and restoration procedure are documented in:

```text
docs/session-archive/2026-06-18-codex-thread/
```

The exact archive is stored under this Git-ignored location:

```text
.runtime-cache/session-archives/019ed623-4296-79f2-a751-ff3cec538edc/
```

The archive directories are mode `0700` and archive files are mode `0600`.
The source JSONL was deleted with one exact absolute path at
`2026-08-08T16:07:33Z` after `lsof`, source/archive hashes, and restoration
hashes passed. All 6,180 other session files present before deletion retained
the same path and inode; two new Codex sessions in the immediate post-delete
snapshot, plus later runtime-created sessions, were left untouched. Repeated
checks found zero missing or inode-changed preexisting sessions. No wildcard
or directory-wide session deletion was used.

The archive was never Git-tracked. Secret-pattern scans found zero matches in
the semantic archive, tracked worktree, all local Git objects, and the live
remote `main` commit. The byte-exact local archive still contains the original
secret-shaped value and must remain private.
