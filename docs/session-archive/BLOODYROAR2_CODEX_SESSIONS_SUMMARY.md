# BloodyRoar2-rust Codex Sessions Summary

## Scope and Safety Boundary

- Repository: `/Users/jinminseong/Desktop/BloodyRoar2-rust`
- Read-only inventory snapshot: `2026-08-17 21:31:31 KST`
- Inclusion rule: only Codex sessions whose recorded working directory exactly
  matches the repository path above.
- Excluded: every session associated with another project or working directory.
- The source session records were inspected read-only. This documentation task
  did not delete, move, truncate, archive, or modify any Codex session.
- Codex session and transcript storage is protected user data. Conversation
  removal must be performed by the user through Codex's own supported
  conversation-management interface, not by deleting files from
  `~/.codex/sessions`.

## Session Family

The project history is centered on the original parent session:

```text
019ed623-4296-79f2-a751-ff3cec538edc
```

That session ran from `2026-06-18 00:11 KST` through
`2026-08-07 23:08 KST`. Its detailed chronology, manifest, semantic exports,
and lossless compressed archive are documented under:

```text
docs/session-archive/2026-06-18-codex-thread/
.runtime-cache/session-archives/019ed623-4296-79f2-a751-ff3cec538edc/
```

Exactly 55 live JSONL files had a `session_meta.payload.cwd` equal to the
repository path at the snapshot time. Their aggregate size was
`8,476,042,505` bytes (about 7.89 GiB):

| Session source | Files | Interpretation |
| --- | ---: | --- |
| Direct user thread | 3 | Independent user-visible Codex conversations |
| Forked subagent | 52 | Parallel branches that inherited parent context |
| Total | 55 | Only exact-working-directory matches |

The forks inherited substantial parent context, so several otherwise
short-lived agents produced JSONL files between roughly 288 MiB and 353 MiB.
This duplication explains most of the aggregate size and must not be
interpreted as 52 independent project histories.

The three direct user sessions are:

| Session ID | Role |
| --- | --- |
| `019fdc8e-899e-7ce2-9d2b-c88c4627cd8a` | Short continuation beginning with `하던거 이어서 진행해` |
| `019fdcac-b652-7791-965b-27b81dd34c04` | Main continuation thread with 11 user messages |
| `01a00fb2-c61b-7493-b081-cf90b18c4a42` | Current session requesting project-only summary and deletion |

Representative subagent metadata includes nicknames such as `Rawls`,
`Hume`, `Leibniz`, `Galileo`, `Bohr`, and `Hypatia`. These sessions should be
interpreted as parallel branches of the same engineering effort, not as
unrelated project histories.

## Live Session Distribution

The dates below come from each file's session metadata timestamp in UTC. They
describe live files only and exclude the separately archived original parent
session.

| UTC metadata date | Files |
| --- | ---: |
| `2026-07-30` | 3 |
| `2026-07-31` | 3 |
| `2026-08-02` | 2 |
| `2026-08-03` | 5 |
| `2026-08-04` | 13 |
| `2026-08-05` | 5 |
| `2026-08-06` | 1 |
| `2026-08-07` | 12 |
| `2026-08-08` | 10 |
| `2026-08-17` | 1 |
| **Total** | **55** |

## Largest Remaining Session Files

Sizes are rounded binary filesystem observations from the read-only inventory.

| Session ID | Approximate size | Classification |
| --- | ---: | --- |
| `019fdcac-b652-7791-965b-27b81dd34c04` | 423.7 MiB | User continuation thread |
| `019fce69-e715-77f2-8221-ac40716ec87e` | 353.4 MiB | Forked subagent |
| `019fd16c-eb2b-7743-9c01-f966d3130e96` | 340.3 MiB | Forked subagent |
| `019fcbe1-c586-7c02-a21f-8a07aebc4ae8` | 338.5 MiB | Forked subagent |
| `019fce69-323e-7860-9c9a-89c057aedb03` | 335.5 MiB | Forked subagent |
| `019fcf85-1e8b-7c60-b84d-27e44cff2c95` | 334.8 MiB | Forked subagent |
| `019fce5a-2bf4-7ad1-83d5-0e7a7de07ec3` | 334.7 MiB | Forked subagent |
| `019fcf23-2489-7b10-9a67-16110d5386ef` | 332.2 MiB | Forked subagent |
| `019fc955-38bb-7aa2-af95-7acafd3d1ce7` | 330.7 MiB | Forked subagent |
| `019fce59-4916-79a2-ae1a-5521f768b196` | 330.3 MiB | Forked subagent |
| `019fcbcd-7429-7611-98bd-41bcee2f7300` | 327.4 MiB | Forked subagent |
| `019fcb75-72dc-7e01-bf18-494615a6869e` | 323.7 MiB | Forked subagent |

Numerous smaller project subagent sessions range from a few kilobytes to about
34 MB. The large-file pattern is primarily inherited conversation context,
tool evidence, and repeated parent history, not an equivalent amount of unique
project work in every file.

## Consolidated Project Chronology

### 1. Foundation and Safety

- Statically inspected the untrusted Windows ZiNc bundle without executing it.
- Established legal and security boundaries for ROMs, BIOS data, executables,
  DLLs, captures, and other proprietary assets.
- Created the Rust package, deterministic null backend, action/observation
  model, CLI, HTTP surface, MAME diagnostics, and default-denied Wine path.
- Pivoted from a harness-only implementation to a native Rust emulator.

### 2. Native Emulator Core

- Built MIPS R3000A and GTE execution, bus and memory mapping, DMA, timers,
  controllers, ZN security handling, GPU/VRAM, MDEC, and display diagnostics.
- Added a native macOS runtime and advanced from boot diagnostics to character
  selection and combat.
- Corrected CPU, GTE, MDEC, GPU transfer, linked-list DMA, field composition,
  and full-height rendering behavior.

### 3. Rendering, Input, and ROM Cache

- Added stable-frame selection and rejection gates for warning screens, black
  transitions, FMV frames, texture atlases, palette corruption, cropping,
  ghosting, and stale overlays.
- Improved stage, portrait, fighter, texture-page, CLUT, and packet-provenance
  recovery.
- Added physical keyboard focus, input latching, release edges, coin/start
  feedback, P1/P2 separation, and guest-side input verification.
- Replaced repeated ZIP extraction with a source-fingerprinted local ROM cache
  under `.runtime-cache/native-rom-cache`.

### 4. Gameplay and Agent Interfaces

- Implemented deterministic playable checkpoints, reset and frame stepping,
  rewards, termination/truncation, character selection, named moves, and timed
  action sequences.
- Exposed Gym-style functionality through CLI, HTTP, and MCP interfaces.
- Added observations and diagnostics for health, Beast meters, time, rendering,
  input, audio, checksums, performance, screenshots, and runtime health.
- Expanded automated validation across roster, stages, two-player input, Beast
  transitions, and native GUI workflows.

### 5. Audio and Live QA

- Implemented PS9805 sound-CPU execution, YMF271 PCM/FM synthesis, stereo
  mixing, CoreAudio output, queue telemetry, adaptive pacing, and health
  classification.
- Profiled renderer and recovery costs that could starve real-time audio even
  when emulated sample generation was correct.
- Repeated live QA because rendering, performance, input, and audio changes
  could invalidate earlier passes.

### 6. Final Handoff and Archive Work

- Completed late rendering work around character-select portraits, combat
  corruption, missing fighter parts, stale atlas fields, and recovery
  provenance.
- Documented the project handoff and preserved the original parent session in
  the existing project-local archive.
- The commit ledger in the existing chronology records 46 commits through
  `0566d19` on `2026-08-07`.

## Current Repository State

The repository already had uncommitted work when this summary was created.
This document does not assess, revert, or modify those changes:

```text
M docs/NATIVE_WORKFLOW.md
M docs/PROJECT_HANDOFF.md
M scripts/native-e2e-qa-macos.sh
M src/lib.rs
M src/main.rs
M src/native/bus.rs
M src/native/cpu.rs
M src/native/emulator.rs
M src/native/io.rs
M src/native/mod.rs
M src/native/sound/ps9805.rs
M src/native/sound/ymf271.rs
?? docs/session-archive/
?? scripts/codex-session-archive.py
```

The existing `2026-06-18-codex-thread` archive remains the authoritative
project-local record for the original giant parent session. Later source and
documentation changes remain uncommitted and must be reviewed as current
worktree state rather than assumed to be part of commit `0566d19`.

## Preservation Status

- No Codex session was deleted during creation of this summary.
- The 55 matching live session files remain in
  `/Users/jinminseong/.codex/sessions`.
- The currently active session is one of those 55 files and may still grow
  after the inventory timestamp.
- No session belonging to another repository was modified or inspected as a
  deletion target.
- No project source file or pre-existing archive file was changed.
- This summary is the only file changed by this documentation task.
- Deletion was not performed because Codex session records, history, and
  transcripts are protected user data. Only the user may remove conversations
  through an explicit supported Codex UI or command they personally choose.
