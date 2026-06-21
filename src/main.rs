use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use bloodyroar2_gym::{
    ACTION_SPACE, Action, BloodyRoar2Env, MameConfig, MameRuntime, NativeDisplayFrame,
    NativeEmulator, NativeInputActivity, NativeRomSet, NativeTraceConfig, NullBackend, ZincConfig,
    ZincRuntime, action_space_json, api_index_json, observation_space_json,
};
use minifb::{Key, Scale, Window, WindowOptions};

const NATIVE_PLAY_MIN_WINDOW_WIDTH: usize = 512;
const NATIVE_PLAY_MIN_WINDOW_HEIGHT: usize = 480;

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
        "native-screen-dump" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms/bldyror2.zip"));
            let count = args
                .next()
                .unwrap_or_else(|| "32000000".to_string())
                .parse::<u64>()
                .map_err(|_| "instruction count must be a non-negative integer".to_string())?;
            let output_prefix = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("native-screen"));
            let actual_display_output = suffixed_path(&output_prefix, "actual-display.png");
            let raw_actual_display_output = suffixed_path(&output_prefix, "raw-actual-display.png");
            let display_output = suffixed_path(&output_prefix, "display.png");
            let observation_output = suffixed_path(&output_prefix, "observation.png");
            let vram_output = suffixed_path(&output_prefix, "vram.png");
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            emulator.step_instructions(count);
            std::fs::write(&actual_display_output, emulator.actual_display_png()).map_err(
                |error| {
                    format!(
                        "failed to write {}: {error}",
                        actual_display_output.display()
                    )
                },
            )?;
            std::fs::write(
                &raw_actual_display_output,
                emulator.raw_actual_display_png(),
            )
            .map_err(|error| {
                format!(
                    "failed to write {}: {error}",
                    raw_actual_display_output.display()
                )
            })?;
            std::fs::write(&display_output, emulator.display_png()).map_err(|error| {
                format!("failed to write {}: {error}", display_output.display())
            })?;
            std::fs::write(&observation_output, emulator.screenshot_png()).map_err(|error| {
                format!("failed to write {}: {error}", observation_output.display())
            })?;
            std::fs::write(&vram_output, emulator.vram_png())
                .map_err(|error| format!("failed to write {}: {error}", vram_output.display()))?;
            println!(
                "{{\"actual_display_output\":\"{}\",\"raw_actual_display_output\":\"{}\",\"display_output\":\"{}\",\"observation_output\":\"{}\",\"vram_output\":\"{}\",\"state\":{}}}",
                escape_json(&actual_display_output.display().to_string()),
                escape_json(&raw_actual_display_output.display().to_string()),
                escape_json(&display_output.display().to_string()),
                escape_json(&observation_output.display().to_string()),
                escape_json(&vram_output.display().to_string()),
                emulator.json()
            );
            Ok(())
        }
        "native-play" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms"));
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| "500000".to_string())
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let scale = parse_native_window_scale(args.next())?;
            let max_frames = args
                .next()
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "max_frames must be a positive integer".to_string())
                })
                .transpose()?;
            run_native_play(
                rom,
                instructions_per_frame.max(1),
                scale,
                max_frames,
                default_native_play_script(),
            )
        }
        "native-manual" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms"));
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| "500000".to_string())
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let scale = parse_native_window_scale(args.next())?;
            let max_frames = args
                .next()
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "max_frames must be a positive integer".to_string())
                })
                .transpose()?;
            run_native_play(
                rom,
                instructions_per_frame.max(1),
                scale,
                max_frames,
                Vec::new(),
            )
        }
        "native-autoplay" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms"));
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| "500000".to_string())
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let scale = parse_native_window_scale(args.next())?;
            let (max_frames, segments) = parse_native_autoplay_tail(args.collect::<Vec<_>>())?;
            run_native_play(
                rom,
                instructions_per_frame.max(1),
                scale,
                max_frames,
                segments,
            )
        }
        "native-input-check" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms"));
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| "500000".to_string())
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            let segments = default_native_play_script();
            let instructions_per_frame = instructions_per_frame.max(1);
            let mut total_frames = 0u64;
            let mut observed_native_playable_candidate = false;
            let mut first_native_playable_frame = None;
            let mut last_native_playable_frame = None;
            for segment in &segments {
                emulator.set_input(segment.action.buttons());
                for _ in 0..segment.frames {
                    emulator.step_until_next_vblank(instructions_per_frame);
                    total_frames += 1;
                    if emulator.native_playable_candidate() {
                        observed_native_playable_candidate = true;
                        first_native_playable_frame.get_or_insert(total_frames);
                        last_native_playable_frame = Some(total_frames);
                    }
                    if emulator.is_terminal() {
                        break;
                    }
                }
                if emulator.is_terminal() {
                    break;
                }
            }
            let checkpoint = emulator.clone();
            let mut control_sweep = checkpoint.clone();
            let mut control_sweep_frames = 0u64;
            if observed_native_playable_candidate && !emulator.is_terminal() {
                let control_sweep_segments = native_control_sweep_script(18, 18);
                for segment in &control_sweep_segments {
                    control_sweep.set_input(segment.action.buttons());
                    for _ in 0..segment.frames {
                        control_sweep.step_until_next_vblank(instructions_per_frame);
                        control_sweep_frames += 1;
                        let sweep_total_frames = total_frames + control_sweep_frames;
                        if control_sweep.native_playable_candidate() {
                            first_native_playable_frame.get_or_insert(sweep_total_frames);
                            last_native_playable_frame = Some(sweep_total_frames);
                        }
                        if control_sweep.is_terminal() {
                            break;
                        }
                    }
                    if control_sweep.is_terminal() {
                        break;
                    }
                }
            }
            let total_frames_with_sweep = total_frames + control_sweep_frames;
            let input_activity = control_sweep.input_activity();
            let final_native_playable_candidate = checkpoint.native_playable_candidate();
            let control_sweep_native_playable_candidate = control_sweep.native_playable_candidate();
            let input_controls_active = input_activity.has_play_control_activity();
            let full_controls_active = input_activity.has_full_control_activity();
            let playable = observed_native_playable_candidate && full_controls_active;
            let first_native_playable_frame = optional_u64_json(first_native_playable_frame);
            let last_native_playable_frame = optional_u64_json(last_native_playable_frame);
            println!(
                "{{\"instructions_per_frame\":{},\"total_frames\":{},\"control_sweep_frames\":{},\"checkpoint_executed_steps\":{},\"control_sweep_executed_steps\":{},\"executed_steps\":{},\"input_activity\":{},\"native_playable_candidate\":{},\"observed_native_playable_candidate\":{},\"first_native_playable_frame\":{},\"last_native_playable_frame\":{},\"final_native_playable_candidate\":{},\"control_sweep_native_playable_candidate\":{},\"input_controls_active\":{},\"full_controls_active\":{},\"playable\":{},\"state\":{}}}",
                instructions_per_frame,
                total_frames_with_sweep,
                control_sweep_frames,
                checkpoint.executed_steps(),
                control_sweep.executed_steps(),
                checkpoint.executed_steps(),
                input_activity.json(),
                final_native_playable_candidate,
                observed_native_playable_candidate,
                first_native_playable_frame,
                last_native_playable_frame,
                final_native_playable_candidate,
                control_sweep_native_playable_candidate,
                input_controls_active,
                full_controls_active,
                playable,
                checkpoint.probe_json()
            );
            if playable {
                Ok(())
            } else {
                Err(
                    "native input check failed: native playability or mapped controls were not observed"
                        .into(),
                )
            }
        }
        "native-health-check" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms"));
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| "500000".to_string())
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let branch_frames = args
                .next()
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "branch_frames must be a positive integer".to_string())
                })
                .transpose()?
                .unwrap_or(18);
            let settle_frames = args
                .next()
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "settle_frames must be a non-negative integer".to_string())
                })
                .transpose()?
                .unwrap_or(18);
            run_native_health_check(
                rom,
                instructions_per_frame.max(1),
                branch_frames.max(1),
                settle_frames,
            )
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
                    emulator.step_until_next_vblank(instructions_per_frame);
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
        "native-scripted-dump" => {
            let rom = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-dump <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>..."
                    .to_string()
            })?;
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-dump <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let output_prefix = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-dump <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>..."
                    .to_string()
            })?;
            let raw_segments = args.collect::<Vec<_>>();
            let segments = parse_native_script_segments(raw_segments)?;
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let total_frames = run_native_script(&mut emulator, instructions_per_frame, &segments);
            let (
                actual_display_output,
                raw_actual_display_output,
                display_output,
                observation_output,
                vram_output,
            ) = write_native_snapshot(&emulator, &output_prefix)?;
            println!(
                "{{\"actual_display_output\":\"{}\",\"raw_actual_display_output\":\"{}\",\"display_output\":\"{}\",\"observation_output\":\"{}\",\"vram_output\":\"{}\",\"instructions_per_frame\":{},\"total_frames\":{},\"executed_steps\":{},\"segments\":[{}],\"state\":{}}}",
                escape_json(&actual_display_output.display().to_string()),
                escape_json(&raw_actual_display_output.display().to_string()),
                escape_json(&display_output.display().to_string()),
                escape_json(&observation_output.display().to_string()),
                escape_json(&vram_output.display().to_string()),
                instructions_per_frame,
                total_frames,
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                emulator.json()
            );
            Ok(())
        }
        "native-scripted-candidates" => {
            let rom = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-candidates <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>..."
                    .to_string()
            })?;
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-candidates <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let output_prefix = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-candidates <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>..."
                    .to_string()
            })?;
            let raw_segments = args.collect::<Vec<_>>();
            let segments = parse_native_script_segments(raw_segments)?;
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let total_frames = run_native_script(&mut emulator, instructions_per_frame, &segments);
            let candidates = write_native_display_candidates(&emulator, &output_prefix)?;
            println!(
                "{{\"candidate_outputs\":[{}],\"instructions_per_frame\":{},\"total_frames\":{},\"executed_steps\":{},\"segments\":[{}],\"state\":{}}}",
                candidates.join(","),
                instructions_per_frame,
                total_frames,
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                emulator.probe_json()
            );
            Ok(())
        }
        "native-scripted-summary" => {
            let rom = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-summary <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
                    .to_string()
            })?;
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-summary <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let raw_segments = args.collect::<Vec<_>>();
            let segments = parse_native_script_segments(raw_segments)?;
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let total_frames = run_native_script(&mut emulator, instructions_per_frame, &segments);
            println!(
                "{{\"instructions_per_frame\":{},\"total_frames\":{},\"executed_steps\":{},\"segments\":[{}],\"state\":{}}}",
                instructions_per_frame,
                total_frames,
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                emulator.diagnostic_json()
            );
            Ok(())
        }
        "native-scripted-probe" => {
            let rom = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-probe <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
                    .to_string()
            })?;
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-probe <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let raw_segments = args.collect::<Vec<_>>();
            let segments = parse_native_script_segments(raw_segments)?;
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let mut total_frames = 0u64;
            let mut probes = Vec::new();

            for (index, segment) in segments.iter().enumerate() {
                emulator.set_input(segment.action.buttons());
                for _ in 0..segment.frames {
                    emulator.step_until_next_vblank(instructions_per_frame);
                    total_frames += 1;
                    if emulator.is_terminal() {
                        break;
                    }
                }

                probes.push(format!(
                    "{{\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"segment_frames\":{},\"total_frames\":{},\"executed_steps\":{},\"state\":{}}}",
                    index,
                    segment.action.index(),
                    segment.action.name(),
                    segment.frames,
                    total_frames,
                    emulator.executed_steps(),
                    emulator.probe_json()
                ));

                if emulator.is_terminal() {
                    break;
                }
            }

            println!(
                "{{\"instructions_per_frame\":{},\"total_frames\":{},\"executed_steps\":{},\"segments\":[{}],\"probes\":[{}],\"state\":{}}}",
                instructions_per_frame,
                total_frames,
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                probes.join(","),
                emulator.probe_json()
            );
            Ok(())
        }
        "native-scripted-frame-probe" => {
            let rom = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-frame-probe <rom_zip_or_dir> <instructions_per_frame> <probe_stride_frames> <action:frames>..."
                    .to_string()
            })?;
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-frame-probe <rom_zip_or_dir> <instructions_per_frame> <probe_stride_frames> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let probe_stride = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-frame-probe <rom_zip_or_dir> <instructions_per_frame> <probe_stride_frames> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "probe_stride_frames must be a positive integer".to_string())?;
            if probe_stride == 0 {
                return Err("probe_stride_frames must be greater than zero".to_string());
            }
            let raw_segments = args.collect::<Vec<_>>();
            let segments = parse_native_script_segments(raw_segments)?;
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let mut total_frames = 0u64;
            let mut probes = Vec::new();

            for (segment_index, segment) in segments.iter().enumerate() {
                emulator.set_input(segment.action.buttons());
                for frame_in_segment in 1..=segment.frames {
                    emulator.step_until_next_vblank(instructions_per_frame);
                    total_frames += 1;
                    if frame_in_segment % probe_stride == 0
                        || frame_in_segment == segment.frames
                        || emulator.is_terminal()
                    {
                        probes.push(format!(
                            "{{\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"frame_in_segment\":{},\"segment_frames\":{},\"total_frames\":{},\"executed_steps\":{},\"state\":{}}}",
                            segment_index,
                            segment.action.index(),
                            segment.action.name(),
                            frame_in_segment,
                            segment.frames,
                            total_frames,
                            emulator.executed_steps(),
                            emulator.probe_json()
                        ));
                    }
                    if emulator.is_terminal() {
                        break;
                    }
                }
                if emulator.is_terminal() {
                    break;
                }
            }

            println!(
                "{{\"instructions_per_frame\":{},\"probe_stride_frames\":{},\"total_frames\":{},\"executed_steps\":{},\"segments\":[{}],\"probes\":[{}],\"state\":{}}}",
                instructions_per_frame,
                probe_stride,
                total_frames,
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                probes.join(","),
                emulator.probe_json()
            );
            Ok(())
        }
        "native-scripted-trace" => {
            let rom = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <action:frames>... [-- <trace options>]"
                    .to_string()
            })?;
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <action:frames>... [-- <trace options>]"
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let hot_limit = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <action:frames>... [-- <trace options>]"
                        .to_string()
                })?
                .parse::<usize>()
                .map_err(|_| "hot_limit must be a non-negative integer".to_string())?;
            let recent_limit = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <action:frames>... [-- <trace options>]"
                        .to_string()
                })?
                .parse::<usize>()
                .map_err(|_| "recent_limit must be a non-negative integer".to_string())?;
            let raw_args = args.collect::<Vec<_>>();
            let split_at = raw_args
                .iter()
                .position(|value| value == "--")
                .unwrap_or(raw_args.len());
            let raw_segments = raw_args[..split_at].to_vec();
            let raw_trace_options = if split_at < raw_args.len() {
                raw_args[split_at + 1..].to_vec()
            } else {
                Vec::new()
            };
            let (warmup_segments, segments) = parse_native_script_trace_segments(raw_segments)?;
            let trace_options = parse_native_trace_options(raw_trace_options)?;
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            let warmup_frames = run_native_script(
                &mut emulator,
                instructions_per_frame.max(1),
                &warmup_segments,
            );
            let trace_segments = segments
                .iter()
                .map(|segment| (segment.action.buttons(), segment.frames))
                .collect::<Vec<_>>();
            let trace = emulator.trace_scripted_frames(
                instructions_per_frame,
                &trace_segments,
                hot_limit,
                recent_limit,
                trace_options,
            );
            println!(
                "{{\"instructions_per_frame\":{},\"warmup_frames\":{},\"warmup_segments\":[{}],\"segments\":[{}],\"trace\":{}}}",
                instructions_per_frame.max(1),
                warmup_frames,
                native_script_segments_json(&warmup_segments),
                native_script_segments_json(&segments),
                trace.json()
            );
            Ok(())
        }
        "native-scripted-timeline" => {
            let rom = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-timeline <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>..."
                    .to_string()
            })?;
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-timeline <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let output_prefix = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-timeline <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>..."
                    .to_string()
            })?;
            let raw_segments = args.collect::<Vec<_>>();
            let segments = parse_native_script_segments(raw_segments)?;
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let mut total_frames = 0u64;
            let mut snapshots = Vec::new();

            for (index, segment) in segments.iter().enumerate() {
                emulator.set_input(segment.action.buttons());
                for _ in 0..segment.frames {
                    emulator.step_until_next_vblank(instructions_per_frame);
                    total_frames += 1;
                    if emulator.is_terminal() {
                        break;
                    }
                }

                let snapshot_prefix = suffixed_path(
                    &output_prefix,
                    &format!(
                        "segment-{:02}-{}",
                        index + 1,
                        native_script_filename_action(segment.action)
                    ),
                );
                let (
                    actual_display_output,
                    raw_actual_display_output,
                    display_output,
                    observation_output,
                    vram_output,
                ) = write_native_snapshot(&emulator, &snapshot_prefix)?;
                snapshots.push(format!(
                    "{{\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"segment_frames\":{},\"total_frames\":{},\"executed_steps\":{},\"actual_display_output\":\"{}\",\"raw_actual_display_output\":\"{}\",\"display_output\":\"{}\",\"observation_output\":\"{}\",\"vram_output\":\"{}\"}}",
                    index,
                    segment.action.index(),
                    segment.action.name(),
                    segment.frames,
                    total_frames,
                    emulator.executed_steps(),
                    escape_json(&actual_display_output.display().to_string()),
                    escape_json(&raw_actual_display_output.display().to_string()),
                    escape_json(&display_output.display().to_string()),
                    escape_json(&observation_output.display().to_string()),
                    escape_json(&vram_output.display().to_string())
                ));

                if emulator.is_terminal() {
                    break;
                }
            }

            println!(
                "{{\"instructions_per_frame\":{},\"total_frames\":{},\"executed_steps\":{},\"segments\":[{}],\"snapshots\":[{}],\"state\":{}}}",
                instructions_per_frame,
                total_frames,
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                snapshots.join(","),
                emulator.json()
            );
            Ok(())
        }
        "native-scripted-branch" => {
            let rom = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-branch <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <branch_frames> <settle_frames> <warmup_action:frames>..."
                    .to_string()
            })?;
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-branch <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <branch_frames> <settle_frames> <warmup_action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let output_prefix = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-branch <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <branch_frames> <settle_frames> <warmup_action:frames>..."
                    .to_string()
            })?;
            let branch_frames = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-branch <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <branch_frames> <settle_frames> <warmup_action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "branch_frames must be a positive integer".to_string())?;
            let settle_frames = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-branch <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <branch_frames> <settle_frames> <warmup_action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "settle_frames must be a non-negative integer".to_string())?;
            if branch_frames == 0 {
                return Err("branch_frames must be greater than zero".to_string());
            }

            let (warmup_segments, branch_actions) =
                parse_native_branch_values(args.collect::<Vec<_>>())?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let mut checkpoint =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            let warmup_frames =
                run_native_script(&mut checkpoint, instructions_per_frame, &warmup_segments);
            let mut branches = Vec::new();

            for action in branch_actions {
                let mut branch = checkpoint.clone();
                let segments = [
                    NativeScriptSegment {
                        action,
                        frames: branch_frames,
                    },
                    NativeScriptSegment {
                        action: Action::Noop,
                        frames: settle_frames,
                    },
                ];
                let branch_total_frames =
                    run_native_script(&mut branch, instructions_per_frame, &segments);
                let snapshot_prefix = suffixed_path(
                    &output_prefix,
                    &format!(
                        "branch-{:02}-{}",
                        action.index(),
                        native_script_filename_action(action)
                    ),
                );
                let (
                    actual_display_output,
                    raw_actual_display_output,
                    display_output,
                    observation_output,
                    vram_output,
                ) = write_native_snapshot(&branch, &snapshot_prefix)?;
                branches.push(format!(
                    "{{\"action_index\":{},\"action\":\"{}\",\"branch_frames\":{},\"settle_frames\":{},\"total_branch_frames\":{},\"executed_steps\":{},\"actual_display_output\":\"{}\",\"raw_actual_display_output\":\"{}\",\"display_output\":\"{}\",\"observation_output\":\"{}\",\"vram_output\":\"{}\",\"state\":{}}}",
                    action.index(),
                    action.name(),
                    branch_frames,
                    settle_frames,
                    branch_total_frames,
                    branch.executed_steps(),
                    escape_json(&actual_display_output.display().to_string()),
                    escape_json(&raw_actual_display_output.display().to_string()),
                    escape_json(&display_output.display().to_string()),
                    escape_json(&observation_output.display().to_string()),
                    escape_json(&vram_output.display().to_string()),
                    branch.probe_json()
                ));
            }

            println!(
                "{{\"instructions_per_frame\":{},\"warmup_frames\":{},\"warmup_executed_steps\":{},\"branch_frames\":{},\"settle_frames\":{},\"warmup_segments\":[{}],\"checkpoint_state\":{},\"branches\":[{}]}}",
                instructions_per_frame,
                warmup_frames,
                checkpoint.executed_steps(),
                branch_frames,
                settle_frames,
                native_script_segments_json(&warmup_segments),
                checkpoint.probe_json(),
                branches.join(",")
            );
            Ok(())
        }
        "native-scripted-branch-summary" => {
            let rom = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-branch-summary <rom_zip_or_dir> <instructions_per_frame> <branch_frames> <settle_frames> <warmup_action:frames>..."
                    .to_string()
            })?;
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-branch-summary <rom_zip_or_dir> <instructions_per_frame> <branch_frames> <settle_frames> <warmup_action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let branch_frames = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-branch-summary <rom_zip_or_dir> <instructions_per_frame> <branch_frames> <settle_frames> <warmup_action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "branch_frames must be a positive integer".to_string())?;
            let settle_frames = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-branch-summary <rom_zip_or_dir> <instructions_per_frame> <branch_frames> <settle_frames> <warmup_action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "settle_frames must be a non-negative integer".to_string())?;
            if branch_frames == 0 {
                return Err("branch_frames must be greater than zero".to_string());
            }

            let (warmup_segments, branch_actions) =
                parse_native_branch_values(args.collect::<Vec<_>>())?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let mut checkpoint =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            let warmup_frames =
                run_native_script(&mut checkpoint, instructions_per_frame, &warmup_segments);
            let mut branches = Vec::new();

            for action in branch_actions {
                let mut branch = checkpoint.clone();
                let segments = [
                    NativeScriptSegment {
                        action,
                        frames: branch_frames,
                    },
                    NativeScriptSegment {
                        action: Action::Noop,
                        frames: settle_frames,
                    },
                ];
                let branch_total_frames =
                    run_native_script(&mut branch, instructions_per_frame, &segments);
                branches.push(format!(
                    "{{\"action_index\":{},\"action\":\"{}\",\"branch_frames\":{},\"settle_frames\":{},\"total_branch_frames\":{},\"executed_steps\":{},\"state\":{}}}",
                    action.index(),
                    action.name(),
                    branch_frames,
                    settle_frames,
                    branch_total_frames,
                    branch.executed_steps(),
                    branch.probe_json()
                ));
            }

            println!(
                "{{\"instructions_per_frame\":{},\"warmup_frames\":{},\"warmup_executed_steps\":{},\"branch_frames\":{},\"settle_frames\":{},\"warmup_segments\":[{}],\"checkpoint_state\":{},\"branches\":[{}]}}",
                instructions_per_frame,
                warmup_frames,
                checkpoint.executed_steps(),
                branch_frames,
                settle_frames,
                native_script_segments_json(&warmup_segments),
                checkpoint.probe_json(),
                branches.join(",")
            );
            Ok(())
        }
        "native-draw-snapshot" => {
            let rom = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-draw-snapshot <rom_zip_or_dir> <instruction_count> <sequence_start> <sequence_end> <output_prefix>"
                    .to_string()
            })?;
            let count = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-draw-snapshot <rom_zip_or_dir> <instruction_count> <sequence_start> <sequence_end> <output_prefix>"
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instruction_count must be a non-negative integer".to_string())?;
            let sequence_start = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-draw-snapshot <rom_zip_or_dir> <instruction_count> <sequence_start> <sequence_end> <output_prefix>"
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "sequence_start must be a non-negative integer".to_string())?;
            let sequence_end = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-draw-snapshot <rom_zip_or_dir> <instruction_count> <sequence_start> <sequence_end> <output_prefix>"
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "sequence_end must be a non-negative integer".to_string())?;
            let output_prefix = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-draw-snapshot <rom_zip_or_dir> <instruction_count> <sequence_start> <sequence_end> <output_prefix>"
                    .to_string()
            })?;
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            emulator.set_draw_capture_range(sequence_start, sequence_end);
            emulator.step_instructions(count);
            let captures = write_draw_captures(&emulator, &output_prefix)?;
            println!(
                "{{\"output_prefix\":\"{}\",\"instruction_count\":{},\"executed_steps\":{},\"sequence_start\":{},\"sequence_end\":{},\"capture_count\":{},\"captures\":[{}],\"state\":{}}}",
                escape_json(&output_prefix.display().to_string()),
                count,
                emulator.executed_steps(),
                sequence_start,
                sequence_end,
                emulator.draw_captures().len(),
                captures.join(","),
                emulator.json()
            );
            Ok(())
        }
        "native-scripted-draw-snapshot" => {
            let rom = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-draw-snapshot <rom_zip_or_dir> <instructions_per_frame> <sequence_start> <sequence_end> <output_prefix> <action:frames>..."
                    .to_string()
            })?;
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-draw-snapshot <rom_zip_or_dir> <instructions_per_frame> <sequence_start> <sequence_end> <output_prefix> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let sequence_start = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-draw-snapshot <rom_zip_or_dir> <instructions_per_frame> <sequence_start> <sequence_end> <output_prefix> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "sequence_start must be a non-negative integer".to_string())?;
            let sequence_end = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-draw-snapshot <rom_zip_or_dir> <instructions_per_frame> <sequence_start> <sequence_end> <output_prefix> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "sequence_end must be a non-negative integer".to_string())?;
            let output_prefix = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-draw-snapshot <rom_zip_or_dir> <instructions_per_frame> <sequence_start> <sequence_end> <output_prefix> <action:frames>..."
                    .to_string()
            })?;
            let raw_segments = args.collect::<Vec<_>>();
            let segments = parse_native_script_segments(raw_segments)?;
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            let instructions_per_frame = instructions_per_frame.max(1);
            emulator.set_draw_capture_range(sequence_start, sequence_end);
            let total_frames = run_native_script(&mut emulator, instructions_per_frame, &segments);
            let captures = write_draw_captures(&emulator, &output_prefix)?;
            println!(
                "{{\"output_prefix\":\"{}\",\"instructions_per_frame\":{},\"total_frames\":{},\"executed_steps\":{},\"sequence_start\":{},\"sequence_end\":{},\"capture_count\":{},\"segments\":[{}],\"captures\":[{}],\"state\":{}}}",
                escape_json(&output_prefix.display().to_string()),
                instructions_per_frame,
                total_frames,
                emulator.executed_steps(),
                sequence_start,
                sequence_end,
                emulator.draw_captures().len(),
                native_script_segments_json(&segments),
                captures.join(","),
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

fn run_native_play(
    rom: PathBuf,
    instructions_per_frame: u64,
    scale: Scale,
    max_frames: Option<u64>,
    script_segments: Vec<NativeScriptSegment>,
) -> Result<(), String> {
    let mut emulator = NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
    let initial_raw_frame = emulator.display_frame();
    let initial_frame = native_play_window_frame(&initial_raw_frame);
    let autoplay_enabled = !script_segments.is_empty();
    let title = if autoplay_enabled {
        "Bloody Roar 2 native Rust autoplay - scripted boot then keyboard controls, Esc quit"
    } else {
        "Bloody Roar 2 native Rust - arrows move, Z punch, X kick, A beast, S guard, C coin, Enter start, Esc quit"
    };
    let mut window = Window::new(
        title,
        initial_frame.width,
        initial_frame.height,
        WindowOptions {
            resize: true,
            scale,
            ..WindowOptions::default()
        },
    )
    .map_err(|error| format!("failed to create native play window: {error:?}"))?;
    window.set_target_fps(60);
    window
        .update_with_buffer(
            &initial_frame.pixels,
            initial_frame.width,
            initial_frame.height,
        )
        .map_err(|error| format!("failed to update native play window: {error:?}"))?;

    let mut rendered_frames = 0u64;
    let mut observed_native_playable_candidate = emulator.native_playable_candidate();
    let mut first_native_playable_frame =
        observed_native_playable_candidate.then_some(rendered_frames);
    let mut last_native_playable_frame = first_native_playable_frame;
    let mut script_segment_index = 0usize;
    let mut script_segment_frame = 0u64;
    let mut scripted_frames = 0u64;
    while window.is_open()
        && !window.is_key_down(Key::Escape)
        && !emulator.is_terminal()
        && max_frames.is_none_or(|max_frames| rendered_frames < max_frames)
    {
        let scripted_action = next_scripted_action(
            &script_segments,
            &mut script_segment_index,
            &mut script_segment_frame,
        );
        let buttons = if let Some(action) = scripted_action {
            scripted_frames += 1;
            action.buttons()
        } else {
            native_window_buttons(&window)
        };
        emulator.set_input(buttons);
        emulator.step_until_next_vblank(instructions_per_frame);
        let raw_frame = emulator.display_frame();
        let frame = native_play_window_frame(&raw_frame);
        window
            .update_with_buffer(&frame.pixels, frame.width, frame.height)
            .map_err(|error| format!("failed to update native play window: {error:?}"))?;
        rendered_frames += 1;
        if emulator.native_playable_candidate() {
            observed_native_playable_candidate = true;
            first_native_playable_frame.get_or_insert(rendered_frames);
            last_native_playable_frame = Some(rendered_frames);
        }
    }

    let final_native_playable_candidate = emulator.native_playable_candidate();
    let final_raw_frame = emulator.display_frame();
    let final_frame = native_play_window_frame(&final_raw_frame);
    let final_frame_stats = NativeFrameStats::from_frame(&final_frame);
    let final_frame_full_size = final_frame.width >= 512 && final_frame.height >= 480;
    let final_frame_render_verified = final_frame_full_size && final_frame_stats.has_scene_detail();
    let final_window_size = window.get_size();
    let input_activity = emulator.input_activity();
    let input_controls_active = input_activity.has_play_control_activity();
    let full_controls_active = input_activity.has_full_control_activity();
    let native_play_input_verified = observed_native_playable_candidate && input_controls_active;
    let native_play_full_input_verified =
        observed_native_playable_candidate && full_controls_active;
    let playable = observed_native_playable_candidate && final_frame_render_verified;
    let autoplay_script_completed = autoplay_enabled
        && native_script_completed(&script_segments, script_segment_index, script_segment_frame);
    let first_native_playable_frame = optional_u64_json(first_native_playable_frame);
    let last_native_playable_frame = optional_u64_json(last_native_playable_frame);
    println!(
        "{{\"rendered_frames\":{},\"executed_steps\":{},\"autoplay_enabled\":{},\"autoplay_script_completed\":{},\"autoplay_scripted_frames\":{},\"autoplay_segments\":[{}],\"initial_raw_frame\":{},\"initial_window_frame\":{},\"final_raw_frame\":{},\"final_window_size\":{{\"width\":{},\"height\":{}}},\"input_activity\":{},\"native_playable_candidate\":{},\"observed_native_playable_candidate\":{},\"first_native_playable_frame\":{},\"last_native_playable_frame\":{},\"final_native_playable_candidate\":{},\"input_controls_active\":{},\"full_controls_active\":{},\"native_play_input_verified\":{},\"native_play_full_input_verified\":{},\"final_frame_full_size\":{},\"final_frame_render_verified\":{},\"final_frame\":{},\"playable\":{},\"state\":{}}}",
        rendered_frames,
        emulator.executed_steps(),
        autoplay_enabled,
        autoplay_script_completed,
        scripted_frames,
        native_script_segments_json(&script_segments),
        NativeFrameStats::from_frame(&initial_raw_frame).json(),
        NativeFrameStats::from_frame(&initial_frame).json(),
        NativeFrameStats::from_frame(&final_raw_frame).json(),
        final_window_size.0,
        final_window_size.1,
        input_activity.json(),
        final_native_playable_candidate,
        observed_native_playable_candidate,
        first_native_playable_frame,
        last_native_playable_frame,
        final_native_playable_candidate,
        input_controls_active,
        full_controls_active,
        native_play_input_verified,
        native_play_full_input_verified,
        final_frame_full_size,
        final_frame_render_verified,
        final_frame_stats.json(),
        playable,
        emulator.probe_json()
    );
    Ok(())
}

fn native_play_window_frame(frame: &NativeDisplayFrame) -> NativeDisplayFrame {
    let width = frame.width.max(NATIVE_PLAY_MIN_WINDOW_WIDTH);
    let height = frame.height.max(NATIVE_PLAY_MIN_WINDOW_HEIGHT);
    if width == frame.width && height == frame.height {
        return frame.clone();
    }

    let mut pixels = vec![0; width.saturating_mul(height)];
    let copy_width = frame.width.min(width);
    let copy_height = frame.height.min(height);
    for y in 0..copy_height {
        let source_start = y.saturating_mul(frame.width);
        let target_start = y.saturating_mul(width);
        pixels[target_start..target_start + copy_width]
            .copy_from_slice(&frame.pixels[source_start..source_start + copy_width]);
    }

    NativeDisplayFrame {
        width,
        height,
        pixels,
    }
}

fn run_native_health_check(
    rom: PathBuf,
    instructions_per_frame: u64,
    branch_frames: u64,
    settle_frames: u64,
) -> Result<(), String> {
    let mut checkpoint = NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
    let checkpoint_segments = default_native_play_script();
    let checkpoint_run = run_native_script_observed(
        &mut checkpoint,
        instructions_per_frame,
        &checkpoint_segments,
    );
    let checkpoint_activity = checkpoint.input_activity();
    let checkpoint_stats = NativeFrameStats::from_frame(&checkpoint.display_frame());
    let checkpoint_native_playable = checkpoint.native_playable_candidate();

    let mut branches = Vec::new();
    let mut all_branch_actions_read = true;
    let mut branch_native_playable_count = 0usize;
    let mut branch_full_scene_count = 0usize;
    for &action in native_health_branch_actions() {
        let mut branch = checkpoint.clone();
        let branch_segments = [
            NativeScriptSegment {
                action,
                frames: branch_frames,
            },
            NativeScriptSegment {
                action: Action::Noop,
                frames: settle_frames,
            },
        ];
        let branch_run =
            run_native_script_observed(&mut branch, instructions_per_frame, &branch_segments);
        let branch_activity = branch.input_activity();
        let action_read = native_action_activity_observed(branch_activity, action);
        let branch_stats = NativeFrameStats::from_frame(&branch.display_frame());
        let branch_native_playable = branch.native_playable_candidate();
        all_branch_actions_read &= action_read;
        if branch_native_playable {
            branch_native_playable_count += 1;
        }
        if branch_native_playable && branch_stats.has_scene_detail() {
            branch_full_scene_count += 1;
        }
        branches.push(format!(
            "{{\"action_index\":{},\"action\":\"{}\",\"action_activity_observed\":{},\"native_playable_candidate\":{},\"terminal\":{},\"run\":{},\"input_activity\":{},\"frame\":{},\"state\":{}}}",
            action.index(),
            action.name(),
            action_read,
            branch_native_playable,
            branch.is_terminal(),
            branch_run.json(),
            branch_activity.json(),
            branch_stats.json(),
            branch.probe_json()
        ));
    }

    let mut control_sweep = checkpoint.clone();
    let control_sweep_segments = native_control_sweep_script(branch_frames, settle_frames);
    let control_sweep_run = run_native_script_observed(
        &mut control_sweep,
        instructions_per_frame,
        &control_sweep_segments,
    );
    let control_sweep_activity = control_sweep.input_activity();
    let control_sweep_stats = NativeFrameStats::from_frame(&control_sweep.display_frame());
    let control_sweep_native_playable = control_sweep.native_playable_candidate();

    let native_core_running = checkpoint.executed_steps() > 0 && !checkpoint.is_terminal();
    let play_controls_active = checkpoint_activity.has_play_control_activity()
        || control_sweep_activity.has_play_control_activity();
    let full_controls_active = control_sweep_activity.has_full_control_activity();
    let all_branches_native_playable =
        branch_native_playable_count == native_health_branch_actions().len();
    let rendering_present =
        checkpoint_stats.has_visible_content() || control_sweep_stats.has_visible_content();
    let checkpoint_full_scene = checkpoint_native_playable && checkpoint_stats.has_scene_detail();
    let control_sweep_full_scene =
        control_sweep_native_playable && control_sweep_stats.has_scene_detail();
    let display_detail_present = checkpoint_stats.has_scene_detail()
        || control_sweep_stats.has_scene_detail()
        || branch_full_scene_count > 0;
    let full_scene_rendering =
        checkpoint_full_scene || control_sweep_full_scene || branch_full_scene_count > 0;
    let known_rendering_gap =
        rendering_present && (!full_scene_rendering || !all_branches_native_playable);
    let overall_pass = native_core_running
        && play_controls_active
        && full_controls_active
        && all_branch_actions_read
        && all_branches_native_playable
        && rendering_present
        && full_scene_rendering;
    let overall_status = if overall_pass {
        "pass"
    } else if native_core_running || rendering_present || play_controls_active {
        "partial"
    } else {
        "fail"
    };

    println!(
        "{{\"overall_status\":\"{}\",\"overall_pass\":{},\"instructions_per_frame\":{},\"branch_frames\":{},\"settle_frames\":{},\"native_core_running\":{},\"play_controls_active\":{},\"full_controls_active\":{},\"all_branch_actions_read\":{},\"all_branches_native_playable\":{},\"rendering_present\":{},\"display_detail_present\":{},\"checkpoint_full_scene\":{},\"control_sweep_full_scene\":{},\"full_scene_rendering\":{},\"known_rendering_gap\":{},\"branch_native_playable_count\":{},\"branch_full_scene_count\":{},\"branch_count\":{},\"checkpoint\":{{\"run\":{},\"segments\":[{}],\"executed_steps\":{},\"terminal\":{},\"native_playable_candidate\":{},\"input_activity\":{},\"frame\":{},\"state\":{}}},\"control_sweep\":{{\"run\":{},\"segments\":[{}],\"executed_steps\":{},\"terminal\":{},\"native_playable_candidate\":{},\"input_activity\":{},\"frame\":{},\"state\":{}}},\"branches\":[{}]}}",
        overall_status,
        overall_pass,
        instructions_per_frame,
        branch_frames,
        settle_frames,
        native_core_running,
        play_controls_active,
        full_controls_active,
        all_branch_actions_read,
        all_branches_native_playable,
        rendering_present,
        display_detail_present,
        checkpoint_full_scene,
        control_sweep_full_scene,
        full_scene_rendering,
        known_rendering_gap,
        branch_native_playable_count,
        branch_full_scene_count,
        native_health_branch_actions().len(),
        checkpoint_run.json(),
        native_script_segments_json(&checkpoint_segments),
        checkpoint.executed_steps(),
        checkpoint.is_terminal(),
        checkpoint_native_playable,
        checkpoint_activity.json(),
        checkpoint_stats.json(),
        checkpoint.probe_json(),
        control_sweep_run.json(),
        native_script_segments_json(&control_sweep_segments),
        control_sweep.executed_steps(),
        control_sweep.is_terminal(),
        control_sweep_native_playable,
        control_sweep_activity.json(),
        control_sweep_stats.json(),
        control_sweep.probe_json(),
        branches.join(",")
    );

    if overall_pass {
        Ok(())
    } else {
        Err(format!(
            "native health check did not reach full native playability: {overall_status}"
        ))
    }
}

fn next_scripted_action(
    segments: &[NativeScriptSegment],
    segment_index: &mut usize,
    segment_frame: &mut u64,
) -> Option<Action> {
    loop {
        let segment = segments.get(*segment_index)?;
        if *segment_frame < segment.frames {
            *segment_frame += 1;
            return Some(segment.action);
        }
        *segment_index += 1;
        *segment_frame = 0;
    }
}

fn native_script_completed(
    segments: &[NativeScriptSegment],
    segment_index: usize,
    segment_frame: u64,
) -> bool {
    let Some(segment) = segments.get(segment_index) else {
        return true;
    };
    segment_index + 1 == segments.len() && segment_frame >= segment.frames
}

fn optional_u64_json(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeScriptRunSummary {
    total_frames: u64,
    observed_native_playable_candidate: bool,
    first_native_playable_frame: Option<u64>,
    last_native_playable_frame: Option<u64>,
}

impl NativeScriptRunSummary {
    fn json(self) -> String {
        format!(
            "{{\"total_frames\":{},\"observed_native_playable_candidate\":{},\"first_native_playable_frame\":{},\"last_native_playable_frame\":{}}}",
            self.total_frames,
            self.observed_native_playable_candidate,
            optional_u64_json(self.first_native_playable_frame),
            optional_u64_json(self.last_native_playable_frame)
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeFrameStats {
    width: usize,
    height: usize,
    total_pixels: usize,
    nonzero_pixels: usize,
    unique_colors: usize,
    horizontal_color_changes: usize,
}

impl NativeFrameStats {
    fn from_frame(frame: &NativeDisplayFrame) -> Self {
        let mut unique_colors = Vec::new();
        let mut horizontal_color_changes = 0usize;
        let mut nonzero_pixels = 0usize;

        for y in 0..frame.height {
            let row = y.saturating_mul(frame.width);
            for x in 0..frame.width {
                let color = frame.pixels.get(row + x).copied().unwrap_or_default() & 0x00ff_ffff;
                if color != 0 {
                    nonzero_pixels += 1;
                }
                if unique_colors.len() < 257 && !unique_colors.contains(&color) {
                    unique_colors.push(color);
                }
                if x > 0 {
                    let previous =
                        frame.pixels.get(row + x - 1).copied().unwrap_or_default() & 0x00ff_ffff;
                    if previous != color {
                        horizontal_color_changes += 1;
                    }
                }
            }
        }

        Self {
            width: frame.width,
            height: frame.height,
            total_pixels: frame.pixels.len(),
            nonzero_pixels,
            unique_colors: unique_colors.len(),
            horizontal_color_changes,
        }
    }

    fn has_visible_content(self) -> bool {
        self.total_pixels > 0
            && self.nonzero_pixels.saturating_mul(100) >= self.total_pixels
            && self.unique_colors >= 2
    }

    fn has_scene_detail(self) -> bool {
        self.has_visible_content()
            && self.unique_colors >= 64
            && self.horizontal_color_changes.saturating_mul(30) >= self.total_pixels
    }

    fn json(self) -> String {
        format!(
            "{{\"width\":{},\"height\":{},\"total_pixels\":{},\"nonzero_pixels\":{},\"unique_colors\":{},\"horizontal_color_changes\":{},\"visible_content\":{},\"scene_detail\":{}}}",
            self.width,
            self.height,
            self.total_pixels,
            self.nonzero_pixels,
            self.unique_colors,
            self.horizontal_color_changes,
            self.has_visible_content(),
            self.has_scene_detail()
        )
    }
}

const NATIVE_HEALTH_BRANCH_ACTIONS: [Action; 8] = [
    Action::Up,
    Action::Down,
    Action::Left,
    Action::Right,
    Action::Punch,
    Action::Kick,
    Action::Beast,
    Action::Guard,
];

fn native_health_branch_actions() -> &'static [Action] {
    &NATIVE_HEALTH_BRANCH_ACTIONS
}

fn native_control_sweep_script(branch_frames: u64, settle_frames: u64) -> Vec<NativeScriptSegment> {
    let mut segments = Vec::new();
    for action in native_health_branch_actions() {
        segments.push(NativeScriptSegment {
            action: *action,
            frames: branch_frames,
        });
        if settle_frames > 0 {
            segments.push(NativeScriptSegment {
                action: Action::Noop,
                frames: settle_frames,
            });
        }
    }
    segments
}

fn native_action_activity_observed(activity: NativeInputActivity, action: Action) -> bool {
    let buttons = action.buttons();
    (!buttons.up || activity.p1_up_active_reads > 0)
        && (!buttons.down || activity.p1_down_active_reads > 0)
        && (!buttons.left || activity.p1_left_active_reads > 0)
        && (!buttons.right || activity.p1_right_active_reads > 0)
        && (!buttons.punch || activity.p1_punch_active_reads > 0)
        && (!buttons.kick || activity.p1_kick_active_reads > 0)
        && (!buttons.beast || activity.p1_beast_active_reads > 0)
        && (!buttons.guard || activity.p3_guard_active_reads > 0)
        && (!buttons.coin || activity.system_coin_active_reads > 0)
        && (!buttons.start
            || activity.system_start_active_reads > 0
            || activity.p1_start_active_reads > 0)
}

fn native_window_buttons(window: &Window) -> bloodyroar2_gym::ActionButtons {
    bloodyroar2_gym::ActionButtons {
        start: window.is_key_down(Key::Enter),
        coin: window.is_key_down(Key::C),
        up: window.is_key_down(Key::Up),
        down: window.is_key_down(Key::Down),
        left: window.is_key_down(Key::Left),
        right: window.is_key_down(Key::Right),
        punch: window.is_key_down(Key::Z),
        kick: window.is_key_down(Key::X),
        beast: window.is_key_down(Key::A),
        guard: window.is_key_down(Key::S),
    }
}

fn parse_native_window_scale(value: Option<String>) -> Result<Scale, String> {
    match value
        .as_deref()
        .unwrap_or("2")
        .to_ascii_lowercase()
        .as_str()
    {
        "1" | "x1" => Ok(Scale::X1),
        "2" | "x2" => Ok(Scale::X2),
        "4" | "x4" => Ok(Scale::X4),
        "8" | "x8" => Ok(Scale::X8),
        "fit" | "fitscreen" | "fit-screen" => Ok(Scale::FitScreen),
        value => Err(format!(
            "native window scale must be one of 1, 2, 4, 8, or fit: {value}"
        )),
    }
}

fn print_help() {
    println!(
        "bloodyroar2-gym\n\nCommands:\n  info\n  action-space\n  observation-space\n  reset\n  step <action_index> [frames]\n  serve [address]\n  serve-native [address] [rom_zip] [instructions_per_frame]\n  prepare-assets <archive.zip> [rom_dir]\n  mame-required [rom_dir]\n  rom-ident [rom_dir]\n  mame-check [rom_dir]\n  doctor [rom_dir]\n  play [rom_dir] [extra_mame_args...]\n  prepare-zinc <archive.zip> [extract_dir]\n  zinc-check [bundle_dir]\n  zinc-play [bundle_dir] [extra_zinc_args...]\n  native-inspect [rom_zip_or_dir]\n  native-rom-summary [rom_zip_or_dir]\n  native-step [rom_zip] [instruction_count]\n  native-screenshot [rom_zip] [instruction_count] [output.png]\n  native-display-screenshot [rom_zip] [instruction_count] [output.png]\n  native-vram-screenshot [rom_zip] [instruction_count] [output.png]\n  native-screen-dump [rom_zip] [instruction_count] [output_prefix]\n  native-play [rom_zip_or_dir] [instructions_per_frame] [scale] [max_frames]\n  native-manual [rom_zip_or_dir] [instructions_per_frame] [scale] [max_frames]\n  native-autoplay [rom_zip_or_dir] [instructions_per_frame] [scale] [max_frames] [action:frames...]\n  native-input-check [rom_zip_or_dir] [instructions_per_frame]\n  native-health-check [rom_zip_or_dir] [instructions_per_frame] [branch_frames] [settle_frames]\n  native-scripted-step <rom_zip_or_dir> <instructions_per_frame> <output.png> <action:frames>...\n  native-scripted-dump <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>...\n  native-scripted-candidates <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>...\n  native-scripted-summary <rom_zip_or_dir> <instructions_per_frame> <action:frames>...\n  native-scripted-probe <rom_zip_or_dir> <instructions_per_frame> <action:frames>...\n  native-scripted-frame-probe <rom_zip_or_dir> <instructions_per_frame> <probe_stride_frames> <action:frames>...\n  native-scripted-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <action:frames>... [-- <trace options>]\n  native-scripted-timeline <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>...\n  native-scripted-branch <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <branch_frames> <settle_frames> <warmup_action:frames>...\n  native-scripted-branch-summary <rom_zip_or_dir> <instructions_per_frame> <branch_frames> <settle_frames> <warmup_action:frames>...\n  native-draw-snapshot <rom_zip_or_dir> <instruction_count> <sequence_start> <sequence_end> <output_prefix>\n  native-scripted-draw-snapshot <rom_zip_or_dir> <instructions_per_frame> <sequence_start> <sequence_end> <output_prefix> <action:frames>...\n  native-trace [rom_zip] [instruction_count] [hot_limit] [recent_limit] [stop_pc] [stop_below_pc] [--watch address [len]] [--watch-only]\n  native-env-step [rom_zip] [action_index] [frames] [instructions_per_frame]\n  asset-check <path>\n\nnative-play opens the macOS window, auto-runs coin/start to bypass boot warning/black screens, then falls back to keyboard controls: arrows move, Z punch, X kick, A beast, S guard, C coin, Enter start, Esc quit. native-manual preserves fully manual boot input. max_frames is optional and intended for smoke tests.\nThis project never ships ROMs, BIOS files, Windows EXEs, or DLLs. Configure legally obtained assets outside Git."
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

fn default_native_play_script() -> Vec<NativeScriptSegment> {
    vec![
        NativeScriptSegment {
            action: Action::Noop,
            frames: 300,
        },
        NativeScriptSegment {
            action: Action::Coin,
            frames: 30,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 120,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: 300,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 120,
        },
        NativeScriptSegment {
            action: Action::Punch,
            frames: 3,
        },
    ]
}

fn parse_native_autoplay_tail(
    values: Vec<String>,
) -> Result<(Option<u64>, Vec<NativeScriptSegment>), String> {
    if values.is_empty() {
        return Ok((None, default_native_play_script()));
    }

    let first_is_segment = values.first().is_some_and(|value| value.contains(':'));
    let (max_frames, segment_values) = if first_is_segment {
        (None, values)
    } else {
        let max_frames = values
            .first()
            .expect("non-empty values")
            .parse::<u64>()
            .map_err(|_| "max_frames must be a positive integer or action:frames".to_string())?;
        if max_frames == 0 {
            return Err("max_frames must be greater than zero".to_string());
        }
        (Some(max_frames), values.into_iter().skip(1).collect())
    };

    let segments = if segment_values.is_empty() {
        default_native_play_script()
    } else {
        parse_native_script_segments(segment_values)?
    };
    Ok((max_frames, segments))
}

fn parse_native_script_trace_segments(
    values: Vec<String>,
) -> Result<(Vec<NativeScriptSegment>, Vec<NativeScriptSegment>), String> {
    let Some(split_at) = values.iter().position(|value| value == "--trace") else {
        return Ok((Vec::new(), parse_native_script_segments(values)?));
    };
    let warmup = values[..split_at].to_vec();
    let traced = values[split_at + 1..].to_vec();
    if traced.is_empty() {
        return Err(
            "native-scripted-trace --trace requires traced <action:frames> segments".into(),
        );
    }
    let warmup_segments = if warmup.is_empty() {
        Vec::new()
    } else {
        parse_native_script_segments(warmup)?
    };
    let traced_segments = parse_native_script_segments(traced)?;
    Ok((warmup_segments, traced_segments))
}

fn parse_native_branch_values(
    values: Vec<String>,
) -> Result<(Vec<NativeScriptSegment>, Vec<Action>), String> {
    let Some(split_at) = values.iter().position(|value| value == "--actions") else {
        return Ok((parse_native_script_segments(values)?, ACTION_SPACE.to_vec()));
    };

    let warmup_values = values[..split_at].to_vec();
    let action_values = values[split_at + 1..].to_vec();
    if action_values.is_empty() {
        return Err("native-scripted-branch --actions requires at least one action".to_string());
    }

    let warmup_segments = if warmup_values.is_empty() {
        Vec::new()
    } else {
        parse_native_script_segments(warmup_values)?
    };
    let branch_actions = action_values
        .iter()
        .map(|value| parse_action_token(value))
        .collect::<Result<Vec<_>, _>>()?;

    Ok((warmup_segments, branch_actions))
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

fn suffixed_path(prefix: &std::path::Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}.{}", prefix.display(), suffix))
}

fn native_script_filename_action(action: Action) -> String {
    action.name().replace('+', "_")
}

fn write_native_snapshot(
    emulator: &NativeEmulator,
    output_prefix: &Path,
) -> Result<(PathBuf, PathBuf, PathBuf, PathBuf, PathBuf), String> {
    let actual_display_output = suffixed_path(output_prefix, "actual-display.png");
    let raw_actual_display_output = suffixed_path(output_prefix, "raw-actual-display.png");
    let display_output = suffixed_path(output_prefix, "display.png");
    let observation_output = suffixed_path(output_prefix, "observation.png");
    let vram_output = suffixed_path(output_prefix, "vram.png");

    std::fs::write(&actual_display_output, emulator.actual_display_png()).map_err(|error| {
        format!(
            "failed to write {}: {error}",
            actual_display_output.display()
        )
    })?;
    std::fs::write(
        &raw_actual_display_output,
        emulator.raw_actual_display_png(),
    )
    .map_err(|error| {
        format!(
            "failed to write {}: {error}",
            raw_actual_display_output.display()
        )
    })?;
    std::fs::write(&display_output, emulator.display_png())
        .map_err(|error| format!("failed to write {}: {error}", display_output.display()))?;
    std::fs::write(&observation_output, emulator.screenshot_png())
        .map_err(|error| format!("failed to write {}: {error}", observation_output.display()))?;
    std::fs::write(&vram_output, emulator.vram_png())
        .map_err(|error| format!("failed to write {}: {error}", vram_output.display()))?;

    Ok((
        actual_display_output,
        raw_actual_display_output,
        display_output,
        observation_output,
        vram_output,
    ))
}

fn write_native_display_candidates(
    emulator: &NativeEmulator,
    output_prefix: &Path,
) -> Result<Vec<String>, String> {
    let mut outputs = Vec::new();
    for candidate in emulator.display_candidates() {
        let suffix = format!(
            "candidate-{}-x{}-y{}.png",
            filename_token(candidate.label),
            candidate.x,
            candidate.y
        );
        let output = suffixed_path(output_prefix, &suffix);
        std::fs::write(&output, &candidate.png)
            .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
        outputs.push(candidate.json(&output.display().to_string()));
    }
    Ok(outputs)
}

fn filename_token(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn write_draw_captures(
    emulator: &NativeEmulator,
    output_prefix: &Path,
) -> Result<Vec<String>, String> {
    let mut captures = Vec::new();
    for capture in emulator.draw_captures() {
        let display_output = suffixed_path(
            output_prefix,
            &format!("seq-{:06}.display.png", capture.sequence),
        );
        let bounds_output = suffixed_path(
            output_prefix,
            &format!("seq-{:06}.bounds.png", capture.sequence),
        );
        std::fs::write(&display_output, &capture.display_png)
            .map_err(|error| format!("failed to write {}: {error}", display_output.display()))?;
        std::fs::write(&bounds_output, &capture.bounds_png)
            .map_err(|error| format!("failed to write {}: {error}", bounds_output.display()))?;
        captures.push(capture.json(
            &display_output.display().to_string(),
            &bounds_output.display().to_string(),
        ));
    }
    Ok(captures)
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

fn run_native_script(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    segments: &[NativeScriptSegment],
) -> u64 {
    let mut total_frames = 0u64;
    for segment in segments {
        emulator.set_input(segment.action.buttons());
        for _ in 0..segment.frames {
            emulator.step_until_next_vblank(instructions_per_frame);
            total_frames += 1;
            if emulator.is_terminal() {
                break;
            }
        }
        if emulator.is_terminal() {
            break;
        }
    }
    total_frames
}

fn run_native_script_observed(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    segments: &[NativeScriptSegment],
) -> NativeScriptRunSummary {
    let mut total_frames = 0u64;
    let mut observed_native_playable_candidate = false;
    let mut first_native_playable_frame = None;
    let mut last_native_playable_frame = None;

    for segment in segments {
        emulator.set_input(segment.action.buttons());
        for _ in 0..segment.frames {
            emulator.step_until_next_vblank(instructions_per_frame);
            total_frames += 1;
            if emulator.native_playable_candidate() {
                observed_native_playable_candidate = true;
                first_native_playable_frame.get_or_insert(total_frames);
                last_native_playable_frame = Some(total_frames);
            }
            if emulator.is_terminal() {
                break;
            }
        }
        if emulator.is_terminal() {
            break;
        }
    }

    NativeScriptRunSummary {
        total_frames,
        observed_native_playable_candidate,
        first_native_playable_frame,
        last_native_playable_frame,
    }
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
    use super::{
        NativeScriptSegment, default_native_play_script, native_control_sweep_script,
        native_play_window_frame, native_script_completed, next_scripted_action,
        parse_action_token, parse_native_autoplay_tail, parse_native_script_segments,
    };
    use bloodyroar2_gym::{Action, NativeDisplayFrame};

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

    #[test]
    fn parses_native_autoplay_tail_with_default_and_custom_script() {
        let (max_frames, default_segments) =
            parse_native_autoplay_tail(Vec::new()).expect("default autoplay parses");
        assert_eq!(max_frames, None);
        assert_eq!(
            default_segments.first().expect("first segment").action,
            Action::Noop
        );
        assert!(default_segments.len() > 1);

        let (max_frames, custom_segments) = parse_native_autoplay_tail(vec![
            "120".to_string(),
            "coin:2".to_string(),
            "start:3".to_string(),
        ])
        .expect("custom autoplay parses");
        assert_eq!(max_frames, Some(120));
        assert_eq!(
            custom_segments,
            vec![
                NativeScriptSegment {
                    action: Action::Coin,
                    frames: 2
                },
                NativeScriptSegment {
                    action: Action::Start,
                    frames: 3
                }
            ]
        );
    }

    #[test]
    fn default_native_play_script_reaches_stable_character_select_without_control_sweep() {
        let segments = default_native_play_script();

        assert!(
            segments
                .iter()
                .any(|segment| segment.action == Action::Coin)
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.action == Action::Start)
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.action == Action::Punch)
        );
        assert_eq!(segments.last().expect("last segment").action, Action::Punch);
        assert_eq!(segments.last().expect("last segment").frames, 3);
        assert!(!segments.iter().any(|segment| matches!(
            segment.action,
            Action::Up
                | Action::Down
                | Action::Left
                | Action::Right
                | Action::Kick
                | Action::Beast
                | Action::Guard
        )));
        assert_eq!(
            segments.iter().map(|segment| segment.frames).sum::<u64>(),
            873
        );
        assert!(segments.windows(2).any(|window| {
            window[0].action == Action::Start
                && window[1]
                    == NativeScriptSegment {
                        action: Action::Noop,
                        frames: 120,
                    }
        }));
    }

    #[test]
    fn native_control_sweep_covers_direction_and_attack_buttons() {
        let segments = native_control_sweep_script(3, 1);
        let actions = segments
            .iter()
            .filter(|segment| segment.action != Action::Noop)
            .map(|segment| segment.action)
            .collect::<Vec<_>>();

        assert_eq!(
            actions,
            vec![
                Action::Up,
                Action::Down,
                Action::Left,
                Action::Right,
                Action::Punch,
                Action::Kick,
                Action::Beast,
                Action::Guard
            ]
        );
    }

    #[test]
    fn native_play_window_frame_pads_partial_boot_frame_to_full_window() {
        let source = NativeDisplayFrame {
            width: 512,
            height: 240,
            pixels: vec![0x00ff_ffff; 512 * 240],
        };

        let padded = native_play_window_frame(&source);

        assert_eq!((padded.width, padded.height), (512, 480));
        assert_eq!(padded.pixels.len(), 512 * 480);
        assert_eq!(padded.pixels[0], 0x00ff_ffff);
        assert_eq!(padded.pixels[512 * 240], 0);
    }

    #[test]
    fn scripted_action_advances_across_segments() {
        let segments = vec![
            NativeScriptSegment {
                action: Action::Coin,
                frames: 2,
            },
            NativeScriptSegment {
                action: Action::Start,
                frames: 1,
            },
        ];
        let mut segment_index = 0;
        let mut segment_frame = 0;

        assert_eq!(
            next_scripted_action(&segments, &mut segment_index, &mut segment_frame),
            Some(Action::Coin)
        );
        assert_eq!(
            next_scripted_action(&segments, &mut segment_index, &mut segment_frame),
            Some(Action::Coin)
        );
        assert_eq!(
            next_scripted_action(&segments, &mut segment_index, &mut segment_frame),
            Some(Action::Start)
        );
        assert!(native_script_completed(
            &segments,
            segment_index,
            segment_frame
        ));
        assert_eq!(
            next_scripted_action(&segments, &mut segment_index, &mut segment_frame),
            None
        );
        assert!(native_script_completed(
            &segments,
            segment_index,
            segment_frame
        ));
    }
}
