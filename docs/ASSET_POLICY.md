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
