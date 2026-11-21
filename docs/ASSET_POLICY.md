# Runtime asset policy

The repository contains Rust source, tests, configuration, and machine-readable
asset expectations. It does not contain or distribute ROMs, BIOS files, disc
images, save states, Windows emulator bundles, DLLs, or generated recompiler
executables.

Provide legally obtained assets at runtime through an ignored path such as
`assets/roms/`, or pass an external absolute path to a CLI, HTTP, or MCP
backend. `native-inspect`, `native-rom-summary`, `asset-check`, and
`asset_manifest` validate or describe expected names, sizes, and hashes without
making the files distributable.

The `.gitignore` and CI policy reject common proprietary extensions. Before a
commit, verify both tracked files and generated output:

```sh
git status --short
git ls-files | rg -i '\.(zip|rom|bin|cue|iso|chd|exe|dll|znc)$'
```

If a local cache is needed for repeated runs, keep it under ignored
`.runtime-cache/` or outside the repository. Do not place an asset archive in a
GitHub release, issue attachment, or generated CI artifact.

## Rights boundary

The source licenses for this repository, BloodyRoar2Recomp, REA, and their
dependencies cover source code and tooling only. They do not grant rights to
Bloody Roar 2 arcade ROMs, PlayStation disc images, retail BIOS dumps,
generated game C, game art, FMV, or audio. Those files remain the responsibility
of the person supplying them and must stay outside GitHub artifacts.

The official BloodyRoar2Recomp release page currently provides Windows x64 and
Linux x64 runtime archives. The archives include the runtime/OpenBIOS but omit
retail disc and retail BIOS data. They are separate host executables, not MAME
ROMs, and cannot be loaded through MAME's `-rompath`. macOS users need a local
build or the Rust-native backend; no proprietary game data is downloaded by
this project.

This is an engineering policy, not jurisdiction-specific legal advice. Confirm
your local law, license terms, and ownership or other authorization for each
asset before using it.
