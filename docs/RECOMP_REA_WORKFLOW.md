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
caller-supplied ROM directory or ZIP and never fetches one. `recomp-mcp`
accepts caller-owned Recomp executable and disc paths. In addition to the Gym
control tools, all MCP servers expose `analysis_manifest`, `analysis_plan`,
`asset_manifest`, and `platform`. The analysis tools are read-only metadata
surfaces; actual REA execution remains an explicit local step.

## macOS Recomp build and MCP control

`BloodyRoar2Recomp` is a native PSX port, so its input is a PlayStation disc
image rather than the `bldyror2` arcade ROM set used by this repository's Rust
native backend. Public releases include generated game code and OpenBIOS, but
they do not include the copyrighted disc data. A local build still needs a
caller-owned `.cue`/`.bin`, `.iso`, or `.chd`.

Run the repository helper from an Apple Silicon macOS checkout:

```sh
scripts/recomp-macos.sh \
  --project-root /path/to/BloodyRoar2Recomp \
  --disc /path/to/your/BloodyRoar2.cue \
  --build-dir /path/to/BloodyRoar2Recomp/build-release
```

The upstream workflow is, in order:

```sh
git -C /path/to/BloodyRoar2Recomp submodule update --init --recursive
cd /path/to/BloodyRoar2Recomp
./psxrecomp/tools/ci/build_emitters.sh
python3 psxrecomp/psxrecomp_cli.py verify-disc \
  --config game.toml --project-root . --disc /path/to/your/BloodyRoar2.cue
python3 psxrecomp/psxrecomp_cli.py generate \
  --config game.toml --project-root . --disc /path/to/your/BloodyRoar2.cue
python3 psxrecomp/psxrecomp_cli.py rebuild \
  --config game.toml --project-root . --build-dir build-release --target psx-runtime \
  --cmake-extra=-DPSX_DEBUG_TOOLS=ON
```

Use `--dry-run` on the helper or `recomp-build-plan` in the CLI/MCP to inspect
the command construction without touching the disc. The helper refuses to use
this `bloodyroar2-gym` checkout as the generated-code project and never stages
disc data, generated C, executables, or BIOS dumps under Git.

The isolated Japan target enables `BR2_BUILD_JAPAN=ON`. The universal target
requires all three regional generated C sets; pass the EU image as `--disc`,
the USA image as `--disc-us`, and the Japan image as `--disc-japan` so the
helper generates each region before invoking `BR2_BUILD_UNIVERSAL=ON`.

Once a debug-tools Recomp executable exists, an agent can control it through:

```sh
cargo run --release -- recomp-mcp \
  /path/to/BloodyRoar2Recomp/build-release/BloodyRoar2_Recompiled \
  /path/to/your/BloodyRoar2.cue 4370
```

The adapter starts the process headlessly and translates P1 action/button
requests to `set_input`/`clear_input`, advances by polling `frame`, and returns
temporary PNG screenshots. The upstream server currently controls pad1 only;
P2 actions are rejected with an explicit diagnostic. `PSX_HEADLESS=1` makes
the process safe for MCP use, but `present_shot` is unavailable in headless
mode, so the adapter uses the stable `screenshot` command. The helper forces
`PSX_DEBUG_TOOLS=ON`; a release built with `PSX_NO_DEBUG_TOOLS` cannot be
attached.
