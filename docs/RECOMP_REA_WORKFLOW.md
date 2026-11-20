# Recompilation and REA workflow

This project keeps the portable Rust runtime separate from the optional
recompilation and reverse-engineering workflows. The two upstream projects are
provenance and tooling inputs; they are not sources of redistributable game
data.

## Inputs and boundaries

1. Obtain a disc image or arcade ROM set that you are legally allowed to use.
   Accepted PSX image forms are `bin/cue`, `iso`, and `chd`. Keep it outside Git
   and outside release artifacts.
2. Use the pinned configuration and seed files from
   [BloodyRoar2Recomp](https://github.com/novapowers0/BloodyRoar2Recomp) when
   generating a caller-owned recompiled build. Generated C/C++, object files,
   executables, and extracted assets stay in a private build directory.
3. Use [REA](https://github.com/morluto/rea) as a bring-your-own local analysis
   provider. REA does not download, package, or make copyrighted binaries
   redistributable.

The Rust-native backend in this repository is a separate runtime. It shares
the input, observation, and MCP contracts with the analysis workflow, but it
does not embed generated recompiler output.

## Inspecting a caller-owned binary

Install REA and its selected local provider according to the upstream
documentation, then run the repository helper:

```sh
scripts/rea-analysis.sh /path/to/caller-owned/binary
```

The helper performs read-only metadata and disassembly requests through REA and
writes JSON evidence under ignored `tmp/rea-analysis/`. It never downloads an
engine, executes the target, or copies the target into this repository. The
provider may have its own command-side effects; review REA's provider policy
before enabling an operation that is broader than static inspection.

The same plan is available without invoking REA:

```sh
cargo run -- analysis-manifest
cargo run -- analysis-plan /path/to/caller-owned/binary
```

`analysis-plan` only serializes the path and suggested commands. It does not
open, hash, execute, or modify that path, which makes it safe for an agent to
request through stdio MCP before a human chooses the analysis step.

The plan mirrors REA's documented commands:

```sh
rea inspect-with-objdump /path/to/binary file_headers --json
rea inspect-with-objdump /path/to/binary section_headers --json
rea inspect-with-objdump /path/to/binary symbols --json
rea inspect-with-objdump /path/to/binary disassemble --json
rea execute-rizin-command /path/to/binary iI --json
```

Treat the resulting JSON as evidence. Store it outside tracked source, record a
digest in a private issue or handoff, and copy only reviewed facts such as a
symbol/config change into this repository.

## Agent control

The deterministic `mcp` command works without assets. `native-mcp` accepts a
caller-supplied ROM directory or ZIP and never fetches one. In addition to the
Gym control tools, both servers expose `analysis_manifest`, `analysis_plan`,
`asset_manifest`, and `platform`. The analysis tools are read-only metadata
surfaces; actual REA execution remains an explicit local step.
