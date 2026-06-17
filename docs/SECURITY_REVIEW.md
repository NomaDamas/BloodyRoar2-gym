# Static Security Review

Date: 2026-06-18

## Reviewed files

The working directory initially contained only a split archive set:

| File | SHA-256 |
| --- | --- |
| `BloodRoar2 (2).zip` | `4ee0e3aefbdd693a9556ebe6ae96b0f37c9dba7f99fb45a58694f36241ff519a` |
| `BloodRoar2 (2).z01` | `ce8f6b8f67a87dc48bef664b4fb56acad25b3009450c722c184c180747cd69a0` |
| `BloodRoar2 (2).z02` | `b74fab481bbea3f2f57e945ab82db7ffe1adcb39bfbd4492b83d2fca4e145282` |

## Static findings

- The archive listing includes Windows executables: `zenith.exe` and `ZiNc.exe`.
- The archive listing includes Windows DLL/plugin-style binaries:
  `s11player.dll`, `renderer-*.znc`, `controller*.znc`, and `sound.znc`.
- The archive listing includes ROM zip files under `BloodRoar2/roms/`, including
  `bldyror2.zip`.
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

## Recommended handling

- Do not execute the bundled Windows binaries on macOS.
- If deeper malware analysis is needed, use an isolated VM or sandbox with no
  sensitive credentials.
- Scan the joined archive with a current antivirus engine before any extraction.
- Keep proprietary ROM/game assets outside the repository.
- Prefer the macOS MAME runtime over the bundled ZiNc Windows executables.
