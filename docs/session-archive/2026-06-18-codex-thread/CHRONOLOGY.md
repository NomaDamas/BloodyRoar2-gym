# BloodyRoar2 Session Chronology

All dates in the commit ledger are Asia/Seoul calendar dates. The source
session itself spans `2026-06-18 00:11 KST` through
`2026-08-07 23:08 KST`.

## 1. Foundation and Safety

`2026-06-18`

- The initial request combined static malware-risk review, macOS adaptation,
  real gameplay, Gym-style agent control, remote setup, legal asset separation,
  and source-only commit/push.
- A Rust package, non-commercial source license, deterministic `NullBackend`,
  38-action P1/P2 control model, observation contract, CLI, HTTP server, and
  Python example were created.
- The Windows ZiNc bundle was treated as untrusted. EXE/DLL/ZNC files were not
  run; ROMs and proprietary archives were excluded from Git.
- MAME helpers and ROM diagnostics were added as an external compatibility
  reference.
- A default-denied ZiNc/Wine compatibility path was added but was not adopted
  as the native solution.
- The project pivoted to a from-scratch native Rust emulator after the user
  rejected a harness-only definition of completion.
- Initial MIPS R3000A/GTE, bus, memory map, DMA, GPU, controller, ROM-set, and
  Gym backend foundations were implemented.

## 2. Native Boot and Hardware Debugging

`2026-06-19` through `2026-06-20`

- CPU, GTE, MDEC, GPU transfer, linked-list DMA, display, and scripted input
  diagnostics expanded rapidly.
- GTE projection/depth behavior and MDEC command/DMA handling were corrected.
- GPU transfer commands were narrowed to exact opcodes after data-like words
  were misread as huge VRAM copies.
- The game advanced from boot diagnostics to character select, but combat
  initially rendered only HUD/flat floor without complete fighters/background.
- The first native macOS window and scripted play path were added.
- The visible top-half-only failure led to 512x480 field composition becoming a
  hard output and validation requirement.

## 3. Playability, Rendering, and Input Handoff

`2026-06-21` through `2026-06-23`

- Autoplay, health validation, full-frame checks, warning auto-boot, GUI window
  scaling, and playability gates were added.
- User screenshots exposed recurring failures: warning-only startup, black
  transitions, cropped display, red/salmon palette corruption, distorted match
  rendering, lag, and keys not reaching the guest.
- Stable-frame selection and actual-display gating prevented nonpresentable
  recovered candidates from replacing warning/live display state.
- Guest input checks grew from host key detection to coin/start edge counters,
  direction/attack reads, branch visual changes, and native playability proof.
- GUI fast-forward settled before handoff instead of opening on unstable
  transitions.
- Display normalization, vertical smoothing, deinterlacing, and title/warning
  rejection were iterated with real-ROM PNG evidence.

## 4. Persistent ROM Cache

`2026-06-24` through `2026-06-25`

- Reopening or extracting the user ZIP on every run was identified as a major
  startup and trust problem.
- ZIP parsing and materialization moved into the Rust runtime without adding a
  dependency that required network access.
- A source-fingerprinted cache was added, then moved outside `target` under
  ignored `.runtime-cache/native-rom-cache`.
- `LATEST_ROM_DIR`, cache-hit reporting, cache-path commands, and handoff-frame
  validation made repeat GUI/API runs use the prepared directory directly.
- Playability classifiers learned to reject FMV/subtitle ghosting and other
  false-positive “gameplay” frames.

## 5. Renderer Recovery and Gameplay Validation

`2026-07-05` through `2026-07-16`

- Stage texture aliases, CLUT selection, stale strips, roster atlases, portrait
  origins, field recovery, and observation/GUI source mismatches were debugged.
- Mixed warning/gameplay fields and right-half atlas corruption were isolated
  through raw display, actual display, candidate, and VRAM captures.
- Character-select and final-match frames became explicit PNG/JSON release
  evidence.
- HTTP Gym smoke validation exercised action space, observation space, reset,
  and step against the real native backend.
- The first broad “complete gameplay and Gym integration” milestone was
  committed only after rendering, input, and API evidence converged.

## 6. GUI Input and Startup Stabilization

`2026-07-24` through `2026-07-29`

- Fixed pre-window fast-forward timeouts that ended before title/character
  selection on slower hosts.
- Separated physical-keyboard proof from sidecar/test input.
- Added bounded GUI E2E input scripts, presented-frame captures, and requested
  guest-button verification.
- Character-select and fighter/stage palette alias collisions were narrowed by
  packet provenance.
- Physical Enter became a feedback-driven coin acceptance, release, then Start
  acceptance sequencer.
- CPU hot paths, display caches, aspect conversion, GUI polling, and AppKit
  foreground activation reduced choppiness and missed input.
- Two-player input, stale replay invalidation, fighter rendering recovery, and
  Beast transitions expanded the verified gameplay surface.
- A Beast-time `AddressLoad` at `EPC=0x8035a488` was diagnosed as a guarded
  unaligned-load recovery gap rather than a GUI/display freeze.

## 7. Audio, Live QA, and Agent APIs

`2026-07-30` through `2026-08-03`

- PS9805 sound-CPU execution, YMF271 PCM/FM synthesis, stereo mixing, queue
  telemetry, CoreAudio callback behavior, pacing, and health classification
  were developed.
- Profiling showed that renderer/recovery work could reduce wall-clock frame
  production and starve the real-time audio consumer even while emulated sample
  generation was correct.
- On `2026-08-01`, a 4,200-frame live QA run reported full GUI/input/Beast
  coverage, zero missed vblanks, bounded frame timing, and low audio underflow.
- Sound and rendering work continued because later changes could invalidate a
  previous live pass.
- The project exposed named character actions, arbitrary action sequences,
  screenshots, runtime health, render/effect/audio progression, and MCP-native
  tools for LLM/RL agents.

## 8. Final Rendering Pass and Handoff

`2026-08-04` through `2026-08-07`

- Late rendering work focused on character-select portraits, combat corruption,
  missing fighter parts, stale atlas fields, recovery provenance, roster
  fallback quality, and false-positive QA gates.
- Repository resource usage, `target`, `tmp`, ROM caches, active processes, and
  the oversized Codex thread were audited with a strict project-only boundary.
- Regenerable project outputs were cleaned while preserving the current release
  binary, active ROM cache, source changes, and evidence.
- `docs/PROJECT_HANDOFF.md` was committed as the stop point.
- The user approved archiving and deleting only the oversized BloodyRoar2
  session, with an explicit prohibition on touching other projects' sessions.

## Commit Ledger

| Commit | Date | Subject |
| --- | --- | --- |
| `bea0e9e` | 2026-06-18 | Create Rust RL harness |
| `5f95898` | 2026-06-18 | Add macOS MAME runtime path |
| `750b08a` | 2026-06-18 | Add MAME ROM diagnostics |
| `f158569` | 2026-06-18 | Add ZiNc compatibility runner |
| `979502c` | 2026-06-18 | Start native emulator core |
| `609b933` | 2026-06-18 | Expand native emulator CPU and IO |
| `d417275` | 2026-06-18 | Connect native emulator to Gym API |
| `a783a1d` | 2026-06-18 | Advance native emulator milestone |
| `710b20c` | 2026-06-18 | Advance native boot emulation |
| `451d805` | 2026-06-19 | Expand native emulator diagnostics and scripted input |
| `907f524` | 2026-06-19 | Improve native GPU DMA and display diagnostics |
| `56d94cf` | 2026-06-20 | Add native macOS emulator runtime |
| `24ed829` | 2026-06-21 | Add native autoplay play path |
| `dba7820` | 2026-06-21 | Add native health validation |
| `8d99118` | 2026-06-21 | Fix native full-frame rendering and playability checks |
| `fabeac1` | 2026-06-21 | Keep native play window full height |
| `a71a81d` | 2026-06-21 | Make native play auto-boot through warnings |
| `02e7027` | 2026-06-22 | Complete native macOS playability validation |
| `e73ee2d` | 2026-06-22 | Complete native BR2 playability path |
| `3d073e1` | 2026-06-23 | Fix native play rendering and input handoff |
| `c31540b` | 2026-06-23 | Fix native play render replay artifacts |
| `e80337e` | 2026-06-24 | Fix native play render handoff and input |
| `f1ad378` | 2026-06-24 | Avoid per-run native ROM zip extraction |
| `c3dd173` | 2026-06-24 | Cache native ROM materialization |
| `9180c37` | 2026-06-24 | Persist native ROM cache outside target |
| `4c91d68` | 2026-06-24 | Align native play validation with handoff frame |
| `fc2fb31` | 2026-06-24 | Improve native play cache reuse visibility |
| `3379f22` | 2026-06-25 | Improve native macOS play validation |
| `92e278e` | 2026-07-05 | Improve native macOS play diagnostics |
| `03ef712` | 2026-07-06 | Complete native macOS gameplay validation |
| `7d8870f` | 2026-07-07 | Complete native macOS gameplay rendering path |
| `a013845` | 2026-07-07 | Fix native play stage frame selection |
| `119c6dc` | 2026-07-07 | Fix native play rendering selection |
| `d50a783` | 2026-07-16 | feat(native): complete macOS gameplay and Gym integration |
| `8705082` | 2026-07-24 | fix(native): stabilize macOS rendering and GUI input |
| `1352606` | 2026-07-27 | Complete native macOS gameplay and Gym integration |
| `7b17f01` | 2026-07-27 | Add end-to-end native GUI input verification |
| `97dd7c3` | 2026-07-27 | Optimize native play startup handoff |
| `ddce272` | 2026-07-28 | Complete native gameplay and Gym API integration |
| `7fa173a` | 2026-07-28 | Fix native macOS rendering and input responsiveness |
| `1de9847` | 2026-07-29 | Fix native title input and smooth macOS rendering |
| `ea970d0` | 2026-07-29 | Complete native two-player input and rendering recovery |
| `b0b630e` | 2026-08-01 | Complete native macOS gameplay audio and live QA |
| `8761274` | 2026-08-03 | feat: complete native macOS gameplay and agent APIs |
| `a6aa5a1` | 2026-08-06 | Fix native macOS gameplay rendering and input |
| `0566d19` | 2026-08-07 | Document project session handoff |
