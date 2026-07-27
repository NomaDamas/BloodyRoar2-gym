# Static Security Review

Date: 2026-06-18

## Reviewed files

The working directory initially contained a user-supplied split/archive set.
Concrete archive names and hashes are intentionally not recorded in this
repository because they identify proprietary runtime content that must remain
local and untracked.

| Category | Repository handling |
| --- | --- |
| User-supplied archive parts | Local only, ignored by Git |
| Extracted ROM/game payloads | Local only, ignored by Git |
| Emulator executable/plugin binaries | Local only, ignored by Git |

## Static findings

- The archive listing includes Windows executable files.
- The archive listing includes Windows DLL/plugin-style binary files.
- The archive listing includes ROM ZIP entries.
- `unzip` reported that the `.zip` claims to be the last disk of a multi-part
  archive, so extraction should not be trusted unless the parts are joined or
  handled by a tool that supports split ZIP archives correctly.
- No executable from the archive was run.

## Risk assessment

The bundle should be treated as untrusted Windows-only emulator/game content.
Static metadata alone cannot prove malware is absent. Running the EXE/DLL/ZNC
files directly, through Wine, or through an emulator plugin loader could affect
the host system if the binary is malicious or vulnerable.

## Safety decision

This repository excludes the archive, extracted game files, ROMs, BIOS files,
EXEs, DLLs, and plugin binaries from Git. The Rust adaptation is a clean-room
control harness that expects legally obtained assets to be supplied locally at
runtime and never committed.
The legacy `zinc-play` command is default-denied and fails before launching Wine
unless `BLOODYROAR2_ALLOW_UNSAFE_ZINC_WINE=1` is set by the operator.

## Recommended handling

- Do not execute the bundled Windows binaries on macOS.
- Do not use `zinc-play` on the host by default; if it is needed for isolated
  compatibility testing, set `BLOODYROAR2_ALLOW_UNSAFE_ZINC_WINE=1` only inside
  a disposable VM or sandbox after scanning the local bundle.
- If deeper malware analysis is needed, use an isolated VM or sandbox with no
  sensitive credentials.
- Scan the joined archive with a current antivirus engine before any extraction.
- Keep proprietary ROM/game assets outside the repository.
- Prefer the macOS MAME runtime over the bundled ZiNc Windows executables.
