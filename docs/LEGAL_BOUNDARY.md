# Legal and distribution boundary

This is an engineering policy and a high-level licensing review, not
jurisdiction-specific legal advice. Before using an asset, confirm the law and
license terms that apply where you live and that you have ownership or another
valid authorization to use the asset.

## What can be published

| Component | What its license covers | Repository policy |
| --- | --- | --- |
| `bloodyroar2-gym` | Rust source and original documentation in this repository | Publish source or host binaries under the repository's Source Available Non-Commercial License; commercial hosting, paid distribution, and sublicensing need separate permission |
| `BloodyRoar2Recomp` | Upstream source code under its published notice | Link to the upstream project and preserve its notice; its own notice does not grant rights to the game disc or other retail data |
| `psxrecomp` | Recompiler tooling under PolyForm Noncommercial 1.0.0 at the pinned revision | Keep it in non-commercial use unless the upstream licensor grants another license |
| `recomp-ui` and `rea` | Their upstream MIT-licensed source | Preserve their copyright and license notices when redistributing their code |
| MAME | External emulator source and its version-specific notices | Use as a separately installed compatibility reference; do not treat its license as a license for Bloody Roar 2 data |

## What is not covered

Bloody Roar 2 arcade ROMs, PlayStation disc images, retail Sony BIOS dumps,
generated game C derived from a retail image, game art, FMV, and audio remain
third-party material. They are not licensed by this repository's `LICENSE`, the
MIT notices in upstream source projects, or the REA tooling license. This
repository therefore does not include, download, proxy, or publish them.

The official [BloodyRoar2Recomp release page](https://github.com/novapowers0/BloodyRoar2Recomp/releases)
currently offers Windows x64 and Linux x64 runtime archives. The current
`v0.7.1` files are [Windows](https://github.com/novapowers0/BloodyRoar2Recomp/releases/download/v0.7.1/BloodyRoar2-v0.7.1.zip)
and [Linux](https://github.com/novapowers0/BloodyRoar2Recomp/releases/download/v0.7.1/BloodyRoar2-linux-x64-0.7.1.zip).
Those archives provide the runtime/OpenBIOS but require the user's own disc and
do not supply a retail BIOS or game image. There is no official macOS archive
in that release.

## MCP boundary

The MCP server accepts absolute paths to local assets and starts a local child
process only for the `recomp-mcp` path. It does not fetch a ROM, upload a disc,
or expose an asset path over a network endpoint. `analysis_plan` and
`recomp_build_plan` only serialize a path and commands; they do not read or
execute the target. REA analysis is an explicit local operation selected by the
caller.

The native Rust path and the Recomp path are separate:

- `native-mcp` consumes a locally supplied MAME-compatible `bldyror2` ROM set
  and Sony ZN BIOS assets.
- `recomp-mcp` launches a caller-owned PS1 Recomp executable with a caller-owned
  `.bin`/`.cue`/`.iso`/`.chd` image and controls its localhost debug server.

A Recomp executable is an ELF or PE host program, not a MAME ROM. MAME's
`-rompath` cannot load it. MAME may be run separately, but the agent adapter
launches Recomp directly.
