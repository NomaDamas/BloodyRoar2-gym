//! Provenance and reverse-engineering handoff metadata.
//!
//! The recompiled source repository and REA are useful inputs to the project,
//! but neither one is a game-data distribution mechanism.  This module keeps
//! the relationship explicit and gives humans and agents the same, portable
//! analysis plan without reading or executing a proprietary file.

use std::path::Path;

use serde_json::{Value, json};

pub const RECOMP_REPOSITORY: &str = "https://github.com/novapowers0/BloodyRoar2Recomp";
pub const RECOMP_REVISION: &str = "eed1e8d123d903d24ec0adf49ebee71a16c5892c";
pub const REA_REPOSITORY: &str = "https://github.com/morluto/rea";
pub const REA_REVISION: &str = "a1dec2d2d502ebb56da8902ccde9bf3dd9dfaf23";

fn source_repositories() -> Value {
    json!([
        {
            "name": "bloodyroar2-recomp",
            "url": RECOMP_REPOSITORY,
            "revision": RECOMP_REVISION,
            "role": "PSX recompiler metadata, region configs, symbols, and seeds",
            "generated_game_code": "caller-generated from a legally owned disc; never committed"
        },
        {
            "name": "rea",
            "url": REA_REPOSITORY,
            "revision": REA_REVISION,
            "role": "local binary inspection and evidence-producing MCP/CLI workflows",
            "asset_distribution": "REA does not provide ROMs, BIOS files, or game binaries"
        },
        {
            "name": "bloodyroar2-gym",
            "url": "https://github.com/NomaDamas/BloodyRoar2-gym",
            "role": "portable Rust emulation, Gym/HTTP/MCP control surfaces"
        }
    ])
}

/// Return the machine-readable provenance contract for this project.
pub fn analysis_manifest() -> Value {
    json!({
        "schema": "bloodyroar2.analysis/v1",
        "title": "Bloody Roar 2 portable analysis handoff",
        "source_repositories": source_repositories(),
        "official_recomp_release": {
            "url": "https://github.com/novapowers0/BloodyRoar2Recomp/releases/latest",
            "known_version": "v0.7.1",
            "assets": [
                {
                    "platform": "windows-x86_64",
                    "url": "https://github.com/novapowers0/BloodyRoar2Recomp/releases/download/v0.7.1/BloodyRoar2-v0.7.1.zip",
                    "format": "PE/Windows executable archive"
                },
                {
                    "platform": "linux-x86_64",
                    "url": "https://github.com/novapowers0/BloodyRoar2Recomp/releases/download/v0.7.1/BloodyRoar2-linux-x64-0.7.1.zip",
                    "format": "ELF/Linux executable archive"
                }
            ],
            "includes": ["compiled recompiler runtime", "OpenBIOS"],
            "excludes": ["retail disc image", "retail BIOS", "user game assets"],
            "macos_archive": false,
            "mame_rom": false,
            "mame_note": "MAME -rompath accepts driver ROM sets, not host ELF/PE Recomp executables"
        },
        "regions": [
            {"name": "Europe", "serial": "SLES-01722", "status": "supported"},
            {"name": "USA", "serial": "SCUS-94424", "status": "supported"},
            {"name": "Japan/Asia", "serial": "SLPS-01842", "status": "experimental"}
        ],
        "inputs": {
            "legal_disc_image": {
                "required_for_recompilation": true,
                "accepted_forms": ["bin/cue", "iso", "chd"],
                "tracked_in_github": false,
                "reason": "copyrighted game data must remain caller-supplied"
            },
            "recomp_config": {
                "tracked": true,
                "files": ["game.toml", "game_us.toml", "game_japan.toml", "symbols.toml"]
            },
            "function_seeds": {
                "tracked": true,
                "files": ["seeds/ghidra_funcs.txt", "seeds/ghidra_funcs_us.txt", "seeds/ghidra_funcs_japan.txt"]
            },
            "native_rom_set": {
                "required_for_native_backend": true,
                "tracked_in_github": false,
                "verification": "native ROM manifest checks size, CRC32, SHA-1, duplicates, and known dump variants"
            }
        },
        "rea_workflow": {
            "provider": "caller-selected REA installation",
            "static_operations": ["file_headers", "section_headers", "symbols", "disassemble", "dwarf"],
            "recommended_cli": [
                "rea inspect-with-objdump <binary> file_headers --json",
                "rea inspect-with-objdump <binary> section_headers --json",
                "rea inspect-with-objdump <binary> symbols --json",
                "rea inspect-with-objdump <binary> disassemble --json",
                "rea execute-rizin-command <binary> iI --json"
            ],
            "evidence_policy": "keep JSON evidence outside tracked source and record its digest in issue/PR notes"
        },
        "outputs": {
            "portable_runtime": "Rust binary built for the host target",
            "agent_control": ["HTTP JSON API", "stdio MCP", "stdio MCP over caller-owned Recomp"],
            "analysis_evidence": "caller-owned file or artifact; not game data",
            "generated_recompilation": "caller-owned build directory; not tracked"
        },
        "recomp_runtime": {
            "adapter": "RecompBackend",
            "launch_command": "bloodyroar2-gym recomp-mcp <recompiled_executable> <disc_image> [debug_port]",
            "debug_server": "JSON-over-newline, one request per localhost TCP connection",
            "default_port": 4370,
            "disc_argument": "caller-supplied .bin/.cue/.iso/.chd",
            "generated_code": "must be generated/built by the caller and remains outside this repository",
            "p1_controls": "mapped to active-low PS1 set_input word",
            "p2_controls": "rejected because upstream set_input exposes pad1 only",
            "screenshots": "temporary caller-host PNG via screenshot command"
        },
        "limitations": [
            "REA can inspect a caller-supplied executable but cannot make proprietary ROMs redistributable.",
            "Deep native decompilation depends on a provider installed on the host.",
            "The portable Rust backend and the PSX recompiled executable are separate runtime paths; they share provenance and control contracts, not generated game code."
        ]
    })
}

/// Return a target-specific plan without opening the target.  This is safe to
/// expose through MCP because it only serializes the path and suggested REA
/// invocations; the caller chooses when and where a binary is analyzed.
pub fn analysis_plan(target: Option<&Path>) -> Value {
    let target = target.map(|path| path.display().to_string());
    let target_value = target.clone().map_or(Value::Null, Value::String);
    let placeholder = target.as_deref().unwrap_or("<binary>");
    json!({
        "schema": "bloodyroar2.analysis-plan/v1",
        "target": target_value,
        "read_only": true,
        "commands": [
            {
                "tool": "rea",
                "operation": "inspect-with-objdump",
                "args": [placeholder, "file_headers", "--json"],
                "purpose": "identify executable format and architecture"
            },
            {
                "tool": "rea",
                "operation": "inspect-with-objdump",
                "args": [placeholder, "section_headers", "--json"],
                "purpose": "map code/data sections before symbol or disassembly work"
            },
            {
                "tool": "rea",
                "operation": "inspect-with-objdump",
                "args": [placeholder, "symbols", "--json"],
                "purpose": "retain symbols when the caller-supplied build has them"
            },
            {
                "tool": "rea",
                "operation": "inspect-with-objdump",
                "args": [placeholder, "disassemble", "--json"],
                "purpose": "produce bounded evidence for native instruction review"
            },
            {
                "tool": "rea",
                "operation": "execute-rizin-command",
                "args": [placeholder, "iI", "--json"],
                "purpose": "cross-check format/architecture with a second local decoder"
            }
        ],
        "next_step": "Feed the resulting evidence to the agent, then update a tracked symbol/config file or Rust module with a reviewed change."
    })
}

pub fn analysis_manifest_json() -> String {
    analysis_manifest().to_string()
}

pub fn analysis_plan_json(target: Option<&Path>) -> String {
    analysis_plan(target).to_string()
}

#[cfg(test)]
mod tests {
    use super::{REA_REVISION, RECOMP_REVISION, analysis_manifest, analysis_plan};
    use serde_json::Value;
    use std::path::Path;

    #[test]
    fn manifest_keeps_recomp_and_rea_provenance_pinned() {
        let manifest = analysis_manifest();
        assert_eq!(
            manifest["source_repositories"][0]["revision"],
            RECOMP_REVISION
        );
        assert_eq!(manifest["source_repositories"][1]["revision"], REA_REVISION);
        assert_eq!(
            manifest["inputs"]["legal_disc_image"]["tracked_in_github"],
            false
        );
        assert_eq!(manifest["recomp_runtime"]["default_port"], 4370);
        assert_eq!(
            manifest["official_recomp_release"]["assets"][0]["platform"],
            "windows-x86_64"
        );
        assert_eq!(manifest["official_recomp_release"]["mame_rom"], false);
    }

    #[test]
    fn target_plan_is_read_only_and_serializes_paths() {
        let plan = analysis_plan(Some(Path::new("/tmp/Bloody Roar 2.bin")));
        assert_eq!(plan["read_only"], true);
        assert_eq!(plan["target"], "/tmp/Bloody Roar 2.bin");
        assert_eq!(plan["commands"][0]["args"][0], "/tmp/Bloody Roar 2.bin");
        assert!(
            plan["commands"]
                .as_array()
                .is_some_and(|commands| commands.len() >= 4)
        );
        let _: Value = serde_json::from_str(&plan.to_string()).expect("valid JSON");
    }
}
