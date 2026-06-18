use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use bloodyroar2_gym::{
    Action, BloodyRoar2Env, MameConfig, MameRuntime, NativeEmulator, NativeRomSet,
    NativeTraceConfig, NullBackend, ZincConfig, ZincRuntime, action_space_json, api_index_json,
    observation_space_json,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".to_string());

    match command.as_str() {
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        "info" => {
            println!("{}", api_index_json());
            Ok(())
        }
        "action-space" => {
            println!("{}", action_space_json());
            Ok(())
        }
        "observation-space" => {
            println!("{}", observation_space_json());
            Ok(())
        }
        "reset" => {
            let mut env = BloodyRoar2Env::new(NullBackend::default());
            let observation = env.reset().map_err(|error| error.to_string())?;
            println!("{{\"observation\":{},\"info\":{{}}}}", observation.json());
            Ok(())
        }
        "step" => {
            let action_index = args
                .next()
                .unwrap_or_else(|| "0".to_string())
                .parse::<usize>()
                .map_err(|_| "action index must be a non-negative integer".to_string())?;
            let frames = args
                .next()
                .unwrap_or_else(|| "1".to_string())
                .parse::<u32>()
                .map_err(|_| "frames must be a non-negative integer".to_string())?;
            let action = Action::from_index(action_index)
                .ok_or_else(|| "action index is outside the action space".to_string())?;
            let mut env = BloodyRoar2Env::new(NullBackend::default());
            env.reset().map_err(|error| error.to_string())?;
            let step = env
                .step(action, frames)
                .map_err(|error| error.to_string())?;
            println!("{}", step.json());
            Ok(())
        }
        "serve" => {
            let address = args.next().unwrap_or_else(|| "127.0.0.1:8765".to_string());
            bloodyroar2_gym::server::serve(&address).map_err(|error| error.to_string())
        }
        "serve-native" => {
            let address = args.next().unwrap_or_else(|| "127.0.0.1:8765".to_string());
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms/bldyror2.zip"));
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| "10000".to_string())
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            bloodyroar2_gym::server::serve_native(&address, rom, instructions_per_frame)
                .map_err(|error| error.to_string())
        }
        "prepare-assets" => {
            let archive = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym prepare-assets <archive.zip> [rom_dir]".to_string()
            })?;
            let config = mame_config(args.next());
            MameRuntime::new(config)
                .prepare_assets(&archive)
                .map_err(|error| error.to_string())?;
            println!("prepared local ROM assets from {}", archive.display());
            Ok(())
        }
        "mame-check" => {
            let config = mame_config(args.next());
            let report = MameRuntime::new(config)
                .check()
                .map_err(|error| error.to_string())?;
            println!("{}", report.trim());
            Ok(())
        }
        "mame-required" => {
            let config = mame_config(args.next());
            let report = MameRuntime::new(config)
                .required_roms()
                .map_err(|error| error.to_string())?;
            println!("{}", report.trim());
            Ok(())
        }
        "rom-ident" => {
            let config = mame_config(args.next());
            let report = MameRuntime::new(config)
                .identify_roms()
                .map_err(|error| error.to_string())?;
            println!("{}", report.trim());
            Ok(())
        }
        "doctor" => {
            let config = mame_config(args.next());
            println!("{}", MameRuntime::new(config).doctor());
            Ok(())
        }
        "play" => {
            let config = mame_config(args.next());
            let extra_args = args.collect::<Vec<_>>();
            MameRuntime::new(config)
                .play(&extra_args)
                .map_err(|error| error.to_string())
        }
        "prepare-zinc" => {
            let archive = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym prepare-zinc <archive.zip> [extract_dir]".to_string()
            })?;
            let extract_dir = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/extracted"));
            let runtime = ZincRuntime::new(zinc_config(None));
            runtime
                .prepare_bundle(&archive, &extract_dir)
                .map_err(|error| error.to_string())?;
            println!("prepared ZiNc bundle under {}", extract_dir.display());
            Ok(())
        }
        "zinc-check" => {
            let config = zinc_config(args.next());
            println!("{}", ZincRuntime::new(config).check());
            Ok(())
        }
        "zinc-play" => {
            let config = zinc_config(args.next());
            let extra_args = args.collect::<Vec<_>>();
            ZincRuntime::new(config)
                .play(&extra_args)
                .map_err(|error| error.to_string())
        }
        "native-inspect" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms/bldyror2.zip"));
            let romset = NativeRomSet::scan(rom).map_err(|error| error.to_string())?;
            println!("{}", romset.json());
            Ok(())
        }
        "native-rom-summary" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms/bldyror2.zip"));
            let romset = NativeRomSet::scan(rom).map_err(|error| error.to_string())?;
            println!("{}", romset.compatibility_report().summary_json());
            Ok(())
        }
        "native-step" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms/bldyror2.zip"));
            let count = args
                .next()
                .unwrap_or_else(|| "1".to_string())
                .parse::<u64>()
                .map_err(|_| "instruction count must be a non-negative integer".to_string())?;
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            emulator.step_instructions(count);
            println!("{}", emulator.json());
            Ok(())
        }
        "native-screenshot" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms/bldyror2.zip"));
            let count = args
                .next()
                .unwrap_or_else(|| "32000000".to_string())
                .parse::<u64>()
                .map_err(|_| "instruction count must be a non-negative integer".to_string())?;
            let output = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("native-frame.png"));
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            emulator.step_instructions(count);
            std::fs::write(&output, emulator.screenshot_png())
                .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
            println!(
                "{{\"output\":\"{}\",\"state\":{}}}",
                output.display(),
                emulator.json()
            );
            Ok(())
        }
        "native-display-screenshot" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms/bldyror2.zip"));
            let count = args
                .next()
                .unwrap_or_else(|| "32000000".to_string())
                .parse::<u64>()
                .map_err(|_| "instruction count must be a non-negative integer".to_string())?;
            let output = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("native-display.png"));
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            emulator.step_instructions(count);
            std::fs::write(&output, emulator.display_png())
                .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
            println!(
                "{{\"output\":\"{}\",\"state\":{}}}",
                output.display(),
                emulator.json()
            );
            Ok(())
        }
        "native-vram-screenshot" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms/bldyror2.zip"));
            let count = args
                .next()
                .unwrap_or_else(|| "32000000".to_string())
                .parse::<u64>()
                .map_err(|_| "instruction count must be a non-negative integer".to_string())?;
            let output = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("native-vram.png"));
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            emulator.step_instructions(count);
            std::fs::write(&output, emulator.vram_png())
                .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
            println!(
                "{{\"output\":\"{}\",\"state\":{}}}",
                output.display(),
                emulator.json()
            );
            Ok(())
        }
        "native-scripted-step" => {
            let rom = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-step <rom_zip_or_dir> <instructions_per_frame> <output.png> <action:frames>..."
                    .to_string()
            })?;
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-step <rom_zip_or_dir> <instructions_per_frame> <output.png> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let output = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-step <rom_zip_or_dir> <instructions_per_frame> <output.png> <action:frames>..."
                    .to_string()
            })?;
            let raw_segments = args.collect::<Vec<_>>();
            let segments = parse_native_script_segments(raw_segments)?;
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let mut total_frames = 0u64;

            for segment in &segments {
                emulator.set_input(segment.action.buttons());
                for _ in 0..segment.frames {
                    emulator.step_instructions(instructions_per_frame);
                    total_frames += 1;
                    if emulator.is_terminal() {
                        break;
                    }
                }
                if emulator.is_terminal() {
                    break;
                }
            }

            std::fs::write(&output, emulator.screenshot_png())
                .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
            println!(
                "{{\"output\":\"{}\",\"instructions_per_frame\":{},\"total_frames\":{},\"executed_steps\":{},\"segments\":[{}],\"state\":{}}}",
                escape_json(&output.display().to_string()),
                instructions_per_frame,
                total_frames,
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                emulator.json()
            );
            Ok(())
        }
        "native-trace" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms/bldyror2.zip"));
            let count = args
                .next()
                .unwrap_or_else(|| "1000000".to_string())
                .parse::<u64>()
                .map_err(|_| "instruction count must be a non-negative integer".to_string())?;
            let hot_limit = args
                .next()
                .unwrap_or_else(|| "12".to_string())
                .parse::<usize>()
                .map_err(|_| "hot_limit must be a non-negative integer".to_string())?;
            let recent_limit = args
                .next()
                .unwrap_or_else(|| "24".to_string())
                .parse::<usize>()
                .map_err(|_| "recent_limit must be a non-negative integer".to_string())?;
            let trace_options = parse_native_trace_options(args.collect::<Vec<_>>())?;
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            let trace = emulator.trace_instructions(count, hot_limit, recent_limit, trace_options);
            println!("{}", trace.json());
            Ok(())
        }
        "native-env-step" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms/bldyror2.zip"));
            let action_index = args
                .next()
                .unwrap_or_else(|| "0".to_string())
                .parse::<usize>()
                .map_err(|_| "action index must be a non-negative integer".to_string())?;
            let frames = args
                .next()
                .unwrap_or_else(|| "1".to_string())
                .parse::<u32>()
                .map_err(|_| "frames must be a non-negative integer".to_string())?;
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| "10000".to_string())
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let action = Action::from_index(action_index)
                .ok_or_else(|| "action index is outside the action space".to_string())?;
            let backend = bloodyroar2_gym::NativeBackend::from_rom_zip(rom, instructions_per_frame)
                .map_err(|error| error.to_string())?;
            let mut env = BloodyRoar2Env::new(backend);
            env.reset().map_err(|error| error.to_string())?;
            let step = env
                .step(action, frames)
                .map_err(|error| error.to_string())?;
            println!("{}", step.json());
            Ok(())
        }
        "asset-check" => {
            let path = args
                .next()
                .ok_or_else(|| "usage: bloodyroar2-gym asset-check <path>".to_string())?;
            asset_check(&path)
        }
        _ => Err(format!("unknown command: {command}")),
    }
}

fn print_help() {
    println!(
        "bloodyroar2-gym\n\nCommands:\n  info\n  action-space\n  observation-space\n  reset\n  step <action_index> [frames]\n  serve [address]\n  serve-native [address] [rom_zip] [instructions_per_frame]\n  prepare-assets <archive.zip> [rom_dir]\n  mame-required [rom_dir]\n  rom-ident [rom_dir]\n  mame-check [rom_dir]\n  doctor [rom_dir]\n  play [rom_dir] [extra_mame_args...]\n  prepare-zinc <archive.zip> [extract_dir]\n  zinc-check [bundle_dir]\n  zinc-play [bundle_dir] [extra_zinc_args...]\n  native-inspect [rom_zip_or_dir]\n  native-rom-summary [rom_zip_or_dir]\n  native-step [rom_zip] [instruction_count]\n  native-screenshot [rom_zip] [instruction_count] [output.png]\n  native-display-screenshot [rom_zip] [instruction_count] [output.png]\n  native-vram-screenshot [rom_zip] [instruction_count] [output.png]\n  native-scripted-step <rom_zip_or_dir> <instructions_per_frame> <output.png> <action:frames>...\n  native-trace [rom_zip] [instruction_count] [hot_limit] [recent_limit] [stop_pc] [stop_below_pc] [--watch address [len]] [--watch-only]\n  native-env-step [rom_zip] [action_index] [frames] [instructions_per_frame]\n  asset-check <path>\n\nThis project never ships ROMs, BIOS files, Windows EXEs, or DLLs. Configure legally obtained assets outside Git."
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeScriptSegment {
    action: Action,
    frames: u64,
}

fn parse_native_script_segments(values: Vec<String>) -> Result<Vec<NativeScriptSegment>, String> {
    if values.is_empty() {
        return Err(
            "native-scripted-step requires at least one <action:frames> segment".to_string(),
        );
    }

    values
        .into_iter()
        .map(|value| parse_native_script_segment(&value))
        .collect()
}

fn parse_native_script_segment(value: &str) -> Result<NativeScriptSegment, String> {
    let (raw_action, raw_frames) = value
        .split_once(':')
        .ok_or_else(|| format!("script segment must use <action:frames>: {value}"))?;
    let action = parse_action_token(raw_action)?;
    let frames = raw_frames
        .parse::<u64>()
        .map_err(|_| format!("script segment frames must be a positive integer: {value}"))?;
    if frames == 0 {
        return Err(format!(
            "script segment frames must be greater than zero: {value}"
        ));
    }
    Ok(NativeScriptSegment { action, frames })
}

fn parse_action_token(value: &str) -> Result<Action, String> {
    if let Ok(index) = value.parse::<usize>() {
        return Action::from_index(index)
            .ok_or_else(|| format!("action index is outside the action space: {index}"));
    }
    Action::from_name(value).ok_or_else(|| format!("unknown action token: {value}"))
}

fn native_script_segments_json(segments: &[NativeScriptSegment]) -> String {
    segments
        .iter()
        .map(|segment| {
            format!(
                "{{\"action_index\":{},\"action\":\"{}\",\"frames\":{}}}",
                segment.action.index(),
                segment.action.name(),
                segment.frames
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_native_trace_options(values: Vec<String>) -> Result<NativeTraceConfig, String> {
    let mut options = NativeTraceConfig::default();
    let mut positional = Vec::new();
    let mut args = values.into_iter().peekable();

    while let Some(value) = args.next() {
        match value.as_str() {
            "--stop-pc" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "--stop-pc requires an address".to_string())?;
                options.stop_pc = Some(parse_trace_u32(&raw, "--stop-pc")?);
            }
            "--stop-below-pc" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "--stop-below-pc requires an address".to_string())?;
                options.stop_below_pc = Some(parse_trace_u32(&raw, "--stop-below-pc")?);
            }
            "--watch" => {
                let raw_address = args
                    .next()
                    .ok_or_else(|| "--watch requires an address".to_string())?;
                let address = parse_trace_u32(&raw_address, "--watch address")?;
                let len = if args.peek().is_some_and(|next| !next.starts_with("--")) {
                    let raw_len = args.next().expect("peeked watch length");
                    parse_watch_len(&raw_len)?
                } else {
                    4
                };
                options.watch_ranges.push((address, len));
            }
            "--watch-only" => {
                options.watch_only = true;
            }
            _ if value.starts_with("--") => {
                return Err(format!("unknown native-trace option: {value}"));
            }
            _ => positional.push(value),
        }
    }

    if let Some(raw) = positional.first() {
        options.stop_pc = Some(parse_trace_u32(raw, "stop_pc")?);
    }
    if let Some(raw) = positional.get(1) {
        options.stop_below_pc = Some(parse_trace_u32(raw, "stop_below_pc")?);
    }
    if positional.len() > 2 {
        return Err(
            "native-trace accepts at most two positional trace options; use --watch for memory watches"
                .to_string(),
        );
    }

    Ok(options)
}

fn parse_trace_u32(value: &str, label: &str) -> Result<u32, String> {
    parse_u32_auto(value)
        .map_err(|_| format!("{label} must be a u32 integer, optionally prefixed with 0x"))
}

fn parse_watch_len(value: &str) -> Result<u32, String> {
    let len = parse_trace_u32(value, "--watch length")?;
    if len == 0 {
        return Err("--watch length must be greater than zero".to_string());
    }
    Ok(len)
}

fn parse_u32_auto(value: &str) -> Result<u32, std::num::ParseIntError> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u32::from_str_radix(hex, 16)
    } else {
        value.parse::<u32>()
    }
}

fn mame_config(rom_dir: Option<String>) -> MameConfig {
    let executable = env::var_os("BLOODYROAR2_MAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("mame"));
    let rom_dir = rom_dir
        .map(PathBuf::from)
        .or_else(|| env::var_os("BLOODYROAR2_ROM_DIR").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("assets/roms"));

    MameConfig {
        executable,
        rom_dir,
        game: env::var("BLOODYROAR2_MAME_GAME").unwrap_or_else(|_| "bldyror2".to_string()),
    }
}

fn zinc_config(bundle_dir: Option<String>) -> ZincConfig {
    let mut config = ZincConfig::default();
    if let Some(wine) = env::var_os("BLOODYROAR2_WINE") {
        config.wine = PathBuf::from(wine);
    }
    if let Some(bundle_dir) = bundle_dir {
        config.bundle_dir = PathBuf::from(bundle_dir);
    } else if let Some(bundle_dir) = env::var_os("BLOODYROAR2_ZINC_DIR") {
        config.bundle_dir = PathBuf::from(bundle_dir);
    }
    if let Ok(renderer) = env::var("BLOODYROAR2_ZINC_RENDERER") {
        config.renderer = renderer;
    }
    if let Ok(renderer_cfg) = env::var("BLOODYROAR2_ZINC_RENDERER_CFG") {
        config.renderer_cfg = renderer_cfg;
    }
    config
}

fn asset_check(path: &str) -> Result<(), String> {
    let metadata = std::fs::metadata(path).map_err(|error| format!("{path}: {error}"))?;
    if !metadata.is_file() {
        return Err(format!("{path}: expected a file"));
    }

    let lowercase = path.to_ascii_lowercase();
    let risky_extension = [".zip", ".bin", ".cue", ".iso", ".chd", ".exe", ".dll"]
        .iter()
        .any(|extension| lowercase.ends_with(extension));

    println!(
        "{{\"path\":\"{}\",\"size_bytes\":{},\"git_policy\":\"keep outside repository\",\"requires_legal_source\":{},\"note\":\"This tool does not validate ownership. Use only assets you are legally allowed to use.\"}}",
        path.replace('"', "'"),
        metadata.len(),
        risky_extension
    );
    Ok(())
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::{NativeScriptSegment, parse_action_token, parse_native_script_segments};
    use bloodyroar2_gym::Action;

    #[test]
    fn parses_native_script_segments_by_index_and_name() {
        let segments =
            parse_native_script_segments(vec!["17:3".to_string(), "coin+start:5".to_string()])
                .expect("script parses");

        assert_eq!(
            segments,
            vec![
                NativeScriptSegment {
                    action: Action::Coin,
                    frames: 3
                },
                NativeScriptSegment {
                    action: Action::CoinStart,
                    frames: 5
                }
            ]
        );
    }

    #[test]
    fn rejects_invalid_script_segments() {
        assert!(parse_native_script_segments(Vec::new()).is_err());
        assert!(parse_native_script_segments(vec!["coin".to_string()]).is_err());
        assert!(parse_native_script_segments(vec!["coin:0".to_string()]).is_err());
        assert!(parse_action_token("999").is_err());
    }
}
