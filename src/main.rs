use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use bloodyroar2_gym::{
    Action, BloodyRoar2Env, MameConfig, MameRuntime, NativeEmulator, NativeRomSet, NullBackend,
    ZincConfig, ZincRuntime, action_space_json, api_index_json, observation_space_json,
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
            let romset = NativeRomSet::inspect(rom).map_err(|error| error.to_string())?;
            println!("{}", romset.json());
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
        "bloodyroar2-gym\n\nCommands:\n  info\n  action-space\n  observation-space\n  reset\n  step <action_index> [frames]\n  serve [address]\n  prepare-assets <archive.zip> [rom_dir]\n  mame-required [rom_dir]\n  rom-ident [rom_dir]\n  mame-check [rom_dir]\n  doctor [rom_dir]\n  play [rom_dir] [extra_mame_args...]\n  prepare-zinc <archive.zip> [extract_dir]\n  zinc-check [bundle_dir]\n  zinc-play [bundle_dir] [extra_zinc_args...]\n  native-inspect [rom_zip]\n  native-step [rom_zip] [instruction_count]\n  asset-check <path>\n\nThis project never ships ROMs, BIOS files, Windows EXEs, or DLLs. Configure legally obtained assets outside Git."
    );
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
