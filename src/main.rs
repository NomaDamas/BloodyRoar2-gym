use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use bloodyroar2_gym::{
    ACTION_SPACE, Action, ActionButtons, BloodyRoar2Env, MameConfig, MameRuntime,
    NativeDisplayFrame, NativeEmulator, NativeGpuDrawCapturePredicate, NativeInputActivity,
    NativeRomSet, NativeTraceConfig, NullBackend, ZincConfig, ZincRuntime, action_space_json,
    api_index_json, native_window_frame_from_display, observation_space_json,
    png_from_rgb888_pixels,
};
use minifb::{Key, KeyRepeat, Scale, ScaleMode, Window, WindowOptions};

const NATIVE_PLAY_MIN_WINDOW_WIDTH: usize = 640;
const NATIVE_PLAY_MIN_WINDOW_HEIGHT: usize = 480;
const NATIVE_PLAY_FAST_FORWARD_MAX_FRAMES: u64 = 9_000;
const NATIVE_PLAY_SCRIPT_INSTRUCTIONS_PER_FRAME: u64 = 500_000;
const NATIVE_PLAY_GUI_DEFAULT_INSTRUCTIONS_PER_FRAME: u64 = 120_000;
const NATIVE_PLAY_GUI_POLL_INSTRUCTION_SLICE: u64 = 5_000;
const NATIVE_PLAY_GUI_POLL_INSTRUCTION_SLICE_ENV: &str = "BR2_NATIVE_GUI_POLL_INSTRUCTION_SLICE";
const NATIVE_PLAY_GUI_DISPLAY_REFRESH_FRAMES: u64 = 15;
const NATIVE_PLAY_GUI_PLAYABILITY_REFRESH_FRAMES: u64 = 15;
const NATIVE_PLAY_WINDOW_TARGET_FPS: usize = 60;
const NATIVE_PLAY_FAST_FORWARD_INSTRUCTIONS_PER_FRAME: u64 =
    NATIVE_PLAY_SCRIPT_INSTRUCTIONS_PER_FRAME;
const NATIVE_SCRIPT_VBLANK_POLL_INSTRUCTION_SLICE: u64 = 5_000;
const NATIVE_WARNING_SKIP_INITIAL_FRAMES: u64 = 30;
const NATIVE_WARNING_SKIP_PULSE_FRAMES: u64 = 12;
const NATIVE_WARNING_SKIP_GAP_FRAMES: u64 = 24;
const NATIVE_PLAY_TITLE_ENTRY_FRAMES: u64 = 1_800;
const NATIVE_PLAY_TITLE_READY_EXTRA_WAIT_FRAMES: u64 = 6_000;
const NATIVE_PLAY_TITLE_EARLY_ADVANCE_MIN_SEGMENT_FRAMES: u64 = 300;
const NATIVE_PLAY_HANDOFF_SETTLE_FRAMES: u64 = 120;
const NATIVE_PLAY_HANDOFF_CHECK_STRIDE_FRAMES: u64 = 6;
const NATIVE_PLAY_HANDOFF_RENDER_CHECK_STRIDE_FRAMES: u64 = 60;
const NATIVE_PLAY_SNAPSHOT_RENDER_SETTLE_FRAMES: u64 = 120;
const NATIVE_PLAY_SNAPSHOT_MATCH_BOOT_WALL_TIMEOUT_SECS: u64 = 90;
const NATIVE_SCRIPT_VBLANK_CAPTURE_INTERVAL: u64 = 6_000;
const NATIVE_SCRIPT_UNLINKED_PRIMITIVE_REPLAY_INTERVAL: Option<u64> = Some(5);
const NATIVE_SCRIPT_FRAME_ATTEMPT_MULTIPLIER: u64 = 4;
const NATIVE_SCRIPT_TIMED_WALL_TIMEOUT_SECS: u64 = 120;
const NATIVE_SCRIPT_TIMED_WALL_TIMEOUT_SECS_ENV: &str = "BR2_NATIVE_SCRIPT_TIMED_WALL_TIMEOUT_SECS";
const NATIVE_INPUT_CHECK_WALL_TIMEOUT_SECS: u64 = 120;
const NATIVE_INPUT_CHECK_BRANCH_FRAMES: u64 = 12;
const NATIVE_INPUT_CHECK_BRANCH_SETTLE_FRAMES: u64 = 12;
const NATIVE_PLAYABLE_SCRIPT_WALL_TIMEOUT_SECS: u64 = 240;
const NATIVE_PLAY_SNAPSHOT_TAIL_WALL_TIMEOUT_SECS: u64 = 120;
const NATIVE_PLAY_SNAPSHOT_WALL_TIMEOUT_CAP_SECS: u64 = 600;
const NATIVE_PLAY_SNAPSHOT_WALL_TIMEOUT_SECS_ENV: &str =
    "BR2_NATIVE_PLAY_SNAPSHOT_WALL_TIMEOUT_SECS";
const NATIVE_GAP_PROBE_WALL_TIMEOUT_CAP_SECS: u64 = 1_800;
const NATIVE_GAP_PROBE_WALL_TIMEOUT_SECS_ENV: &str = "BR2_NATIVE_GAP_PROBE_WALL_TIMEOUT_SECS";
const NATIVE_GAP_PROBE_OBSERVATION_STRIDE_FRAMES: u64 = 30;
const NATIVE_LIVE_PROBE_WALL_TIMEOUT_SECS: u64 = 90;
const NATIVE_HEALTH_CHECK_WALL_TIMEOUT_SECS: u64 = 600;
const NATIVE_HEALTH_MATCH_ENTRY_WALL_TIMEOUT_SECS: u64 = 360;
const NATIVE_HEALTH_CONTROL_SWEEP_RESERVE_SECS: u64 = 60;
const NATIVE_CREDIT_MAPPING_PROBE_WALL_TIMEOUT_SECS: u64 = 240;
const NATIVE_CREDIT_MAPPING_PROBE_WALL_TIMEOUT_SECS_ENV: &str =
    "BR2_NATIVE_CREDIT_MAPPING_PROBE_WALL_TIMEOUT_SECS";
const NATIVE_PLAY_INPUT_LATCH_POLLS: u8 = 45;
const NATIVE_VBLANK_TRACE_MAX_INSTRUCTIONS: u64 = 1_000_000;
const NATIVE_PLAY_DEFAULT_INSTRUCTIONS_PER_FRAME_ARG: &str = "500000";
const NATIVE_RAM_DIFF_MAX_RANGE_SAMPLES: usize = 64;
const NATIVE_RAM_DIFF_MAX_WORD_SAMPLES: usize = 128;
const NATIVE_SCRATCHPAD_BASE_ADDRESS: usize = 0x1f80_0000;
const NATIVE_MIPS_IMM_SEARCH_DEFAULT_LIMIT: usize = 256;
const NATIVE_MIPS_IMM_SEARCH_MAX_LIMIT: usize = 4096;
const NATIVE_BR2_RUNTIME_CODE_START: u32 = 0x002c_0000;
const NATIVE_BR2_RUNTIME_CODE_END: u32 = 0x0037_0000;

fn default_native_rom_source() -> PathBuf {
    if let Some(path) = NativeRomSet::latest_cached_runtime_rom_dir() {
        return path;
    }

    let combined_zip = PathBuf::from("assets/BloodRoar2-combined.zip");
    if combined_zip.is_file() {
        return combined_zip;
    }

    PathBuf::from("assets/roms")
}

fn next_native_rom_source_or_default<I>(args: &mut std::iter::Peekable<I>) -> PathBuf
where
    I: Iterator<Item = String>,
{
    if args
        .peek()
        .is_some_and(|value| native_arg_looks_like_rom_source(value))
    {
        return PathBuf::from(args.next().expect("peeked native ROM argument"));
    }

    default_native_rom_source()
}

fn native_emulator_from_rom_source(rom: impl Into<PathBuf>) -> Result<NativeEmulator, String> {
    let mut emulator = NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
    emulator.apply_auto_coin_input_mapping();
    Ok(emulator)
}

fn native_arg_looks_like_rom_source(value: &str) -> bool {
    if value.is_empty() || value.starts_with('-') {
        return false;
    }

    let path = Path::new(value);
    let lower = value.to_ascii_lowercase();
    path.exists()
        || value.contains('/')
        || lower.ends_with(".zip")
        || lower.ends_with(".bin")
        || lower.ends_with(".cue")
        || lower.ends_with(".iso")
        || lower.ends_with(".chd")
}

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
    let mut args = env::args().skip(1).peekable();
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
            let rom = next_native_rom_source_or_default(&mut args);
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
            println!("{}", doctor_report(config));
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
            let rom = next_native_rom_source_or_default(&mut args);
            let romset = NativeRomSet::scan(rom).map_err(|error| error.to_string())?;
            println!("{}", romset.json());
            Ok(())
        }
        "native-rom-summary" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let scan =
                NativeRomSet::scan_cached_with_report(rom).map_err(|error| error.to_string())?;
            println!("{}", scan.romset.compatibility_report().summary_json());
            Ok(())
        }
        "native-cache-prepare" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let scan = if let Some(cache_root) = args.next() {
                NativeRomSet::scan_cached_with_report_in_cache_root(rom, cache_root)
            } else {
                NativeRomSet::scan_cached_with_report(rom)
            }
            .map_err(|error| error.to_string())?;
            println!(
                "{{\"cache\":{},\"compatibility\":{}}}",
                scan.cache.json(),
                scan.romset.compatibility_report().summary_json()
            );
            Ok(())
        }
        "native-cache-path" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let scan = if let Some(cache_root) = args.next() {
                NativeRomSet::scan_cached_with_report_in_cache_root(rom, cache_root)
            } else {
                NativeRomSet::scan_cached_with_report(rom)
            }
            .map_err(|error| error.to_string())?;
            println!("{}", scan.cache.resolved_path.display());
            Ok(())
        }
        "native-step" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let count = args
                .next()
                .unwrap_or_else(|| "1".to_string())
                .parse::<u64>()
                .map_err(|_| "instruction count must be a non-negative integer".to_string())?;
            let mut emulator = native_emulator_from_rom_source(rom)?;
            emulator.step_instructions(count);
            println!("{}", emulator.json());
            Ok(())
        }
        "native-screenshot" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let count = args
                .next()
                .unwrap_or_else(|| "32000000".to_string())
                .parse::<u64>()
                .map_err(|_| "instruction count must be a non-negative integer".to_string())?;
            let output = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("native-frame.png"));
            let mut emulator = native_emulator_from_rom_source(rom)?;
            emulator.step_instructions(count);
            write_output_file(&output, emulator.screenshot_png())?;
            println!(
                "{{\"output\":\"{}\",\"state\":{}}}",
                output.display(),
                emulator.json()
            );
            Ok(())
        }
        "native-display-screenshot" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let count = args
                .next()
                .unwrap_or_else(|| "32000000".to_string())
                .parse::<u64>()
                .map_err(|_| "instruction count must be a non-negative integer".to_string())?;
            let output = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("native-display.png"));
            let mut emulator = native_emulator_from_rom_source(rom)?;
            emulator.step_instructions(count);
            write_output_file(&output, emulator.display_png())?;
            println!(
                "{{\"output\":\"{}\",\"state\":{}}}",
                output.display(),
                emulator.json()
            );
            Ok(())
        }
        "native-vram-screenshot" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let count = args
                .next()
                .unwrap_or_else(|| "32000000".to_string())
                .parse::<u64>()
                .map_err(|_| "instruction count must be a non-negative integer".to_string())?;
            let output = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("native-vram.png"));
            let mut emulator = native_emulator_from_rom_source(rom)?;
            emulator.step_instructions(count);
            write_output_file(&output, emulator.vram_png())?;
            println!(
                "{{\"output\":\"{}\",\"state\":{}}}",
                output.display(),
                emulator.json()
            );
            Ok(())
        }
        "native-screen-dump" => {
            let rom = next_native_rom_source_or_default(&mut args);
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
            let mut emulator = native_emulator_from_rom_source(rom)?;
            emulator.step_instructions(count);
            write_output_file(&actual_display_output, emulator.actual_display_png())?;
            write_output_file(
                &raw_actual_display_output,
                emulator.raw_actual_display_png(),
            )?;
            write_output_file(&display_output, emulator.display_png())?;
            write_output_file(&observation_output, emulator.screenshot_png())?;
            write_output_file(&vram_output, emulator.vram_png())?;
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
        "native-window-snapshot" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let count = args
                .next()
                .unwrap_or_else(|| "32000000".to_string())
                .parse::<u64>()
                .map_err(|_| "instruction count must be a non-negative integer".to_string())?;
            let output = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("native-window.png"));
            let mut emulator = native_emulator_from_rom_source(rom)?;
            emulator.step_instructions(count);
            let raw_frame = emulator.display_frame();
            let window_frame = native_play_window_frame(&raw_frame);
            write_native_display_frame_png(&window_frame, &output)?;
            println!(
                "{{\"output\":\"{}\",\"instruction_count\":{},\"raw_frame\":{},\"window_frame\":{},\"state\":{}}}",
                escape_json(&output.display().to_string()),
                count,
                NativeFrameStats::from_frame(&raw_frame).json(),
                NativeFrameStats::from_frame(&window_frame).json(),
                emulator.compact_probe_json()
            );
            Ok(())
        }
        "native-play-snapshot" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| NATIVE_PLAY_DEFAULT_INSTRUCTIONS_PER_FRAME_ARG.to_string())
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let output_prefix = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("native-play-snapshot"));
            let mut complete_script = false;
            let mut match_script = false;
            let mut require_gameplay_scene = false;
            let mut full_state = false;
            let mut fast_forward_max_frames = NATIVE_PLAY_FAST_FORWARD_MAX_FRAMES;
            let mut draw_capture_range = None;
            let mut draw_capture_predicates = Vec::new();
            let mut draw_capture_predicates_from_boot = false;
            let mut tail_values = Vec::new();
            let mut tail_args = args.peekable();
            while let Some(value) = tail_args.next() {
                match value.as_str() {
                    "--complete-script" => complete_script = true,
                    "--full-state" => full_state = true,
                    "--match-script" => {
                        match_script = true;
                        complete_script = true;
                        require_gameplay_scene = true;
                    }
                    "--require-gameplay-scene" => require_gameplay_scene = true,
                    "--fast-forward-frames" => {
                        let raw_frames = tail_args.next().ok_or_else(|| {
                            "--fast-forward-frames requires a positive integer".to_string()
                        })?;
                        fast_forward_max_frames = raw_frames.parse::<u64>().map_err(|_| {
                            "--fast-forward-frames must be a positive integer".to_string()
                        })?;
                        if fast_forward_max_frames == 0 {
                            return Err(
                                "--fast-forward-frames must be greater than zero".to_string()
                            );
                        }
                    }
                    "--draw-capture" => {
                        let raw_start = tail_args.next().ok_or_else(|| {
                            "--draw-capture requires <sequence_start> <sequence_end>".to_string()
                        })?;
                        let raw_end = tail_args.next().ok_or_else(|| {
                            "--draw-capture requires <sequence_start> <sequence_end>".to_string()
                        })?;
                        let start = raw_start.parse::<u64>().map_err(|_| {
                            "--draw-capture sequence_start must be a non-negative integer"
                                .to_string()
                        })?;
                        let end = raw_end.parse::<u64>().map_err(|_| {
                            "--draw-capture sequence_end must be a non-negative integer".to_string()
                        })?;
                        if start > end {
                            return Err(
                                "--draw-capture sequence_start must be <= sequence_end".to_string()
                            );
                        }
                        draw_capture_range = Some((start, end));
                    }
                    "--draw-capture-predicate" => {
                        let raw_predicate = tail_args.next().ok_or_else(|| {
                            "--draw-capture-predicate requires a key=value list".to_string()
                        })?;
                        draw_capture_predicates
                            .push(parse_native_draw_capture_predicate(&raw_predicate)?);
                    }
                    "--draw-capture-predicate-from-boot" => {
                        draw_capture_predicates_from_boot = true;
                    }
                    _ => tail_values.push(value),
                }
            }
            let tail_segments = if tail_values.is_empty() {
                Vec::new()
            } else {
                parse_native_script_segments(tail_values)?
            };
            run_native_play_snapshot(
                rom,
                instructions_per_frame.max(1),
                &output_prefix,
                complete_script,
                match_script,
                require_gameplay_scene,
                fast_forward_max_frames,
                tail_segments,
                full_state,
                draw_capture_range,
                draw_capture_predicates,
                draw_capture_predicates_from_boot,
            )
        }
        "native-gap-probe" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| NATIVE_PLAY_DEFAULT_INSTRUCTIONS_PER_FRAME_ARG.to_string())
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let mut match_script = true;
            let mut max_frames = NATIVE_PLAY_FAST_FORWARD_MAX_FRAMES;
            let mut values = args.peekable();
            while let Some(value) = values.next() {
                match value.as_str() {
                    "--match-script" => match_script = true,
                    "--handoff-only" => match_script = false,
                    "--max-frames" | "--fast-forward-frames" => {
                        let raw = values
                            .next()
                            .ok_or_else(|| format!("{value} requires a positive frame count"))?;
                        max_frames = raw
                            .parse::<u64>()
                            .map_err(|_| format!("{value} must be a positive integer"))?;
                        if max_frames == 0 {
                            return Err(format!("{value} must be greater than zero"));
                        }
                    }
                    raw => {
                        max_frames = raw
                            .parse::<u64>()
                            .map_err(|_| format!("unknown native-gap-probe option: {raw}"))?;
                        if max_frames == 0 {
                            return Err("max_frames must be greater than zero".to_string());
                        }
                    }
                }
            }
            run_native_gap_probe(rom, instructions_per_frame.max(1), max_frames, match_script)
        }
        "native-play" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| NATIVE_PLAY_DEFAULT_INSTRUCTIONS_PER_FRAME_ARG.to_string())
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
                native_snapshot_match_entry_script(),
                true,
                true,
            )
        }
        "native-manual" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| NATIVE_PLAY_DEFAULT_INSTRUCTIONS_PER_FRAME_ARG.to_string())
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
                false,
                false,
            )
        }
        "native-autoplay" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| NATIVE_PLAY_DEFAULT_INSTRUCTIONS_PER_FRAME_ARG.to_string())
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
                true,
                false,
            )
        }
        "native-input-check" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| NATIVE_PLAY_DEFAULT_INSTRUCTIONS_PER_FRAME_ARG.to_string())
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let mut emulator = native_emulator_from_rom_source(rom)?;
            emulator.apply_auto_coin_input_mapping();
            let instructions_per_frame = instructions_per_frame.max(1);
            let script_instructions_per_frame =
                native_fast_forward_instructions_per_frame(instructions_per_frame);
            let handoff_segments = native_snapshot_match_entry_script();
            let boot_wait_segments = native_play_boot_wait_segments(&handoff_segments);
            let boot_progress = run_native_script_observed_with_stop(
                &mut emulator,
                script_instructions_per_frame,
                &boot_wait_segments,
                NativeScriptStopMode::TitleScreen,
                Some(native_script_max_frames_for_segments(&boot_wait_segments)),
            );
            let tail_segments = remaining_native_script_segments_after_title_ready(
                &handoff_segments,
                boot_progress.segment_index,
                boot_progress.segment_frame,
            );
            let tail_progress = run_native_script_observed_with_stop(
                &mut emulator,
                script_instructions_per_frame,
                &tail_segments,
                NativeScriptStopMode::FastTimed,
                Some(native_script_max_frames_for_segments(&tail_segments)),
            );
            let render_settle_segments = [NativeScriptSegment {
                action: Action::Noop,
                frames: NATIVE_PLAY_SNAPSHOT_RENDER_SETTLE_FRAMES,
            }];
            let render_settle_run = run_native_script_observed_with_stop(
                &mut emulator,
                script_instructions_per_frame,
                &render_settle_segments,
                NativeScriptStopMode::Timed,
                Some(NATIVE_PLAY_SNAPSHOT_RENDER_SETTLE_FRAMES),
            )
            .summary;
            let boot_run = boot_progress.summary;
            let tail_run = tail_progress.summary;
            let checkpoint_run = NativeScriptRunSummary {
                total_frames: boot_run
                    .total_frames
                    .saturating_add(tail_run.total_frames)
                    .saturating_add(render_settle_run.total_frames),
                frame_attempts: boot_run
                    .frame_attempts
                    .saturating_add(tail_run.frame_attempts)
                    .saturating_add(render_settle_run.frame_attempts),
                missed_vblank_frames: boot_run
                    .missed_vblank_frames
                    .saturating_add(tail_run.missed_vblank_frames)
                    .saturating_add(render_settle_run.missed_vblank_frames),
                observed_native_playable_candidate: boot_run.observed_native_playable_candidate
                    || tail_run.observed_native_playable_candidate
                    || render_settle_run.observed_native_playable_candidate,
                first_native_playable_frame: boot_run
                    .first_native_playable_frame
                    .or_else(|| {
                        tail_run
                            .first_native_playable_frame
                            .map(|frame| boot_run.total_frames.saturating_add(frame))
                    })
                    .or_else(|| {
                        render_settle_run.first_native_playable_frame.map(|frame| {
                            boot_run
                                .total_frames
                                .saturating_add(tail_run.total_frames)
                                .saturating_add(frame)
                        })
                    }),
                last_native_playable_frame: render_settle_run
                    .last_native_playable_frame
                    .map(|frame| {
                        boot_run
                            .total_frames
                            .saturating_add(tail_run.total_frames)
                            .saturating_add(frame)
                    })
                    .or_else(|| {
                        tail_run
                            .last_native_playable_frame
                            .map(|frame| boot_run.total_frames.saturating_add(frame))
                    })
                    .or(boot_run.last_native_playable_frame),
                stop_reason: tail_run.stop_reason,
            };
            let total_frames = checkpoint_run.total_frames;
            let checkpoint_native_playable_candidate =
                native_play_inferred_playable_candidate(&emulator);
            let observed_native_playable_candidate = checkpoint_run
                .observed_native_playable_candidate
                || checkpoint_native_playable_candidate;
            let mut first_native_playable_frame = checkpoint_run.first_native_playable_frame;
            let mut last_native_playable_frame = checkpoint_run.last_native_playable_frame;
            if checkpoint_native_playable_candidate {
                first_native_playable_frame.get_or_insert(total_frames);
                last_native_playable_frame = Some(total_frames);
            }
            let checkpoint = emulator.clone();
            let checkpoint_frame = native_play_window_frame_for_emulator_with_context(
                &checkpoint,
                checkpoint_native_playable_candidate,
            );
            let checkpoint_frame_stats = NativeFrameStats::from_frame(&checkpoint_frame);
            let checkpoint_frame_checksum = native_frame_checksum(&checkpoint_frame);
            let mut control_sweep = checkpoint.clone();
            let mut control_sweep_frames = 0u64;
            let mut control_sweep_missed_vblank_frames = 0u64;
            let checkpoint_activity = checkpoint.input_activity();
            let mut control_sweep_activity = NativeInputActivity::default();
            let mut control_sweep_native_playable_candidate = false;
            let mut branch_action_reads = 0usize;
            let mut branch_visual_changes = 0usize;
            let mut branches = Vec::new();
            if !emulator.is_terminal() {
                for &action in native_health_branch_actions() {
                    let mut branch = checkpoint.clone();
                    let branch_segments = [
                        NativeScriptSegment {
                            action,
                            frames: NATIVE_INPUT_CHECK_BRANCH_FRAMES,
                        },
                        NativeScriptSegment {
                            action: Action::Noop,
                            frames: NATIVE_INPUT_CHECK_BRANCH_SETTLE_FRAMES,
                        },
                    ];
                    let branch_run = run_native_script_observed_with_stop(
                        &mut branch,
                        script_instructions_per_frame,
                        &branch_segments,
                        NativeScriptStopMode::FastTimed,
                        Some(native_script_max_frames_for_segments(&branch_segments)),
                    )
                    .summary;
                    control_sweep = branch;
                    control_sweep_frames =
                        control_sweep_frames.saturating_add(branch_run.total_frames);
                    control_sweep_missed_vblank_frames = control_sweep_missed_vblank_frames
                        .saturating_add(branch_run.missed_vblank_frames);
                    control_sweep_activity = control_sweep_activity.saturating_added(
                        control_sweep
                            .input_activity()
                            .saturating_subtracted(checkpoint_activity),
                    );
                    let branch_native_playable_candidate =
                        native_play_inferred_playable_candidate(&control_sweep);
                    control_sweep_native_playable_candidate |= branch_native_playable_candidate;
                    let branch_activity = control_sweep.input_activity();
                    let branch_delta = branch_activity.saturating_subtracted(checkpoint_activity);
                    let action_read = native_action_activity_observed(branch_delta, action);
                    let branch_frame = native_play_window_frame_for_emulator_with_context(
                        &control_sweep,
                        branch_native_playable_candidate,
                    );
                    let branch_frame_stats = NativeFrameStats::from_frame(&branch_frame);
                    let branch_frame_checksum = native_frame_checksum(&branch_frame);
                    let visual_changed = branch_frame_checksum != checkpoint_frame_checksum;
                    if action_read {
                        branch_action_reads += 1;
                    }
                    if visual_changed {
                        branch_visual_changes += 1;
                    }
                    branches.push(format!(
                        "{{\"action_index\":{},\"action\":\"{}\",\"action_activity_observed\":{},\"visual_changed\":{},\"frame_checksum\":{},\"native_playable_candidate\":{},\"run\":{},\"input_delta\":{},\"frame\":{},\"state\":{}}}",
                        action.index(),
                        action.name(),
                        action_read,
                        visual_changed,
                        branch_frame_checksum,
                        branch_native_playable_candidate,
                        branch_run.json(),
                        branch_delta.json(),
                        branch_frame_stats.json(),
                        control_sweep.input_check_probe_json()
                    ));
                    if let Some(first) = branch_run.first_native_playable_frame {
                        first_native_playable_frame
                            .get_or_insert(total_frames.saturating_add(first));
                    }
                    if let Some(last) = branch_run.last_native_playable_frame {
                        last_native_playable_frame = Some(total_frames.saturating_add(last));
                    }
                }
            }
            let total_frames_with_sweep = total_frames + control_sweep_frames;
            let input_activity = checkpoint_activity.saturating_added(control_sweep_activity);
            let final_native_playable_candidate =
                native_play_inferred_playable_candidate(&checkpoint);
            let input_controls_active = input_activity.has_play_control_activity();
            let full_controls_active = input_activity.has_full_control_activity();
            let all_branch_actions_read =
                branch_action_reads == native_health_branch_actions().len();
            let branch_visual_response = branch_visual_changes > 0;
            let manual_checkpoint_ready = !checkpoint.is_terminal()
                && (native_play_frame_ready_with_context(
                    final_native_playable_candidate,
                    &checkpoint_frame,
                ) || native_play_gameplay_scene_with_context(
                    final_native_playable_candidate,
                    checkpoint_frame_stats,
                ));
            let branch_control_response =
                branch_visual_response || (manual_checkpoint_ready && all_branch_actions_read);
            let manual_controls_operable = manual_checkpoint_ready
                && all_branch_actions_read
                && branch_control_response
                && full_controls_active;
            let input_mapping_operable =
                all_branch_actions_read && branch_control_response && full_controls_active;
            let native_playability_confirmed =
                final_native_playable_candidate || control_sweep_native_playable_candidate;
            let playable = manual_controls_operable && native_playability_confirmed;
            let first_native_playable_frame = optional_u64_json(first_native_playable_frame);
            let last_native_playable_frame = optional_u64_json(last_native_playable_frame);
            println!(
                "{{\"instructions_per_frame\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"checkpoint_stop_reason\":\"{}\",\"control_sweep_frames\":{},\"control_sweep_missed_vblank_frames\":{},\"checkpoint_executed_steps\":{},\"control_sweep_executed_steps\":{},\"executed_steps\":{},\"input_activity\":{},\"native_playable_candidate\":{},\"observed_native_playable_candidate\":{},\"first_native_playable_frame\":{},\"last_native_playable_frame\":{},\"final_native_playable_candidate\":{},\"control_sweep_native_playable_candidate\":{},\"input_controls_active\":{},\"full_controls_active\":{},\"all_branch_actions_read\":{},\"branch_action_reads\":{},\"branch_count\":{},\"branch_visual_response\":{},\"branch_visual_changes\":{},\"branch_control_response\":{},\"input_mapping_operable\":{},\"manual_checkpoint_ready\":{},\"manual_controls_operable\":{},\"native_playability_confirmed\":{},\"checkpoint_frame_checksum\":{},\"checkpoint_frame\":{},\"playable\":{},\"branches\":[{}],\"state\":{}}}",
                instructions_per_frame,
                total_frames_with_sweep,
                checkpoint_run.missed_vblank_frames,
                checkpoint_run.stop_reason,
                control_sweep_frames,
                control_sweep_missed_vblank_frames,
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
                all_branch_actions_read,
                branch_action_reads,
                native_health_branch_actions().len(),
                branch_visual_response,
                branch_visual_changes,
                branch_control_response,
                input_mapping_operable,
                manual_checkpoint_ready,
                manual_controls_operable,
                native_playability_confirmed,
                checkpoint_frame_checksum,
                checkpoint_frame_stats.json(),
                playable,
                branches.join(","),
                checkpoint.input_check_probe_json()
            );
            if input_mapping_operable {
                Ok(())
            } else {
                Err("native input check failed: mapped controls or visual branch response was not observed".into())
            }
        }
        "native-health-check" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| NATIVE_PLAY_DEFAULT_INSTRUCTIONS_PER_FRAME_ARG.to_string())
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
            let wall_timeout_secs = args
                .next()
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "wall_timeout_secs must be a positive integer".to_string())
                })
                .transpose()?
                .unwrap_or(NATIVE_HEALTH_CHECK_WALL_TIMEOUT_SECS);
            run_native_health_check(
                rom,
                instructions_per_frame.max(1),
                branch_frames.max(1),
                settle_frames,
                wall_timeout_secs.max(1),
            )
        }
        "native-credit-mapping-probe" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| NATIVE_PLAY_DEFAULT_INSTRUCTIONS_PER_FRAME_ARG.to_string())
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let max_frames = args
                .next()
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "max_frames must be a positive integer".to_string())
                })
                .transpose()?
                .unwrap_or_else(|| {
                    native_script_max_frames_for_segments(&default_native_play_script())
                });
            let mappings = args.collect::<Vec<_>>();
            run_native_credit_mapping_probe(
                rom,
                instructions_per_frame.max(1),
                max_frames.max(1),
                mappings,
            )
        }
        "native-credit-bit-probe" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| NATIVE_PLAY_DEFAULT_INSTRUCTIONS_PER_FRAME_ARG.to_string())
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let max_frames =
                args.next()
                    .map(|value| {
                        value
                            .parse::<u64>()
                            .map_err(|_| "max_frames must be a positive integer".to_string())
                    })
                    .transpose()?
                    .unwrap_or_else(|| {
                        native_script_max_frames_for_segments(
                            &native_credit_mapping_probe_tail_script(),
                        )
                    });
            let mapping = args.next().unwrap_or_else(|| "legacy-zinc".to_string());
            let bits = parse_native_credit_bit_values(args.collect::<Vec<_>>())?;
            run_native_credit_bit_probe(
                rom,
                instructions_per_frame.max(1),
                max_frames.max(1),
                mapping,
                bits,
            )
        }
        "native-credit-projection-probe" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| NATIVE_PLAY_DEFAULT_INSTRUCTIONS_PER_FRAME_ARG.to_string())
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let max_frames =
                args.next()
                    .map(|value| {
                        value
                            .parse::<u64>()
                            .map_err(|_| "max_frames must be a positive integer".to_string())
                    })
                    .transpose()?
                    .unwrap_or_else(|| {
                        native_script_max_frames_for_segments(
                            &native_credit_mapping_probe_tail_script(),
                        )
                    });
            let mapping = args.next().unwrap_or_else(|| "legacy-zinc".to_string());
            let bit = args
                .next()
                .map(|value| parse_trace_u32(&value, "credit projection bit"))
                .transpose()?
                .unwrap_or(0x0000_0008);
            if bit == 0 {
                return Err("credit projection bit must be non-zero".to_string());
            }
            let projections = args.collect::<Vec<_>>();
            run_native_credit_projection_probe(
                rom,
                instructions_per_frame.max(1),
                max_frames.max(1),
                mapping,
                bit,
                projections,
            )
        }
        "native-credit-ram-probe" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| NATIVE_PLAY_DEFAULT_INSTRUCTIONS_PER_FRAME_ARG.to_string())
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
                .unwrap_or(120);
            let mapping = args.next().unwrap_or_else(|| "legacy-zinc".to_string());
            let projection = args.next().unwrap_or_else(|| "default".to_string());
            let bits = parse_native_credit_bit_values(args.collect::<Vec<_>>())?;
            run_native_credit_ram_probe(
                rom,
                instructions_per_frame.max(1),
                branch_frames.max(1),
                settle_frames,
                mapping,
                projection,
                bits,
            )
        }
        "native-credit-state-probe" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| NATIVE_PLAY_DEFAULT_INSTRUCTIONS_PER_FRAME_ARG.to_string())
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
                .unwrap_or(120);
            let actions = parse_native_credit_state_probe_actions(args.collect::<Vec<_>>())?;
            run_native_credit_state_probe(
                rom,
                instructions_per_frame.max(1),
                branch_frames.max(1),
                settle_frames,
                actions,
            )
        }
        "native-credit-timeline-probe" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| NATIVE_PLAY_DEFAULT_INSTRUCTIONS_PER_FRAME_ARG.to_string())
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let sample_stride = args
                .next()
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "sample_stride must be a positive integer".to_string())
                })
                .transpose()?
                .unwrap_or(1)
                .max(1);
            let mapping = args.next().unwrap_or_else(|| "auto".to_string());
            let projection = args.next().unwrap_or_else(|| "default".to_string());
            let segment_values = args.collect::<Vec<_>>();
            let segments = if segment_values.is_empty() {
                native_credit_mapping_probe_tail_script()
            } else {
                parse_native_script_segments(segment_values)?
            };
            run_native_credit_timeline_probe(
                rom,
                instructions_per_frame.max(1),
                sample_stride,
                mapping,
                projection,
                segments,
            )
        }
        "native-scripted-step" => {
            let rom = next_native_rom_source_or_default(&mut args);
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
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let max_frames = native_script_max_frames_for_segments(&segments);
            let script_progress = run_native_script_observed_with_stop(
                &mut emulator,
                instructions_per_frame,
                &segments,
                NativeScriptStopMode::None,
                Some(max_frames),
            );
            let script_run = script_progress.summary;
            let total_frames = script_run.total_frames;

            let raw_frame = emulator.display_frame();
            let window_frame = native_play_window_frame(&raw_frame);
            write_native_display_frame_png(&window_frame, &output)?;
            println!(
                "{{\"output\":\"{}\",\"output_kind\":\"native_play_window_frame\",\"raw_frame\":{},\"window_frame\":{},\"instructions_per_frame\":{},\"total_frames\":{},\"script_run\":{},\"executed_steps\":{},\"segments\":[{}],\"state\":{}}}",
                escape_json(&output.display().to_string()),
                NativeFrameStats::from_frame(&raw_frame).json(),
                NativeFrameStats::from_frame(&window_frame).json(),
                instructions_per_frame,
                total_frames,
                script_run.json(),
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                emulator.json()
            );
            Ok(())
        }
        "native-scripted-dump" => {
            let rom = next_native_rom_source_or_default(&mut args);
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
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let max_frames = native_script_max_frames_for_segments(&segments);
            let script_progress = run_native_script_observed_with_stop(
                &mut emulator,
                instructions_per_frame,
                &segments,
                NativeScriptStopMode::None,
                Some(max_frames),
            );
            let script_run = script_progress.summary;
            let total_frames = script_run.total_frames;
            let (
                actual_display_output,
                raw_actual_display_output,
                display_output,
                observation_output,
                vram_output,
            ) = write_native_snapshot(&emulator, &output_prefix)?;
            let raw_window_frame = emulator.display_frame();
            let window_frame = native_play_window_frame(&raw_window_frame);
            let window_output = suffixed_path(&output_prefix, "window.png");
            write_native_display_frame_png(&window_frame, &window_output)?;
            println!(
                "{{\"actual_display_output\":\"{}\",\"raw_actual_display_output\":\"{}\",\"display_output\":\"{}\",\"observation_output\":\"{}\",\"vram_output\":\"{}\",\"window_output\":\"{}\",\"window_frame\":{},\"instructions_per_frame\":{},\"max_frames\":{},\"total_frames\":{},\"script_run\":{},\"script_progress\":{},\"executed_steps\":{},\"segments\":[{}],\"sync\":{},\"input\":{},\"state\":{}}}",
                escape_json(&actual_display_output.display().to_string()),
                escape_json(&raw_actual_display_output.display().to_string()),
                escape_json(&display_output.display().to_string()),
                escape_json(&observation_output.display().to_string()),
                escape_json(&vram_output.display().to_string()),
                escape_json(&window_output.display().to_string()),
                NativeFrameStats::from_frame(&window_frame).json(),
                instructions_per_frame,
                max_frames,
                total_frames,
                script_run.json(),
                script_progress.json(),
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                emulator.native_sync_compact_json(),
                emulator.input_summary_json(),
                emulator.json()
            );
            Ok(())
        }
        "native-scripted-fast-dump" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-fast-dump <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let output_prefix = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-fast-dump <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>..."
                    .to_string()
            })?;
            let segments = parse_native_script_segments(args.collect::<Vec<_>>())?;
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let max_frames = native_script_max_frames_for_segments(&segments);
            let script_progress = run_native_script_observed_with_stop(
                &mut emulator,
                instructions_per_frame,
                &segments,
                NativeScriptStopMode::Timed,
                Some(max_frames),
            );
            let script_run = script_progress.summary;
            let total_frames = script_run.total_frames;
            let (
                actual_display_output,
                raw_actual_display_output,
                display_output,
                observation_output,
                vram_output,
            ) = write_native_snapshot(&emulator, &output_prefix)?;
            let raw_window_frame = emulator.display_frame();
            let window_frame = native_play_window_frame(&raw_window_frame);
            let window_output = suffixed_path(&output_prefix, "window.png");
            write_native_display_frame_png(&window_frame, &window_output)?;
            println!(
                "{{\"actual_display_output\":\"{}\",\"raw_actual_display_output\":\"{}\",\"display_output\":\"{}\",\"observation_output\":\"{}\",\"vram_output\":\"{}\",\"window_output\":\"{}\",\"window_frame\":{},\"instructions_per_frame\":{},\"max_frames\":{},\"total_frames\":{},\"script_run\":{},\"script_progress\":{},\"executed_steps\":{},\"segments\":[{}],\"state\":{}}}",
                escape_json(&actual_display_output.display().to_string()),
                escape_json(&raw_actual_display_output.display().to_string()),
                escape_json(&display_output.display().to_string()),
                escape_json(&observation_output.display().to_string()),
                escape_json(&vram_output.display().to_string()),
                escape_json(&window_output.display().to_string()),
                NativeFrameStats::from_frame(&window_frame).json(),
                instructions_per_frame,
                max_frames,
                total_frames,
                script_run.json(),
                script_progress.json(),
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                emulator.compact_probe_json()
            );
            Ok(())
        }
        "native-scripted-fast-summary" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-fast-summary <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let segments = parse_native_script_segments(args.collect::<Vec<_>>())?;
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let max_frames = native_script_max_frames_for_segments(&segments);
            let script_progress = run_native_script_observed_with_stop(
                &mut emulator,
                instructions_per_frame,
                &segments,
                NativeScriptStopMode::Timed,
                Some(max_frames),
            );
            let script_run = script_progress.summary;
            let total_frames = script_run.total_frames;
            let raw_window_frame = emulator.display_frame();
            let window_frame = native_play_window_frame(&raw_window_frame);
            println!(
                "{{\"instructions_per_frame\":{},\"max_frames\":{},\"total_frames\":{},\"script_run\":{},\"script_progress\":{},\"window_frame\":{},\"executed_steps\":{},\"segments\":[{}],\"diagnosis\":{}}}",
                instructions_per_frame,
                max_frames,
                total_frames,
                script_run.json(),
                script_progress.json(),
                NativeFrameStats::from_frame(&window_frame).json(),
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                emulator.input_check_probe_json()
            );
            Ok(())
        }
        "native-scripted-input-probe" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-input-probe <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let segments = parse_native_script_segments(args.collect::<Vec<_>>())?;
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let max_frames = native_script_max_frames_for_segments(&segments);
            let progress = run_native_script_observed_with_stop(
                &mut emulator,
                instructions_per_frame,
                &segments,
                NativeScriptStopMode::InputProbe,
                Some(max_frames),
            );
            println!(
                "{{\"instructions_per_frame\":{},\"max_frames\":{},\"progress\":{},\"executed_steps\":{},\"segments\":[{}],\"input\":{}}}",
                instructions_per_frame,
                max_frames,
                progress.json(),
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                emulator.input_compact_probe_json()
            );
            Ok(())
        }
        "native-scripted-code-dump" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-code-dump <rom_zip_or_dir> <instructions_per_frame> <address> <words> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let address = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-code-dump <rom_zip_or_dir> <instructions_per_frame> <address> <words> <action:frames>..."
                        .to_string()
                })
                .and_then(|raw| parse_trace_u32(&raw, "address"))?;
            let words = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-code-dump <rom_zip_or_dir> <instructions_per_frame> <address> <words> <action:frames>..."
                        .to_string()
                })?
                .parse::<usize>()
                .map_err(|_| "words must be a positive integer".to_string())?;
            if words == 0 {
                return Err("words must be greater than zero".to_string());
            }
            if words > 256 {
                return Err("words must be 256 or fewer for compact diagnostics".to_string());
            }
            let segments = parse_native_script_segments(args.collect::<Vec<_>>())?;
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let script_run =
                run_native_script_quiet_vblank(&mut emulator, instructions_per_frame, &segments);
            println!(
                "{{\"instructions_per_frame\":{},\"script_run\":{},\"segments\":[{}],\"base_address\":{},\"base_address_hex\":\"0x{:08x}\",\"physical_base_address\":{},\"physical_base_address_hex\":\"0x{:08x}\",\"words\":[{}],\"state\":{}}}",
                instructions_per_frame,
                script_run.json(),
                native_script_segments_json(&segments),
                address,
                address,
                native_debug_physical_address(address),
                native_debug_physical_address(address),
                native_code_words_json(&emulator, address, words),
                emulator.input_check_probe_json()
            );
            Ok(())
        }
        "native-scripted-mips-imm-search" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-mips-imm-search <rom_zip_or_dir> <instructions_per_frame> [limit] [imm...] -- [action:frames...]"
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let mut values = args.collect::<Vec<_>>();
            let limit = if values.first().is_some_and(|value| {
                !value.contains(':') && value != "--" && parse_u32_auto(value).is_ok()
            }) {
                values
                    .remove(0)
                    .parse::<usize>()
                    .map_err(|_| "limit must be a positive integer".to_string())?
            } else {
                NATIVE_MIPS_IMM_SEARCH_DEFAULT_LIMIT
            };
            if limit == 0 {
                return Err("limit must be greater than zero".to_string());
            }
            if limit > NATIVE_MIPS_IMM_SEARCH_MAX_LIMIT {
                return Err(format!(
                    "limit must be {} or fewer",
                    NATIVE_MIPS_IMM_SEARCH_MAX_LIMIT
                ));
            }
            let (immediates, segments) = parse_native_mips_imm_search_values(values)?;
            run_native_scripted_mips_imm_search(
                rom,
                instructions_per_frame.max(1),
                limit,
                immediates,
                segments,
            )
        }
        "native-scripted-candidates" => {
            let rom = next_native_rom_source_or_default(&mut args);
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
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let max_frames = native_script_max_frames_for_segments(&segments);
            let script_progress = run_native_script_observed_with_stop(
                &mut emulator,
                instructions_per_frame,
                &segments,
                NativeScriptStopMode::Timed,
                Some(max_frames),
            );
            let script_run = script_progress.summary;
            let total_frames = script_run.total_frames;
            let candidates = write_native_display_candidates(&emulator, &output_prefix)?;
            let (
                actual_display_output,
                raw_actual_display_output,
                display_output,
                observation_output,
                vram_output,
            ) = write_native_snapshot(&emulator, &output_prefix)?;
            let raw_window_frame = emulator.display_frame();
            let window_frame = native_play_window_frame(&raw_window_frame);
            let window_output = suffixed_path(&output_prefix, "window.png");
            write_native_display_frame_png(&window_frame, &window_output)?;
            println!(
                "{{\"candidate_outputs\":[{}],\"actual_display_output\":\"{}\",\"raw_actual_display_output\":\"{}\",\"display_output\":\"{}\",\"observation_output\":\"{}\",\"vram_output\":\"{}\",\"window_output\":\"{}\",\"raw_frame\":{},\"window_frame\":{},\"instructions_per_frame\":{},\"max_frames\":{},\"total_frames\":{},\"script_run\":{},\"script_progress\":{},\"executed_steps\":{},\"segments\":[{}],\"sync\":{},\"input\":{},\"display_diagnosis\":{},\"state\":{}}}",
                candidates.join(","),
                escape_json(&actual_display_output.display().to_string()),
                escape_json(&raw_actual_display_output.display().to_string()),
                escape_json(&display_output.display().to_string()),
                escape_json(&observation_output.display().to_string()),
                escape_json(&vram_output.display().to_string()),
                escape_json(&window_output.display().to_string()),
                NativeFrameStats::from_frame(&raw_window_frame).json(),
                NativeFrameStats::from_frame(&window_frame).json(),
                instructions_per_frame,
                max_frames,
                total_frames,
                script_run.json(),
                script_progress.json(),
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                emulator.native_sync_compact_json(),
                emulator.input_summary_json(),
                emulator.display_diagnosis_json(),
                emulator.probe_json()
            );
            Ok(())
        }
        "native-scripted-summary" => {
            let rom = next_native_rom_source_or_default(&mut args);
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
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let max_frames = native_script_max_frames_for_segments(&segments);
            let script_progress = run_native_script_observed_with_stop(
                &mut emulator,
                instructions_per_frame,
                &segments,
                NativeScriptStopMode::None,
                Some(max_frames),
            );
            let script_run = script_progress.summary;
            let total_frames = script_run.total_frames;
            println!(
                "{{\"instructions_per_frame\":{},\"max_frames\":{},\"total_frames\":{},\"script_run\":{},\"executed_steps\":{},\"segments\":[{}],\"state\":{}}}",
                instructions_per_frame,
                max_frames,
                total_frames,
                script_run.json(),
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                emulator.diagnostic_json()
            );
            Ok(())
        }
        "native-scripted-probe" => {
            let rom = next_native_rom_source_or_default(&mut args);
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
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let mut total_frames = 0u64;
            let mut missed_vblank_frames = 0u64;
            let mut probes = Vec::new();

            for (index, segment) in segments.iter().enumerate() {
                let segment_progress = run_native_script_observed_with_stop(
                    &mut emulator,
                    instructions_per_frame,
                    &[*segment],
                    NativeScriptStopMode::None,
                    Some(native_script_max_frames_for_segments(&[*segment])),
                );
                let segment_run = segment_progress.summary;
                total_frames += segment_run.total_frames;
                missed_vblank_frames += segment_run.missed_vblank_frames;

                probes.push(format!(
                    "{{\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"segment_frames\":{},\"segment_run\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"executed_steps\":{},\"state\":{}}}",
                    index,
                    segment.action.index(),
                    segment.action.name(),
                    segment.frames,
                    segment_run.json(),
                    total_frames,
                    missed_vblank_frames,
                    emulator.executed_steps(),
                    emulator.probe_json()
                ));

                if emulator.is_terminal() {
                    break;
                }
            }

            println!(
                "{{\"instructions_per_frame\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"executed_steps\":{},\"segments\":[{}],\"probes\":[{}],\"state\":{}}}",
                instructions_per_frame,
                total_frames,
                missed_vblank_frames,
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                probes.join(","),
                emulator.probe_json()
            );
            Ok(())
        }
        "native-scripted-frame-probe" => {
            let rom = next_native_rom_source_or_default(&mut args);
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
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let original_diagnostics_settings = apply_native_script_diagnostics(&mut emulator);
            let mut total_frames = 0u64;
            let mut missed_vblank_frames = 0u64;
            let mut probes = Vec::new();

            for (segment_index, segment) in segments.iter().enumerate() {
                emulator.set_input(segment.action.buttons());
                for frame_in_segment in 1..=segment.frames {
                    let vblank_advanced =
                        step_until_next_vblank_checked(&mut emulator, instructions_per_frame);
                    if !vblank_advanced {
                        missed_vblank_frames = missed_vblank_frames.saturating_add(1);
                    }

                    total_frames += 1;
                    if frame_in_segment % probe_stride == 0
                        || frame_in_segment == segment.frames
                        || emulator.is_terminal()
                    {
                        probes.push(format!(
                            "{{\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"frame_in_segment\":{},\"segment_frames\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"executed_steps\":{},\"state\":{}}}",
                            segment_index,
                            segment.action.index(),
                            segment.action.name(),
                            frame_in_segment,
                            segment.frames,
                            total_frames,
                            missed_vblank_frames,
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

            emulator.capture_vblank_presented_frame();
            restore_native_script_diagnostics(&mut emulator, original_diagnostics_settings);
            println!(
                "{{\"instructions_per_frame\":{},\"probe_stride_frames\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"executed_steps\":{},\"segments\":[{}],\"probes\":[{}],\"state\":{}}}",
                instructions_per_frame,
                probe_stride,
                total_frames,
                missed_vblank_frames,
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                probes.join(","),
                emulator.probe_json()
            );
            Ok(())
        }
        "native-scripted-compact-probe" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-compact-probe <rom_zip_or_dir> <instructions_per_frame> <probe_stride_frames> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let probe_stride = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-compact-probe <rom_zip_or_dir> <instructions_per_frame> <probe_stride_frames> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "probe_stride_frames must be a positive integer".to_string())?;
            if probe_stride == 0 {
                return Err("probe_stride_frames must be greater than zero".to_string());
            }
            let segments = parse_native_script_segments(args.collect::<Vec<_>>())?;
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let original_diagnostics_settings = apply_native_script_diagnostics(&mut emulator);
            let mut total_frames = 0u64;
            let mut missed_vblank_frames = 0u64;
            let mut probes = Vec::new();

            for (segment_index, segment) in segments.iter().enumerate() {
                emulator.set_input(segment.action.buttons());
                for frame_in_segment in 1..=segment.frames {
                    let vblank_advanced =
                        step_until_next_vblank_checked(&mut emulator, instructions_per_frame);
                    if !vblank_advanced {
                        missed_vblank_frames = missed_vblank_frames.saturating_add(1);
                    }

                    total_frames += 1;
                    if frame_in_segment % probe_stride == 0
                        || frame_in_segment == segment.frames
                        || emulator.is_terminal()
                    {
                        let frame_stats = NativeFrameStats::from_frame(&emulator.display_frame());
                        probes.push(format!(
                            "{{\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"frame_in_segment\":{},\"segment_frames\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"frame\":{},\"state\":{}}}",
                            segment_index,
                            segment.action.index(),
                            segment.action.name(),
                            frame_in_segment,
                            segment.frames,
                            total_frames,
                            missed_vblank_frames,
                            frame_stats.json(),
                            emulator.compact_probe_json()
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

            emulator.capture_vblank_presented_frame();
            restore_native_script_diagnostics(&mut emulator, original_diagnostics_settings);
            println!(
                "{{\"instructions_per_frame\":{},\"probe_stride_frames\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"executed_steps\":{},\"segments\":[{}],\"probes\":[{}],\"state\":{}}}",
                instructions_per_frame,
                probe_stride,
                total_frames,
                missed_vblank_frames,
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                probes.join(","),
                emulator.compact_probe_json()
            );
            Ok(())
        }
        "native-scripted-lean-probe" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-lean-probe <rom_zip_or_dir> <instructions_per_frame> <probe_stride_frames> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let probe_stride = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-lean-probe <rom_zip_or_dir> <instructions_per_frame> <probe_stride_frames> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "probe_stride_frames must be a positive integer".to_string())?;
            if probe_stride == 0 {
                return Err("probe_stride_frames must be greater than zero".to_string());
            }
            let segments = parse_native_script_segments(args.collect::<Vec<_>>())?;
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let original_diagnostics_settings = apply_native_script_diagnostics(&mut emulator);
            let mut total_frames = 0u64;
            let mut missed_vblank_frames = 0u64;
            let mut probes = Vec::new();

            for (segment_index, segment) in segments.iter().enumerate() {
                emulator.set_input(segment.action.buttons());
                for frame_in_segment in 1..=segment.frames {
                    let vblank_advanced =
                        step_until_next_vblank_checked(&mut emulator, instructions_per_frame);
                    if !vblank_advanced {
                        missed_vblank_frames = missed_vblank_frames.saturating_add(1);
                    }

                    total_frames = total_frames.saturating_add(1);
                    if frame_in_segment % probe_stride == 0
                        || frame_in_segment == segment.frames
                        || emulator.is_terminal()
                    {
                        let raw_frame = emulator.display_frame();
                        let window_frame = native_play_window_frame(&raw_frame);
                        probes.push(format!(
                            "{{\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"frame_in_segment\":{},\"segment_frames\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"raw_frame\":{},\"window_frame\":{},\"state\":{}}}",
                            segment_index,
                            segment.action.index(),
                            segment.action.name(),
                            frame_in_segment,
                            segment.frames,
                            total_frames,
                            missed_vblank_frames,
                            NativeFrameStats::from_frame(&raw_frame).json(),
                            NativeFrameStats::from_frame(&window_frame).json(),
                            emulator.lean_probe_json()
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

            emulator.capture_vblank_presented_frame();
            restore_native_script_diagnostics(&mut emulator, original_diagnostics_settings);
            let raw_frame = emulator.display_frame();
            let window_frame = native_play_window_frame(&raw_frame);
            println!(
                "{{\"instructions_per_frame\":{},\"probe_stride_frames\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"executed_steps\":{},\"segments\":[{}],\"raw_frame\":{},\"window_frame\":{},\"probes\":[{}],\"state\":{}}}",
                instructions_per_frame,
                probe_stride,
                total_frames,
                missed_vblank_frames,
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                NativeFrameStats::from_frame(&raw_frame).json(),
                NativeFrameStats::from_frame(&window_frame).json(),
                probes.join(","),
                emulator.lean_probe_json()
            );
            Ok(())
        }
        "native-display-diagnose" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-display-diagnose <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let segments = parse_native_script_segments(args.collect::<Vec<_>>())?;
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let original_diagnostics_settings = apply_native_script_diagnostics(&mut emulator);
            let mut total_frames = 0u64;
            let mut missed_vblank_frames = 0u64;
            let mut segment_reports = Vec::new();
            let started_at = Instant::now();
            let wall_timeout = native_script_wall_timeout(NativeScriptStopMode::Timed);
            let mut stop_reason = "script_completed";

            'script: for (segment_index, segment) in segments.iter().enumerate() {
                emulator.set_input(segment.action.buttons());
                let start_frames = total_frames;
                let start_missed = missed_vblank_frames;
                for _ in 0..segment.frames {
                    let step = step_until_next_vblank_checked_with_wall_timeout(
                        &mut emulator,
                        instructions_per_frame,
                        started_at,
                        wall_timeout,
                    );
                    if step.wall_timeout {
                        stop_reason = "wall_timeout";
                        break;
                    }
                    let vblank_advanced = step.vblank_advanced;
                    if !vblank_advanced {
                        missed_vblank_frames = missed_vblank_frames.saturating_add(1);
                    }
                    total_frames = total_frames.saturating_add(1);
                    if emulator.is_terminal() {
                        stop_reason = "terminal";
                        break;
                    }
                }

                let raw_frame = emulator.display_frame();
                let window_frame = native_play_window_frame(&raw_frame);
                segment_reports.push(format!(
                    "{{\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"segment_frames\":{},\"total_frames\":{},\"segment_missed_vblank_frames\":{},\"raw_frame\":{},\"window_frame\":{},\"diagnosis\":{}}}",
                    segment_index,
                    segment.action.index(),
                    segment.action.name(),
                    total_frames.saturating_sub(start_frames),
                    total_frames,
                    missed_vblank_frames.saturating_sub(start_missed),
                    NativeFrameStats::from_frame(&raw_frame).json(),
                    NativeFrameStats::from_frame(&window_frame).json(),
                    emulator.display_diagnosis_json()
                ));

                if emulator.is_terminal() {
                    break;
                }
                if stop_reason != "script_completed" {
                    break 'script;
                }
            }

            emulator.capture_vblank_presented_frame();
            restore_native_script_diagnostics(&mut emulator, original_diagnostics_settings);
            let raw_frame = emulator.display_frame();
            let window_frame = native_play_window_frame(&raw_frame);
            println!(
                "{{\"instructions_per_frame\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"stop_reason\":\"{}\",\"executed_steps\":{},\"segments\":[{}],\"raw_frame\":{},\"window_frame\":{},\"diagnosis\":{}}}",
                instructions_per_frame,
                total_frames,
                missed_vblank_frames,
                stop_reason,
                emulator.executed_steps(),
                segment_reports.join(","),
                NativeFrameStats::from_frame(&raw_frame).json(),
                NativeFrameStats::from_frame(&window_frame).json(),
                emulator.display_diagnosis_json()
            );
            Ok(())
        }
        "native-scripted-playability-diagnose" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-playability-diagnose <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let segments = parse_native_script_segments(args.collect::<Vec<_>>())?;
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let max_frames = native_script_max_frames_for_segments(&segments);
            let script_progress = run_native_script_observed_with_stop(
                &mut emulator,
                instructions_per_frame,
                &segments,
                NativeScriptStopMode::None,
                Some(max_frames),
            );
            let raw_frame = emulator.display_frame();
            let window_frame = native_play_window_frame(&raw_frame);

            println!(
                "{{\"instructions_per_frame\":{},\"total_frames\":{},\"script_progress\":{},\"executed_steps\":{},\"segments\":[{}],\"raw_frame\":{},\"window_frame\":{},\"diagnosis\":{}}}",
                instructions_per_frame,
                script_progress.summary.total_frames,
                script_progress.json(),
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                NativeFrameStats::from_frame(&raw_frame).json(),
                NativeFrameStats::from_frame(&window_frame).json(),
                emulator.full_playability_diagnosis_json()
            );
            Ok(())
        }
        "native-scripted-transition-diagnose" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-transition-diagnose <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let segments = parse_native_script_segments(args.collect::<Vec<_>>())?;
            run_native_scripted_transition_diagnose(rom, instructions_per_frame.max(1), segments)
        }
        "native-security-probe" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-security-probe <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let segments = parse_native_script_segments(args.collect::<Vec<_>>())?;
            run_native_security_probe(rom, instructions_per_frame.max(1), segments)
        }
        "native-compact-script-probe" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-compact-script-probe <rom_zip_or_dir> <instructions_per_frame> <emit_stride_frames> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let emit_stride = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-compact-script-probe <rom_zip_or_dir> <instructions_per_frame> <emit_stride_frames> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "emit_stride_frames must be a positive integer".to_string())?;
            if emit_stride == 0 {
                return Err("emit_stride_frames must be greater than zero".to_string());
            }
            let segments = parse_native_script_segments(args.collect::<Vec<_>>())?;
            run_native_compact_script_probe(
                rom,
                instructions_per_frame.max(1),
                emit_stride,
                segments,
            )
        }
        "native-scripted-live-probe" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-live-probe <rom_zip_or_dir> <instructions_per_frame> <emit_stride_frames> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let emit_stride = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-live-probe <rom_zip_or_dir> <instructions_per_frame> <emit_stride_frames> <action:frames>..."
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "emit_stride_frames must be a positive integer".to_string())?;
            if emit_stride == 0 {
                return Err("emit_stride_frames must be greater than zero".to_string());
            }
            let segments = parse_native_script_segments(args.collect::<Vec<_>>())?;
            run_native_scripted_live_probe(
                rom,
                instructions_per_frame.max(1),
                emit_stride,
                segments,
            )
        }
        "native-scripted-trace" => {
            let rom = next_native_rom_source_or_default(&mut args);
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
            let mut emulator = native_emulator_from_rom_source(rom)?;
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
        "native-scripted-vblank-trace" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-vblank-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <warmup_action:frames>... --trace <trace_action:frames>... [-- <trace options>]"
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let hot_limit = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-vblank-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <warmup_action:frames>... --trace <trace_action:frames>... [-- <trace options>]"
                        .to_string()
                })?
                .parse::<usize>()
                .map_err(|_| "hot_limit must be a non-negative integer".to_string())?;
            let recent_limit = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-vblank-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <warmup_action:frames>... --trace <trace_action:frames>... [-- <trace options>]"
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
            let (warmup_segments, trace_segments) =
                parse_native_script_trace_segments(raw_segments)?;
            let trace_options = parse_native_trace_options(raw_trace_options)?;
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let mut stdout = io::stdout();
            writeln!(
                stdout,
                "{{\"event\":\"start\",\"instructions_per_frame\":{},\"warmup_segments\":[{}],\"trace_segments\":[{}]}}",
                instructions_per_frame.max(1),
                native_script_segments_json(&warmup_segments),
                native_script_segments_json(&trace_segments)
            )
            .map_err(|error| format!("failed to write vblank trace start: {error}"))?;
            stdout
                .flush()
                .map_err(|error| format!("failed to flush vblank trace start: {error}"))?;

            let warmup_run = run_native_script_quiet_vblank(
                &mut emulator,
                instructions_per_frame.max(1),
                &warmup_segments,
            );
            writeln!(
                stdout,
                "{{\"event\":\"warmup\",\"warmup_run\":{},\"state\":{}}}",
                warmup_run.json(),
                emulator.input_check_probe_json()
            )
            .map_err(|error| format!("failed to write vblank trace warmup: {error}"))?;
            stdout
                .flush()
                .map_err(|error| format!("failed to flush vblank trace warmup: {error}"))?;

            let trace_instruction_limit =
                instructions_per_frame.clamp(1, NATIVE_VBLANK_TRACE_MAX_INSTRUCTIONS);
            let mut traced_frames = 0u64;
            let mut stopped_reason = "trace_completed";

            'trace: for (segment_index, segment) in trace_segments.iter().enumerate() {
                emulator.set_input(segment.action.buttons());
                for frame_in_segment in 1..=segment.frames {
                    let start_vblank = emulator.vblank_count();
                    let (trace, vblank_advanced) = emulator.trace_until_next_vblank(
                        trace_instruction_limit,
                        hot_limit,
                        recent_limit,
                        trace_options.clone(),
                    );
                    let end_vblank = emulator.vblank_count();
                    traced_frames = traced_frames.saturating_add(1);
                    let trace_json = if trace_options.watch_ranges.is_empty() {
                        trace.compact_json()
                    } else {
                        trace.json()
                    };
                    writeln!(
                        stdout,
                        "{{\"event\":\"trace_frame\",\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"frame_in_segment\":{},\"segment_frames\":{},\"traced_frames\":{},\"start_vblank\":{},\"end_vblank\":{},\"vblank_advanced\":{},\"trace\":{},\"native_sync\":{},\"state\":{}}}",
                        segment_index,
                        segment.action.index(),
                        segment.action.name(),
                        frame_in_segment,
                        segment.frames,
                        traced_frames,
                        start_vblank,
                        end_vblank,
                        vblank_advanced,
                        trace_json,
                        emulator.native_sync_compact_json(),
                        emulator.input_check_probe_json()
                    )
                    .map_err(|error| format!("failed to write vblank trace frame: {error}"))?;
                    stdout
                        .flush()
                        .map_err(|error| format!("failed to flush vblank trace frame: {error}"))?;
                    if !vblank_advanced {
                        stopped_reason = "missed_vblank";
                        break 'trace;
                    }
                    if emulator.is_terminal() {
                        stopped_reason = "terminal";
                        break 'trace;
                    }
                }
            }

            writeln!(
                stdout,
                "{{\"event\":\"finish\",\"instructions_per_frame\":{},\"trace_instruction_limit\":{},\"traced_frames\":{},\"stopped_reason\":\"{}\",\"state\":{}}}",
                instructions_per_frame.max(1),
                trace_instruction_limit,
                traced_frames,
                stopped_reason,
                emulator.input_check_probe_json()
            )
            .map_err(|error| format!("failed to write vblank trace finish: {error}"))?;
            stdout
                .flush()
                .map_err(|error| format!("failed to flush vblank trace finish: {error}"))?;
            Ok(())
        }
        "native-scripted-vblank-watch-trace" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-vblank-watch-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <warmup_action:frames>... --trace <trace_action:frames>... -- <trace options>"
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let hot_limit = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-vblank-watch-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <warmup_action:frames>... --trace <trace_action:frames>... -- <trace options>"
                        .to_string()
                })?
                .parse::<usize>()
                .map_err(|_| "hot_limit must be a non-negative integer".to_string())?;
            let recent_limit = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-vblank-watch-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <warmup_action:frames>... --trace <trace_action:frames>... -- <trace options>"
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
            let (warmup_segments, trace_segments) =
                parse_native_script_trace_segments(raw_segments)?;
            let mut trace_options = parse_native_trace_options(raw_trace_options)?;
            if trace_options.watch_ranges.is_empty() {
                return Err(
                    "native-scripted-vblank-watch-trace requires at least one --watch range"
                        .to_string(),
                );
            }
            trace_options.watch_only = true;
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let mut stdout = io::stdout();
            writeln!(
                stdout,
                "{{\"event\":\"start\",\"instructions_per_frame\":{},\"warmup_segments\":[{}],\"trace_segments\":[{}],\"watch_ranges\":{}}}",
                instructions_per_frame.max(1),
                native_script_segments_json(&warmup_segments),
                native_script_segments_json(&trace_segments),
                native_trace_watch_ranges_json(&trace_options.watch_ranges)
            )
            .map_err(|error| format!("failed to write watch trace start: {error}"))?;
            stdout
                .flush()
                .map_err(|error| format!("failed to flush watch trace start: {error}"))?;

            let warmup_run = run_native_script_quiet_vblank(
                &mut emulator,
                instructions_per_frame.max(1),
                &warmup_segments,
            );
            writeln!(
                stdout,
                "{{\"event\":\"warmup\",\"warmup_run\":{},\"input_state\":{},\"state\":{}}}",
                warmup_run.json(),
                emulator.input_compact_probe_json(),
                emulator.input_check_probe_json()
            )
            .map_err(|error| format!("failed to write watch trace warmup: {error}"))?;
            stdout
                .flush()
                .map_err(|error| format!("failed to flush watch trace warmup: {error}"))?;

            let trace_instruction_limit =
                instructions_per_frame.clamp(1, NATIVE_VBLANK_TRACE_MAX_INSTRUCTIONS);
            let mut traced_frames = 0u64;
            let mut stopped_reason = "trace_completed";

            'trace: for (segment_index, segment) in trace_segments.iter().enumerate() {
                emulator.set_input(segment.action.buttons());
                for frame_in_segment in 1..=segment.frames {
                    let start_vblank = emulator.vblank_count();
                    let (trace, vblank_advanced) = emulator.trace_until_next_vblank(
                        trace_instruction_limit,
                        hot_limit,
                        recent_limit,
                        trace_options.clone(),
                    );
                    let end_vblank = emulator.vblank_count();
                    traced_frames = traced_frames.saturating_add(1);
                    writeln!(
                        stdout,
                        "{{\"event\":\"trace_frame\",\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"frame_in_segment\":{},\"segment_frames\":{},\"traced_frames\":{},\"start_vblank\":{},\"end_vblank\":{},\"vblank_advanced\":{},\"trace_summary\":{},\"bus_access_trace\":[{}],\"native_sync\":{},\"input_state\":{},\"state\":{}}}",
                        segment_index,
                        segment.action.index(),
                        segment.action.name(),
                        frame_in_segment,
                        segment.frames,
                        traced_frames,
                        start_vblank,
                        end_vblank,
                        vblank_advanced,
                        trace.compact_json(),
                        trace.bus_access_trace_json(),
                        emulator.native_sync_compact_json(),
                        emulator.input_compact_probe_json(),
                        emulator.input_check_probe_json()
                    )
                    .map_err(|error| format!("failed to write watch trace frame: {error}"))?;
                    stdout
                        .flush()
                        .map_err(|error| format!("failed to flush watch trace frame: {error}"))?;
                    if !vblank_advanced {
                        stopped_reason = "missed_vblank";
                        break 'trace;
                    }
                    if emulator.is_terminal() {
                        stopped_reason = "terminal";
                        break 'trace;
                    }
                }
            }

            writeln!(
                stdout,
                "{{\"event\":\"finish\",\"instructions_per_frame\":{},\"trace_instruction_limit\":{},\"traced_frames\":{},\"stopped_reason\":\"{}\",\"input_state\":{},\"state\":{}}}",
                instructions_per_frame.max(1),
                trace_instruction_limit,
                traced_frames,
                stopped_reason,
                emulator.input_compact_probe_json(),
                emulator.input_check_probe_json()
            )
            .map_err(|error| format!("failed to write watch trace finish: {error}"))?;
            stdout
                .flush()
                .map_err(|error| format!("failed to flush watch trace finish: {error}"))?;
            Ok(())
        }
        "native-scripted-watch-followup-trace" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-watch-followup-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <followup_steps> <warmup_action:frames>... --trace <trace_action:frames>... -- <trace options>"
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let hot_limit = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-watch-followup-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <followup_steps> <warmup_action:frames>... --trace <trace_action:frames>... -- <trace options>"
                        .to_string()
                })?
                .parse::<usize>()
                .map_err(|_| "hot_limit must be a non-negative integer".to_string())?;
            let recent_limit = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-watch-followup-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <followup_steps> <warmup_action:frames>... --trace <trace_action:frames>... -- <trace options>"
                        .to_string()
                })?
                .parse::<usize>()
                .map_err(|_| "recent_limit must be a non-negative integer".to_string())?;
            let followup_steps = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-watch-followup-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <followup_steps> <warmup_action:frames>... --trace <trace_action:frames>... -- <trace options>"
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "followup_steps must be a positive integer".to_string())?;
            if followup_steps == 0 {
                return Err("followup_steps must be greater than zero".to_string());
            }
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
            let (warmup_segments, trace_segments) =
                parse_native_script_trace_segments(raw_segments)?;
            let (mut trace_options, followup_options) =
                parse_native_watch_followup_trace_options(raw_trace_options)?;
            if trace_options.watch_ranges.is_empty() {
                return Err(
                    "native-scripted-watch-followup-trace requires at least one --watch range"
                        .to_string(),
                );
            }
            trace_options.watch_only = true;
            trace_options.stop_watch_access = true;
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let warmup_run = run_native_script_quiet_vblank(
                &mut emulator,
                instructions_per_frame.max(1),
                &warmup_segments,
            );
            let trace_instruction_limit =
                instructions_per_frame.clamp(1, NATIVE_VBLANK_TRACE_MAX_INSTRUCTIONS);
            let mut traced_frames = 0u64;
            let mut stopped_reason = "trace_completed";
            let mut watch_trace_json = None;
            let mut followup_trace_json = None;
            let mut hit_segment_index = None;
            let mut hit_frame_in_segment = None;
            let mut hit_vblank = None;

            'trace: for (segment_index, segment) in trace_segments.iter().enumerate() {
                emulator.set_input(segment.action.buttons());
                for frame_in_segment in 1..=segment.frames {
                    let start_vblank = emulator.vblank_count();
                    let (trace, vblank_advanced) = emulator.trace_until_next_vblank(
                        trace_instruction_limit,
                        hot_limit,
                        recent_limit,
                        trace_options.clone(),
                    );
                    traced_frames = traced_frames.saturating_add(1);
                    let watch_hit = !trace.bus_access_trace_json().trim().is_empty();
                    if watch_hit {
                        hit_segment_index = Some(segment_index);
                        hit_frame_in_segment = Some(frame_in_segment);
                        hit_vblank = Some(start_vblank);
                        watch_trace_json = Some(trace.json());
                        let followup_trace = emulator.trace_instructions(
                            followup_steps,
                            hot_limit,
                            recent_limit,
                            followup_options.clone(),
                        );
                        followup_trace_json = Some(followup_trace.json());
                        stopped_reason = "watch_followup_complete";
                        break 'trace;
                    }
                    if !vblank_advanced {
                        stopped_reason = "missed_vblank_before_watch";
                        break 'trace;
                    }
                    if emulator.is_terminal() {
                        stopped_reason = "terminal_before_watch";
                        break 'trace;
                    }
                }
            }

            println!(
                "{{\"instructions_per_frame\":{},\"trace_instruction_limit\":{},\"followup_steps\":{},\"traced_frames\":{},\"stopped_reason\":\"{}\",\"hit_segment_index\":{},\"hit_frame_in_segment\":{},\"hit_vblank\":{},\"warmup_run\":{},\"warmup_segments\":[{}],\"trace_segments\":[{}],\"watch_ranges\":{},\"watch_trace\":{},\"followup_trace\":{},\"state\":{}}}",
                instructions_per_frame.max(1),
                trace_instruction_limit,
                followup_steps,
                traced_frames,
                stopped_reason,
                optional_usize_json(hit_segment_index),
                optional_u64_json(hit_frame_in_segment),
                optional_u64_json(hit_vblank),
                warmup_run.json(),
                native_script_segments_json(&warmup_segments),
                native_script_segments_json(&trace_segments),
                native_trace_watch_ranges_json(&trace_options.watch_ranges),
                watch_trace_json.unwrap_or_else(|| "null".to_string()),
                followup_trace_json.unwrap_or_else(|| "null".to_string()),
                emulator.input_check_probe_json()
            );
            Ok(())
        }
        "native-scripted-timeline" => {
            let rom = next_native_rom_source_or_default(&mut args);
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
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let mut total_frames = 0u64;
            let mut missed_vblank_frames = 0u64;
            let mut snapshots = Vec::new();

            for (index, segment) in segments.iter().enumerate() {
                let segment_run =
                    run_native_script_observed(&mut emulator, instructions_per_frame, &[*segment]);
                total_frames += segment_run.total_frames;
                missed_vblank_frames += segment_run.missed_vblank_frames;

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
                    "{{\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"segment_frames\":{},\"segment_run\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"executed_steps\":{},\"actual_display_output\":\"{}\",\"raw_actual_display_output\":\"{}\",\"display_output\":\"{}\",\"observation_output\":\"{}\",\"vram_output\":\"{}\"}}",
                    index,
                    segment.action.index(),
                    segment.action.name(),
                    segment.frames,
                    segment_run.json(),
                    total_frames,
                    missed_vblank_frames,
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
                "{{\"instructions_per_frame\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"executed_steps\":{},\"segments\":[{}],\"snapshots\":[{}],\"state\":{}}}",
                instructions_per_frame,
                total_frames,
                missed_vblank_frames,
                emulator.executed_steps(),
                native_script_segments_json(&segments),
                snapshots.join(","),
                emulator.json()
            );
            Ok(())
        }
        "native-scripted-branch" => {
            let rom = next_native_rom_source_or_default(&mut args);
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
            let mut checkpoint = native_emulator_from_rom_source(rom)?;
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
            let rom = next_native_rom_source_or_default(&mut args);
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
            let mut checkpoint = native_emulator_from_rom_source(rom)?;
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
                    branch.input_check_probe_json()
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
                checkpoint.input_check_probe_json(),
                branches.join(",")
            );
            Ok(())
        }
        "native-scripted-ram-diff" => {
            let rom = next_native_rom_source_or_default(&mut args);
            let instructions_per_frame = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-ram-diff <rom_zip_or_dir> <instructions_per_frame> <branch_frames> <settle_frames> <warmup_action:frames>... [--actions action...]"
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let branch_frames = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-ram-diff <rom_zip_or_dir> <instructions_per_frame> <branch_frames> <settle_frames> <warmup_action:frames>... [--actions action...]"
                        .to_string()
                })?
                .parse::<u64>()
                .map_err(|_| "branch_frames must be a positive integer".to_string())?;
            let settle_frames = args
                .next()
                .ok_or_else(|| {
                    "usage: bloodyroar2-gym native-scripted-ram-diff <rom_zip_or_dir> <instructions_per_frame> <branch_frames> <settle_frames> <warmup_action:frames>... [--actions action...]"
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
            let mut checkpoint = native_emulator_from_rom_source(rom)?;
            let warmup_run = run_native_script_quiet_vblank(
                &mut checkpoint,
                instructions_per_frame,
                &warmup_segments,
            );
            let checkpoint_ram = checkpoint.ram_snapshot();
            let checkpoint_scratchpad = checkpoint.scratchpad_snapshot();
            let checkpoint_activity = checkpoint.input_activity();
            let checkpoint_frame = native_play_window_frame(&checkpoint.display_frame());
            let checkpoint_frame_checksum = native_frame_checksum(&checkpoint_frame);

            let mut noop_baseline = checkpoint.clone();
            let noop_segments = [NativeScriptSegment {
                action: Action::Noop,
                frames: branch_frames.saturating_add(settle_frames),
            }];
            let noop_run = run_native_script_quiet_vblank(
                &mut noop_baseline,
                instructions_per_frame,
                &noop_segments,
            );
            let noop_ram = noop_baseline.ram_snapshot();
            let noop_scratchpad = noop_baseline.scratchpad_snapshot();
            let noop_frame = native_play_window_frame(&noop_baseline.display_frame());
            let noop_frame_checksum = native_frame_checksum(&noop_frame);
            let noop_activity = noop_baseline
                .input_activity()
                .saturating_subtracted(checkpoint_activity);
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
                let branch_run =
                    run_native_script_quiet_vblank(&mut branch, instructions_per_frame, &segments);
                let branch_ram = branch.ram_snapshot();
                let branch_scratchpad = branch.scratchpad_snapshot();
                let branch_frame = native_play_window_frame(&branch.display_frame());
                let branch_frame_checksum = native_frame_checksum(&branch_frame);
                let input_delta = branch
                    .input_activity()
                    .saturating_subtracted(checkpoint_activity);
                let input_delta_from_noop = branch
                    .input_activity()
                    .saturating_subtracted(noop_baseline.input_activity());

                branches.push(format!(
                    "{{\"action_index\":{},\"action\":\"{}\",\"branch_frames\":{},\"settle_frames\":{},\"run\":{},\"input_delta_from_checkpoint\":{},\"input_delta_from_noop\":{},\"frame_checksum\":{},\"frame_changed_from_checkpoint\":{},\"frame_changed_from_noop\":{},\"frame\":{},\"ram_diff_from_checkpoint\":{},\"ram_diff_from_noop\":{},\"scratchpad_diff_from_checkpoint\":{},\"scratchpad_diff_from_noop\":{},\"state\":{}}}",
                    action.index(),
                    action.name(),
                    branch_frames,
                    settle_frames,
                    branch_run.json(),
                    input_delta.json(),
                    input_delta_from_noop.json(),
                    branch_frame_checksum,
                    branch_frame_checksum != checkpoint_frame_checksum,
                    branch_frame_checksum != noop_frame_checksum,
                    NativeFrameStats::from_frame(&branch_frame).json(),
                    native_ram_diff_json(
                        &checkpoint_ram,
                        &branch_ram,
                        NATIVE_RAM_DIFF_MAX_RANGE_SAMPLES,
                        NATIVE_RAM_DIFF_MAX_WORD_SAMPLES
                    ),
                    native_ram_diff_json(
                        &noop_ram,
                        &branch_ram,
                        NATIVE_RAM_DIFF_MAX_RANGE_SAMPLES,
                        NATIVE_RAM_DIFF_MAX_WORD_SAMPLES
                    ),
                    native_scratchpad_diff_json(
                        &checkpoint_scratchpad,
                        &branch_scratchpad,
                        NATIVE_RAM_DIFF_MAX_RANGE_SAMPLES,
                        NATIVE_RAM_DIFF_MAX_WORD_SAMPLES
                    ),
                    native_scratchpad_diff_json(
                        &noop_scratchpad,
                        &branch_scratchpad,
                        NATIVE_RAM_DIFF_MAX_RANGE_SAMPLES,
                        NATIVE_RAM_DIFF_MAX_WORD_SAMPLES
                    ),
                    branch.input_check_probe_json()
                ));
            }

            println!(
                "{{\"instructions_per_frame\":{},\"warmup_run\":{},\"warmup_segments\":[{}],\"checkpoint_frame_checksum\":{},\"checkpoint_frame\":{},\"checkpoint_state\":{},\"noop_baseline\":{{\"run\":{},\"input_delta\":{},\"frame_checksum\":{},\"frame_changed_from_checkpoint\":{},\"frame\":{},\"ram_diff_from_checkpoint\":{},\"scratchpad_diff_from_checkpoint\":{},\"state\":{}}},\"branches\":[{}]}}",
                instructions_per_frame,
                warmup_run.json(),
                native_script_segments_json(&warmup_segments),
                checkpoint_frame_checksum,
                NativeFrameStats::from_frame(&checkpoint_frame).json(),
                checkpoint.input_check_probe_json(),
                noop_run.json(),
                noop_activity.json(),
                noop_frame_checksum,
                noop_frame_checksum != checkpoint_frame_checksum,
                NativeFrameStats::from_frame(&noop_frame).json(),
                native_ram_diff_json(
                    &checkpoint_ram,
                    &noop_ram,
                    NATIVE_RAM_DIFF_MAX_RANGE_SAMPLES,
                    NATIVE_RAM_DIFF_MAX_WORD_SAMPLES
                ),
                native_scratchpad_diff_json(
                    &checkpoint_scratchpad,
                    &noop_scratchpad,
                    NATIVE_RAM_DIFF_MAX_RANGE_SAMPLES,
                    NATIVE_RAM_DIFF_MAX_WORD_SAMPLES
                ),
                noop_baseline.input_check_probe_json(),
                branches.join(",")
            );
            Ok(())
        }
        "native-draw-snapshot" => {
            let rom = next_native_rom_source_or_default(&mut args);
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
            let mut emulator = native_emulator_from_rom_source(rom)?;
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
            let rom = next_native_rom_source_or_default(&mut args);
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
            let (arm_after_frames, raw_segments) =
                parse_native_scripted_draw_snapshot_args(args.collect::<Vec<_>>())?;
            let segments = parse_native_script_segments(raw_segments)?;
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let instructions_per_frame = instructions_per_frame.max(1);
            let warmup_progress = if arm_after_frames > 0 {
                run_native_script_observed_with_stop(
                    &mut emulator,
                    instructions_per_frame,
                    &segments,
                    NativeScriptStopMode::Timed,
                    Some(arm_after_frames),
                )
            } else {
                NativeScriptProgress::skipped("not_requested")
            };
            let capture_segments = if arm_after_frames > 0 {
                remaining_native_script_segments(
                    &segments,
                    warmup_progress.segment_index,
                    warmup_progress.segment_frame,
                )
            } else {
                segments.clone()
            };
            emulator.set_draw_capture_range(sequence_start, sequence_end);
            let max_frames = native_script_max_frames_for_segments(&capture_segments);
            let script_progress = run_native_script_observed_with_stop(
                &mut emulator,
                instructions_per_frame,
                &capture_segments,
                NativeScriptStopMode::DrawCaptureRange { sequence_end },
                Some(max_frames),
            );
            let script_run = script_progress.summary;
            let total_frames = script_run.total_frames;
            let captures = write_draw_captures(&emulator, &output_prefix)?;
            println!(
                "{{\"output_prefix\":\"{}\",\"instructions_per_frame\":{},\"arm_after_frames\":{},\"warmup_run\":{},\"warmup_progress\":{},\"max_frames\":{},\"total_frames\":{},\"script_run\":{},\"script_progress\":{},\"executed_steps\":{},\"sequence_start\":{},\"sequence_end\":{},\"capture_count\":{},\"segments\":[{}],\"capture_segments\":[{}],\"captures\":[{}],\"state\":{}}}",
                escape_json(&output_prefix.display().to_string()),
                instructions_per_frame,
                arm_after_frames,
                warmup_progress.summary.json(),
                warmup_progress.json(),
                max_frames,
                total_frames,
                script_run.json(),
                script_progress.json(),
                emulator.executed_steps(),
                sequence_start,
                sequence_end,
                emulator.draw_captures().len(),
                native_script_segments_json(&segments),
                native_script_segments_json(&capture_segments),
                captures.join(","),
                emulator.json()
            );
            Ok(())
        }
        "native-trace" => {
            let rom = next_native_rom_source_or_default(&mut args);
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
            let mut emulator = native_emulator_from_rom_source(rom)?;
            let trace = emulator.trace_instructions(count, hot_limit, recent_limit, trace_options);
            println!("{}", trace.json());
            Ok(())
        }
        "native-env-step" => {
            let rom = next_native_rom_source_or_default(&mut args);
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
    fast_forward_script: bool,
    stop_script_at_first_playable: bool,
) -> Result<(), String> {
    let (mut emulator, cache_report) =
        NativeEmulator::from_rom_zip_with_cache_report(rom).map_err(|error| error.to_string())?;
    if let Some(mapping) = emulator.apply_auto_coin_input_mapping() {
        eprintln!("native input mapping auto-selected: {mapping}");
    }
    eprintln!(
        "native ROM cache: used_cache={} cache_hit={} materialized={} resolved_path={}",
        cache_report.used_cache,
        cache_report.cache_hit,
        cache_report.materialized,
        cache_report.resolved_path.display()
    );
    let autoplay_enabled = !script_segments.is_empty();
    let mut rendered_frames = 0u64;
    let mut gui_rendered_frames = 0u64;
    let mut manual_override_frames = 0u64;
    let mut scripted_frames = 0u64;
    let mut scripted_missed_vblank_frames = 0u64;
    let mut manual_input_observed = false;
    let mut last_effective_buttons = ActionButtons::default();
    let initial_native_playable_candidate = emulator.native_playable_candidate();
    let initial_window_candidate_frame = native_play_window_frame_for_emulator_with_context(
        &emulator,
        initial_native_playable_candidate,
    );
    let mut observed_render_ready = native_play_render_ready_frame(&initial_window_candidate_frame);
    let mut first_render_ready_frame = observed_render_ready.then_some(rendered_frames);
    let mut last_render_ready_frame = first_render_ready_frame;
    let mut observed_native_playable_candidate = native_play_frame_ready_with_context(
        initial_native_playable_candidate,
        &initial_window_candidate_frame,
    );
    let mut first_native_playable_frame =
        observed_native_playable_candidate.then_some(rendered_frames);
    let mut last_native_playable_frame = first_native_playable_frame;
    let mut script_segment_index = 0usize;
    let mut script_segment_frame = 0u64;
    let mut boot_fast_forwarded = false;
    let mut boot_fast_forwarded_frames = 0u64;
    let mut boot_fast_forward_stop_reason = "not_requested";

    if autoplay_enabled && fast_forward_script {
        let fast_forward_instructions_per_frame =
            native_fast_forward_instructions_per_frame(instructions_per_frame);
        let fast_forward_script_frame_budget =
            native_script_max_frames_for_segments(&script_segments);
        let fast_forward_max_frames =
            NATIVE_PLAY_FAST_FORWARD_MAX_FRAMES.max(fast_forward_script_frame_budget);
        eprintln!(
            "native-play fast-forward starting: segments={} instructions_per_frame={} max_frames={}",
            script_segments.len(),
            fast_forward_instructions_per_frame,
            fast_forward_max_frames
        );
        let script_progress = if stop_script_at_first_playable {
            run_native_play_match_entry_fast_forward_progress(
                &mut emulator,
                fast_forward_instructions_per_frame,
                &script_segments,
                fast_forward_script_frame_budget,
            )
        } else {
            run_native_script_observed_with_stop(
                &mut emulator,
                fast_forward_instructions_per_frame,
                &script_segments,
                NativeScriptStopMode::None,
                Some(fast_forward_max_frames),
            )
        };
        let scripted_run = script_progress.summary;
        rendered_frames = scripted_run.total_frames;
        scripted_frames = scripted_run.total_frames;
        scripted_missed_vblank_frames = scripted_run.missed_vblank_frames;
        observed_native_playable_candidate = scripted_run.observed_native_playable_candidate;
        first_native_playable_frame = scripted_run.first_native_playable_frame;
        last_native_playable_frame = scripted_run.last_native_playable_frame;
        script_segment_index = script_progress.segment_index;
        script_segment_frame = script_progress.segment_frame;
        if stop_script_at_first_playable && observed_native_playable_candidate {
            script_segment_index = script_segments.len();
            script_segment_frame = 0;
            emulator.set_input(ActionButtons::default());
        }
        eprintln!(
            "native-play fast-forward: frames={} stop_reason={} first_playable={} last_playable={}",
            scripted_run.total_frames,
            scripted_run.stop_reason,
            optional_u64_json(scripted_run.first_native_playable_frame),
            optional_u64_json(scripted_run.last_native_playable_frame)
        );
    }
    if autoplay_enabled && !fast_forward_script {
        let boot_wait_segments = native_play_boot_wait_segments(&script_segments);
        if !boot_wait_segments.is_empty() {
            let fast_forward_instructions_per_frame =
                native_fast_forward_instructions_per_frame(instructions_per_frame);
            eprintln!(
                "native-play boot wait fast-forward starting: segments={} instructions_per_frame={}",
                boot_wait_segments.len(),
                fast_forward_instructions_per_frame
            );
            let script_progress = run_native_title_ready_boot_wait_with_timeout(
                &mut emulator,
                fast_forward_instructions_per_frame,
                &boot_wait_segments,
                None,
                "skipped_native_play_boot_timeout",
            );
            let scripted_run = script_progress.summary;
            boot_fast_forwarded = scripted_run.total_frames > 0;
            boot_fast_forwarded_frames = scripted_run.total_frames;
            boot_fast_forward_stop_reason = scripted_run.stop_reason;
            rendered_frames = rendered_frames.saturating_add(scripted_run.total_frames);
            scripted_frames = scripted_frames.saturating_add(scripted_run.total_frames);
            scripted_missed_vblank_frames =
                scripted_missed_vblank_frames.saturating_add(scripted_run.missed_vblank_frames);
            observed_native_playable_candidate |= scripted_run.observed_native_playable_candidate;
            if first_native_playable_frame.is_none() {
                first_native_playable_frame = scripted_run.first_native_playable_frame;
            }
            if scripted_run.last_native_playable_frame.is_some() {
                last_native_playable_frame = scripted_run.last_native_playable_frame;
            }
            if native_title_ready_boot_progress_allows_tail(&emulator, script_progress) {
                script_segment_index = boot_wait_segments.len();
                script_segment_frame = 0;
            } else {
                script_segment_index = script_segments.len();
                script_segment_frame = 0;
            }
            eprintln!(
                "native-play boot wait fast-forward: frames={} stop_reason={}",
                boot_fast_forwarded_frames, boot_fast_forward_stop_reason
            );
        }
    }
    let gui_instructions_per_frame = native_play_gui_instructions_per_frame(instructions_per_frame);
    emulator.set_vblank_presentation_capture_interval(Some(NATIVE_SCRIPT_VBLANK_CAPTURE_INTERVAL));
    emulator
        .set_unlinked_primitive_replay_interval(NATIVE_SCRIPT_UNLINKED_PRIMITIVE_REPLAY_INTERVAL);

    let mut gui_native_playable_candidate = emulator.native_playable_candidate();
    let mut gui_gpu_native_playable_candidate = emulator.gpu_native_playable_candidate();
    let mut gui_native_3d_gameplay_signal = emulator.native_3d_gameplay_signal();
    let initial_raw_frame =
        native_play_display_frame_with_context(&emulator, gui_native_playable_candidate);
    let initial_frame = native_play_window_frame_with_context(
        &initial_raw_frame,
        gui_native_playable_candidate,
        native_play_gpu_gameplay_context(&emulator),
    );
    let mut current_visible_window_frame = initial_frame.clone();
    let mut last_safe_window_frame =
        native_play_presentable_frame(&initial_frame).then_some(initial_frame.clone());
    let title = if autoplay_enabled && fast_forward_script {
        "Bloody Roar 2 native Rust - entry script fast-forwarded, manual keys active, Esc quit"
    } else if boot_fast_forwarded {
        "Bloody Roar 2 native Rust - warning wait skipped, keys layer with entry script, Esc quit"
    } else if autoplay_enabled {
        "Bloody Roar 2 native Rust - visible autoplay, keys layer with entry script, Esc quit"
    } else {
        "Bloody Roar 2 native Rust - arrows/WASD move, Z/Space/J punch, X/K kick, Q/L beast, E/I guard, C coin, V service, Enter coin+start, P start, Esc quit"
    };
    let mut window = Window::new(
        title,
        initial_frame.width,
        initial_frame.height,
        WindowOptions {
            resize: true,
            scale,
            scale_mode: ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    )
    .map_err(|error| format!("failed to create native play window: {error:?}"))?;
    window.set_target_fps(NATIVE_PLAY_WINDOW_TARGET_FPS);
    window.set_title(&native_play_runtime_title(
        autoplay_enabled,
        script_segments.get(script_segment_index).is_some(),
        false,
        ActionButtons::default(),
        gui_native_playable_candidate,
        gui_gpu_native_playable_candidate,
        gui_native_3d_gameplay_signal,
    ));
    let mut input_latch = NativeInputLatch::default();
    window
        .update_with_buffer(
            &initial_frame.pixels,
            initial_frame.width,
            initial_frame.height,
        )
        .map_err(|error| format!("failed to update native play window: {error:?}"))?;

    while window.is_open()
        && !window.is_key_down(Key::Escape)
        && !emulator.is_terminal()
        && max_frames.is_none_or(|max_frames| gui_rendered_frames < max_frames)
    {
        let scripted_action = script_segments
            .get(script_segment_index)
            .map(|segment| segment.action);
        let manual_override_enabled = true;
        let step = step_native_play_window_frame_checked(
            &mut emulator,
            gui_instructions_per_frame,
            &mut window,
            scripted_action,
            manual_override_enabled,
            &mut input_latch,
        )?;
        if step.window_closed_or_escape {
            break;
        }
        if step.vblank_advanced {
            emulator.capture_vblank_presented_frame();
        }
        if step.manual_override {
            manual_override_frames = manual_override_frames.saturating_add(1);
            manual_input_observed = true;
        }
        let buttons_changed = step.buttons != last_effective_buttons;
        if scripted_action.is_some() {
            if step.vblank_advanced {
                scripted_frames += 1;
                script_segment_frame += 1;
            } else {
                scripted_missed_vblank_frames = scripted_missed_vblank_frames.saturating_add(1);
            }
            if let Some(segment) = script_segments.get(script_segment_index)
                && script_segment_frame >= segment.frames
            {
                script_segment_index += 1;
                script_segment_frame = 0;
            }
        }
        let refresh_gui_playability = gui_rendered_frames == 0
            || buttons_changed
            || gui_rendered_frames % NATIVE_PLAY_GUI_PLAYABILITY_REFRESH_FRAMES == 0;
        if refresh_gui_playability {
            gui_native_playable_candidate = emulator.native_playable_candidate();
            gui_gpu_native_playable_candidate = emulator.gpu_native_playable_candidate();
            gui_native_3d_gameplay_signal = emulator.native_3d_gameplay_signal();
            window.set_title(&native_play_runtime_title(
                autoplay_enabled,
                scripted_action.is_some(),
                step.manual_override,
                step.buttons,
                gui_native_playable_candidate,
                gui_gpu_native_playable_candidate,
                gui_native_3d_gameplay_signal,
            ));
        }
        last_effective_buttons = step.buttons;
        let refresh_display = gui_rendered_frames == 0
            || buttons_changed
            || gui_rendered_frames % NATIVE_PLAY_GUI_DISPLAY_REFRESH_FRAMES == 0;
        if refresh_display {
            let candidate_frame = native_play_window_frame_for_emulator_with_context(
                &emulator,
                gui_native_playable_candidate,
            );
            let candidate_stats = NativeFrameStats::from_frame(&candidate_frame);
            current_visible_window_frame = native_play_select_visible_window_frame_for_emulator(
                &emulator,
                candidate_frame,
                candidate_stats,
                &last_safe_window_frame,
                &current_visible_window_frame,
            );
        }
        let frame = current_visible_window_frame.clone();
        let frame_render_ready = native_play_render_ready_frame(&frame);
        let frame_presentable = native_play_presentable_frame(&frame);
        window
            .update_with_buffer(&frame.pixels, frame.width, frame.height)
            .map_err(|error| format!("failed to update native play window: {error:?}"))?;
        rendered_frames += 1;
        gui_rendered_frames += 1;
        if frame_presentable {
            last_safe_window_frame = Some(frame.clone());
        }
        if frame_render_ready {
            observed_render_ready = true;
            first_render_ready_frame.get_or_insert(rendered_frames);
            last_render_ready_frame = Some(rendered_frames);
        }
        let handoff_frame_ready =
            native_play_frame_ready_with_context(gui_native_playable_candidate, &frame);
        if gui_native_playable_candidate && handoff_frame_ready {
            observed_native_playable_candidate = true;
            first_native_playable_frame.get_or_insert(rendered_frames);
            last_native_playable_frame = Some(rendered_frames);
        }
    }

    let final_native_playable_candidate = emulator.native_playable_candidate();
    let final_raw_frame = current_visible_window_frame.clone();
    let final_frame = current_visible_window_frame;
    let final_frame_stats = NativeFrameStats::from_frame(&final_frame);
    let final_gpu_verified_scene =
        native_play_gpu_verified_window_frame(&emulator, &final_frame, final_frame_stats);
    let final_frame_full_size = final_frame.width >= 512 && final_frame.height >= 480;
    let final_frame_visible_content = final_frame_stats.has_visible_content();
    let final_frame_scene_detail = final_frame_stats.has_scene_detail();
    let final_frame_context_ready =
        native_play_frame_ready_with_context(final_native_playable_candidate, &final_frame)
            || final_gpu_verified_scene;
    let final_frame_render_ready = native_play_render_ready_frame(&final_frame)
        || final_frame_context_ready
        || final_gpu_verified_scene;
    let final_frame_gameplay_scene =
        native_play_gameplay_scene_with_context(final_native_playable_candidate, final_frame_stats)
            || final_gpu_verified_scene;
    let final_frame_handoff_scene = final_frame_stats.has_handoff_scene();
    let final_frame_render_verified = final_frame_full_size
        && final_frame_render_ready
        && final_frame_gameplay_scene
        && final_frame_context_ready;
    let final_window_size = window.get_size();
    let input_activity = emulator.input_activity();
    let any_input_controls_active = input_activity.has_any_control_activity();
    let input_controls_active = input_activity.has_play_control_activity();
    let full_controls_active = input_activity.has_full_control_activity();
    let native_playable_observed_or_final = observed_native_playable_candidate
        || (final_native_playable_candidate && final_frame_render_verified);
    let native_play_any_input_verified =
        native_playable_observed_or_final && any_input_controls_active;
    let native_play_input_verified = native_playable_observed_or_final && input_controls_active;
    let native_play_full_input_verified = native_playable_observed_or_final && full_controls_active;
    let render_ready =
        (observed_render_ready || final_frame_render_verified) && final_frame_render_ready;
    let gameplay_ready = native_playable_observed_or_final
        && final_native_playable_candidate
        && final_frame_render_verified;
    let playable = native_playable_observed_or_final
        && final_native_playable_candidate
        && final_frame_render_verified;
    let scripted_target_frames = script_segments
        .iter()
        .map(|segment| segment.frames)
        .sum::<u64>();
    let autoplay_script_completed = autoplay_enabled
        && (scripted_frames >= scripted_target_frames
            || native_script_completed(
                &script_segments,
                script_segment_index,
                script_segment_frame,
            ));
    let first_native_playable_frame = optional_u64_json(first_native_playable_frame);
    let last_native_playable_frame = optional_u64_json(last_native_playable_frame);
    let first_render_ready_frame = optional_u64_json(first_render_ready_frame);
    let last_render_ready_frame = optional_u64_json(last_render_ready_frame);
    println!(
        "{{\"rendered_frames\":{},\"gui_rendered_frames\":{},\"manual_override_frames\":{},\"manual_input_observed\":{},\"last_effective_buttons\":{},\"executed_steps\":{},\"autoplay_enabled\":{},\"autoplay_fast_forwarded\":{},\"autoplay_stop_at_first_playable\":{},\"boot_fast_forwarded\":{},\"boot_fast_forwarded_frames\":{},\"boot_fast_forward_stop_reason\":\"{}\",\"autoplay_fast_forward_max_frames\":{},\"autoplay_fast_forward_instructions_per_frame\":{},\"gui_instructions_per_frame\":{},\"autoplay_script_completed\":{},\"autoplay_scripted_frames\":{},\"autoplay_missed_vblank_frames\":{},\"autoplay_segments\":[{}],\"initial_raw_frame\":{},\"initial_window_frame\":{},\"final_raw_frame\":{},\"final_window_size\":{{\"width\":{},\"height\":{}}},\"input_activity\":{},\"render_ready\":{},\"observed_render_ready\":{},\"first_render_ready_frame\":{},\"last_render_ready_frame\":{},\"gameplay_ready\":{},\"native_playable_candidate\":{},\"observed_native_playable_candidate\":{},\"first_native_playable_frame\":{},\"last_native_playable_frame\":{},\"final_native_playable_candidate\":{},\"any_input_controls_active\":{},\"input_controls_active\":{},\"full_controls_active\":{},\"native_play_any_input_verified\":{},\"native_play_input_verified\":{},\"native_play_full_input_verified\":{},\"final_frame_full_size\":{},\"final_frame_visible_content\":{},\"final_frame_scene_detail\":{},\"final_frame_render_ready\":{},\"final_frame_gameplay_scene\":{},\"final_frame_handoff_scene\":{},\"final_frame_render_verified\":{},\"final_frame\":{},\"playable\":{},\"state\":{}}}",
        rendered_frames,
        gui_rendered_frames,
        manual_override_frames,
        manual_input_observed,
        last_effective_buttons.json(),
        emulator.executed_steps(),
        autoplay_enabled,
        autoplay_enabled && fast_forward_script,
        autoplay_enabled && fast_forward_script && stop_script_at_first_playable,
        boot_fast_forwarded,
        boot_fast_forwarded_frames,
        boot_fast_forward_stop_reason,
        if autoplay_enabled && fast_forward_script {
            NATIVE_PLAY_FAST_FORWARD_MAX_FRAMES
                .max(native_script_max_frames_for_segments(&script_segments))
        } else {
            0
        },
        if autoplay_enabled && fast_forward_script {
            native_fast_forward_instructions_per_frame(instructions_per_frame)
        } else {
            0
        },
        gui_instructions_per_frame,
        autoplay_script_completed,
        scripted_frames,
        scripted_missed_vblank_frames,
        native_script_segments_json(&script_segments),
        NativeFrameStats::from_frame(&initial_raw_frame).json(),
        NativeFrameStats::from_frame(&initial_frame).json(),
        NativeFrameStats::from_frame(&final_raw_frame).json(),
        final_window_size.0,
        final_window_size.1,
        input_activity.json(),
        render_ready,
        observed_render_ready,
        first_render_ready_frame,
        last_render_ready_frame,
        gameplay_ready,
        final_native_playable_candidate,
        observed_native_playable_candidate,
        first_native_playable_frame,
        last_native_playable_frame,
        final_native_playable_candidate,
        any_input_controls_active,
        input_controls_active,
        full_controls_active,
        native_play_any_input_verified,
        native_play_input_verified,
        native_play_full_input_verified,
        final_frame_full_size,
        final_frame_visible_content,
        final_frame_scene_detail,
        final_frame_render_ready,
        final_frame_gameplay_scene,
        final_frame_handoff_scene,
        final_frame_render_verified,
        final_frame_stats.json(),
        playable,
        emulator.display_diagnosis_json()
    );
    Ok(())
}

fn run_native_play_match_entry_fast_forward_progress(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    segments: &[NativeScriptSegment],
    script_frame_budget: u64,
) -> NativeScriptProgress {
    let started_at = Instant::now();
    let wall_timeout = native_play_snapshot_wall_timeout(script_frame_budget);
    let boot_wait_segments = native_play_boot_wait_segments(segments);
    let boot_progress =
        if let Some(boot_timeout) = native_remaining_wall_timeout(started_at, wall_timeout) {
            run_native_title_ready_boot_wait_with_timeout(
                emulator,
                instructions_per_frame,
                &boot_wait_segments,
                Some(native_match_snapshot_boot_wall_timeout(boot_timeout)),
                "skipped_native_play_boot_timeout",
            )
        } else {
            NativeScriptProgress::skipped("skipped_native_play_boot_timeout")
        };

    let tail_segments = remaining_native_script_segments_after_title_ready(
        segments,
        boot_progress.segment_index,
        boot_progress.segment_frame,
    );
    let tail_progress = if boot_progress
        .summary
        .stop_reason
        .starts_with("skipped_native_play_boot")
        || !native_title_ready_boot_progress_allows_tail(emulator, boot_progress)
    {
        NativeScriptProgress::skipped("skipped_native_play_boot_timeout")
    } else if let Some(tail_timeout) = native_remaining_wall_timeout(started_at, wall_timeout) {
        run_native_script_observed_with_stop_and_timeout(
            emulator,
            instructions_per_frame,
            &tail_segments,
            NativeScriptStopMode::PlayableCandidate,
            Some(native_script_max_frames_for_segments(&tail_segments)),
            Some(tail_timeout),
        )
    } else {
        NativeScriptProgress::skipped("skipped_native_play_tail_timeout")
    };

    let tail_stopped_at_playable = matches!(
        tail_progress.summary.stop_reason,
        "playable_candidate_settled" | "playable_candidate_snapshot"
    );
    let render_settle_progress = if tail_stopped_at_playable {
        NativeScriptProgress::skipped("skipped_native_play_render_after_playable")
    } else if matches!(
        tail_progress.summary.stop_reason,
        "wall_timeout" | "skipped_native_play_boot_timeout" | "skipped_native_play_tail_timeout"
    ) {
        NativeScriptProgress::skipped("skipped_native_play_render_timeout")
    } else if let Some(render_timeout) = native_remaining_wall_timeout(started_at, wall_timeout) {
        run_native_script_observed_with_stop_and_timeout(
            emulator,
            instructions_per_frame,
            &[NativeScriptSegment {
                action: Action::Noop,
                frames: NATIVE_PLAY_SNAPSHOT_RENDER_SETTLE_FRAMES,
            }],
            NativeScriptStopMode::Timed,
            Some(NATIVE_PLAY_SNAPSHOT_RENDER_SETTLE_FRAMES),
            Some(render_timeout),
        )
    } else {
        NativeScriptProgress::skipped("skipped_native_play_render_timeout")
    };

    NativeScriptProgress {
        summary: combine_native_script_run_summaries(
            combine_native_script_run_summaries(boot_progress.summary, tail_progress.summary),
            render_settle_progress.summary,
        ),
        segment_index: segments.len(),
        segment_frame: 0,
    }
}

fn run_native_title_ready_boot_wait_with_timeout(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    boot_wait_segments: &[NativeScriptSegment],
    wall_timeout: Option<Duration>,
    skipped_reason: &'static str,
) -> NativeScriptProgress {
    let started_at = Instant::now();
    let boot_progress = if let Some(wall_timeout) = wall_timeout {
        if let Some(boot_timeout) = native_remaining_wall_timeout(started_at, wall_timeout) {
            run_native_script_observed_with_stop_and_timeout(
                emulator,
                instructions_per_frame,
                boot_wait_segments,
                NativeScriptStopMode::TitleScreen,
                Some(native_script_max_frames_for_segments(boot_wait_segments)),
                Some(boot_timeout),
            )
        } else {
            NativeScriptProgress::skipped(skipped_reason)
        }
    } else {
        run_native_script_observed_with_stop(
            emulator,
            instructions_per_frame,
            boot_wait_segments,
            NativeScriptStopMode::TitleScreen,
            Some(native_script_max_frames_for_segments(boot_wait_segments)),
        )
    };

    if native_title_ready_boot_progress_allows_tail(emulator, boot_progress)
        || native_script_run_hit_runtime_limit(boot_progress.summary)
        || emulator.is_terminal()
    {
        return native_title_ready_normalized_boot_progress(
            emulator,
            boot_wait_segments.len(),
            boot_progress,
        );
    }

    let wait_segment = [NativeScriptSegment {
        action: Action::Noop,
        frames: NATIVE_PLAY_TITLE_READY_EXTRA_WAIT_FRAMES,
    }];
    let wait_progress = if let Some(wall_timeout) = wall_timeout {
        if let Some(wait_timeout) = native_remaining_wall_timeout(started_at, wall_timeout) {
            run_native_script_observed_with_stop_and_timeout(
                emulator,
                instructions_per_frame,
                &wait_segment,
                NativeScriptStopMode::TitleScreen,
                Some(NATIVE_PLAY_TITLE_READY_EXTRA_WAIT_FRAMES),
                Some(wait_timeout),
            )
        } else {
            NativeScriptProgress::skipped(skipped_reason)
        }
    } else {
        run_native_script_observed_with_stop(
            emulator,
            instructions_per_frame,
            &wait_segment,
            NativeScriptStopMode::TitleScreen,
            Some(NATIVE_PLAY_TITLE_READY_EXTRA_WAIT_FRAMES),
        )
    };

    let combined = NativeScriptProgress {
        summary: combine_native_script_run_summaries(boot_progress.summary, wait_progress.summary),
        segment_index: boot_progress.segment_index,
        segment_frame: boot_progress.segment_frame,
    };
    native_title_ready_normalized_boot_progress(emulator, boot_wait_segments.len(), combined)
}

fn native_title_ready_normalized_boot_progress(
    emulator: &NativeEmulator,
    boot_wait_segment_len: usize,
    progress: NativeScriptProgress,
) -> NativeScriptProgress {
    if native_title_ready_boot_progress_allows_tail(emulator, progress) {
        NativeScriptProgress {
            segment_index: boot_wait_segment_len,
            segment_frame: 0,
            ..progress
        }
    } else {
        progress
    }
}

fn native_title_ready_boot_progress_allows_tail(
    emulator: &NativeEmulator,
    progress: NativeScriptProgress,
) -> bool {
    matches!(
        progress.summary.stop_reason,
        "title_screen_ready" | "handoff_frame_ready" | "playable_candidate_settled"
    ) || native_play_emulator_title_screen_input_ready(emulator)
        || native_play_emulator_handoff_ready(emulator)
}

#[allow(clippy::too_many_arguments)]
fn run_native_play_snapshot(
    rom: PathBuf,
    instructions_per_frame: u64,
    output_prefix: &Path,
    complete_script: bool,
    match_script: bool,
    require_gameplay_scene: bool,
    fast_forward_max_frames: u64,
    tail_segments: Vec<NativeScriptSegment>,
    full_state: bool,
    draw_capture_range: Option<(u64, u64)>,
    draw_capture_predicates: Vec<NativeGpuDrawCapturePredicate>,
    draw_capture_predicates_from_boot: bool,
) -> Result<(), String> {
    let mut emulator = native_emulator_from_rom_source(rom)?;
    if let Some((sequence_start, sequence_end)) = draw_capture_range {
        emulator.set_draw_capture_range(sequence_start, sequence_end);
    }
    if draw_capture_predicates_from_boot && !draw_capture_predicates.is_empty() {
        emulator.set_draw_capture_predicates(draw_capture_predicates.clone());
    }
    if let Some(mapping) = emulator.apply_auto_coin_input_mapping() {
        eprintln!("native play snapshot input mapping auto-selected: {mapping}");
    }
    let snapshot_started_at = Instant::now();
    let handoff_segments = native_snapshot_handoff_script(match_script);
    let handoff_fast_forward_max_frames =
        fast_forward_max_frames.max(native_script_max_frames_for_segments(&handoff_segments));
    let script_name = if match_script { "match" } else { "handoff" };
    let fast_forward_instructions_per_frame =
        native_snapshot_instructions_per_frame(instructions_per_frame);
    let snapshot_wall_timeout = native_play_snapshot_wall_timeout(handoff_fast_forward_max_frames);
    let handoff_wall_timeout =
        native_remaining_wall_timeout(snapshot_started_at, snapshot_wall_timeout);
    let boot_wait_segments = native_play_boot_wait_segments(&handoff_segments);
    let progress = if let Some(handoff_wall_timeout) = handoff_wall_timeout {
        if match_script && !boot_wait_segments.is_empty() {
            // Match snapshots must not burn the whole wall timeout waiting for
            // handoff heuristics before sending credit/start/selection input.
            // Run the title/warning skip plus a bounded dynamic title-ready wait,
            // then let the tail execute only after the title can accept input.
            let boot_wall_timeout = native_match_snapshot_boot_wall_timeout(handoff_wall_timeout);
            run_native_title_ready_boot_wait_with_timeout(
                &mut emulator,
                fast_forward_instructions_per_frame,
                &boot_wait_segments,
                Some(boot_wall_timeout),
                "skipped_snapshot_wall_timeout",
            )
        } else if match_script {
            // Full-entry validation must press later direction/attack controls
            // even when the playable heuristic is still failing.
            run_native_script_observed_with_stop_and_timeout(
                &mut emulator,
                fast_forward_instructions_per_frame,
                &handoff_segments,
                NativeScriptStopMode::Timed,
                Some(handoff_fast_forward_max_frames),
                Some(handoff_wall_timeout),
            )
        } else {
            run_native_script_observed_until_playable_with_limit_and_timeout(
                &mut emulator,
                fast_forward_instructions_per_frame,
                &handoff_segments,
                handoff_fast_forward_max_frames,
                handoff_wall_timeout,
            )
        }
    } else {
        NativeScriptProgress::skipped("skipped_snapshot_wall_timeout")
    };
    let boot_summary = progress.summary;
    let boot_prefix = suffixed_path(output_prefix, "boot-playable");
    let (
        boot_actual_display_output,
        boot_raw_actual_display_output,
        boot_display_output,
        boot_observation_output,
        boot_vram_output,
    ) = write_native_snapshot(&emulator, &boot_prefix)?;
    let boot_window_frame = native_play_window_frame_for_emulator(&emulator);
    let boot_window_output = suffixed_path(&boot_prefix, "window.png");
    write_native_display_frame_png(&boot_window_frame, &boot_window_output)?;
    let snapshot_safe_window_frame =
        native_play_presentable_frame(&boot_window_frame).then_some(boot_window_frame.clone());
    let boot_native_playable_candidate = emulator.native_playable_candidate();
    let boot_frame_stats = NativeFrameStats::from_frame(&boot_window_frame);
    let handoff_frame_gameplay_scene = boot_frame_stats.has_gameplay_scene();
    let handoff_frame_handoff_scene = boot_frame_stats.has_handoff_scene();
    let handoff_frame_full_size = boot_frame_stats.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
        && boot_frame_stats.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT;
    let handoff_frame_scene_accepted =
        native_play_frame_ready_with_context(boot_native_playable_candidate, &boot_window_frame);
    let handoff_render_verified = handoff_frame_full_size && handoff_frame_scene_accepted;
    let handoff_playable =
        boot_summary.observed_native_playable_candidate && handoff_render_verified;
    if !draw_capture_predicates_from_boot && !draw_capture_predicates.is_empty() {
        emulator.set_draw_capture_predicates(draw_capture_predicates.clone());
    }

    let has_explicit_tail_segments = !tail_segments.is_empty();
    let title_ready_for_tail = native_title_ready_boot_progress_allows_tail(&emulator, progress);
    let handoff_satisfies_required_stage =
        handoff_playable && (!require_gameplay_scene || handoff_frame_gameplay_scene);
    let mut effective_tail_segments = if complete_script
        && title_ready_for_tail
        && (has_explicit_tail_segments || !handoff_satisfies_required_stage)
    {
        if match_script {
            remaining_native_script_segments_after_title_ready(
                &handoff_segments,
                progress.segment_index,
                progress.segment_frame,
            )
        } else {
            remaining_native_script_segments(
                &handoff_segments,
                progress.segment_index,
                progress.segment_frame,
            )
        }
    } else {
        Vec::new()
    };
    effective_tail_segments.extend(tail_segments);
    let final_stage_required = match_script || !effective_tail_segments.is_empty();
    let tail_run_json = if effective_tail_segments.is_empty() {
        "null".to_string()
    } else {
        let tail_max_frames = fast_forward_max_frames.max(native_script_max_frames_for_segments(
            &effective_tail_segments,
        ));
        if let Some(tail_wall_timeout) =
            native_remaining_wall_timeout(snapshot_started_at, snapshot_wall_timeout)
        {
            run_native_script_observed_with_stop_and_timeout(
                &mut emulator,
                fast_forward_instructions_per_frame,
                &effective_tail_segments,
                NativeScriptStopMode::FastTimed,
                Some(tail_max_frames),
                Some(tail_wall_timeout),
            )
            .summary
            .json()
        } else {
            NativeScriptRunSummary::skipped("skipped_snapshot_wall_timeout").json()
        }
    };
    let render_settle_run_json = if final_stage_required {
        if let Some(render_wall_timeout) =
            native_remaining_wall_timeout(snapshot_started_at, snapshot_wall_timeout)
        {
            run_native_script_observed_with_stop_and_timeout(
                &mut emulator,
                fast_forward_instructions_per_frame,
                &[NativeScriptSegment {
                    action: Action::Noop,
                    frames: NATIVE_PLAY_SNAPSHOT_RENDER_SETTLE_FRAMES,
                }],
                NativeScriptStopMode::Timed,
                Some(NATIVE_PLAY_SNAPSHOT_RENDER_SETTLE_FRAMES),
                Some(render_wall_timeout),
            )
            .summary
            .json()
        } else {
            NativeScriptRunSummary::skipped("skipped_snapshot_wall_timeout").json()
        }
    } else {
        "null".to_string()
    };

    let (
        actual_display_output,
        raw_actual_display_output,
        display_output,
        observation_output,
        vram_output,
    ) = write_native_snapshot(&emulator, output_prefix)?;
    let draw_capture_requested =
        draw_capture_range.is_some() || !draw_capture_predicates.is_empty();
    let draw_captures = if draw_capture_requested {
        write_draw_captures(&emulator, output_prefix)?
    } else {
        Vec::new()
    };
    let draw_capture_range_json = draw_capture_range.map_or_else(
        || "null".to_string(),
        |(start, end)| format!("{{\"sequence_start\":{},\"sequence_end\":{}}}", start, end),
    );
    let draw_capture_predicates_json = draw_capture_predicates
        .iter()
        .map(NativeGpuDrawCapturePredicate::json)
        .collect::<Vec<_>>()
        .join(",");
    let final_window_candidate = native_play_window_frame_for_emulator(&emulator);
    let final_window_candidate_stats = NativeFrameStats::from_frame(&final_window_candidate);
    let final_window_candidate_output = suffixed_path(output_prefix, "candidate-window.png");
    write_native_display_frame_png(&final_window_candidate, &final_window_candidate_output)?;
    let final_window_frame = native_play_select_visible_window_frame_for_emulator(
        &emulator,
        final_window_candidate,
        final_window_candidate_stats,
        &snapshot_safe_window_frame,
        &boot_window_frame,
    );
    let window_output = suffixed_path(output_prefix, "window.png");
    write_native_display_frame_png(&final_window_frame, &window_output)?;
    let final_frame_stats = NativeFrameStats::from_frame(&final_window_frame);
    let final_native_playable_candidate = emulator.native_playable_candidate();
    let final_gpu_verified_scene =
        native_play_gpu_verified_window_frame(&emulator, &final_window_frame, final_frame_stats);
    let final_frame_gameplay_scene =
        native_play_gameplay_scene_with_context(final_native_playable_candidate, final_frame_stats)
            || final_gpu_verified_scene;
    let final_frame_handoff_scene = final_frame_stats.has_handoff_scene();
    let final_frame_context_scene =
        native_play_frame_ready_with_context(final_native_playable_candidate, &final_window_frame)
            || final_gpu_verified_scene;
    let final_render_ready_scene = final_frame_stats.has_render_ready_scene()
        || final_frame_context_scene
        || final_gpu_verified_scene;
    let final_render_verified = final_frame_stats.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
        && final_frame_stats.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT
        && final_render_ready_scene
        && if require_gameplay_scene {
            final_frame_gameplay_scene
        } else {
            final_frame_gameplay_scene || final_frame_handoff_scene || final_frame_context_scene
        };
    let final_playable = final_native_playable_candidate && final_render_verified;
    let required_playable = if final_stage_required {
        final_playable
    } else {
        handoff_playable
    };
    let required_frame_gameplay_scene = if final_stage_required {
        final_frame_gameplay_scene
    } else {
        handoff_frame_gameplay_scene
    };
    let required_stage = if final_stage_required && require_gameplay_scene {
        "gameplay_scene"
    } else if final_stage_required {
        "final"
    } else if require_gameplay_scene {
        "gameplay_handoff"
    } else {
        "handoff"
    };
    let final_frame_full_size = final_frame_stats.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
        && final_frame_stats.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT;
    let final_frame_scene_accepted = if require_gameplay_scene {
        final_frame_gameplay_scene
    } else {
        final_frame_gameplay_scene || final_frame_handoff_scene
    };
    let input_activity = emulator.input_activity();
    let br2_credit_hle_accepted_seen = emulator.br2_native_credit_hle_accepted_seen();
    let rom_compatibility = emulator.rom_compatibility();
    let assets_complete = rom_compatibility.native_runtime_usable();
    let exact_mame_assets = rom_compatibility.compatible();
    let rom_compatibility_json = rom_compatibility.summary_json();
    let security_state_json = emulator.security_probe_json();
    let blocker_report_json = native_play_blocker_report_json(
        required_playable,
        assets_complete,
        exact_mame_assets,
        input_activity,
        handoff_playable,
        handoff_render_verified,
        final_stage_required,
        final_native_playable_candidate,
        final_render_verified,
        final_frame_full_size,
        final_frame_scene_accepted,
        final_frame_gameplay_scene,
        final_frame_handoff_scene,
        require_gameplay_scene,
        &final_frame_stats,
        br2_credit_hle_accepted_seen,
    );

    let summary_output = suffixed_path(output_prefix, "summary.json");
    let state_output = full_state.then(|| suffixed_path(output_prefix, "state.json"));
    let state_output_json = state_output.as_ref().map_or_else(
        || "null".to_string(),
        |path| format!("\"{}\"", escape_json(&path.display().to_string())),
    );
    let final_state_json = if full_state {
        emulator.json()
    } else {
        emulator.display_diagnosis_json()
    };
    if let Some(state_output) = &state_output {
        write_output_file(state_output, final_state_json.as_bytes())?;
    }
    let summary_json = format!(
        "{{\"output_prefix\":\"{}\",\"summary_output\":\"{}\",\"state_output\":{},\"instructions_per_frame\":{},\"fast_forward_instructions_per_frame\":{},\"fast_forward_max_frames\":{},\"script_name\":\"{}\",\"complete_script\":{},\"match_script\":{},\"require_gameplay_scene\":{},\"required_stage\":\"{}\",\"handoff_playable\":{},\"final_playable\":{},\"final_render_verified\":{},\"playable\":{},\"assets_complete\":{},\"exact_mame_assets\":{},\"rom_compatibility\":{},\"blocker_report\":{},\"boot\":{{\"run\":{},\"window_output\":\"{}\",\"window_frame\":{},\"full_size\":{},\"scene_accepted\":{},\"render_verified\":{},\"gameplay_scene\":{},\"handoff_scene\":{},\"playable\":{}}},\"tail_segments\":[{}],\"tail_run\":{},\"render_settle_run\":{},\"draw_capture_range\":{},\"draw_capture_predicates\":[{}],\"draw_captures\":[{}],\"final\":{{\"actual_display_output\":\"{}\",\"raw_actual_display_output\":\"{}\",\"display_output\":\"{}\",\"observation_output\":\"{}\",\"vram_output\":\"{}\",\"window_output\":\"{}\",\"window_frame\":{},\"full_size\":{},\"scene_accepted\":{},\"render_verified\":{},\"gameplay_scene\":{},\"handoff_scene\":{},\"native_playable_candidate\":{},\"playable\":{}}},\"input_activity\":{},\"security_state\":{},\"display_diagnosis\":{},\"executed_steps\":{}}}",
        escape_json(&output_prefix.display().to_string()),
        escape_json(&summary_output.display().to_string()),
        state_output_json,
        instructions_per_frame,
        fast_forward_instructions_per_frame,
        handoff_fast_forward_max_frames,
        script_name,
        complete_script,
        match_script,
        require_gameplay_scene,
        required_stage,
        handoff_playable,
        final_playable,
        final_render_verified,
        required_playable,
        assets_complete,
        exact_mame_assets,
        rom_compatibility_json,
        blocker_report_json,
        boot_summary.json(),
        escape_json(&boot_window_output.display().to_string()),
        boot_frame_stats.json(),
        handoff_frame_full_size,
        handoff_frame_scene_accepted,
        handoff_render_verified,
        handoff_frame_gameplay_scene,
        handoff_frame_handoff_scene,
        handoff_playable,
        native_script_segments_json(&effective_tail_segments),
        tail_run_json,
        render_settle_run_json,
        draw_capture_range_json,
        draw_capture_predicates_json,
        draw_captures.join(","),
        escape_json(&actual_display_output.display().to_string()),
        escape_json(&raw_actual_display_output.display().to_string()),
        escape_json(&display_output.display().to_string()),
        escape_json(&observation_output.display().to_string()),
        escape_json(&vram_output.display().to_string()),
        escape_json(&window_output.display().to_string()),
        final_frame_stats.json(),
        final_frame_full_size,
        final_frame_scene_accepted,
        final_render_verified,
        final_frame_gameplay_scene,
        final_frame_handoff_scene,
        final_native_playable_candidate,
        final_playable,
        input_activity.json(),
        security_state_json,
        emulator.display_diagnosis_json(),
        emulator.executed_steps()
    );
    write_output_file(&summary_output, summary_json.as_bytes())?;

    println!(
        "{{\"output_prefix\":\"{}\",\"summary_output\":\"{}\",\"state_output\":{},\"instructions_per_frame\":{},\"fast_forward_instructions_per_frame\":{},\"fast_forward_max_frames\":{},\"script_name\":\"{}\",\"complete_script\":{},\"match_script\":{},\"require_gameplay_scene\":{},\"required_stage\":\"{}\",\"handoff_playable\":{},\"final_playable\":{},\"final_render_verified\":{},\"playable\":{},\"assets_complete\":{},\"exact_mame_assets\":{},\"rom_compatibility\":{},\"blocker_report\":{},\"boot\":{{\"run\":{},\"actual_display_output\":\"{}\",\"raw_actual_display_output\":\"{}\",\"display_output\":\"{}\",\"observation_output\":\"{}\",\"vram_output\":\"{}\",\"window_output\":\"{}\",\"window_frame\":{},\"full_size\":{},\"scene_accepted\":{},\"render_verified\":{},\"gameplay_scene\":{},\"handoff_scene\":{},\"playable\":{}}},\"tail_segments\":[{}],\"tail_run\":{},\"render_settle_run\":{},\"draw_capture_range\":{},\"draw_capture_predicates\":[{}],\"draw_captures\":[{}],\"final\":{{\"actual_display_output\":\"{}\",\"raw_actual_display_output\":\"{}\",\"display_output\":\"{}\",\"observation_output\":\"{}\",\"vram_output\":\"{}\",\"window_output\":\"{}\",\"window_frame\":{},\"full_size\":{},\"scene_accepted\":{},\"render_verified\":{},\"gameplay_scene\":{},\"handoff_scene\":{},\"native_playable_candidate\":{},\"playable\":{}}},\"input_activity\":{},\"security_state\":{},\"executed_steps\":{},\"state\":{}}}",
        escape_json(&output_prefix.display().to_string()),
        escape_json(&summary_output.display().to_string()),
        state_output_json,
        instructions_per_frame,
        fast_forward_instructions_per_frame,
        handoff_fast_forward_max_frames,
        script_name,
        complete_script,
        match_script,
        require_gameplay_scene,
        required_stage,
        handoff_playable,
        final_playable,
        final_render_verified,
        required_playable,
        assets_complete,
        exact_mame_assets,
        rom_compatibility_json,
        blocker_report_json,
        boot_summary.json(),
        escape_json(&boot_actual_display_output.display().to_string()),
        escape_json(&boot_raw_actual_display_output.display().to_string()),
        escape_json(&boot_display_output.display().to_string()),
        escape_json(&boot_observation_output.display().to_string()),
        escape_json(&boot_vram_output.display().to_string()),
        escape_json(&boot_window_output.display().to_string()),
        boot_frame_stats.json(),
        handoff_frame_full_size,
        handoff_frame_scene_accepted,
        handoff_render_verified,
        handoff_frame_gameplay_scene,
        handoff_frame_handoff_scene,
        handoff_playable,
        native_script_segments_json(&effective_tail_segments),
        tail_run_json,
        render_settle_run_json,
        draw_capture_range_json,
        draw_capture_predicates_json,
        draw_captures.join(","),
        escape_json(&actual_display_output.display().to_string()),
        escape_json(&raw_actual_display_output.display().to_string()),
        escape_json(&display_output.display().to_string()),
        escape_json(&observation_output.display().to_string()),
        escape_json(&vram_output.display().to_string()),
        escape_json(&window_output.display().to_string()),
        final_frame_stats.json(),
        final_frame_full_size,
        final_frame_scene_accepted,
        final_render_verified,
        final_frame_gameplay_scene,
        final_frame_handoff_scene,
        final_native_playable_candidate,
        final_playable,
        input_activity.json(),
        security_state_json,
        emulator.executed_steps(),
        final_state_json
    );

    if required_playable {
        Ok(())
    } else if !match_script && !boot_summary.observed_native_playable_candidate {
        Err(format!(
            "native play snapshot failed: {script_name} script did not observe a playable frame (stop_reason={}, frames={}, segment_index={}, segment_frame={})",
            boot_summary.stop_reason,
            boot_summary.total_frames,
            progress.segment_index,
            progress.segment_frame
        ))
    } else if require_gameplay_scene && !required_frame_gameplay_scene {
        Err(
            "native play snapshot failed: required frame did not meet gameplay scene criteria"
                .into(),
        )
    } else if !final_render_verified && final_stage_required {
        Err(format!(
            "native play snapshot failed: final frame did not meet render criteria (script_name={script_name}, tail_run={tail_run_json})"
        ))
    } else {
        Err(format!(
            "native play snapshot failed: handoff frame did not meet gameplay render criteria (script_name={script_name}, stop_reason={})",
            boot_summary.stop_reason
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeGapObservedFrame {
    total_frames: u64,
    segment_index: usize,
    segment_frame: u64,
    action: Action,
    source: &'static str,
    checksum: u32,
    native_playable_candidate: bool,
    scene_accepted: bool,
    render_verified: bool,
    playable: bool,
    score: u64,
    stats: NativeFrameStats,
}

impl NativeGapObservedFrame {
    fn json(self) -> String {
        format!(
            "{{\"total_frames\":{},\"segment_index\":{},\"segment_frame\":{},\"action_index\":{},\"action\":\"{}\",\"source\":\"{}\",\"checksum\":{},\"native_playable_candidate\":{},\"scene_accepted\":{},\"render_verified\":{},\"playable\":{},\"score\":{},\"frame\":{}}}",
            self.total_frames,
            self.segment_index,
            self.segment_frame,
            self.action.index(),
            self.action.name(),
            self.source,
            self.checksum,
            self.native_playable_candidate,
            self.scene_accepted,
            self.render_verified,
            self.playable,
            self.score,
            self.stats.json()
        )
    }
}

fn native_gap_observed_frame_json(frame: Option<NativeGapObservedFrame>) -> String {
    frame.map_or_else(|| "null".to_string(), NativeGapObservedFrame::json)
}

fn native_gap_observed_frame_score(
    stats: NativeFrameStats,
    native_playable_candidate: bool,
    scene_accepted: bool,
) -> u64 {
    let mut score = 0u64;
    if stats.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH && stats.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT
    {
        score = score.saturating_add(1_000_000_000);
    }
    if stats.has_visible_content() {
        score = score.saturating_add(500_000_000);
    }
    if stats.has_scene_detail() {
        score = score.saturating_add(1_000_000_000);
    }
    if stats.has_render_ready_scene() {
        score = score.saturating_add(2_000_000_000);
    }
    if stats.has_handoff_scene() {
        score = score.saturating_add(4_000_000_000);
    }
    if stats.has_gameplay_scene() {
        score = score.saturating_add(8_000_000_000);
    }
    if scene_accepted {
        score = score.saturating_add(2_000_000_000);
    }
    if native_playable_candidate {
        score = score.saturating_add(2_000_000_000);
    }

    score
        .saturating_add((stats.nonzero_pixels as u64).saturating_mul(8))
        .saturating_add((stats.horizontal_color_changes as u64).saturating_mul(4))
        .saturating_add((stats.unique_colors as u64).saturating_mul(4_096))
        .saturating_sub((stats.warm_pixels as u64).saturating_mul(2))
        .saturating_sub((stats.red_dominant_pixels as u64).saturating_mul(2))
        .saturating_sub((stats.magenta_dominant_pixels as u64).saturating_mul(4))
}

fn native_gap_probe_should_sample_observed_frame(
    total_frames: u64,
    segment_frame: u64,
    segment_frames: u64,
) -> bool {
    total_frames == 1
        || total_frames.is_multiple_of(NATIVE_GAP_PROBE_OBSERVATION_STRIDE_FRAMES)
        || segment_frame == segment_frames
}

fn native_gap_probe_observed_frame_from_emulator(
    emulator: &NativeEmulator,
    total_frames: u64,
    segment_index: usize,
    segment_frame: u64,
    action: Action,
) -> NativeGapObservedFrame {
    let stable_frame = emulator.display_frame();
    let actual_frame = emulator.actual_display_frame();
    let native_playable_candidate = emulator.native_playable_candidate();
    let selected_frame =
        native_play_display_frame_with_context(emulator, native_playable_candidate);
    let window_frame = native_play_window_frame(&selected_frame);
    let stats = NativeFrameStats::from_frame(&window_frame);
    let scene_accepted = stats.has_gameplay_scene()
        || stats.has_handoff_scene()
        || native_play_frame_ready_with_context(native_playable_candidate, &window_frame);
    let render_verified = stats.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
        && stats.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT
        && scene_accepted;
    let playable = native_playable_candidate && render_verified;
    let selected_checksum = native_frame_checksum(&selected_frame);
    let source = if selected_checksum == native_frame_checksum(&actual_frame)
        && selected_checksum != native_frame_checksum(&stable_frame)
    {
        "actual"
    } else if selected_checksum == native_frame_checksum(&stable_frame) {
        "stable"
    } else {
        "selected"
    };

    NativeGapObservedFrame {
        total_frames,
        segment_index,
        segment_frame,
        action,
        source,
        checksum: native_frame_checksum(&window_frame),
        native_playable_candidate,
        scene_accepted,
        render_verified,
        playable,
        score: native_gap_observed_frame_score(stats, native_playable_candidate, scene_accepted),
        stats,
    }
}

fn native_gap_probe_update_best_observed_frame(
    best: &mut Option<NativeGapObservedFrame>,
    emulator: &NativeEmulator,
    total_frames: u64,
    segment_index: usize,
    segment_frame: u64,
    action: Action,
) {
    let candidate = native_gap_probe_observed_frame_from_emulator(
        emulator,
        total_frames,
        segment_index,
        segment_frame,
        action,
    );
    if best.is_none_or(|current| candidate.score > current.score) {
        *best = Some(candidate);
    }
}

fn run_native_gap_probe_observed_with_timeout(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    segments: &[NativeScriptSegment],
    max_frames: u64,
    wall_timeout: Duration,
) -> (NativeScriptProgress, Option<NativeGapObservedFrame>) {
    let instructions_per_frame = instructions_per_frame.max(1);
    let target_script_frames = segments
        .iter()
        .map(|segment| segment.frames)
        .sum::<u64>()
        .max(1);
    let frame_attempt_budget = max_frames
        .max(target_script_frames)
        .saturating_mul(NATIVE_SCRIPT_FRAME_ATTEMPT_MULTIPLIER)
        .max(target_script_frames);
    let started_at = Instant::now();
    let mut total_frames = 0u64;
    let mut frame_attempts = 0u64;
    let mut missed_vblank_frames = 0u64;
    let mut segment_index = 0usize;
    let mut segment_frame = 0u64;
    let mut stop_reason = "script_completed";
    let mut stopped = false;
    let mut best_observed_frame = None;
    let mut observed_native_playable_candidate = false;
    let mut first_native_playable_frame = None;
    let mut last_native_playable_frame = None;

    'script: for (index, segment) in segments.iter().enumerate() {
        segment_index = index;
        segment_frame = 0;
        emulator.set_input(segment.action.buttons());
        while segment_frame < segment.frames {
            if started_at.elapsed() >= wall_timeout {
                stop_reason = "wall_timeout";
                stopped = true;
                break 'script;
            }

            let step = step_until_next_vblank_checked_with_wall_timeout(
                emulator,
                instructions_per_frame,
                started_at,
                Some(wall_timeout),
            );
            if step.wall_timeout {
                stop_reason = "wall_timeout";
                stopped = true;
                break 'script;
            }

            frame_attempts = frame_attempts.saturating_add(1);
            if !step.vblank_advanced {
                missed_vblank_frames = missed_vblank_frames.saturating_add(1);
                if frame_attempts >= frame_attempt_budget {
                    stop_reason = "max_frame_attempts";
                    stopped = true;
                    break 'script;
                }
                if emulator.is_terminal() {
                    stop_reason = "terminal";
                    stopped = true;
                    break 'script;
                }
                continue;
            }

            total_frames = total_frames.saturating_add(1);
            segment_frame = segment_frame.saturating_add(1);
            if emulator.native_playable_candidate() {
                observed_native_playable_candidate = true;
                first_native_playable_frame.get_or_insert(total_frames);
                last_native_playable_frame = Some(total_frames);
            }
            if native_gap_probe_should_sample_observed_frame(
                total_frames,
                segment_frame,
                segment.frames,
            ) {
                emulator.capture_vblank_presented_frame();
                native_gap_probe_update_best_observed_frame(
                    &mut best_observed_frame,
                    emulator,
                    total_frames,
                    segment_index,
                    segment_frame,
                    segment.action,
                );
            }

            if native_script_title_wait_ready_to_advance(
                emulator,
                *segment,
                segment_frame,
                total_frames,
                NativeScriptStopMode::FastTimed,
            ) {
                native_gap_probe_update_best_observed_frame(
                    &mut best_observed_frame,
                    emulator,
                    total_frames,
                    segment_index,
                    segment_frame,
                    segment.action,
                );
                segment_frame = segment.frames;
                continue;
            }
            if total_frames >= max_frames {
                stop_reason = "max_frames";
                stopped = true;
                break 'script;
            }
            if emulator.is_terminal() {
                stop_reason = "terminal";
                stopped = true;
                break 'script;
            }
        }
        if segment_frame >= segment.frames {
            segment_index = index + 1;
            segment_frame = 0;
        }
    }

    if !stopped {
        segment_index = segments.len();
        segment_frame = 0;
    }

    emulator.capture_vblank_presented_frame();
    native_gap_probe_update_best_observed_frame(
        &mut best_observed_frame,
        emulator,
        total_frames,
        segment_index,
        segment_frame,
        Action::Noop,
    );

    (
        NativeScriptProgress {
            summary: NativeScriptRunSummary {
                total_frames,
                frame_attempts,
                missed_vblank_frames,
                observed_native_playable_candidate,
                first_native_playable_frame,
                last_native_playable_frame,
                stop_reason,
            },
            segment_index,
            segment_frame,
        },
        best_observed_frame,
    )
}

fn run_native_gap_probe(
    rom: PathBuf,
    instructions_per_frame: u64,
    max_frames: u64,
    match_script: bool,
) -> Result<(), String> {
    let (mut emulator, cache_report) =
        NativeEmulator::from_rom_zip_with_cache_report(rom).map_err(|error| error.to_string())?;
    let auto_mapping = emulator.apply_auto_coin_input_mapping();
    emulator.set_vblank_presentation_capture_interval(Some(NATIVE_SCRIPT_VBLANK_CAPTURE_INTERVAL));
    emulator
        .set_unlinked_primitive_replay_interval(NATIVE_SCRIPT_UNLINKED_PRIMITIVE_REPLAY_INTERVAL);

    let segments = if match_script {
        native_aggressive_match_entry_script()
    } else {
        native_warning_skip_to_title_script()
    };
    let fast_forward_instructions_per_frame =
        native_fast_forward_instructions_per_frame(instructions_per_frame);
    let frame_limit = max_frames.max(1);
    let started_at = Instant::now();
    let wall_timeout =
        native_remaining_wall_timeout(started_at, native_gap_probe_wall_timeout(frame_limit));
    let (progress, best_observed_frame) = if let Some(wall_timeout) = wall_timeout {
        run_native_gap_probe_observed_with_timeout(
            &mut emulator,
            fast_forward_instructions_per_frame,
            &segments,
            frame_limit,
            wall_timeout,
        )
    } else {
        (
            NativeScriptProgress::skipped("skipped_gap_probe_wall_timeout"),
            None,
        )
    };

    let final_native_playable_candidate = emulator.native_playable_candidate();
    let gpu_gameplay_context = native_play_gpu_gameplay_context(&emulator);
    let stable_frame = emulator.display_frame();
    let actual_frame = emulator.actual_display_frame();
    let selected_frame =
        native_play_display_frame_with_context(&emulator, final_native_playable_candidate);
    let window_frame = native_play_window_frame_with_context(
        &selected_frame,
        final_native_playable_candidate,
        gpu_gameplay_context,
    );
    let stable_window_frame = native_play_window_frame_with_context(
        &stable_frame,
        final_native_playable_candidate,
        gpu_gameplay_context,
    );
    let actual_window_frame = native_play_actual_window_frame_for_emulator(&emulator);
    let final_frame_stats = NativeFrameStats::from_frame(&window_frame);
    let final_frame_full_size = final_frame_stats.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
        && final_frame_stats.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT;
    let final_frame_gameplay_scene =
        native_play_gameplay_scene_with_context(final_native_playable_candidate, final_frame_stats);
    let final_frame_handoff_scene = final_frame_stats.has_handoff_scene();
    let final_frame_scene_accepted = final_frame_gameplay_scene
        || final_frame_handoff_scene
        || native_play_frame_ready_with_context(final_native_playable_candidate, &window_frame);
    let final_render_verified = final_frame_full_size && final_frame_scene_accepted;
    let final_playable = final_native_playable_candidate && final_render_verified;
    let best_observed_render_verified =
        best_observed_frame.is_some_and(|frame| frame.render_verified);
    let best_observed_playable = best_observed_frame.is_some_and(|frame| frame.playable);
    let observed_playable = final_playable || best_observed_playable;
    let input_activity = emulator.input_activity();
    let rom_compatibility = emulator.rom_compatibility();
    let assets_complete = rom_compatibility.native_runtime_usable();
    let exact_mame_assets = rom_compatibility.compatible();
    let blocker_report = native_play_blocker_report_json(
        final_playable,
        assets_complete,
        exact_mame_assets,
        input_activity,
        false,
        false,
        true,
        final_native_playable_candidate,
        final_render_verified,
        final_frame_full_size,
        final_frame_scene_accepted,
        final_frame_gameplay_scene,
        final_frame_handoff_scene,
        true,
        &final_frame_stats,
        emulator.br2_native_credit_hle_accepted_seen(),
    );
    let selected_source =
        if native_frame_checksum(&selected_frame) == native_frame_checksum(&actual_frame) {
            "actual"
        } else {
            "stable"
        };

    println!(
        "{{\"command\":\"native-gap-probe\",\"instructions_per_frame\":{},\"fast_forward_instructions_per_frame\":{},\"max_frames\":{},\"match_script\":{},\"auto_input_mapping\":{},\"cache\":{},\"run\":{},\"script\":{},\"script_segment_count\":{},\"script_total_frames\":{},\"playable\":{},\"final_playable\":{},\"observed_playable\":{},\"best_observed_playable\":{},\"best_observed_render_verified\":{},\"best_observed_frame\":{},\"final_native_playable_candidate\":{},\"final_render_verified\":{},\"final_frame_gameplay_scene\":{},\"final_frame_handoff_scene\":{},\"selected_source\":\"{}\",\"frame_checksums\":{{\"stable\":{},\"actual\":{},\"selected_window\":{}}},\"frames\":{{\"stable_window\":{},\"actual_window\":{},\"selected_window\":{}}},\"assets_complete\":{},\"exact_mame_assets\":{},\"rom_compatibility\":{},\"input_activity\":{},\"blocker_report\":{},\"diagnosis\":{}}}",
        instructions_per_frame,
        fast_forward_instructions_per_frame,
        frame_limit,
        match_script,
        auto_mapping.map_or_else(
            || "null".to_string(),
            |mapping| format!("\"{}\"", escape_json(mapping))
        ),
        cache_report.json(),
        progress.json(),
        native_script_progress_position_json(
            &segments,
            progress.segment_index,
            progress.segment_frame
        ),
        segments.len(),
        native_script_max_frames_for_segments(&segments),
        observed_playable,
        final_playable,
        observed_playable,
        best_observed_playable,
        best_observed_render_verified,
        native_gap_observed_frame_json(best_observed_frame),
        final_native_playable_candidate,
        final_render_verified,
        final_frame_gameplay_scene,
        final_frame_handoff_scene,
        selected_source,
        native_frame_checksum(&stable_window_frame),
        native_frame_checksum(&actual_window_frame),
        native_frame_checksum(&window_frame),
        NativeFrameStats::from_frame(&stable_window_frame).json(),
        NativeFrameStats::from_frame(&actual_window_frame).json(),
        final_frame_stats.json(),
        assets_complete,
        exact_mame_assets,
        rom_compatibility.summary_json(),
        input_activity.json(),
        blocker_report,
        native_gap_probe_diagnosis_json(&emulator)
    );
    Ok(())
}

fn native_gap_probe_diagnosis_json(emulator: &NativeEmulator) -> String {
    format!(
        "{{\"cpu\":{{\"pc\":{},\"pc_hex\":\"0x{:08x}\",\"next_pc\":{},\"next_pc_hex\":\"0x{:08x}\",\"cycles\":{},\"halted\":{}}},\"vblank_count\":{},\"draw_sequence\":{},\"gpu_playable_candidate\":{},\"gpu_rendered_scene_candidate\":{},\"gte_gameplay_signal\":{},\"executed_steps\":{},\"terminal\":{},\"memory\":{},\"credit_debug_words\":[{}],\"input_state\":{}}}",
        emulator.cpu.pc,
        emulator.cpu.pc,
        emulator.cpu.next_pc,
        emulator.cpu.next_pc,
        emulator.cpu.cycles,
        emulator.cpu.halted,
        emulator.vblank_count(),
        emulator.draw_sequence(),
        emulator.gpu_native_playable_candidate(),
        emulator.gpu_native_rendered_scene_candidate(),
        emulator.native_3d_gameplay_signal(),
        emulator.executed_steps(),
        emulator.is_terminal(),
        native_runtime_memory_digest_json(emulator),
        native_credit_debug_words_json(emulator),
        emulator.input_compact_probe_json()
    )
}

fn native_runtime_memory_digest_json(emulator: &NativeEmulator) -> String {
    let ram = emulator.ram_snapshot();
    let scratchpad = emulator.scratchpad_snapshot();
    let ram_checksum = native_bytes_checksum(&ram);
    let scratchpad_checksum = native_bytes_checksum(&scratchpad);
    format!(
        "{{\"ram_len\":{},\"ram_checksum\":{},\"ram_checksum_hex\":\"0x{:08x}\",\"scratchpad_len\":{},\"scratchpad_checksum\":{},\"scratchpad_checksum_hex\":\"0x{:08x}\"}}",
        ram.len(),
        ram_checksum,
        ram_checksum,
        scratchpad.len(),
        scratchpad_checksum,
        scratchpad_checksum
    )
}

fn remaining_native_script_segments(
    segments: &[NativeScriptSegment],
    segment_index: usize,
    segment_frame: u64,
) -> Vec<NativeScriptSegment> {
    let mut remaining = Vec::new();
    if let Some(segment) = segments.get(segment_index) {
        let frames = segment.frames.saturating_sub(segment_frame);
        if frames > 0 {
            remaining.push(NativeScriptSegment {
                action: segment.action,
                frames,
            });
        }
    }
    remaining.extend(segments.iter().skip(segment_index + 1).copied());
    remaining
}

fn remaining_native_script_segments_after_title_ready(
    segments: &[NativeScriptSegment],
    segment_index: usize,
    segment_frame: u64,
) -> Vec<NativeScriptSegment> {
    let remaining = remaining_native_script_segments(segments, segment_index, segment_frame);
    let first_credit = remaining
        .iter()
        .position(|segment| native_script_segment_is_credit_attempt(*segment))
        .unwrap_or(0);
    remaining.into_iter().skip(first_credit).collect()
}

fn write_native_display_frame_png(frame: &NativeDisplayFrame, output: &Path) -> Result<(), String> {
    write_output_file(
        output,
        png_from_rgb888_pixels(frame.width, frame.height, &frame.pixels),
    )
}

fn native_play_window_frame(frame: &NativeDisplayFrame) -> NativeDisplayFrame {
    native_window_frame_from_display(frame)
}

fn native_play_window_frame_with_context(
    frame: &NativeDisplayFrame,
    native_playable_candidate: bool,
    gpu_gameplay_context: bool,
) -> NativeDisplayFrame {
    let window = native_play_window_frame(frame);
    let raw_stats = NativeFrameStats::from_frame(frame);
    let raw_without_deinterlace = native_play_window_frame_without_deinterlace(frame);
    let raw_without_deinterlace_stats = NativeFrameStats::from_frame(&raw_without_deinterlace);
    let window_stats = NativeFrameStats::from_frame(&window);
    if native_play_should_preserve_raw_title_frame_over_deinterlace(
        raw_stats,
        raw_without_deinterlace_stats,
        window_stats,
    ) {
        return raw_without_deinterlace;
    }
    if native_play_should_preserve_raw_frame_over_deinterlace(
        raw_stats,
        window_stats,
        native_playable_candidate,
        gpu_gameplay_context,
    ) {
        return raw_without_deinterlace;
    }
    native_play_repair_context_live_stage_window_frame(
        window,
        native_playable_candidate,
        gpu_gameplay_context,
    )
}

fn native_play_should_preserve_raw_title_frame_over_deinterlace(
    raw_stats: NativeFrameStats,
    raw_without_deinterlace_stats: NativeFrameStats,
    window_stats: NativeFrameStats,
) -> bool {
    let title_stats = if raw_stats.has_title_screen_frame() {
        raw_stats
    } else {
        raw_without_deinterlace_stats
    };

    title_stats.has_title_screen_frame()
        && title_stats.has_visible_content()
        && !title_stats.has_low_palette_noise()
        && !title_stats.has_low_palette_logo_transition()
        && !title_stats.has_saturated_transition_planes()
        && !window_stats.has_title_screen_frame()
        && (title_stats.occupied_row_span > window_stats.occupied_row_span.saturating_add(64)
            || title_stats.nonzero_pixels > window_stats.nonzero_pixels.saturating_mul(3))
}

fn native_play_window_frame_without_deinterlace(frame: &NativeDisplayFrame) -> NativeDisplayFrame {
    let width = NATIVE_PLAY_MIN_WINDOW_WIDTH;
    let height = NATIVE_PLAY_MIN_WINDOW_HEIGHT;
    let mut pixels = vec![0; width.saturating_mul(height)];
    if frame.width == 0
        || frame.height == 0
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
    {
        return NativeDisplayFrame {
            width,
            height,
            pixels,
        };
    }

    for y in 0..height {
        let source_y = y
            .saturating_mul(frame.height)
            .checked_div(height)
            .unwrap_or_default()
            .min(frame.height.saturating_sub(1));
        for x in 0..width {
            let source_x = x
                .saturating_mul(frame.width)
                .checked_div(width)
                .unwrap_or_default()
                .min(frame.width.saturating_sub(1));
            pixels[y * width + x] = frame.pixels[source_y * frame.width + source_x];
        }
    }

    NativeDisplayFrame {
        width,
        height,
        pixels,
    }
}

fn native_play_should_preserve_raw_frame_over_deinterlace(
    raw_stats: NativeFrameStats,
    window_stats: NativeFrameStats,
    native_playable_candidate: bool,
    gpu_gameplay_context: bool,
) -> bool {
    native_playable_candidate
        && gpu_gameplay_context
        && window_stats.has_stage_texture_field_smear()
        && !raw_stats.has_stage_texture_field_smear()
        && raw_stats.has_visible_content()
        && raw_stats.has_scene_detail()
        && (raw_stats.has_handoff_scene()
            || raw_stats.has_render_ready_scene()
            || raw_stats.has_native_context_scene())
}

fn native_play_repair_context_live_stage_window_frame(
    frame: NativeDisplayFrame,
    native_playable_candidate: bool,
    gpu_gameplay_context: bool,
) -> NativeDisplayFrame {
    if !native_playable_candidate || !gpu_gameplay_context {
        return frame;
    }

    let stats = NativeFrameStats::from_frame(&frame);
    if !stats.has_context_live_stage_hud() || !stats.has_texture_page_fragment_artifact() {
        return frame;
    }

    frame
}

fn native_play_display_frame_with_context(
    emulator: &NativeEmulator,
    native_playable_candidate: bool,
) -> NativeDisplayFrame {
    let stable = emulator.display_frame();
    let actual = emulator.actual_display_frame();
    let raw_actual = emulator.raw_actual_display_frame();
    let gpu_gameplay_context = native_play_gpu_gameplay_context(emulator);
    if native_play_should_prefer_raw_actual_display(
        &raw_actual,
        &actual,
        &stable,
        native_playable_candidate,
        gpu_gameplay_context,
    ) {
        raw_actual
    } else if native_play_should_prefer_actual_display(&actual, &stable, native_playable_candidate)
        || native_play_should_prefer_gpu_verified_actual_display(
            &actual,
            &stable,
            native_playable_candidate,
            emulator.actual_display_supports_playable_candidate(),
            gpu_gameplay_context,
        )
    {
        actual
    } else {
        stable
    }
}

fn native_play_should_prefer_raw_actual_display(
    raw_actual: &NativeDisplayFrame,
    actual: &NativeDisplayFrame,
    stable: &NativeDisplayFrame,
    native_playable_candidate: bool,
    gpu_gameplay_context: bool,
) -> bool {
    let raw_without_deinterlace = native_play_window_frame_without_deinterlace(raw_actual);
    let raw_context_window = native_play_window_frame_with_context(
        raw_actual,
        native_playable_candidate,
        gpu_gameplay_context,
    );
    let raw_window = if native_play_title_screen_ready_from_frame(&raw_without_deinterlace) {
        raw_without_deinterlace
    } else {
        raw_context_window
    };
    let actual_window = native_play_window_frame_with_context(
        actual,
        native_playable_candidate,
        gpu_gameplay_context,
    );
    let stable_window = native_play_window_frame_with_context(
        stable,
        native_playable_candidate,
        gpu_gameplay_context,
    );
    let raw_stats = NativeFrameStats::from_frame(&raw_window);
    let actual_stats = NativeFrameStats::from_frame(&actual_window);
    let stable_stats = NativeFrameStats::from_frame(&stable_window);

    if native_play_title_screen_ready_from_frame(&raw_window)
        && !raw_stats.has_corrupt_title_texture_fragment()
        && !raw_stats.has_saturated_transition_planes()
    {
        let actual_is_weaker =
            native_play_window_frame_is_weaker_than_raw_title(actual_stats, raw_stats);
        let stable_is_weaker =
            native_play_window_frame_is_weaker_than_raw_title(stable_stats, raw_stats);
        return actual_is_weaker && stable_is_weaker;
    }

    false
}

fn native_play_window_frame_is_weaker_than_raw_title(
    candidate: NativeFrameStats,
    raw_title: NativeFrameStats,
) -> bool {
    !candidate.has_title_screen_frame()
        || candidate.nonzero_pixels.saturating_mul(3) < raw_title.nonzero_pixels
        || candidate.occupied_row_span.saturating_add(64) < raw_title.occupied_row_span
}

fn native_play_window_frame_for_emulator(emulator: &NativeEmulator) -> NativeDisplayFrame {
    native_play_window_frame_for_emulator_with_context(
        emulator,
        emulator.native_playable_candidate(),
    )
}

fn native_play_window_frame_for_emulator_with_context(
    emulator: &NativeEmulator,
    native_playable_candidate: bool,
) -> NativeDisplayFrame {
    let gpu_gameplay_context = native_play_gpu_gameplay_context(emulator);
    native_play_window_frame_with_context(
        &native_play_display_frame_with_context(emulator, native_playable_candidate),
        native_playable_candidate,
        gpu_gameplay_context,
    )
}

fn native_play_actual_window_frame_for_emulator(emulator: &NativeEmulator) -> NativeDisplayFrame {
    let gpu_gameplay_context = native_play_gpu_gameplay_context(emulator);
    native_play_window_frame_with_context(
        &emulator.actual_display_frame(),
        emulator.native_playable_candidate(),
        gpu_gameplay_context,
    )
}

fn native_play_gpu_gameplay_context(emulator: &NativeEmulator) -> bool {
    emulator.gpu_native_playable_candidate()
        || (emulator.gpu_native_rendered_scene_candidate() && emulator.native_3d_gameplay_signal())
        || (emulator.native_3d_gameplay_signal()
            && emulator.actual_display_supports_playable_candidate())
}

fn native_play_should_prefer_actual_display(
    actual: &NativeDisplayFrame,
    stable: &NativeDisplayFrame,
    native_playable_candidate: bool,
) -> bool {
    let actual_window =
        native_play_window_frame_with_context(actual, native_playable_candidate, false);
    let stable_window =
        native_play_window_frame_with_context(stable, native_playable_candidate, false);
    let actual_stats = NativeFrameStats::from_frame(&actual_window);
    let stable_stats = NativeFrameStats::from_frame(&stable_window);
    let actual_title_ready = native_play_title_screen_ready_from_frame(&actual_window);
    let stable_title_ready = native_play_title_screen_ready_from_frame(&stable_window);
    let stable_has_richer_presentable_scene = stable_stats.has_presentable_scene()
        && stable_stats.has_scene_detail()
        && stable_stats.nonzero_pixels.saturating_mul(3)
            >= actual_stats.nonzero_pixels.saturating_mul(5)
        && stable_stats.occupied_row_span > actual_stats.occupied_row_span.saturating_add(64)
        && !stable_stats.has_blocking_display_artifact();
    let actual_is_low_field_of_stable = actual.width == stable.width
        && actual.height > 0
        && actual.height.saturating_mul(2) == stable.height;
    let stable_has_deinterlaced_scene = actual_is_low_field_of_stable
        && stable.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT
        && stable_stats.has_visible_content()
        && stable_stats.has_scene_detail()
        && !stable_stats.has_title_screen_frame()
        && !stable_stats.has_blocking_texture_page_fragment_artifact()
        && !stable_stats.has_corrupt_title_texture_fragment()
        && !stable_stats.has_repeating_tile_grid_artifact()
        && !stable_stats.has_saturated_transition_planes()
        && !stable_stats.has_warm_dominant_fill_frame();
    if stable_has_deinterlaced_scene && !actual_title_ready {
        return false;
    }
    if actual_title_ready
        && !actual_stats.has_texture_page_fragment_artifact()
        && !actual_stats.has_saturated_transition_planes()
        && !stable_has_richer_presentable_scene
        && (!stable_title_ready
            || (actual.height < stable.height
                && actual_stats.occupied_row_span < stable_stats.occupied_row_span))
    {
        return true;
    }

    let actual_fragment = actual_stats.has_blocking_texture_page_fragment_artifact();
    let stable_fragment = stable_stats.has_blocking_texture_page_fragment_artifact();
    if actual_fragment != stable_fragment {
        return !actual_fragment;
    }

    let actual_ready =
        native_play_frame_ready_with_context(native_playable_candidate, &actual_window)
            || actual_stats.has_gameplay_scene()
            || actual_stats.has_handoff_scene();
    let stable_ready =
        native_play_frame_ready_with_context(native_playable_candidate, &stable_window)
            || stable_stats.has_gameplay_scene()
            || stable_stats.has_handoff_scene();

    if actual_ready != stable_ready {
        return actual_ready;
    }
    actual_ready
        && actual_stats.has_gameplay_scene()
        && !stable_stats.has_gameplay_scene()
        && !actual_stats.has_saturated_transition_planes()
}

fn native_play_should_prefer_gpu_verified_actual_display(
    actual: &NativeDisplayFrame,
    stable: &NativeDisplayFrame,
    native_playable_candidate: bool,
    gpu_actual_supports_playable: bool,
    gpu_gameplay_context: bool,
) -> bool {
    if !native_playable_candidate || !gpu_actual_supports_playable || !gpu_gameplay_context {
        return false;
    }

    let actual_window = native_play_window_frame(actual);
    let stable_window = native_play_window_frame(stable);
    let actual_stats = NativeFrameStats::from_frame(&actual_window);
    let stable_stats = NativeFrameStats::from_frame(&stable_window);
    let actual_visibly_usable = native_play_gpu_verified_frame_stats(actual_stats);
    let stable_is_stale_or_blocking = stable_stats.has_title_screen_frame()
        || stable_stats.has_corrupt_title_texture_fragment()
        || stable_stats.has_blocking_display_artifact()
        || !stable_stats.has_visible_content()
        || (!stable_stats.has_gameplay_scene() && !stable_stats.has_handoff_scene());

    actual_visibly_usable && stable_is_stale_or_blocking
}

fn step_until_next_vblank_checked(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
) -> bool {
    let start_vblank = emulator.vblank_count();
    emulator.step_until_next_vblank(instructions_per_frame);
    emulator.vblank_count() != start_vblank
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeScriptVblankStep {
    vblank_advanced: bool,
    wall_timeout: bool,
}

fn step_until_next_vblank_checked_with_wall_timeout(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    started_at: Instant,
    wall_timeout: Option<Duration>,
) -> NativeScriptVblankStep {
    let start_vblank = emulator.vblank_count();
    let mut remaining = instructions_per_frame.max(1);

    while remaining > 0 && emulator.vblank_count() == start_vblank && !emulator.is_terminal() {
        if wall_timeout.is_some_and(|wall_timeout| started_at.elapsed() >= wall_timeout) {
            return NativeScriptVblankStep {
                vblank_advanced: false,
                wall_timeout: true,
            };
        }

        let slice = remaining.min(NATIVE_SCRIPT_VBLANK_POLL_INSTRUCTION_SLICE);
        emulator.step_instructions(slice);
        remaining = remaining.saturating_sub(slice);
    }

    NativeScriptVblankStep {
        vblank_advanced: emulator.vblank_count() != start_vblank,
        wall_timeout: wall_timeout.is_some_and(|wall_timeout| {
            emulator.vblank_count() == start_vblank && started_at.elapsed() >= wall_timeout
        }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativePlayWindowStep {
    vblank_advanced: bool,
    window_closed_or_escape: bool,
    buttons: ActionButtons,
    manual_override: bool,
}

fn step_native_play_window_frame_checked(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    window: &mut Window,
    scripted_action: Option<Action>,
    manual_override_enabled: bool,
    input_latch: &mut NativeInputLatch,
) -> Result<NativePlayWindowStep, String> {
    let start_vblank = emulator.vblank_count();
    let mut remaining = instructions_per_frame.max(1);
    let poll_instruction_slice = native_play_gui_poll_instruction_slice();

    let mut manual_override = false;
    let mut buttons = ActionButtons::default();
    while remaining > 0
        && emulator.vblank_count() == start_vblank
        && !emulator.is_terminal()
        && window.is_open()
        && !window.is_key_down(Key::Escape)
    {
        window.update();
        let manual_buttons = input_latch.buttons(native_window_buttons(window));
        manual_override |= manual_override_enabled && action_buttons_any(manual_buttons);
        buttons = native_play_effective_buttons(manual_buttons, scripted_action);
        emulator.set_input(buttons);
        let slice = remaining.min(poll_instruction_slice);
        emulator.step_instructions(slice);
        remaining = remaining.saturating_sub(slice);
    }

    Ok(NativePlayWindowStep {
        vblank_advanced: emulator.vblank_count() != start_vblank,
        window_closed_or_escape: !window.is_open() || window.is_key_down(Key::Escape),
        buttons,
        manual_override,
    })
}

fn run_native_scripted_transition_diagnose(
    rom: PathBuf,
    instructions_per_frame: u64,
    segments: Vec<NativeScriptSegment>,
) -> Result<(), String> {
    let mut emulator = native_emulator_from_rom_source(rom)?;
    let mut stdout = io::stdout();
    let mut total_frames = 0u64;
    let mut missed_vblank_frames = 0u64;
    let mut stop_reason = "script_completed";
    let wall_timeout = native_script_wall_timeout(NativeScriptStopMode::Timed);
    let mut previous_checksum =
        native_frame_checksum(&native_play_window_frame(&emulator.display_frame()));

    writeln!(
        stdout,
        "{{\"event\":\"start\",\"instructions_per_frame\":{},\"wall_timeout_secs\":{},\"segments\":[{}],\"initial_frame_checksum\":{},\"initial_state\":{}}}",
        instructions_per_frame,
        optional_u64_json(wall_timeout.map(|timeout| timeout.as_secs())),
        native_script_segments_json(&segments),
        previous_checksum,
        emulator.input_summary_json()
    )
    .map_err(|error| format!("failed to write transition diagnose start: {error}"))?;
    stdout
        .flush()
        .map_err(|error| format!("failed to flush transition diagnose start: {error}"))?;

    for (segment_index, segment) in segments.iter().enumerate() {
        let before_activity = emulator.input_activity();
        let before_steps = emulator.executed_steps();
        let before_vblank = emulator.vblank_count();
        let segment_progress = run_native_script_observed_with_stop_and_timeout(
            &mut emulator,
            instructions_per_frame,
            &[*segment],
            NativeScriptStopMode::Timed,
            Some(native_script_max_frames_for_segments(&[*segment])),
            wall_timeout,
        );
        let segment_run = segment_progress.summary;
        stop_reason = segment_run.stop_reason;
        total_frames = total_frames.saturating_add(segment_run.total_frames);
        missed_vblank_frames =
            missed_vblank_frames.saturating_add(segment_run.missed_vblank_frames);
        let after_activity = emulator.input_activity();
        let input_delta = after_activity.saturating_subtracted(before_activity);
        let raw_frame = emulator.display_frame();
        let window_frame = native_play_window_frame(&raw_frame);
        let window_frame_stats = NativeFrameStats::from_frame(&window_frame);
        let frame_checksum = native_frame_checksum(&window_frame);
        let frame_changed = frame_checksum != previous_checksum;
        previous_checksum = frame_checksum;

        writeln!(
            stdout,
            "{{\"event\":\"segment\",\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"requested_frames\":{},\"segment_run\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"before_vblank\":{},\"after_vblank\":{},\"before_executed_steps\":{},\"after_executed_steps\":{},\"executed_step_delta\":{},\"frame_checksum\":{},\"frame_changed\":{},\"raw_frame\":{},\"window_frame\":{},\"native_playable_candidate\":{},\"input_delta\":{},\"diagnosis\":{},\"display_diagnosis\":{},\"native_sync\":{}}}",
            segment_index,
            segment.action.index(),
            segment.action.name(),
            segment.frames,
            segment_run.json(),
            total_frames,
            missed_vblank_frames,
            before_vblank,
            emulator.vblank_count(),
            before_steps,
            emulator.executed_steps(),
            emulator.executed_steps().saturating_sub(before_steps),
            frame_checksum,
            frame_changed,
            NativeFrameStats::from_frame(&raw_frame).json(),
            window_frame_stats.json(),
            emulator.native_playable_candidate(),
            input_delta.json(),
            emulator.input_check_probe_json(),
            emulator.display_diagnosis_json(),
            emulator.native_sync_compact_json()
        )
        .map_err(|error| format!("failed to write transition diagnose segment: {error}"))?;
        stdout
            .flush()
            .map_err(|error| format!("failed to flush transition diagnose segment: {error}"))?;

        if emulator.is_terminal() {
            break;
        }
        if segment_run.stop_reason != "script_completed" {
            break;
        }
    }

    writeln!(
        stdout,
        "{{\"event\":\"finish\",\"instructions_per_frame\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"stop_reason\":\"{}\",\"executed_steps\":{},\"terminal\":{},\"final_state\":{}}}",
        instructions_per_frame,
        total_frames,
        missed_vblank_frames,
        stop_reason,
        emulator.executed_steps(),
        emulator.is_terminal(),
        emulator.compact_probe_json()
    )
    .map_err(|error| format!("failed to write transition diagnose finish: {error}"))?;
    stdout
        .flush()
        .map_err(|error| format!("failed to flush transition diagnose finish: {error}"))?;
    Ok(())
}

fn run_native_security_probe(
    rom: PathBuf,
    instructions_per_frame: u64,
    segments: Vec<NativeScriptSegment>,
) -> Result<(), String> {
    let mut emulator = native_emulator_from_rom_source(rom)?;
    let mut stdout = io::stdout();
    let mut total_frames = 0u64;
    let mut missed_vblank_frames = 0u64;
    let mut stop_reason = "script_completed";
    let wall_timeout = native_script_wall_timeout(NativeScriptStopMode::Timed);

    writeln!(
        stdout,
        "{{\"event\":\"start\",\"instructions_per_frame\":{},\"wall_timeout_secs\":{},\"segments\":[{}],\"initial_state\":{}}}",
        instructions_per_frame,
        optional_u64_json(wall_timeout.map(|timeout| timeout.as_secs())),
        native_script_segments_json(&segments),
        emulator.security_probe_json()
    )
    .map_err(|error| format!("failed to write security probe start: {error}"))?;
    stdout
        .flush()
        .map_err(|error| format!("failed to flush security probe start: {error}"))?;

    for (segment_index, segment) in segments.iter().enumerate() {
        let before_activity = emulator.input_activity();
        let before_steps = emulator.executed_steps();
        let before_vblank = emulator.vblank_count();
        let segment_progress = run_native_script_observed_with_stop_and_timeout(
            &mut emulator,
            instructions_per_frame,
            &[*segment],
            NativeScriptStopMode::Timed,
            Some(native_script_max_frames_for_segments(&[*segment])),
            wall_timeout,
        );
        let segment_run = segment_progress.summary;
        stop_reason = segment_run.stop_reason;
        total_frames = total_frames.saturating_add(segment_run.total_frames);
        missed_vblank_frames =
            missed_vblank_frames.saturating_add(segment_run.missed_vblank_frames);
        let input_delta = emulator
            .input_activity()
            .saturating_subtracted(before_activity);

        writeln!(
            stdout,
            "{{\"event\":\"segment\",\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"requested_frames\":{},\"segment_run\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"before_vblank\":{},\"after_vblank\":{},\"before_executed_steps\":{},\"after_executed_steps\":{},\"executed_step_delta\":{},\"input_delta\":{},\"state\":{}}}",
            segment_index,
            segment.action.index(),
            segment.action.name(),
            segment.frames,
            segment_run.json(),
            total_frames,
            missed_vblank_frames,
            before_vblank,
            emulator.vblank_count(),
            before_steps,
            emulator.executed_steps(),
            emulator.executed_steps().saturating_sub(before_steps),
            input_delta.json(),
            emulator.security_probe_json()
        )
        .map_err(|error| format!("failed to write security probe segment: {error}"))?;
        stdout
            .flush()
            .map_err(|error| format!("failed to flush security probe segment: {error}"))?;

        if emulator.is_terminal() {
            break;
        }
        if segment_run.stop_reason != "script_completed" {
            break;
        }
    }

    writeln!(
        stdout,
        "{{\"event\":\"finish\",\"instructions_per_frame\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"stop_reason\":\"{}\",\"executed_steps\":{},\"terminal\":{},\"final_state\":{}}}",
        instructions_per_frame,
        total_frames,
        missed_vblank_frames,
        stop_reason,
        emulator.executed_steps(),
        emulator.is_terminal(),
        emulator.security_probe_json()
    )
    .map_err(|error| format!("failed to write security probe finish: {error}"))?;
    stdout
        .flush()
        .map_err(|error| format!("failed to flush security probe finish: {error}"))?;
    Ok(())
}

fn run_native_scripted_live_probe(
    rom: PathBuf,
    instructions_per_frame: u64,
    emit_stride: u64,
    segments: Vec<NativeScriptSegment>,
) -> Result<(), String> {
    let mut emulator = native_emulator_from_rom_source(rom)?;
    let mut total_frames = 0u64;
    let mut frame_attempts = 0u64;
    let mut missed_vblank_frames = 0u64;
    let mut stdout = io::stdout();
    let started_at = Instant::now();
    let wall_timeout = native_live_probe_wall_timeout();
    let mut stop_reason = "script_completed";

    writeln!(
        stdout,
        "{{\"event\":\"start\",\"instructions_per_frame\":{},\"emit_stride_frames\":{},\"segments\":[{}],\"state\":{}}}",
        instructions_per_frame,
        emit_stride,
        native_script_segments_json(&segments),
        emulator.compact_probe_json()
    )
    .map_err(|error| format!("failed to write live probe start: {error}"))?;
    stdout
        .flush()
        .map_err(|error| format!("failed to flush live probe start: {error}"))?;

    'script: for (segment_index, segment) in segments.iter().enumerate() {
        emulator.set_input(segment.action.buttons());
        for frame_in_segment in 1..=segment.frames {
            if wall_timeout.is_some_and(|wall_timeout| started_at.elapsed() >= wall_timeout) {
                stop_reason = "wall_timeout";
                break 'script;
            }
            let step = step_until_next_vblank_checked_with_wall_timeout(
                &mut emulator,
                instructions_per_frame,
                started_at,
                wall_timeout,
            );
            if step.wall_timeout {
                stop_reason = "wall_timeout";
                break 'script;
            }
            let vblank_advanced = step.vblank_advanced;
            frame_attempts = frame_attempts.saturating_add(1);
            if !vblank_advanced {
                missed_vblank_frames = missed_vblank_frames.saturating_add(1);
            }

            total_frames = total_frames.saturating_add(1);
            let should_emit =
                frame_in_segment % emit_stride == 0 || frame_in_segment == segment.frames;
            if should_emit || emulator.is_terminal() {
                let frame_stats = NativeFrameStats::from_frame(&emulator.display_frame());
                writeln!(
                    stdout,
                    "{{\"event\":\"frame\",\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"frame_in_segment\":{},\"segment_frames\":{},\"total_frames\":{},\"vblank_advanced\":{},\"missed_vblank_frames\":{},\"frame\":{},\"state\":{}}}",
                    segment_index,
                    segment.action.index(),
                    segment.action.name(),
                    frame_in_segment,
                    segment.frames,
                    total_frames,
                    vblank_advanced,
                    missed_vblank_frames,
                    frame_stats.json(),
                    emulator.compact_probe_json()
                )
                .map_err(|error| format!("failed to write live probe frame: {error}"))?;
                stdout
                    .flush()
                    .map_err(|error| format!("failed to flush live probe frame: {error}"))?;
            }
            if emulator.is_terminal() {
                stop_reason = "terminal";
                break 'script;
            }
        }
    }

    writeln!(
        stdout,
        "{{\"event\":\"finish\",\"instructions_per_frame\":{},\"emit_stride_frames\":{},\"total_frames\":{},\"frame_attempts\":{},\"missed_vblank_frames\":{},\"executed_steps\":{},\"terminal\":{},\"stop_reason\":\"{}\",\"state\":{}}}",
        instructions_per_frame,
        emit_stride,
        total_frames,
        frame_attempts,
        missed_vblank_frames,
        emulator.executed_steps(),
        emulator.is_terminal(),
        stop_reason,
        emulator.compact_probe_json()
    )
    .map_err(|error| format!("failed to write live probe finish: {error}"))?;
    stdout
        .flush()
        .map_err(|error| format!("failed to flush live probe finish: {error}"))?;
    Ok(())
}

fn run_native_compact_script_probe(
    rom: PathBuf,
    instructions_per_frame: u64,
    emit_stride: u64,
    segments: Vec<NativeScriptSegment>,
) -> Result<(), String> {
    let mut emulator = native_emulator_from_rom_source(rom)?;
    let mut total_frames = 0u64;
    let mut frame_attempts = 0u64;
    let mut missed_vblank_frames = 0u64;
    let mut stdout = io::stdout();
    let started_at = Instant::now();
    let wall_timeout = native_live_probe_wall_timeout();
    let mut stop_reason = "script_completed";

    writeln!(
        stdout,
        "{{\"event\":\"start\",\"instructions_per_frame\":{},\"emit_stride_frames\":{},\"wall_timeout_secs\":{},\"segments\":[{}]}}",
        instructions_per_frame,
        emit_stride,
        optional_u64_json(wall_timeout.map(|timeout| timeout.as_secs())),
        native_script_segments_json(&segments)
    )
    .map_err(|error| format!("failed to write compact probe start: {error}"))?;
    stdout
        .flush()
        .map_err(|error| format!("failed to flush compact probe start: {error}"))?;

    'script: for (segment_index, segment) in segments.iter().enumerate() {
        emulator.set_input(segment.action.buttons());
        for frame_in_segment in 1..=segment.frames {
            if wall_timeout.is_some_and(|wall_timeout| started_at.elapsed() >= wall_timeout) {
                stop_reason = "wall_timeout";
                break 'script;
            }

            let step = step_until_next_vblank_checked_with_wall_timeout(
                &mut emulator,
                instructions_per_frame,
                started_at,
                wall_timeout,
            );
            if step.wall_timeout {
                stop_reason = "wall_timeout";
                break 'script;
            }
            frame_attempts = frame_attempts.saturating_add(1);
            if !step.vblank_advanced {
                missed_vblank_frames = missed_vblank_frames.saturating_add(1);
            }
            total_frames = total_frames.saturating_add(1);

            let should_emit =
                frame_in_segment % emit_stride == 0 || frame_in_segment == segment.frames;
            if should_emit || emulator.is_terminal() {
                write_native_compact_probe_frame(
                    &mut stdout,
                    &emulator,
                    segment_index,
                    *segment,
                    frame_in_segment,
                    total_frames,
                    step.vblank_advanced,
                    missed_vblank_frames,
                )?;
            }
            if emulator.is_terminal() {
                stop_reason = "terminal";
                break 'script;
            }
        }
    }

    writeln!(
        stdout,
        "{{\"event\":\"finish\",\"instructions_per_frame\":{},\"emit_stride_frames\":{},\"total_frames\":{},\"frame_attempts\":{},\"missed_vblank_frames\":{},\"executed_steps\":{},\"terminal\":{},\"stop_reason\":\"{}\",\"state\":{}}}",
        instructions_per_frame,
        emit_stride,
        total_frames,
        frame_attempts,
        missed_vblank_frames,
        emulator.executed_steps(),
        emulator.is_terminal(),
        stop_reason,
        emulator.input_check_probe_json()
    )
    .map_err(|error| format!("failed to write compact probe finish: {error}"))?;
    stdout
        .flush()
        .map_err(|error| format!("failed to flush compact probe finish: {error}"))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_native_compact_probe_frame(
    stdout: &mut io::Stdout,
    emulator: &NativeEmulator,
    segment_index: usize,
    segment: NativeScriptSegment,
    frame_in_segment: u64,
    total_frames: u64,
    vblank_advanced: bool,
    missed_vblank_frames: u64,
) -> Result<(), String> {
    let raw_frame = emulator.display_frame();
    let actual_frame = emulator.actual_display_frame();
    let raw_window_frame = native_play_window_frame(&raw_frame);
    let actual_window_frame = native_play_window_frame(&actual_frame);
    let selected_window_frame = native_play_window_frame_for_emulator(emulator);
    let native_playable_candidate = emulator.native_playable_candidate();

    writeln!(
        stdout,
        "{{\"event\":\"frame\",\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"frame_in_segment\":{},\"segment_frames\":{},\"total_frames\":{},\"vblank_advanced\":{},\"missed_vblank_frames\":{},\"native_playable_candidate\":{},\"raw_frame\":{},\"actual_frame\":{},\"raw_window_frame\":{},\"actual_window_frame\":{},\"selected_window_frame\":{},\"input_activity\":{},\"state\":{}}}",
        segment_index,
        segment.action.index(),
        segment.action.name(),
        frame_in_segment,
        segment.frames,
        total_frames,
        vblank_advanced,
        missed_vblank_frames,
        native_playable_candidate,
        NativeFrameStats::from_frame(&raw_frame).json(),
        NativeFrameStats::from_frame(&actual_frame).json(),
        NativeFrameStats::from_frame(&raw_window_frame).json(),
        NativeFrameStats::from_frame(&actual_window_frame).json(),
        NativeFrameStats::from_frame(&selected_window_frame).json(),
        emulator.input_activity_json(),
        emulator.input_check_probe_json()
    )
    .map_err(|error| format!("failed to write compact probe frame: {error}"))?;
    stdout
        .flush()
        .map_err(|error| format!("failed to flush compact probe frame: {error}"))
}

fn run_native_health_check(
    rom: PathBuf,
    instructions_per_frame: u64,
    branch_frames: u64,
    settle_frames: u64,
    wall_timeout_secs: u64,
) -> Result<(), String> {
    let health_started_at = Instant::now();
    let health_wall_timeout = Duration::from_secs(wall_timeout_secs.max(1));
    let instructions_per_frame = instructions_per_frame.max(1);
    let health_script_instructions_per_frame =
        native_fast_forward_instructions_per_frame(instructions_per_frame);
    let romset = NativeRomSet::scan_cached(rom.clone()).map_err(|error| error.to_string())?;
    let rom_compatibility = romset.compatibility_report();
    let assets_complete = rom_compatibility.native_runtime_usable();
    let exact_mame_assets = rom_compatibility.compatible();

    let mut checkpoint = native_emulator_from_rom_source(rom.clone())?;
    checkpoint.apply_auto_coin_input_mapping();
    let checkpoint_segments = native_health_checkpoint_script();
    let checkpoint_progress = run_native_health_checkpoint_progress(
        &mut checkpoint,
        health_script_instructions_per_frame,
        &checkpoint_segments,
        health_started_at,
        health_wall_timeout,
    );
    let checkpoint_run = checkpoint_progress.summary;
    let checkpoint_activity = checkpoint.input_activity();
    let checkpoint_raw_frame = checkpoint.display_frame();
    let checkpoint_stats =
        NativeFrameStats::from_frame(&native_play_window_frame(&checkpoint_raw_frame));
    let checkpoint_native_playable = checkpoint.native_playable_candidate();
    let checkpoint_window_frame = native_play_window_frame(&checkpoint_raw_frame);
    let checkpoint_full_scene =
        native_play_frame_ready_with_context(checkpoint_native_playable, &checkpoint_window_frame);
    let checkpoint_runtime_limited = native_script_run_hit_runtime_limit(checkpoint_run);
    let checkpoint_followup_blocked = checkpoint_runtime_limited
        && (!checkpoint_stats.has_visible_content() || checkpoint.is_terminal());

    let checkpoint_playable_match_entry = checkpoint_native_playable && checkpoint_full_scene;
    let mut match_entry_segments = if checkpoint_playable_match_entry {
        Vec::new()
    } else {
        remaining_native_script_segments(
            &checkpoint_segments,
            checkpoint_progress.segment_index,
            checkpoint_progress.segment_frame,
        )
    };
    if !checkpoint_playable_match_entry {
        match_entry_segments.extend(native_health_match_entry_tail_script());
    }
    let (match_entry, match_entry_run) = if checkpoint_followup_blocked {
        (
            checkpoint.clone(),
            NativeScriptRunSummary::skipped("skipped_checkpoint_not_playable"),
        )
    } else if checkpoint_playable_match_entry {
        (
            checkpoint.clone(),
            NativeScriptRunSummary::skipped("skipped_checkpoint_already_playable"),
        )
    } else if let Some(match_entry_timeout) = native_health_stage_wall_timeout_with_reserve(
        health_started_at,
        health_wall_timeout,
        Duration::from_secs(NATIVE_HEALTH_MATCH_ENTRY_WALL_TIMEOUT_SECS),
        Duration::from_secs(NATIVE_HEALTH_CONTROL_SWEEP_RESERVE_SECS),
    ) {
        let mut match_entry = checkpoint.clone();
        let match_entry_progress = run_native_script_observed_until_playable_with_limit_and_timeout(
            &mut match_entry,
            health_script_instructions_per_frame,
            &match_entry_segments,
            native_script_max_frames_for_segments(&match_entry_segments),
            match_entry_timeout,
        );
        (match_entry, match_entry_progress.summary)
    } else {
        (
            checkpoint.clone(),
            NativeScriptRunSummary::skipped("skipped_health_wall_timeout"),
        )
    };
    let match_entry_activity = match_entry.input_activity();
    let match_entry_raw_frame = match_entry.display_frame();
    let match_entry_stats = NativeFrameStats::from_frame(&match_entry_raw_frame);
    let match_entry_window_stats =
        NativeFrameStats::from_frame(&native_play_window_frame(&match_entry_raw_frame));
    let match_entry_native_playable = match_entry.native_playable_candidate();
    let match_entry_full_size = match_entry_window_stats.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
        && match_entry_window_stats.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT;
    let match_entry_window_frame = native_play_window_frame(&match_entry_raw_frame);
    let match_entry_full_scene = match_entry_full_size
        && native_play_frame_ready_with_context(
            match_entry_native_playable,
            &match_entry_window_frame,
        );
    let match_entry_known_rendering_gap =
        !match_entry_full_scene && match_entry_window_stats.has_visible_content();
    let match_entry_json = format!(
        "{{\"run\":{},\"resume_from_checkpoint\":true,\"segments\":[{}],\"executed_steps\":{},\"terminal\":{},\"native_playable_candidate\":{},\"input_activity\":{},\"frame\":{},\"raw_frame\":{},\"window_frame\":{},\"full_size\":{},\"full_scene\":{},\"known_rendering_gap\":{},\"state\":{}}}",
        match_entry_run.json(),
        native_script_segments_json(&match_entry_segments),
        match_entry.executed_steps(),
        match_entry.is_terminal(),
        match_entry_native_playable,
        match_entry_activity.json(),
        match_entry_window_stats.json(),
        match_entry_stats.json(),
        match_entry_window_stats.json(),
        match_entry_full_size,
        match_entry_full_scene,
        match_entry_known_rendering_gap,
        match_entry.input_check_probe_json()
    );

    let mut control_sweep = checkpoint.clone();
    let control_sweep_segments = native_control_sweep_script(branch_frames, settle_frames);
    let control_sweep_run = if checkpoint_followup_blocked {
        NativeScriptRunSummary::skipped("skipped_checkpoint_not_playable")
    } else if let Some(control_sweep_timeout) = native_health_stage_wall_timeout(
        health_started_at,
        health_wall_timeout,
        Duration::from_secs(NATIVE_SCRIPT_TIMED_WALL_TIMEOUT_SECS),
    ) {
        run_native_script_observed_with_stop_and_timeout(
            &mut control_sweep,
            health_script_instructions_per_frame,
            &control_sweep_segments,
            NativeScriptStopMode::Timed,
            Some(native_script_max_frames_for_segments(
                &control_sweep_segments,
            )),
            Some(control_sweep_timeout),
        )
        .summary
    } else {
        NativeScriptRunSummary::skipped("skipped_health_wall_timeout")
    };
    let control_sweep_activity = control_sweep.input_activity();
    let control_sweep_raw_frame = control_sweep.display_frame();
    let control_sweep_stats =
        NativeFrameStats::from_frame(&native_play_window_frame(&control_sweep_raw_frame));
    let control_sweep_native_playable = control_sweep.native_playable_candidate();

    let direct_input_probe = if checkpoint_followup_blocked {
        NativeDirectInputProbe::skipped("skipped_checkpoint_not_playable")
    } else {
        let mut direct_input = checkpoint.clone();
        run_native_direct_input_probe(&mut direct_input, native_health_branch_actions())
    };

    let mut branches = Vec::new();
    let mut branch_attempt_count = 0usize;
    let mut branch_action_read_count = 0usize;
    let mut branch_native_playable_count = 0usize;
    let mut branch_full_scene_count = 0usize;
    let mut branch_input_activity = NativeInputActivity::default();
    let branch_soak_needed = !(control_sweep_native_playable
        && native_combined_full_controls_active(checkpoint_activity, control_sweep_activity));
    if !checkpoint_followup_blocked && branch_soak_needed {
        for &action in native_health_branch_actions() {
            let Some(branch_timeout) = native_health_stage_wall_timeout(
                health_started_at,
                health_wall_timeout,
                Duration::from_secs(NATIVE_SCRIPT_TIMED_WALL_TIMEOUT_SECS),
            ) else {
                break;
            };
            branch_attempt_count += 1;
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
            let branch_run = run_native_script_observed_with_stop_and_timeout(
                &mut branch,
                health_script_instructions_per_frame,
                &branch_segments,
                NativeScriptStopMode::Timed,
                Some(native_script_max_frames_for_segments(&branch_segments)),
                Some(branch_timeout),
            )
            .summary;
            let branch_activity = branch.input_activity();
            let branch_delta = branch_activity.saturating_subtracted(checkpoint_activity);
            let action_read = native_action_activity_observed(branch_delta, action);
            branch_input_activity = branch_input_activity.saturating_added(branch_delta);
            let branch_raw_frame = branch.display_frame();
            let branch_stats =
                NativeFrameStats::from_frame(&native_play_window_frame(&branch_raw_frame));
            let branch_native_playable = branch.native_playable_candidate();
            if action_read {
                branch_action_read_count += 1;
            }
            if branch_native_playable {
                branch_native_playable_count += 1;
            }
            let branch_window_frame = native_play_window_frame(&branch_raw_frame);
            let branch_full_scene =
                native_play_frame_ready_with_context(branch_native_playable, &branch_window_frame);
            if branch_full_scene {
                branch_full_scene_count += 1;
            }
            branches.push(format!(
                "{{\"action_index\":{},\"action\":\"{}\",\"action_activity_observed\":{},\"native_playable_candidate\":{},\"gameplay_scene\":{},\"terminal\":{},\"run\":{},\"input_activity\":{},\"frame\":{},\"state\":{}}}",
                action.index(),
                action.name(),
                action_read,
                branch_native_playable,
                branch_full_scene,
                branch.is_terminal(),
                branch_run.json(),
                branch_activity.json(),
                branch_stats.json(),
                branch.input_check_probe_json()
            ));
        }
    }

    let native_core_running = checkpoint.executed_steps() > 0 && !checkpoint.is_terminal();
    let combined_control_activity = control_sweep_activity
        .saturating_added(branch_input_activity)
        .saturating_added(direct_input_probe.activity);
    let match_entry_delta_activity =
        match_entry_activity.saturating_subtracted(checkpoint_activity);
    let total_input_activity = checkpoint_activity
        .saturating_added(combined_control_activity)
        .saturating_added(match_entry_delta_activity);
    let play_controls_active =
        native_combined_play_controls_active(checkpoint_activity, combined_control_activity);
    let full_controls_active =
        native_combined_full_controls_active(checkpoint_activity, combined_control_activity);
    let branch_count = native_health_branch_actions().len();
    let all_branch_actions_attempted = branch_attempt_count == branch_count;
    let all_branch_actions_read =
        all_branch_actions_attempted && branch_action_read_count == branch_count;
    let control_sweep_actions_read = full_controls_active;
    let direct_input_actions_read = direct_input_probe.action_read_count == branch_count;
    let all_control_actions_read =
        all_branch_actions_read || control_sweep_actions_read || direct_input_actions_read;
    let all_branches_native_playable = branch_native_playable_count == branch_count;
    let rendering_present =
        checkpoint_stats.has_visible_content() || control_sweep_stats.has_visible_content();
    let checkpoint_render_ready = checkpoint_stats.has_render_ready_scene();
    let control_sweep_render_ready = control_sweep_stats.has_render_ready_scene();
    let manual_checkpoint_ready =
        checkpoint_stats.has_visible_content() && !checkpoint.is_terminal();
    let control_sweep_window_frame = native_play_window_frame(&control_sweep_raw_frame);
    let control_sweep_full_scene = native_play_frame_ready_with_context(
        control_sweep_native_playable,
        &control_sweep_window_frame,
    );
    let display_detail_present = checkpoint_stats.has_scene_detail()
        || control_sweep_stats.has_scene_detail()
        || branch_full_scene_count > 0;
    let native_play_handoff_full_scene =
        checkpoint_full_scene || checkpoint_native_playable && checkpoint_stats.has_handoff_scene();
    let match_entry_rendering_complete = match_entry_full_scene && !match_entry_known_rendering_gap;
    let match_entry_render_ready = match_entry_window_stats.has_render_ready_scene();
    let full_scene_rendering = native_play_handoff_full_scene && match_entry_rendering_complete;
    let control_sweep_reaches_playable = control_sweep_native_playable
        || branch_native_playable_count > 0
        || match_entry_native_playable;
    let render_ready = native_core_running
        && assets_complete
        && (checkpoint_render_ready || control_sweep_render_ready || match_entry_render_ready);
    let input_poll_ready = play_controls_active && full_controls_active && all_control_actions_read;
    let credit_probe_ready = native_credit_probe_ready(total_input_activity);
    let credit_register_ready = native_credit_register_ready(total_input_activity);
    let credit_transition_verified = native_credit_transition_verified(
        credit_probe_ready,
        match_entry_native_playable,
        match_entry_full_scene,
    );
    let credit_system_port_ready =
        native_credit_system_port_ready(input_poll_ready, total_input_activity, credit_probe_ready);
    let credit_adapter_ready = native_credit_adapter_ready(total_input_activity);
    let credit_ready = native_credit_ready(
        credit_probe_ready,
        credit_register_ready,
        credit_transition_verified,
        credit_adapter_ready,
    );
    let credit_gap_reason = native_credit_gap_reason(
        input_poll_ready,
        total_input_activity,
        credit_probe_ready,
        credit_register_ready,
        credit_transition_verified,
        credit_system_port_ready,
    );
    let input_ready = input_poll_ready && credit_ready;
    let gameplay_ready =
        control_sweep_reaches_playable && full_scene_rendering && match_entry_rendering_complete;
    let known_rendering_gap = match_entry_known_rendering_gap
        || rendering_present && (!full_scene_rendering || !control_sweep_reaches_playable);
    let input_ok_rendering_gap = input_ready && known_rendering_gap && !gameplay_ready;
    let rendering_gap_reason = native_health_rendering_gap_reason(
        rendering_present,
        control_sweep_reaches_playable,
        full_scene_rendering,
        match_entry_rendering_complete,
        known_rendering_gap,
    );
    let health_wall_timed_out = health_started_at.elapsed() >= health_wall_timeout;
    let overall_pass = native_core_running
        && assets_complete
        && manual_checkpoint_ready
        && input_ready
        && render_ready
        && gameplay_ready;
    let overall_status = if overall_pass {
        "pass"
    } else if native_core_running || rendering_present || play_controls_active {
        "partial"
    } else {
        "fail"
    };

    println!(
        "{{\"overall_status\":\"{}\",\"overall_pass\":{},\"instructions_per_frame\":{},\"health_script_instructions_per_frame\":{},\"branch_frames\":{},\"settle_frames\":{},\"health_wall_timeout_secs\":{},\"health_wall_timed_out\":{},\"assets_complete\":{},\"exact_mame_assets\":{},\"rom_compatibility\":{},\"native_core_running\":{},\"manual_checkpoint_ready\":{},\"render_ready\":{},\"input_ready\":{},\"input_poll_ready\":{},\"credit_ready\":{},\"credit_probe_ready\":{},\"credit_register_ready\":{},\"credit_transition_verified\":{},\"credit_adapter_ready\":{},\"credit_system_port_ready\":{},\"credit_gap_reason\":\"{}\",\"total_input_activity\":{},\"gameplay_ready\":{},\"play_controls_active\":{},\"full_controls_active\":{},\"all_branch_actions_attempted\":{},\"all_branch_actions_read\":{},\"control_sweep_actions_read\":{},\"direct_input_actions_read\":{},\"all_control_actions_read\":{},\"branch_attempt_count\":{},\"branch_action_read_count\":{},\"direct_input_action_read_count\":{},\"all_branches_native_playable\":{},\"control_sweep_reaches_playable\":{},\"rendering_present\":{},\"display_detail_present\":{},\"checkpoint_render_ready\":{},\"checkpoint_full_scene\":{},\"checkpoint_runtime_limited\":{},\"checkpoint_followup_blocked\":{},\"control_sweep_render_ready\":{},\"control_sweep_full_scene\":{},\"match_entry_render_ready\":{},\"match_entry_full_scene\":{},\"match_entry_known_rendering_gap\":{},\"match_entry_rendering_complete\":{},\"native_play_handoff_full_scene\":{},\"full_scene_rendering\":{},\"known_rendering_gap\":{},\"input_ok_rendering_gap\":{},\"rendering_gap_reason\":\"{}\",\"branch_native_playable_count\":{},\"branch_full_scene_count\":{},\"branch_count\":{},\"checkpoint\":{{\"run\":{},\"segments\":[{}],\"executed_steps\":{},\"terminal\":{},\"native_playable_candidate\":{},\"input_activity\":{},\"frame\":{},\"state\":{}}},\"control_sweep\":{{\"run\":{},\"segments\":[{}],\"executed_steps\":{},\"terminal\":{},\"native_playable_candidate\":{},\"input_activity\":{},\"frame\":{},\"state\":{}}},\"direct_input_probe\":{},\"match_entry\":{},\"branches\":[{}]}}",
        overall_status,
        overall_pass,
        instructions_per_frame,
        health_script_instructions_per_frame,
        branch_frames,
        settle_frames,
        wall_timeout_secs,
        health_wall_timed_out,
        assets_complete,
        exact_mame_assets,
        rom_compatibility.summary_json(),
        native_core_running,
        manual_checkpoint_ready,
        render_ready,
        input_ready,
        input_poll_ready,
        credit_ready,
        credit_probe_ready,
        credit_register_ready,
        credit_transition_verified,
        credit_adapter_ready,
        credit_system_port_ready,
        credit_gap_reason,
        total_input_activity.json(),
        gameplay_ready,
        play_controls_active,
        full_controls_active,
        all_branch_actions_attempted,
        all_branch_actions_read,
        control_sweep_actions_read,
        direct_input_actions_read,
        all_control_actions_read,
        branch_attempt_count,
        branch_action_read_count,
        direct_input_probe.action_read_count,
        all_branches_native_playable,
        control_sweep_reaches_playable,
        rendering_present,
        display_detail_present,
        checkpoint_render_ready,
        checkpoint_full_scene,
        checkpoint_runtime_limited,
        checkpoint_followup_blocked,
        control_sweep_render_ready,
        control_sweep_full_scene,
        match_entry_render_ready,
        match_entry_full_scene,
        match_entry_known_rendering_gap,
        match_entry_rendering_complete,
        native_play_handoff_full_scene,
        full_scene_rendering,
        known_rendering_gap,
        input_ok_rendering_gap,
        rendering_gap_reason,
        branch_native_playable_count,
        branch_full_scene_count,
        branch_count,
        checkpoint_run.json(),
        native_script_segments_json(&checkpoint_segments),
        checkpoint.executed_steps(),
        checkpoint.is_terminal(),
        checkpoint_native_playable,
        checkpoint_activity.json(),
        checkpoint_stats.json(),
        checkpoint.input_check_probe_json(),
        control_sweep_run.json(),
        native_script_segments_json(&control_sweep_segments),
        control_sweep.executed_steps(),
        control_sweep.is_terminal(),
        control_sweep_native_playable,
        control_sweep_activity.json(),
        control_sweep_stats.json(),
        control_sweep.input_check_probe_json(),
        direct_input_probe.json(),
        match_entry_json,
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

fn run_native_health_checkpoint_progress(
    checkpoint: &mut NativeEmulator,
    instructions_per_frame: u64,
    checkpoint_segments: &[NativeScriptSegment],
    health_started_at: Instant,
    health_wall_timeout: Duration,
) -> NativeScriptProgress {
    let boot_wait_segments = native_play_boot_wait_segments(checkpoint_segments);
    let boot_progress = if let Some(boot_timeout) = native_health_stage_wall_timeout(
        health_started_at,
        health_wall_timeout,
        Duration::from_secs(NATIVE_INPUT_CHECK_WALL_TIMEOUT_SECS),
    ) {
        run_native_title_ready_boot_wait_with_timeout(
            checkpoint,
            instructions_per_frame,
            &boot_wait_segments,
            Some(boot_timeout),
            "skipped_health_wall_timeout",
        )
    } else {
        NativeScriptProgress::skipped("skipped_health_wall_timeout")
    };

    let tail_segments = remaining_native_script_segments_after_title_ready(
        checkpoint_segments,
        boot_progress.segment_index,
        boot_progress.segment_frame,
    );
    let tail_progress = if boot_progress.summary.stop_reason == "wall_timeout"
        || !native_title_ready_boot_progress_allows_tail(checkpoint, boot_progress)
    {
        NativeScriptProgress::skipped("skipped_checkpoint_boot_timeout")
    } else if let Some(tail_timeout) = native_health_stage_wall_timeout(
        health_started_at,
        health_wall_timeout,
        Duration::from_secs(NATIVE_INPUT_CHECK_WALL_TIMEOUT_SECS),
    ) {
        run_native_script_observed_with_stop_and_timeout(
            checkpoint,
            instructions_per_frame,
            &tail_segments,
            NativeScriptStopMode::FastTimed,
            Some(native_script_max_frames_for_segments(&tail_segments)),
            Some(tail_timeout),
        )
    } else {
        NativeScriptProgress::skipped("skipped_health_wall_timeout")
    };

    let render_settle_progress = if matches!(
        tail_progress.summary.stop_reason,
        "wall_timeout" | "skipped_health_wall_timeout" | "skipped_checkpoint_boot_timeout"
    ) {
        NativeScriptProgress::skipped("skipped_checkpoint_tail_timeout")
    } else if let Some(render_timeout) = native_health_stage_wall_timeout(
        health_started_at,
        health_wall_timeout,
        Duration::from_secs(NATIVE_INPUT_CHECK_WALL_TIMEOUT_SECS),
    ) {
        run_native_script_observed_with_stop_and_timeout(
            checkpoint,
            instructions_per_frame,
            &[NativeScriptSegment {
                action: Action::Noop,
                frames: NATIVE_PLAY_SNAPSHOT_RENDER_SETTLE_FRAMES,
            }],
            NativeScriptStopMode::Timed,
            Some(NATIVE_PLAY_SNAPSHOT_RENDER_SETTLE_FRAMES),
            Some(render_timeout),
        )
    } else {
        NativeScriptProgress::skipped("skipped_health_wall_timeout")
    };

    NativeScriptProgress {
        summary: combine_native_script_run_summaries(
            combine_native_script_run_summaries(boot_progress.summary, tail_progress.summary),
            render_settle_progress.summary,
        ),
        segment_index: checkpoint_segments.len(),
        segment_frame: 0,
    }
}

fn combine_native_script_run_summaries(
    first: NativeScriptRunSummary,
    second: NativeScriptRunSummary,
) -> NativeScriptRunSummary {
    NativeScriptRunSummary {
        total_frames: first.total_frames.saturating_add(second.total_frames),
        frame_attempts: first.frame_attempts.saturating_add(second.frame_attempts),
        missed_vblank_frames: first
            .missed_vblank_frames
            .saturating_add(second.missed_vblank_frames),
        observed_native_playable_candidate: first.observed_native_playable_candidate
            || second.observed_native_playable_candidate,
        first_native_playable_frame: first.first_native_playable_frame.or_else(|| {
            second
                .first_native_playable_frame
                .map(|frame| first.total_frames.saturating_add(frame))
        }),
        last_native_playable_frame: second
            .last_native_playable_frame
            .map(|frame| first.total_frames.saturating_add(frame))
            .or(first.last_native_playable_frame),
        stop_reason: second.stop_reason,
    }
}

fn native_health_rendering_gap_reason(
    rendering_present: bool,
    control_sweep_reaches_playable: bool,
    full_scene_rendering: bool,
    match_entry_rendering_complete: bool,
    known_rendering_gap: bool,
) -> &'static str {
    if !rendering_present {
        return "no_rendering_present";
    } else if !control_sweep_reaches_playable {
        return "gpu_scene_not_rendered";
    } else if !full_scene_rendering {
        return "full_scene_not_composed";
    } else if !match_entry_rendering_complete {
        return "match_entry_not_full_scene";
    } else if known_rendering_gap {
        return "known_rendering_gap";
    }

    "none"
}

fn native_credit_mapping_probe_default_mappings() -> &'static [&'static str] {
    &[
        "mame",
        "legacy-zinc",
        "system20",
        "system10-system20",
        "service01",
        "service02",
        "service10",
        "service02-service10",
        "coin-register-bit0",
        "all",
    ]
}

fn native_credit_mapping_probe_tail_script() -> Vec<NativeScriptSegment> {
    vec![
        NativeScriptSegment {
            action: Action::Coin,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Coin,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 60,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 36,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 240,
        },
        NativeScriptSegment {
            action: Action::CoinStart,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 60,
        },
        NativeScriptSegment {
            action: Action::Punch,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 180,
        },
    ]
}

fn run_native_credit_mapping_probe(
    rom: PathBuf,
    instructions_per_frame: u64,
    max_frames: u64,
    mappings: Vec<String>,
) -> Result<(), String> {
    let mut full_script = false;
    let mut requested_mappings = Vec::new();
    for mapping in mappings {
        if mapping == "--full-script" {
            full_script = true;
        } else {
            requested_mappings.push(mapping);
        }
    }
    let mapping_names = if requested_mappings.is_empty() {
        native_credit_mapping_probe_default_mappings()
            .iter()
            .map(|mapping| (*mapping).to_string())
            .collect::<Vec<_>>()
    } else {
        requested_mappings
    };
    let boot_segments = native_warning_skip_to_title_script();
    let (title_emulator, boot_run_json, boot_segment_json) = if full_script {
        (None, "null".to_string(), "[]".to_string())
    } else {
        let mut emulator = native_emulator_from_rom_source(rom.clone())
            .map_err(|error| format!("failed to create title probe emulator: {error}"))?;
        let boot_run = run_native_title_ready_boot_wait_with_timeout(
            &mut emulator,
            native_fast_forward_instructions_per_frame(instructions_per_frame),
            &boot_segments,
            Some(native_credit_mapping_probe_wall_timeout()),
            "skipped_credit_mapping_boot_timeout",
        )
        .summary;
        (
            Some(emulator),
            boot_run.json(),
            native_script_segments_json(&boot_segments),
        )
    };
    let segments = if full_script {
        default_native_play_script()
    } else {
        native_credit_mapping_probe_tail_script()
    };
    let script_total_frames = native_script_max_frames_for_segments(&segments);
    let script_max_frames = max_frames.max(1);
    let mut reports = Vec::new();

    for mapping in mapping_names {
        let mut emulator = if let Some(title_emulator) = &title_emulator {
            title_emulator.clone()
        } else {
            native_emulator_from_rom_source(rom.clone())
                .map_err(|error| format!("failed to create emulator for {mapping}: {error}"))?
        };
        if !emulator.set_coin_input_mapping_name(&mapping) {
            return Err(format!("unknown coin input mapping: {mapping}"));
        }
        let run = run_native_script_observed_with_stop_and_timeout(
            &mut emulator,
            native_fast_forward_instructions_per_frame(instructions_per_frame),
            &segments,
            NativeScriptStopMode::Timed,
            Some(script_max_frames),
            Some(native_credit_mapping_probe_wall_timeout()),
        )
        .summary;
        let frame = native_play_window_frame_for_emulator(&emulator);
        let frame_stats = NativeFrameStats::from_frame(&frame);
        let native_playable_candidate = emulator.native_playable_candidate();
        let frame_ready = native_play_frame_ready_with_context(native_playable_candidate, &frame);
        let activity = emulator.input_activity();
        let input_poll_ready = activity.p1_input_reads > 0
            && activity.system_input_reads > 0
            && activity.has_any_play_control_activity()
            && activity.has_credit_probe_activity();
        let credit_probe_ready = native_credit_probe_ready(activity);
        let credit_register_ready = native_credit_register_ready(activity);
        let credit_transition_verified = native_credit_transition_verified(
            credit_probe_ready,
            native_playable_candidate,
            frame_ready,
        );
        let _credit_system_port_ready =
            native_credit_system_port_ready(input_poll_ready, activity, credit_probe_ready);
        let credit_adapter_ready = native_credit_adapter_ready(activity);
        let credit_ready = native_credit_ready(
            credit_probe_ready,
            credit_register_ready,
            credit_transition_verified,
            credit_adapter_ready,
        );
        let input_ready = input_poll_ready && credit_ready;
        let gameplay_scene =
            native_play_gameplay_scene_with_context(native_playable_candidate, frame_stats);
        let reached_gameplay = input_ready && native_playable_candidate && gameplay_scene;
        eprintln!(
            "native-credit-mapping-probe mapping={} frames={} stop_reason={} input_ready={} reached_gameplay={} playable={}",
            mapping,
            run.total_frames,
            run.stop_reason,
            input_ready,
            reached_gameplay,
            native_playable_candidate
        );
        reports.push(format!(
            "{{\"mapping\":\"{}\",\"instructions_per_frame\":{},\"max_frames\":{},\"script_total_frames\":{},\"run\":{},\"executed_steps\":{},\"native_playable_candidate\":{},\"frame_ready\":{},\"gameplay_scene\":{},\"title_logo_overlay\":{},\"input_poll_ready\":{},\"input_ready\":{},\"credit_ready\":{},\"credit_probe_ready\":{},\"credit_register_ready\":{},\"credit_transition_verified\":{},\"credit_adapter_ready\":{},\"reached_gameplay\":{},\"input_activity\":{},\"frame\":{},\"state\":{}}}",
            escape_json(&mapping),
            instructions_per_frame,
            script_max_frames,
            script_total_frames,
            run.json(),
            emulator.executed_steps(),
            native_playable_candidate,
            frame_ready,
            gameplay_scene,
            frame_stats.has_title_logo_overlay(),
            input_poll_ready,
            input_ready,
            credit_ready,
            credit_probe_ready,
            credit_register_ready,
            credit_transition_verified,
            credit_adapter_ready,
            reached_gameplay,
            activity.json(),
            frame_stats.json(),
            emulator.input_compact_probe_json()
        ));
    }

    println!(
        "{{\"instructions_per_frame\":{},\"script_instructions_per_frame\":{},\"max_frames\":{},\"script_total_frames\":{},\"full_script\":{},\"boot_run\":{},\"boot_segments\":[{}],\"wall_timeout_secs_per_mapping\":{},\"segments\":[{}],\"reports\":[{}]}}",
        instructions_per_frame,
        native_fast_forward_instructions_per_frame(instructions_per_frame),
        script_max_frames,
        script_total_frames,
        full_script,
        boot_run_json,
        boot_segment_json,
        native_credit_mapping_probe_wall_timeout().as_secs(),
        native_script_segments_json(&segments),
        reports.join(",")
    );
    Ok(())
}

fn native_credit_bit_probe_default_bits() -> &'static [u32] {
    &[
        0x0000_0001,
        0x0000_0002,
        0x0000_0004,
        0x0000_0008,
        0x0000_0010,
        0x0000_0020,
        0x0000_0040,
        0x0000_0080,
        0x0000_0100,
        0x0000_0200,
        0x0000_0400,
        0x0000_0800,
        0x0800_0000,
        0x0800_0800,
    ]
}

fn parse_native_credit_bit_values(values: Vec<String>) -> Result<Vec<u32>, String> {
    if values.is_empty() {
        return Ok(native_credit_bit_probe_default_bits().to_vec());
    }

    values
        .into_iter()
        .map(|value| {
            let bit = parse_trace_u32(&value, "credit bit")?;
            if bit == 0 {
                return Err("credit bit must be non-zero".to_string());
            }
            Ok(bit)
        })
        .collect()
}

fn run_native_credit_bit_probe(
    rom: PathBuf,
    instructions_per_frame: u64,
    max_frames: u64,
    mapping: String,
    bits: Vec<u32>,
) -> Result<(), String> {
    if bits.is_empty() {
        return Err("at least one credit bit is required".to_string());
    }

    let boot_segments = native_warning_skip_to_title_script();
    let mut title_emulator = native_emulator_from_rom_source(rom)
        .map_err(|error| format!("failed to create credit bit probe emulator: {error}"))?;
    let boot_run = run_native_title_ready_boot_wait_with_timeout(
        &mut title_emulator,
        native_fast_forward_instructions_per_frame(instructions_per_frame),
        &boot_segments,
        Some(native_credit_mapping_probe_wall_timeout()),
        "skipped_credit_bit_boot_timeout",
    )
    .summary;
    let segments = native_credit_mapping_probe_tail_script();
    let script_total_frames = native_script_max_frames_for_segments(&segments);
    let script_max_frames = max_frames.max(1);
    let mut reports = Vec::new();

    for bit in bits {
        let mut emulator = title_emulator.clone();
        if !emulator.set_coin_input_mapping_name(&mapping) {
            return Err(format!("unknown coin input mapping: {mapping}"));
        }
        if !emulator.set_native_credit_adapter_input_bit(bit) {
            return Err(format!("invalid native credit adapter bit: 0x{bit:08x}"));
        }
        let run = run_native_script_observed_with_stop_and_timeout(
            &mut emulator,
            native_fast_forward_instructions_per_frame(instructions_per_frame),
            &segments,
            NativeScriptStopMode::Timed,
            Some(script_max_frames),
            Some(native_credit_mapping_probe_wall_timeout()),
        )
        .summary;
        let frame = native_play_window_frame_for_emulator(&emulator);
        let frame_stats = NativeFrameStats::from_frame(&frame);
        let native_playable_candidate = emulator.native_playable_candidate();
        let frame_ready = native_play_frame_ready_with_context(native_playable_candidate, &frame);
        let activity = emulator.input_activity();
        let input_poll_ready = activity.p1_input_reads > 0
            && activity.system_input_reads > 0
            && activity.has_any_play_control_activity()
            && activity.has_credit_probe_activity();
        let credit_probe_ready = native_credit_probe_ready(activity);
        let credit_register_ready = native_credit_register_ready(activity);
        let credit_transition_verified = native_credit_transition_verified(
            credit_probe_ready,
            native_playable_candidate,
            frame_ready,
        );
        let _credit_system_port_ready =
            native_credit_system_port_ready(input_poll_ready, activity, credit_probe_ready);
        let credit_adapter_ready = native_credit_adapter_ready(activity);
        let credit_ready = native_credit_ready(
            credit_probe_ready,
            credit_register_ready,
            credit_transition_verified,
            credit_adapter_ready,
        );
        let input_ready = input_poll_ready && credit_ready;
        let gameplay_scene =
            native_play_gameplay_scene_with_context(native_playable_candidate, frame_stats);
        let reached_gameplay = input_ready && native_playable_candidate && gameplay_scene;
        eprintln!(
            "native-credit-bit-probe bit=0x{bit:08x} frames={} stop_reason={} input_ready={} reached_gameplay={} playable={}",
            run.total_frames,
            run.stop_reason,
            input_ready,
            reached_gameplay,
            native_playable_candidate
        );
        reports.push(format!(
            "{{\"bit\":{},\"bit_hex\":\"0x{:08x}\",\"mapping\":\"{}\",\"instructions_per_frame\":{},\"max_frames\":{},\"script_total_frames\":{},\"run\":{},\"executed_steps\":{},\"native_playable_candidate\":{},\"frame_ready\":{},\"gameplay_scene\":{},\"title_logo_overlay\":{},\"input_poll_ready\":{},\"input_ready\":{},\"credit_ready\":{},\"credit_probe_ready\":{},\"credit_register_ready\":{},\"credit_transition_verified\":{},\"credit_adapter_ready\":{},\"reached_gameplay\":{},\"native_credit_adapter_writes\":{},\"native_credit_adapter_edges\":{},\"p1_input_reads\":{},\"system_input_reads\":{},\"system_coin_active_reads\":{},\"system_start_active_reads\":{},\"last_system_input_hex\":\"0x{:08x}\",\"frame\":{}}}",
            bit,
            bit,
            escape_json(&mapping),
            instructions_per_frame,
            script_max_frames,
            script_total_frames,
            run.json(),
            emulator.executed_steps(),
            native_playable_candidate,
            frame_ready,
            gameplay_scene,
            frame_stats.has_title_logo_overlay(),
            input_poll_ready,
            input_ready,
            credit_ready,
            credit_probe_ready,
            credit_register_ready,
            credit_transition_verified,
            credit_adapter_ready,
            reached_gameplay,
            activity.native_credit_adapter_writes,
            activity.native_credit_adapter_edges,
            activity.p1_input_reads,
            activity.system_input_reads,
            activity.system_coin_active_reads,
            activity.system_start_active_reads,
            activity.last_system_input,
            frame_stats.json()
        ));
    }

    println!(
        "{{\"instructions_per_frame\":{},\"script_instructions_per_frame\":{},\"max_frames\":{},\"script_total_frames\":{},\"mapping\":\"{}\",\"boot_run\":{},\"boot_segments\":[{}],\"wall_timeout_secs_per_bit\":{},\"segments\":[{}],\"reports\":[{}]}}",
        instructions_per_frame,
        native_fast_forward_instructions_per_frame(instructions_per_frame),
        script_max_frames,
        script_total_frames,
        escape_json(&mapping),
        boot_run.json(),
        native_script_segments_json(&boot_segments),
        native_credit_mapping_probe_wall_timeout().as_secs(),
        native_script_segments_json(&segments),
        reports.join(",")
    );
    Ok(())
}

fn native_credit_projection_probe_default_names() -> &'static [&'static str] {
    &["default", "current", "edge", "previous", "copies", "wide"]
}

fn native_credit_debug_word_addresses() -> &'static [u32] {
    &[
        0x1f80_0068,
        0x1f80_006c,
        0x1f80_0070,
        0x1f80_0074,
        0x1f80_0078,
        0x1f80_007c,
        0x1f80_0080,
        0x1f80_009c,
        0x1f80_00b8,
        0x1f80_00cc,
        0x803b_fa00,
        0x803b_fa04,
        0x803b_fa08,
        0x803b_fa0c,
        0x803b_fa10,
        0x803b_fa14,
        0x803b_fa18,
        0x803b_fa1c,
    ]
}

fn native_credit_debug_words_json(emulator: &NativeEmulator) -> String {
    let ram = emulator.ram_snapshot();
    let scratchpad = emulator.scratchpad_snapshot();
    native_credit_debug_word_addresses()
        .iter()
        .map(|address| native_debug_word_json(emulator, *address, &ram, &scratchpad))
        .collect::<Vec<_>>()
        .join(",")
}

fn run_native_credit_projection_probe(
    rom: PathBuf,
    instructions_per_frame: u64,
    max_frames: u64,
    mapping: String,
    bit: u32,
    projections: Vec<String>,
) -> Result<(), String> {
    let projection_names = if projections.is_empty() {
        native_credit_projection_probe_default_names()
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>()
    } else {
        projections
    };
    let boot_segments = native_warning_skip_to_title_script();
    let mut title_emulator = native_emulator_from_rom_source(rom)
        .map_err(|error| format!("failed to create credit projection probe emulator: {error}"))?;
    let boot_run = run_native_title_ready_boot_wait_with_timeout(
        &mut title_emulator,
        native_fast_forward_instructions_per_frame(instructions_per_frame),
        &boot_segments,
        Some(native_credit_mapping_probe_wall_timeout()),
        "skipped_credit_projection_boot_timeout",
    )
    .summary;
    let segments = native_credit_mapping_probe_tail_script();
    let script_total_frames = native_script_max_frames_for_segments(&segments);
    let script_max_frames = max_frames.max(1);
    let mut reports = Vec::new();

    for projection in projection_names {
        let mut emulator = title_emulator.clone();
        if !emulator.set_coin_input_mapping_name(&mapping) {
            return Err(format!("unknown coin input mapping: {mapping}"));
        }
        if !emulator.set_native_credit_adapter_input_bit(bit) {
            return Err(format!("invalid native credit adapter bit: 0x{bit:08x}"));
        }
        if !emulator.set_native_credit_projection_name(&projection) {
            return Err(format!("unknown native credit projection: {projection}"));
        }
        let run = run_native_script_observed_with_stop_and_timeout(
            &mut emulator,
            native_fast_forward_instructions_per_frame(instructions_per_frame),
            &segments,
            NativeScriptStopMode::Timed,
            Some(script_max_frames),
            Some(native_credit_mapping_probe_wall_timeout()),
        )
        .summary;
        let frame = native_play_window_frame_for_emulator(&emulator);
        let frame_stats = NativeFrameStats::from_frame(&frame);
        let native_playable_candidate = emulator.native_playable_candidate();
        let frame_ready = native_play_frame_ready_with_context(native_playable_candidate, &frame);
        let activity = emulator.input_activity();
        let input_poll_ready = activity.p1_input_reads > 0
            && activity.system_input_reads > 0
            && activity.has_any_play_control_activity()
            && activity.has_credit_probe_activity();
        let credit_probe_ready = native_credit_probe_ready(activity);
        let credit_register_ready = native_credit_register_ready(activity);
        let credit_transition_verified = native_credit_transition_verified(
            credit_probe_ready,
            native_playable_candidate,
            frame_ready,
        );
        let _credit_system_port_ready =
            native_credit_system_port_ready(input_poll_ready, activity, credit_probe_ready);
        let credit_adapter_ready = native_credit_adapter_ready(activity);
        let credit_ready = native_credit_ready(
            credit_probe_ready,
            credit_register_ready,
            credit_transition_verified,
            credit_adapter_ready,
        );
        let input_ready = input_poll_ready && credit_ready;
        let gameplay_scene =
            native_play_gameplay_scene_with_context(native_playable_candidate, frame_stats);
        let reached_gameplay = input_ready && native_playable_candidate && gameplay_scene;
        let debug_words = native_credit_debug_words_json(&emulator);
        eprintln!(
            "native-credit-projection-probe projection={} bit=0x{bit:08x} frames={} stop_reason={} input_ready={} reached_gameplay={} playable={}",
            projection,
            run.total_frames,
            run.stop_reason,
            input_ready,
            reached_gameplay,
            native_playable_candidate
        );
        reports.push(format!(
            "{{\"projection\":\"{}\",\"bit\":{},\"bit_hex\":\"0x{:08x}\",\"mapping\":\"{}\",\"instructions_per_frame\":{},\"max_frames\":{},\"script_total_frames\":{},\"run\":{},\"executed_steps\":{},\"native_playable_candidate\":{},\"frame_ready\":{},\"gameplay_scene\":{},\"title_logo_overlay\":{},\"input_poll_ready\":{},\"input_ready\":{},\"credit_ready\":{},\"credit_probe_ready\":{},\"credit_register_ready\":{},\"credit_transition_verified\":{},\"credit_adapter_ready\":{},\"reached_gameplay\":{},\"native_credit_adapter_writes\":{},\"native_credit_adapter_edges\":{},\"p1_input_reads\":{},\"system_input_reads\":{},\"system_coin_active_reads\":{},\"system_start_active_reads\":{},\"last_system_input_hex\":\"0x{:08x}\",\"frame\":{},\"debug_words\":[{}],\"state\":{}}}",
            escape_json(&projection),
            bit,
            bit,
            escape_json(&mapping),
            instructions_per_frame,
            script_max_frames,
            script_total_frames,
            run.json(),
            emulator.executed_steps(),
            native_playable_candidate,
            frame_ready,
            gameplay_scene,
            frame_stats.has_title_logo_overlay(),
            input_poll_ready,
            input_ready,
            credit_ready,
            credit_probe_ready,
            credit_register_ready,
            credit_transition_verified,
            credit_adapter_ready,
            reached_gameplay,
            activity.native_credit_adapter_writes,
            activity.native_credit_adapter_edges,
            activity.p1_input_reads,
            activity.system_input_reads,
            activity.system_coin_active_reads,
            activity.system_start_active_reads,
            activity.last_system_input,
            frame_stats.json(),
            debug_words,
            emulator.input_compact_probe_json()
        ));
    }

    println!(
        "{{\"instructions_per_frame\":{},\"script_instructions_per_frame\":{},\"max_frames\":{},\"script_total_frames\":{},\"mapping\":\"{}\",\"bit\":{},\"bit_hex\":\"0x{:08x}\",\"boot_run\":{},\"boot_segments\":[{}],\"wall_timeout_secs_per_projection\":{},\"segments\":[{}],\"reports\":[{}]}}",
        instructions_per_frame,
        native_fast_forward_instructions_per_frame(instructions_per_frame),
        script_max_frames,
        script_total_frames,
        escape_json(&mapping),
        bit,
        bit,
        boot_run.json(),
        native_script_segments_json(&boot_segments),
        native_credit_mapping_probe_wall_timeout().as_secs(),
        native_script_segments_json(&segments),
        reports.join(",")
    );
    Ok(())
}

fn run_native_credit_ram_probe(
    rom: PathBuf,
    instructions_per_frame: u64,
    branch_frames: u64,
    settle_frames: u64,
    mapping: String,
    projection: String,
    bits: Vec<u32>,
) -> Result<(), String> {
    if bits.is_empty() {
        return Err("at least one credit bit is required".to_string());
    }

    let warmup_segments = native_warning_skip_to_title_script();
    let mut checkpoint = native_emulator_from_rom_source(rom)
        .map_err(|error| format!("failed to create credit ram probe emulator: {error}"))?;
    let warmup_run = run_native_title_ready_boot_wait_with_timeout(
        &mut checkpoint,
        native_fast_forward_instructions_per_frame(instructions_per_frame),
        &warmup_segments,
        Some(native_credit_mapping_probe_wall_timeout()),
        "skipped_credit_ram_boot_timeout",
    )
    .summary;
    let checkpoint_ram = checkpoint.ram_snapshot();
    let checkpoint_scratchpad = checkpoint.scratchpad_snapshot();
    let checkpoint_activity = checkpoint.input_activity();
    let checkpoint_frame = native_play_window_frame(&checkpoint.display_frame());
    let checkpoint_frame_checksum = native_frame_checksum(&checkpoint_frame);

    let noop_segments = [NativeScriptSegment {
        action: Action::Noop,
        frames: branch_frames.saturating_add(settle_frames),
    }];
    let mut noop_baseline = checkpoint.clone();
    let noop_run =
        run_native_script_quiet_vblank(&mut noop_baseline, instructions_per_frame, &noop_segments);
    let noop_ram = noop_baseline.ram_snapshot();
    let noop_scratchpad = noop_baseline.scratchpad_snapshot();
    let noop_frame = native_play_window_frame(&noop_baseline.display_frame());
    let noop_frame_checksum = native_frame_checksum(&noop_frame);

    let mut reports = Vec::new();
    for bit in bits {
        let mut emulator = checkpoint.clone();
        if !emulator.set_coin_input_mapping_name(&mapping) {
            return Err(format!("unknown coin input mapping: {mapping}"));
        }
        if !emulator.set_native_credit_projection_name(&projection) {
            return Err(format!("unknown native credit projection: {projection}"));
        }
        if !emulator.set_native_credit_adapter_input_bit(bit) {
            return Err(format!("invalid native credit adapter bit: 0x{bit:08x}"));
        }
        let segments = [
            NativeScriptSegment {
                action: Action::CoinStart,
                frames: branch_frames,
            },
            NativeScriptSegment {
                action: Action::Noop,
                frames: settle_frames,
            },
        ];
        let run = run_native_script_quiet_vblank(&mut emulator, instructions_per_frame, &segments);
        let ram = emulator.ram_snapshot();
        let scratchpad = emulator.scratchpad_snapshot();
        let frame = native_play_window_frame(&emulator.display_frame());
        let frame_checksum = native_frame_checksum(&frame);
        let input_delta = emulator
            .input_activity()
            .saturating_subtracted(checkpoint_activity);
        let input_delta_from_noop = emulator
            .input_activity()
            .saturating_subtracted(noop_baseline.input_activity());
        let debug_words = native_credit_debug_words_json(&emulator);
        let frame_stats = NativeFrameStats::from_frame(&frame);
        let native_playable_candidate = emulator.native_playable_candidate();
        let gameplay_scene =
            native_play_gameplay_scene_with_context(native_playable_candidate, frame_stats);
        let reached_gameplay = native_playable_candidate && gameplay_scene;
        eprintln!(
            "native-credit-ram-probe bit=0x{bit:08x} frames={} ram_changed={} scratch_changed={} frame_changed={} reached_gameplay={}",
            run.total_frames,
            native_bytes_checksum(&ram) != native_bytes_checksum(&noop_ram),
            native_bytes_checksum(&scratchpad) != native_bytes_checksum(&noop_scratchpad),
            frame_checksum != noop_frame_checksum,
            reached_gameplay
        );
        reports.push(format!(
            "{{\"bit\":{},\"bit_hex\":\"0x{:08x}\",\"mapping\":\"{}\",\"projection\":\"{}\",\"run\":{},\"input_delta_from_checkpoint\":{},\"input_delta_from_noop\":{},\"frame_checksum\":{},\"frame_changed_from_checkpoint\":{},\"frame_changed_from_noop\":{},\"native_playable_candidate\":{},\"reached_gameplay\":{},\"frame\":{},\"ram_diff_from_noop\":{},\"scratchpad_diff_from_noop\":{},\"debug_words\":[{}],\"state\":{}}}",
            bit,
            bit,
            escape_json(&mapping),
            escape_json(&projection),
            run.json(),
            input_delta.json(),
            input_delta_from_noop.json(),
            frame_checksum,
            frame_checksum != checkpoint_frame_checksum,
            frame_checksum != noop_frame_checksum,
            native_playable_candidate,
            reached_gameplay,
            frame_stats.json(),
            native_ram_diff_json(&noop_ram, &ram, 8, 16),
            native_scratchpad_diff_json(&noop_scratchpad, &scratchpad, 8, 16),
            debug_words,
            emulator.input_compact_probe_json()
        ));
    }

    println!(
        "{{\"instructions_per_frame\":{},\"branch_frames\":{},\"settle_frames\":{},\"mapping\":\"{}\",\"projection\":\"{}\",\"warmup_run\":{},\"warmup_segments\":[{}],\"checkpoint_frame\":{},\"noop_baseline\":{{\"run\":{},\"frame_checksum\":{},\"frame\":{},\"ram_diff_from_checkpoint\":{},\"scratchpad_diff_from_checkpoint\":{},\"state\":{}}},\"reports\":[{}]}}",
        instructions_per_frame,
        branch_frames,
        settle_frames,
        escape_json(&mapping),
        escape_json(&projection),
        warmup_run.json(),
        native_script_segments_json(&warmup_segments),
        NativeFrameStats::from_frame(&checkpoint_frame).json(),
        noop_run.json(),
        noop_frame_checksum,
        NativeFrameStats::from_frame(&noop_frame).json(),
        native_ram_diff_json(&checkpoint_ram, &noop_ram, 4, 8),
        native_scratchpad_diff_json(&checkpoint_scratchpad, &noop_scratchpad, 4, 8),
        noop_baseline.input_compact_probe_json(),
        reports.join(",")
    );
    Ok(())
}

fn native_credit_state_probe_default_actions() -> Vec<Action> {
    vec![
        Action::Coin,
        Action::Start,
        Action::CoinStart,
        Action::Punch,
    ]
}

fn parse_native_credit_state_probe_actions(values: Vec<String>) -> Result<Vec<Action>, String> {
    if values.is_empty() {
        return Ok(native_credit_state_probe_default_actions());
    }
    values
        .iter()
        .map(|value| parse_action_token(value))
        .collect()
}

fn native_credit_state_probe_addresses() -> &'static [u32] {
    &[
        0x1f80_0068,
        0x1f80_006c,
        0x1f80_0070,
        0x1f80_0074,
        0x1f80_0078,
        0x1f80_007c,
        0x1f80_0080,
        0x1f80_009c,
        0x1f80_00b8,
        0x1f80_00cc,
        0x803b_fa00,
        0x803b_fa04,
        0x803b_fa08,
        0x803b_fa0c,
        0x803b_fa10,
        0x803b_fa14,
        0x803b_fa18,
        0x803b_fa1c,
        0x8037_d204,
        0x8037_d20c,
        0x8037_d210,
        0x8037_d214,
        0x8037_d218,
        0x8037_d21c,
        0x8037_d220,
        0x8037_d224,
        0x8037_d228,
        0x8037_d22c,
        0x8037_d230,
        0x8037_d2a8,
        0x8037_d2ac,
        0x8037_d2b0,
        0x8037_d2b4,
        0x8037_d2b8,
        0x803a_0050,
        0x803a_0054,
        0x803a_0058,
        0x803a_0060,
        0x803a_2210,
    ]
}

fn native_debug_words_for_addresses_json(emulator: &NativeEmulator, addresses: &[u32]) -> String {
    let ram = emulator.ram_snapshot();
    let scratchpad = emulator.scratchpad_snapshot();
    addresses
        .iter()
        .map(|address| native_debug_word_json(emulator, *address, &ram, &scratchpad))
        .collect::<Vec<_>>()
        .join(",")
}

fn run_native_credit_state_probe(
    rom: PathBuf,
    instructions_per_frame: u64,
    branch_frames: u64,
    settle_frames: u64,
    actions: Vec<Action>,
) -> Result<(), String> {
    if actions.is_empty() {
        return Err("native-credit-state-probe requires at least one action".to_string());
    }

    let mut checkpoint = native_emulator_from_rom_source(rom)?;
    let auto_mapping = checkpoint
        .apply_auto_coin_input_mapping()
        .unwrap_or("unchanged");
    checkpoint.set_native_credit_projection_name("none");
    let warmup_segments = native_warning_skip_to_title_script();
    let warmup_run = run_native_title_ready_boot_wait_with_timeout(
        &mut checkpoint,
        native_fast_forward_instructions_per_frame(instructions_per_frame),
        &warmup_segments,
        Some(native_credit_mapping_probe_wall_timeout()),
        "skipped_credit_state_boot_timeout",
    )
    .summary;

    let checkpoint_ram = checkpoint.ram_snapshot();
    let checkpoint_scratchpad = checkpoint.scratchpad_snapshot();
    let checkpoint_activity = checkpoint.input_activity();
    let checkpoint_frame = native_play_window_frame_for_emulator(&checkpoint);
    let checkpoint_frame_checksum = native_frame_checksum(&checkpoint_frame);
    let checkpoint_words =
        native_debug_words_for_addresses_json(&checkpoint, native_credit_state_probe_addresses());

    let noop_segments = [NativeScriptSegment {
        action: Action::Noop,
        frames: branch_frames.saturating_add(settle_frames),
    }];
    let mut noop = checkpoint.clone();
    let noop_run =
        run_native_script_quiet_vblank(&mut noop, instructions_per_frame, &noop_segments);
    let noop_ram = noop.ram_snapshot();
    let noop_scratchpad = noop.scratchpad_snapshot();
    let noop_frame = native_play_window_frame_for_emulator(&noop);
    let noop_frame_checksum = native_frame_checksum(&noop_frame);
    let noop_words =
        native_debug_words_for_addresses_json(&noop, native_credit_state_probe_addresses());
    let noop_activity = noop
        .input_activity()
        .saturating_subtracted(checkpoint_activity);

    let mut reports = Vec::new();
    for action in actions {
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
            run_native_script_quiet_vblank(&mut branch, instructions_per_frame, &branch_segments);
        let branch_ram = branch.ram_snapshot();
        let branch_scratchpad = branch.scratchpad_snapshot();
        let branch_frame = native_play_window_frame_for_emulator(&branch);
        let branch_frame_checksum = native_frame_checksum(&branch_frame);
        let branch_words =
            native_debug_words_for_addresses_json(&branch, native_credit_state_probe_addresses());
        let input_delta = branch
            .input_activity()
            .saturating_subtracted(checkpoint_activity);
        let input_delta_from_noop = branch
            .input_activity()
            .saturating_subtracted(noop.input_activity());
        let frame_stats = NativeFrameStats::from_frame(&branch_frame);
        let native_playable_candidate = branch.native_playable_candidate();

        reports.push(format!(
            "{{\"action_index\":{},\"action\":\"{}\",\"branch_frames\":{},\"settle_frames\":{},\"run\":{},\"input_delta\":{},\"input_delta_from_noop\":{},\"frame_checksum\":{},\"frame_changed_from_checkpoint\":{},\"frame_changed_from_noop\":{},\"native_playable_candidate\":{},\"frame\":{},\"candidate_words\":[{}],\"ram_diff_from_noop\":{},\"scratchpad_diff_from_noop\":{},\"ram_diff_from_checkpoint\":{},\"scratchpad_diff_from_checkpoint\":{},\"state\":{}}}",
            action.index(),
            action.name(),
            branch_frames,
            settle_frames,
            branch_run.json(),
            input_delta.json(),
            input_delta_from_noop.json(),
            branch_frame_checksum,
            branch_frame_checksum != checkpoint_frame_checksum,
            branch_frame_checksum != noop_frame_checksum,
            native_playable_candidate,
            frame_stats.json(),
            branch_words,
            native_ram_diff_json(&noop_ram, &branch_ram, 12, 24),
            native_scratchpad_diff_json(&noop_scratchpad, &branch_scratchpad, 12, 24),
            native_ram_diff_json(&checkpoint_ram, &branch_ram, 12, 24),
            native_scratchpad_diff_json(&checkpoint_scratchpad, &branch_scratchpad, 12, 24),
            branch.input_compact_probe_json()
        ));
    }

    println!(
        "{{\"instructions_per_frame\":{},\"script_instructions_per_frame\":{},\"branch_frames\":{},\"settle_frames\":{},\"auto_mapping\":\"{}\",\"credit_projection\":\"none\",\"warmup_run\":{},\"warmup_segments\":[{}],\"candidate_addresses\":[{}],\"checkpoint\":{{\"frame_checksum\":{},\"frame\":{},\"candidate_words\":[{}],\"state\":{}}},\"noop\":{{\"run\":{},\"input_delta\":{},\"frame_checksum\":{},\"frame_changed_from_checkpoint\":{},\"frame\":{},\"candidate_words\":[{}],\"ram_diff_from_checkpoint\":{},\"scratchpad_diff_from_checkpoint\":{},\"state\":{}}},\"reports\":[{}]}}",
        instructions_per_frame,
        native_fast_forward_instructions_per_frame(instructions_per_frame),
        branch_frames,
        settle_frames,
        escape_json(auto_mapping),
        warmup_run.json(),
        native_script_segments_json(&warmup_segments),
        native_credit_state_probe_addresses()
            .iter()
            .map(|address| format!("\"0x{address:08x}\""))
            .collect::<Vec<_>>()
            .join(","),
        checkpoint_frame_checksum,
        NativeFrameStats::from_frame(&checkpoint_frame).json(),
        checkpoint_words,
        checkpoint.input_compact_probe_json(),
        noop_run.json(),
        noop_activity.json(),
        noop_frame_checksum,
        noop_frame_checksum != checkpoint_frame_checksum,
        NativeFrameStats::from_frame(&noop_frame).json(),
        noop_words,
        native_ram_diff_json(&checkpoint_ram, &noop_ram, 12, 24),
        native_scratchpad_diff_json(&checkpoint_scratchpad, &noop_scratchpad, 12, 24),
        noop.input_compact_probe_json(),
        reports.join(",")
    );
    Ok(())
}

fn run_native_credit_timeline_probe(
    rom: PathBuf,
    instructions_per_frame: u64,
    sample_stride: u64,
    mapping: String,
    projection: String,
    segments: Vec<NativeScriptSegment>,
) -> Result<(), String> {
    if segments.is_empty() {
        return Err("native-credit-timeline-probe requires at least one segment".to_string());
    }

    let mut emulator = NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
    let auto_mapping = emulator.apply_auto_coin_input_mapping();
    if mapping != "auto" && !emulator.set_coin_input_mapping_name(&mapping) {
        return Err(format!("unknown coin input mapping: {mapping}"));
    }
    if projection != "auto" && !emulator.set_native_credit_projection_name(&projection) {
        return Err(format!("unknown native credit projection: {projection}"));
    }

    let warmup_segments = native_warning_skip_to_title_script();
    let warmup_run = run_native_title_ready_boot_wait_with_timeout(
        &mut emulator,
        native_fast_forward_instructions_per_frame(instructions_per_frame),
        &warmup_segments,
        Some(native_credit_mapping_probe_wall_timeout()),
        "skipped_credit_timeline_boot_timeout",
    )
    .summary;

    let checkpoint_activity = emulator.input_activity();
    let checkpoint_frame = native_play_window_frame_for_emulator(&emulator);
    let checkpoint_frame_checksum = native_frame_checksum(&checkpoint_frame);
    let checkpoint_frame_stats = NativeFrameStats::from_frame(&checkpoint_frame);
    let checkpoint_debug_words = native_credit_debug_words_direct_json(&emulator);
    let checkpoint_state = emulator.input_compact_probe_json();
    let mut previous_activity = checkpoint_activity;
    let mut total_frames = 0u64;
    let mut frame_attempts = 0u64;
    let mut missed_vblank_frames = 0u64;
    let first_native_playable_frame = None;
    let last_native_playable_frame = None;
    let mut samples = Vec::new();
    let mut stop_reason = "script_completed";

    'script: for (segment_index, segment) in segments.iter().enumerate() {
        emulator.set_input(segment.action.buttons());
        for segment_frame in 0..segment.frames {
            let vblank_advanced =
                step_until_next_vblank_checked(&mut emulator, instructions_per_frame);
            frame_attempts = frame_attempts.saturating_add(1);
            if vblank_advanced {
                emulator.capture_vblank_presented_frame();
            } else {
                missed_vblank_frames = missed_vblank_frames.saturating_add(1);
            }

            total_frames = total_frames.saturating_add(1);
            let activity = emulator.input_activity();
            let delta = activity.saturating_subtracted(previous_activity);
            let total_delta = activity.saturating_subtracted(checkpoint_activity);
            let sample_due = total_frames == 1
                || total_frames % sample_stride == 0
                || segment.action != Action::Noop
                || native_credit_timeline_delta_important(delta);
            if sample_due {
                samples.push(native_credit_timeline_sample_json(
                    &emulator,
                    total_frames,
                    segment_index,
                    segment_frame.saturating_add(1),
                    *segment,
                    vblank_advanced,
                    delta,
                    total_delta,
                ));
            }

            previous_activity = activity;
            if emulator.is_terminal() {
                stop_reason = "terminal";
                break 'script;
            }
        }
    }

    let final_frame = native_play_window_frame_for_emulator(&emulator);
    let final_frame_checksum = native_frame_checksum(&final_frame);
    let final_frame_stats = NativeFrameStats::from_frame(&final_frame);
    let final_activity = emulator.input_activity();
    let final_native_playable_candidate = emulator.native_playable_candidate();
    let final_debug_words = native_credit_debug_words_direct_json(&emulator);
    let final_state = emulator.input_compact_probe_json();
    let observed_native_playable_candidate = final_native_playable_candidate;
    let first_native_playable_frame = if final_native_playable_candidate {
        Some(total_frames)
    } else {
        first_native_playable_frame
    };
    let last_native_playable_frame = if final_native_playable_candidate {
        Some(total_frames)
    } else {
        last_native_playable_frame
    };

    println!(
        "{{\"instructions_per_frame\":{},\"script_instructions_per_frame\":{},\"sample_stride\":{},\"mapping\":\"{}\",\"projection\":\"{}\",\"auto_mapping\":{},\"warmup_run\":{},\"warmup_segments\":[{}],\"segments\":[{}],\"run\":{{\"total_frames\":{},\"frame_attempts\":{},\"missed_vblank_frames\":{},\"observed_native_playable_candidate\":{},\"first_native_playable_frame\":{},\"last_native_playable_frame\":{},\"stop_reason\":\"{}\"}},\"checkpoint\":{{\"frame_checksum\":{},\"frame\":{},\"activity\":{},\"debug_words\":[{}],\"state\":{}}},\"samples\":[{}],\"final\":{{\"frame_checksum\":{},\"frame_changed_from_checkpoint\":{},\"frame\":{},\"native_playable_candidate\":{},\"activity_delta_from_checkpoint\":{},\"activity\":{},\"debug_words\":[{}],\"state\":{}}}}}",
        instructions_per_frame,
        native_fast_forward_instructions_per_frame(instructions_per_frame),
        sample_stride,
        escape_json(&mapping),
        escape_json(&projection),
        auto_mapping.map_or_else(
            || "null".to_string(),
            |mapping| format!("\"{}\"", escape_json(mapping))
        ),
        warmup_run.json(),
        native_script_segments_json(&warmup_segments),
        native_script_segments_json(&segments),
        total_frames,
        frame_attempts,
        missed_vblank_frames,
        observed_native_playable_candidate,
        optional_u64_json(first_native_playable_frame),
        optional_u64_json(last_native_playable_frame),
        stop_reason,
        checkpoint_frame_checksum,
        checkpoint_frame_stats.json(),
        checkpoint_activity.json(),
        checkpoint_debug_words,
        checkpoint_state,
        samples.join(","),
        final_frame_checksum,
        final_frame_checksum != checkpoint_frame_checksum,
        final_frame_stats.json(),
        final_native_playable_candidate,
        final_activity
            .saturating_subtracted(checkpoint_activity)
            .json(),
        final_activity.json(),
        final_debug_words,
        final_state
    );
    Ok(())
}

fn native_credit_timeline_delta_important(delta: NativeInputActivity) -> bool {
    delta.system_coin_active_reads > 0
        || delta.system_start_active_reads > 0
        || delta.coin_insert_edges > 0
        || delta.legacy_system_coin_latch_edges > 0
        || delta.legacy_system_start_latch_edges > 0
        || delta.native_credit_adapter_writes > 0
        || delta.native_credit_adapter_edges > 0
        || delta.coin_register_reads > 0
        || delta.coin_register_active_reads > 0
        || delta.coin_register_writes > 0
}

#[allow(clippy::too_many_arguments)]
fn native_credit_timeline_sample_json(
    emulator: &NativeEmulator,
    frame_index: u64,
    segment_index: usize,
    segment_frame: u64,
    segment: NativeScriptSegment,
    vblank_advanced: bool,
    activity_delta: NativeInputActivity,
    activity_delta_from_checkpoint: NativeInputActivity,
) -> String {
    format!(
        "{{\"frame_index\":{},\"segment_index\":{},\"segment_frame\":{},\"action_index\":{},\"action\":\"{}\",\"vblank_advanced\":{},\"activity_delta\":{},\"activity_delta_from_checkpoint\":{},\"debug_words\":[{}]}}",
        frame_index,
        segment_index,
        segment_frame,
        segment.action.index(),
        segment.action.name(),
        vblank_advanced,
        activity_delta.json(),
        activity_delta_from_checkpoint.json(),
        native_credit_debug_words_direct_json(emulator)
    )
}

fn native_credit_debug_words_direct_json(emulator: &NativeEmulator) -> String {
    native_credit_debug_word_addresses()
        .iter()
        .map(|address| {
            let word = emulator.debug_read_u32(*address);
            format!(
                "{{\"address\":{},\"address_hex\":\"0x{:08x}\",\"region\":\"{}\",\"word\":{},\"word_hex\":\"0x{:08x}\"}}",
                address,
                address,
                native_debug_word_region(*address),
                word,
                word
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn native_debug_word_region(address: u32) -> &'static str {
    let physical = address & 0x1fff_ffff;
    if (NATIVE_SCRATCHPAD_BASE_ADDRESS as u32..NATIVE_SCRATCHPAD_BASE_ADDRESS as u32 + 1024)
        .contains(&physical)
    {
        "scratchpad"
    } else if physical < 4 * 1024 * 1024 {
        "ram"
    } else {
        "unknown"
    }
}

fn native_credit_probe_ready(activity: NativeInputActivity) -> bool {
    activity.has_credit_probe_activity()
}

fn native_credit_register_ready(activity: NativeInputActivity) -> bool {
    activity.has_coin_register_active_activity()
}

fn native_credit_transition_verified(
    credit_probe_ready: bool,
    match_entry_native_playable: bool,
    match_entry_full_scene: bool,
) -> bool {
    credit_probe_ready && (match_entry_native_playable || match_entry_full_scene)
}

fn native_credit_system_port_ready(
    input_poll_ready: bool,
    activity: NativeInputActivity,
    credit_probe_ready: bool,
) -> bool {
    input_poll_ready && credit_probe_ready && activity.has_credit_probe_activity()
}

fn native_credit_adapter_ready(activity: NativeInputActivity) -> bool {
    activity.has_native_credit_adapter_activity()
}

fn native_credit_ready(
    credit_probe_ready: bool,
    credit_register_ready: bool,
    credit_transition_verified: bool,
    _credit_adapter_ready: bool,
) -> bool {
    credit_probe_ready && (credit_register_ready || credit_transition_verified)
}

fn native_credit_gap_reason(
    input_poll_ready: bool,
    activity: NativeInputActivity,
    credit_probe_ready: bool,
    credit_register_ready: bool,
    credit_transition_verified: bool,
    credit_system_port_ready: bool,
) -> &'static str {
    if !input_poll_ready {
        "controls_not_polled"
    } else if !activity.has_coin_edge_activity() {
        "coin_edge_not_generated"
    } else if !activity.has_coin_probe_activity() {
        "coin_not_observed_by_system_port"
    } else if !activity.has_start_edge_activity() {
        "start_edge_not_generated"
    } else if !activity.has_start_probe_activity() {
        "start_not_observed_by_input_port"
    } else if !credit_probe_ready {
        "credit_probe_not_ready"
    } else if native_credit_adapter_ready(activity)
        && !credit_register_ready
        && !credit_transition_verified
    {
        "credit_scratchpad_projected_but_not_accepted"
    } else if credit_system_port_ready && !credit_register_ready && !credit_transition_verified {
        "system_port_seen_but_credit_transition_not_verified"
    } else if !credit_register_ready && !credit_transition_verified {
        "coin_register_or_transition_not_verified"
    } else {
        "none"
    }
}

fn native_health_stage_wall_timeout(
    started_at: Instant,
    health_wall_timeout: Duration,
    stage_wall_timeout: Duration,
) -> Option<Duration> {
    native_remaining_wall_timeout(started_at, health_wall_timeout)
        .map(|remaining| remaining.min(stage_wall_timeout))
}

fn native_health_stage_wall_timeout_with_reserve(
    started_at: Instant,
    health_wall_timeout: Duration,
    stage_wall_timeout: Duration,
    reserved_for_followup: Duration,
) -> Option<Duration> {
    native_health_stage_wall_timeout_with_reserve_for_elapsed(
        started_at.elapsed(),
        health_wall_timeout,
        stage_wall_timeout,
        reserved_for_followup,
    )
}

fn native_health_stage_wall_timeout_with_reserve_for_elapsed(
    elapsed: Duration,
    health_wall_timeout: Duration,
    stage_wall_timeout: Duration,
    reserved_for_followup: Duration,
) -> Option<Duration> {
    native_remaining_wall_timeout_for_elapsed(elapsed, health_wall_timeout)
        .and_then(|remaining| remaining.checked_sub(reserved_for_followup))
        .map(|available| available.min(stage_wall_timeout))
        .filter(|available| !available.is_zero())
}

fn native_remaining_wall_timeout(started_at: Instant, wall_timeout: Duration) -> Option<Duration> {
    native_remaining_wall_timeout_for_elapsed(started_at.elapsed(), wall_timeout)
}

fn native_remaining_wall_timeout_for_elapsed(
    elapsed: Duration,
    wall_timeout: Duration,
) -> Option<Duration> {
    wall_timeout
        .checked_sub(elapsed)
        .filter(|remaining| !remaining.is_zero())
}

#[cfg(test)]
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

fn optional_u32_json(value: Option<u32>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn optional_u32_hex_json(value: Option<u32>) -> String {
    value.map_or_else(|| "null".to_string(), |value| format!("\"0x{value:08x}\""))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeScriptRunSummary {
    total_frames: u64,
    frame_attempts: u64,
    missed_vblank_frames: u64,
    observed_native_playable_candidate: bool,
    first_native_playable_frame: Option<u64>,
    last_native_playable_frame: Option<u64>,
    stop_reason: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeScriptProgress {
    summary: NativeScriptRunSummary,
    segment_index: usize,
    segment_frame: u64,
}

impl NativeScriptProgress {
    fn skipped(stop_reason: &'static str) -> Self {
        Self {
            summary: NativeScriptRunSummary::skipped(stop_reason),
            segment_index: 0,
            segment_frame: 0,
        }
    }
}

impl NativeScriptRunSummary {
    fn skipped(stop_reason: &'static str) -> Self {
        Self {
            total_frames: 0,
            frame_attempts: 0,
            missed_vblank_frames: 0,
            observed_native_playable_candidate: false,
            first_native_playable_frame: None,
            last_native_playable_frame: None,
            stop_reason,
        }
    }

    fn json(self) -> String {
        format!(
            "{{\"total_frames\":{},\"frame_attempts\":{},\"missed_vblank_frames\":{},\"observed_native_playable_candidate\":{},\"first_native_playable_frame\":{},\"last_native_playable_frame\":{},\"stop_reason\":\"{}\"}}",
            self.total_frames,
            self.frame_attempts,
            self.missed_vblank_frames,
            self.observed_native_playable_candidate,
            optional_u64_json(self.first_native_playable_frame),
            optional_u64_json(self.last_native_playable_frame),
            self.stop_reason
        )
    }
}

impl NativeScriptProgress {
    fn json(&self) -> String {
        format!(
            "{{\"summary\":{},\"segment_index\":{},\"segment_frame\":{}}}",
            self.summary.json(),
            self.segment_index,
            self.segment_frame
        )
    }
}

fn native_script_run_hit_runtime_limit(run: NativeScriptRunSummary) -> bool {
    matches!(
        run.stop_reason,
        "wall_timeout" | "max_frame_attempts" | "max_frames"
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeFrameStats {
    width: usize,
    height: usize,
    total_pixels: usize,
    nonzero_pixels: usize,
    black_pixels: usize,
    warm_pixels: usize,
    red_dominant_pixels: usize,
    magenta_dominant_pixels: usize,
    unique_colors: usize,
    dominant_color_bucket_pixels: usize,
    horizontal_color_changes: usize,
    periodic_horizontal_match_per_mille: usize,
    occupied_rows: usize,
    occupied_row_span: usize,
    right_title_red_pixels: usize,
    right_title_bright_pixels: usize,
    bottom_caption_pixels: usize,
    bottom_dark_pixels: usize,
}

impl NativeFrameStats {
    fn from_frame(frame: &NativeDisplayFrame) -> Self {
        let mut unique_colors = Vec::new();
        let mut color_buckets = [0usize; 4096];
        let mut horizontal_color_changes = 0usize;
        let mut nonzero_pixels = 0usize;
        let mut black_pixels = 0usize;
        let mut warm_pixels = 0usize;
        let mut red_dominant_pixels = 0usize;
        let mut magenta_dominant_pixels = 0usize;
        let row_content_cutoff = (frame.width / 20).max(1);
        let mut occupied_rows = 0usize;
        let mut first_occupied_row = None;
        let mut last_occupied_row = None;
        let title_region_x_start = frame.width.saturating_mul(52) / 100;
        let title_region_x_end = frame.width.saturating_mul(99) / 100;
        let title_region_y_start = frame.height.saturating_mul(48) / 100;
        let title_region_y_end = frame.height.saturating_mul(84) / 100;
        let mut right_title_red_pixels = 0usize;
        let mut right_title_bright_pixels = 0usize;
        let bottom_band_start = frame.height.saturating_mul(4) / 5;
        let mut bottom_caption_pixels = 0usize;
        let mut bottom_dark_pixels = 0usize;

        for y in 0..frame.height {
            let row = y.saturating_mul(frame.width);
            let mut row_nonzero_pixels = 0usize;
            for x in 0..frame.width {
                let color = frame.pixels.get(row + x).copied().unwrap_or_default() & 0x00ff_ffff;
                let red = (color >> 16) & 0xff;
                let green = (color >> 8) & 0xff;
                let blue = color & 0xff;
                let max_channel = red.max(green).max(blue);
                let min_channel = red.min(green).min(blue);
                let luma =
                    (red.saturating_mul(30) + green.saturating_mul(59) + blue.saturating_mul(11))
                        / 100;
                if color != 0 {
                    nonzero_pixels += 1;
                    row_nonzero_pixels += 1;
                } else {
                    black_pixels += 1;
                }
                if red > 128 && red > green.saturating_add(32) && red > blue.saturating_add(32) {
                    warm_pixels += 1;
                }
                if red > green.saturating_add(32) && red > blue.saturating_add(32) {
                    red_dominant_pixels += 1;
                }
                if red.max(blue) > 128
                    && red > green.saturating_add(48)
                    && blue > green.saturating_add(48)
                {
                    magenta_dominant_pixels += 1;
                }
                if x >= title_region_x_start
                    && x < title_region_x_end
                    && y >= title_region_y_start
                    && y < title_region_y_end
                {
                    if red >= 128 && red > green.saturating_add(48) && red > blue.saturating_add(48)
                    {
                        right_title_red_pixels += 1;
                    }
                    if luma >= 150 && max_channel.saturating_sub(min_channel) <= 104 {
                        right_title_bright_pixels += 1;
                    }
                }
                if y >= bottom_band_start {
                    if luma < 40 {
                        bottom_dark_pixels += 1;
                    } else if luma > 156 && max_channel.saturating_sub(min_channel) <= 96 {
                        bottom_caption_pixels += 1;
                    }
                }
                if unique_colors.len() < 257 && !unique_colors.contains(&color) {
                    unique_colors.push(color);
                }
                let bucket = (((red >> 4) as usize) << 8)
                    | (((green >> 4) as usize) << 4)
                    | ((blue >> 4) as usize);
                color_buckets[bucket] += 1;
                if x > 0 {
                    let previous =
                        frame.pixels.get(row + x - 1).copied().unwrap_or_default() & 0x00ff_ffff;
                    if previous != color {
                        horizontal_color_changes += 1;
                    }
                }
            }
            if row_nonzero_pixels >= row_content_cutoff {
                occupied_rows += 1;
                first_occupied_row.get_or_insert(y);
                last_occupied_row = Some(y);
            }
        }
        let occupied_row_span = first_occupied_row
            .zip(last_occupied_row)
            .map_or(0, |(first, last)| {
                last.saturating_sub(first).saturating_add(1)
            });

        Self {
            width: frame.width,
            height: frame.height,
            total_pixels: frame.pixels.len(),
            nonzero_pixels,
            black_pixels,
            warm_pixels,
            red_dominant_pixels,
            magenta_dominant_pixels,
            unique_colors: unique_colors.len(),
            dominant_color_bucket_pixels: color_buckets.into_iter().max().unwrap_or(0),
            horizontal_color_changes,
            periodic_horizontal_match_per_mille: native_frame_periodic_horizontal_match_per_mille(
                frame,
            ),
            occupied_rows,
            occupied_row_span,
            right_title_red_pixels,
            right_title_bright_pixels,
            bottom_caption_pixels,
            bottom_dark_pixels,
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

    fn has_render_ready_scene(self) -> bool {
        self.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
            && self.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT
            && self.has_scene_detail()
            && !self.has_title_screen_frame()
            && !self.has_low_palette_noise()
            && !self.has_low_palette_logo_transition()
            && !self.has_sparse_texture_fragment_handoff_artifact()
            && !self.has_blocking_texture_page_fragment_artifact()
            && !self.has_stage_texture_field_smear()
            && !self.has_corrupt_title_texture_fragment()
            && !self.has_repeating_tile_grid_artifact()
            && !self.has_saturated_transition_planes()
            && !self.has_warm_dominant_fill_frame()
            && !self.has_blocking_title_logo_overlay()
            && !self.has_periodic_horizontal_ghosting()
    }

    fn has_presentable_scene(self) -> bool {
        self.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
            && self.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT
            && self.has_visible_content()
            && self.unique_colors >= 8
            && !self.has_low_information_fill_frame()
            && !self.has_sparse_texture_fragment_handoff_artifact()
            && !self.has_low_palette_noise()
            && !self.has_low_palette_logo_transition()
            && !self.has_blocking_texture_page_fragment_artifact()
            && !self.has_stage_texture_field_smear()
            && !self.has_corrupt_title_texture_fragment()
            && !self.has_repeating_tile_grid_artifact()
            && !self.has_saturated_transition_planes()
            && !self.has_warm_dominant_fill_frame()
            && !self.has_bottom_caption_band()
            && !self.has_periodic_horizontal_ghosting()
    }

    fn has_low_palette_noise(self) -> bool {
        self.total_pixels > 0
            && self.unique_colors <= 96
            && self.horizontal_color_changes.saturating_mul(100)
                >= self.total_pixels.saturating_mul(45)
    }

    fn has_handoff_scene(self) -> bool {
        if !self.has_scene_detail()
            || self.has_title_screen_frame()
            || self.has_low_palette_noise()
            || self.has_low_palette_logo_transition()
            || self.has_sparse_texture_fragment_handoff_artifact()
            || self.has_blocking_texture_page_fragment_artifact()
            || self.has_stage_texture_field_smear()
            || self.has_corrupt_title_texture_fragment()
            || self.has_repeating_tile_grid_artifact()
            || self.has_saturated_transition_planes()
            || self.has_warm_dominant_fill_frame()
            || self.has_blocking_title_logo_overlay()
            || self.has_periodic_horizontal_ghosting()
            || self.warm_pixels.saturating_mul(100) > self.total_pixels.saturating_mul(72)
            || self.dominant_color_bucket_pixels.saturating_mul(100)
                > self.total_pixels.saturating_mul(80)
        {
            return false;
        }

        !self.has_intro_caption_band()
            && (!self.has_bottom_caption_band()
                || self.horizontal_color_changes.saturating_mul(100)
                    >= self.total_pixels.saturating_mul(35))
    }

    fn has_gameplay_scene(self) -> bool {
        if !self.has_scene_detail()
            || self.has_title_screen_frame()
            || self.has_low_palette_noise()
            || self.has_low_palette_logo_transition()
            || self.has_sparse_texture_fragment_handoff_artifact()
            || self.has_blocking_texture_page_fragment_artifact()
            || self.has_stage_texture_field_smear()
            || self.has_corrupt_title_texture_fragment()
            || self.has_repeating_tile_grid_artifact()
            || self.has_saturated_transition_planes()
            || self.has_warm_dominant_fill_frame()
            || self.has_blocking_title_logo_overlay()
            || self.has_periodic_horizontal_ghosting()
            || self.warm_pixels.saturating_mul(100) > self.total_pixels.saturating_mul(72)
            || self.has_bottom_caption_band()
            || (self.black_pixels.saturating_mul(100) > self.total_pixels.saturating_mul(40)
                && self.dominant_color_bucket_pixels.saturating_mul(100)
                    > self.total_pixels.saturating_mul(45))
            || self.dominant_color_bucket_pixels.saturating_mul(100)
                > self.total_pixels.saturating_mul(80)
        {
            return false;
        }

        self.nonzero_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(35)
            && self.black_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(45)
            && self.dominant_color_bucket_pixels.saturating_mul(100)
                <= self.total_pixels.saturating_mul(55)
    }

    fn has_context_stage_letterbox(self) -> bool {
        if self.width == 0 || self.height == 0 || self.total_pixels == 0 {
            return false;
        }

        let bottom_band_pixels = self.width.saturating_mul((self.height / 5).max(1));
        bottom_band_pixels > 0
            && self.nonzero_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(35)
            && self.black_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(55)
            && self.bottom_dark_pixels.saturating_mul(100) >= bottom_band_pixels.saturating_mul(70)
            && self.occupied_rows.saturating_mul(100) >= self.height.saturating_mul(55)
            && self.occupied_row_span.saturating_mul(100) >= self.height.saturating_mul(60)
            && self.dominant_color_bucket_pixels.saturating_mul(100)
                <= self.total_pixels.saturating_mul(55)
    }

    fn has_native_context_scene(self) -> bool {
        (!self.has_intro_caption_band() && !self.has_bottom_caption_band()
            || self.has_native_context_caption_ui())
            && !self.has_large_bottom_caption_video_band()
            && !self.has_title_screen_frame()
            && !self.has_low_palette_noise()
            && !self.has_low_palette_logo_transition()
            && !self.has_sparse_texture_fragment_handoff_artifact()
            && !self.has_blocking_texture_page_fragment_artifact()
            && !self.has_stage_texture_field_smear()
            && !self.has_corrupt_title_texture_fragment()
            && !self.has_repeating_tile_grid_artifact()
            && !self.has_saturated_transition_planes()
            && !self.has_warm_dominant_fill_frame()
            && !self.has_blocking_title_logo_overlay()
            && !self.has_periodic_horizontal_ghosting()
            && self.warm_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(72)
            && self.nonzero_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(35)
            && (self.black_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(45)
                || self.has_context_stage_letterbox())
            && self.dominant_color_bucket_pixels.saturating_mul(100)
                <= self.total_pixels.saturating_mul(80)
    }

    fn has_letterboxed_live_stage_scene(self) -> bool {
        self.has_scene_detail()
            && !self.has_large_bottom_caption_video_band()
            && !self.has_title_screen_frame()
            && !self.has_low_palette_noise()
            && !self.has_low_palette_logo_transition()
            && !self.has_sparse_texture_fragment_handoff_artifact()
            && !self.has_stage_texture_field_smear()
            && (!self.has_texture_page_fragment_artifact()
                || self.has_letterboxed_stage_playfield_over_dark_band())
            && !self.has_corrupt_title_texture_fragment()
            && !self.has_repeating_tile_grid_artifact()
            && !self.has_saturated_transition_planes()
            && !self.has_warm_dominant_fill_frame()
            && !self.has_periodic_horizontal_ghosting()
            && self.nonzero_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(18)
            && self.black_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(82)
            && self.occupied_rows.saturating_mul(100) >= self.height.saturating_mul(25)
            && self.occupied_row_span.saturating_mul(100) >= self.height.saturating_mul(35)
            && self.dominant_color_bucket_pixels.saturating_mul(100)
                <= self.total_pixels.saturating_mul(84)
    }

    fn has_letterboxed_stage_playfield_over_dark_band(self) -> bool {
        if self.total_pixels == 0 || !self.has_texture_page_fragment_artifact() {
            return false;
        }

        let bottom_band_pixels = self.width.saturating_mul((self.height / 5).max(1));
        let title_mark_pixels = self
            .right_title_red_pixels
            .saturating_add(self.right_title_bright_pixels);
        bottom_band_pixels > 0
            && !self.has_bottom_caption_band()
            && self.bottom_dark_pixels.saturating_mul(100) >= bottom_band_pixels.saturating_mul(95)
            && self.black_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(55)
            && self.nonzero_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(45)
            && self.nonzero_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(65)
            && self.horizontal_color_changes.saturating_mul(100)
                >= self.total_pixels.saturating_mul(8)
            && self.horizontal_color_changes.saturating_mul(100)
                <= self.total_pixels.saturating_mul(22)
            && !self.has_stage_texture_field_smear()
            && self.magenta_dominant_pixels.saturating_mul(100)
                <= self.total_pixels.saturating_mul(3)
            && title_mark_pixels.saturating_mul(100) >= self.total_pixels
            && self.right_title_red_pixels.saturating_mul(1_000)
                >= self.total_pixels.saturating_mul(6)
            && self.right_title_bright_pixels.saturating_mul(1_000)
                >= self.total_pixels.saturating_mul(2)
    }

    fn has_native_context_caption_ui(self) -> bool {
        self.total_pixels > 0
            && (self.has_intro_caption_band() || self.has_bottom_caption_band())
            && self.bottom_dark_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(12)
            && self.bottom_caption_pixels.saturating_mul(100) <= self.total_pixels
    }

    fn has_title_logo_overlay(self) -> bool {
        if self.total_pixels == 0
            || self.width < NATIVE_PLAY_MIN_WINDOW_WIDTH
            || self.height < NATIVE_PLAY_MIN_WINDOW_HEIGHT
        {
            return false;
        }

        let title_mark_pixels = self
            .right_title_red_pixels
            .saturating_add(self.right_title_bright_pixels);
        self.right_title_red_pixels.saturating_mul(1_000) >= self.total_pixels.saturating_mul(8)
            && self.right_title_bright_pixels.saturating_mul(1_000)
                >= self.total_pixels.saturating_mul(2)
            && title_mark_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(1)
            && self.black_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(10)
            && self.horizontal_color_changes.saturating_mul(100)
                >= self.total_pixels.saturating_mul(20)
    }

    fn has_title_screen_frame(self) -> bool {
        if self.total_pixels == 0
            || self.width < NATIVE_PLAY_MIN_WINDOW_WIDTH
            || self.height < NATIVE_PLAY_MIN_WINDOW_HEIGHT
        {
            return false;
        }

        let title_mark_pixels = self
            .right_title_red_pixels
            .saturating_add(self.right_title_bright_pixels);
        let caption_band_allowed =
            !self.has_bottom_caption_band() || self.has_minor_bottom_caption_title_mark();
        let intro_caption_allowed =
            !self.has_intro_caption_band() || self.has_minor_bottom_caption_title_mark();
        title_mark_pixels.saturating_mul(200) >= self.total_pixels
            && self.black_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(55)
            && self.nonzero_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(25)
            && self.unique_colors >= 8
            && caption_band_allowed
            && intro_caption_allowed
    }

    fn has_texture_page_fragment_artifact(self) -> bool {
        if self.total_pixels == 0
            || self.width < NATIVE_PLAY_MIN_WINDOW_WIDTH
            || self.height < NATIVE_PLAY_MIN_WINDOW_HEIGHT
            || !self.has_scene_detail()
        {
            return false;
        }

        let bottom_band_pixels = self.width.saturating_mul((self.height / 5).max(1));
        bottom_band_pixels > 0
            && self.unique_colors <= 160
            && self.bottom_dark_pixels.saturating_mul(100) >= bottom_band_pixels.saturating_mul(92)
            && self.black_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(35)
            && self.dominant_color_bucket_pixels.saturating_mul(100)
                >= self.total_pixels.saturating_mul(32)
            && self.occupied_row_span.saturating_mul(100) <= self.height.saturating_mul(85)
            && self.nonzero_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(35)
            && self.nonzero_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(70)
            && self.horizontal_color_changes.saturating_mul(100)
                <= self.total_pixels.saturating_mul(30)
    }

    fn has_corrupt_title_texture_fragment(self) -> bool {
        if self.total_pixels == 0
            || self.width < NATIVE_PLAY_MIN_WINDOW_WIDTH
            || self.height < NATIVE_PLAY_MIN_WINDOW_HEIGHT
            || !self.has_title_screen_frame()
            || !self.has_scene_detail()
        {
            return false;
        }

        self.occupied_row_span.saturating_mul(100) >= self.height.saturating_mul(90)
            && self.black_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(65)
            && self.nonzero_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(18)
            && self.nonzero_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(30)
            && self.dominant_color_bucket_pixels.saturating_mul(100)
                >= self.total_pixels.saturating_mul(60)
    }

    fn has_context_live_stage_hud(self) -> bool {
        if !self.has_context_stage_letterbox() {
            return false;
        }
        if self.has_large_bottom_caption_video_band() {
            return false;
        }
        let bottom_band_pixels = self.width.saturating_mul((self.height / 5).max(1));
        let minor_bottom_caption =
            self.bottom_caption_pixels.saturating_mul(100) <= bottom_band_pixels.saturating_mul(10);
        if self.has_intro_caption_band() && !minor_bottom_caption {
            return false;
        }
        if self.has_bottom_caption_band() && !minor_bottom_caption {
            return false;
        }

        let title_mark_pixels = self
            .right_title_red_pixels
            .saturating_add(self.right_title_bright_pixels);
        self.right_title_red_pixels.saturating_mul(1_000) >= self.total_pixels.saturating_mul(8)
            && self.right_title_bright_pixels.saturating_mul(1_000)
                >= self.total_pixels.saturating_mul(2)
            && title_mark_pixels.saturating_mul(100) >= self.total_pixels
    }

    fn has_blocking_texture_page_fragment_artifact(self) -> bool {
        self.has_stage_texture_field_smear() || self.has_texture_page_fragment_artifact()
    }

    fn has_stage_texture_field_smear(self) -> bool {
        if self.total_pixels == 0
            || self.width < NATIVE_PLAY_MIN_WINDOW_WIDTH
            || self.height < NATIVE_PLAY_MIN_WINDOW_HEIGHT
            || !self.has_scene_detail()
        {
            return false;
        }

        let bottom_band_pixels = self.width.saturating_mul((self.height / 5).max(1));
        bottom_band_pixels > 0
            && self.nonzero_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(45)
            && self.nonzero_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(70)
            && self.black_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(30)
            && self.black_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(55)
            && self.bottom_dark_pixels.saturating_mul(100) >= bottom_band_pixels.saturating_mul(85)
            && self.dominant_color_bucket_pixels.saturating_mul(100)
                >= self.total_pixels.saturating_mul(32)
            && self.horizontal_color_changes.saturating_mul(100)
                <= self.total_pixels.saturating_mul(28)
            && self.occupied_row_span.saturating_mul(100) <= self.height.saturating_mul(88)
            && self.unique_colors <= 257
            && !self.has_strong_context_stage_hud()
    }

    fn has_sparse_texture_fragment_handoff_artifact(self) -> bool {
        if self.total_pixels == 0
            || self.width < NATIVE_PLAY_MIN_WINDOW_WIDTH
            || self.height < NATIVE_PLAY_MIN_WINDOW_HEIGHT
            || !self.has_scene_detail()
            || self.has_title_screen_frame()
        {
            return false;
        }

        let bottom_band_pixels = self.width.saturating_mul((self.height / 5).max(1));
        bottom_band_pixels > 0
            && self.nonzero_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(35)
            && self.black_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(60)
            && self.occupied_row_span.saturating_mul(100) <= self.height.saturating_mul(55)
            && self.bottom_dark_pixels.saturating_mul(100) >= bottom_band_pixels.saturating_mul(70)
            && self.dominant_color_bucket_pixels.saturating_mul(100)
                >= self.total_pixels.saturating_mul(55)
            && self.unique_colors <= 180
    }

    fn has_strong_context_stage_hud(self) -> bool {
        if self.total_pixels == 0 {
            return false;
        }

        let title_mark_pixels = self
            .right_title_red_pixels
            .saturating_add(self.right_title_bright_pixels);
        title_mark_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(5)
            && self.right_title_red_pixels.saturating_mul(1_000)
                >= self.total_pixels.saturating_mul(20)
            && self.right_title_bright_pixels.saturating_mul(1_000)
                >= self.total_pixels.saturating_mul(20)
    }

    fn has_low_information_fill_frame(self) -> bool {
        if self.total_pixels == 0
            || self.width < NATIVE_PLAY_MIN_WINDOW_WIDTH
            || self.height < NATIVE_PLAY_MIN_WINDOW_HEIGHT
            || !self.has_visible_content()
        {
            return false;
        }

        self.unique_colors <= 4
            && self.occupied_row_span.saturating_mul(100) >= self.height.saturating_mul(90)
            && self.horizontal_color_changes <= self.width.saturating_mul(4)
            && !self.has_bottom_caption_band()
            && !self.has_intro_caption_band()
    }

    fn has_blocking_display_artifact(self) -> bool {
        self.has_blocking_texture_page_fragment_artifact()
            || self.has_sparse_texture_fragment_handoff_artifact()
            || self.has_stage_texture_field_smear()
            || self.has_corrupt_title_texture_fragment()
            || self.has_repeating_tile_grid_artifact()
            || self.has_low_information_fill_frame()
            || self.has_warm_dominant_fill_frame()
    }

    fn has_repeating_tile_grid_artifact(self) -> bool {
        if self.total_pixels == 0
            || self.width < NATIVE_PLAY_MIN_WINDOW_WIDTH
            || self.height < NATIVE_PLAY_MIN_WINDOW_HEIGHT
            || !self.has_scene_detail()
        {
            return false;
        }

        // Late broken gameplay frames can expose a repeated texture-page tile
        // grid while still looking colorful enough to pass broad scene checks.
        self.unique_colors <= 160
            && self.nonzero_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(70)
            && self.black_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(32)
            && self.horizontal_color_changes.saturating_mul(100)
                >= self.total_pixels.saturating_mul(30)
            && self.occupied_row_span.saturating_mul(100) >= self.height.saturating_mul(95)
            && self.right_title_bright_pixels.saturating_mul(1_000)
                >= self.total_pixels.saturating_mul(30)
    }

    fn has_blocking_title_logo_overlay(self) -> bool {
        self.has_title_logo_overlay() && !self.has_context_live_stage_hud()
    }

    fn has_low_palette_logo_transition(self) -> bool {
        self.total_pixels > 0
            && self.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
            && self.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT
            && self.unique_colors <= 160
            && self.red_dominant_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(32)
            && self.black_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(5)
            && self.black_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(25)
            && self.dominant_color_bucket_pixels.saturating_mul(100)
                >= self.total_pixels.saturating_mul(12)
    }

    fn has_saturated_transition_planes(self) -> bool {
        self.total_pixels > 0
            && self.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
            && self.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT
            && self.unique_colors <= 230
            && self.red_dominant_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(40)
            && (self.warm_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(15)
                || self.magenta_dominant_pixels.saturating_mul(100)
                    >= self.total_pixels.saturating_mul(18))
            && self.black_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(5)
            && self.black_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(25)
            && self.dominant_color_bucket_pixels.saturating_mul(100)
                >= self.total_pixels.saturating_mul(12)
    }

    fn has_warm_dominant_fill_frame(self) -> bool {
        self.total_pixels > 0
            && self.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
            && self.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT
            && self.has_visible_content()
            && self.warm_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(80)
            && self.red_dominant_pixels.saturating_mul(100) >= self.total_pixels.saturating_mul(55)
            && self.black_pixels.saturating_mul(100) <= self.total_pixels.saturating_mul(12)
    }

    fn has_periodic_horizontal_ghosting(self) -> bool {
        if self.periodic_horizontal_match_per_mille >= 500 {
            return self.horizontal_color_changes.saturating_mul(100)
                >= self.total_pixels.saturating_mul(10);
        }

        self.periodic_horizontal_match_per_mille >= 350
            && self.horizontal_color_changes.saturating_mul(100)
                >= self.total_pixels.saturating_mul(20)
    }

    fn has_bottom_caption_band(self) -> bool {
        if self.width == 0 || self.height == 0 || self.total_pixels == 0 {
            return false;
        }
        let bottom_band_pixels = self.width.saturating_mul((self.height / 5).max(1));
        self.bottom_caption_pixels.saturating_mul(1_000) >= self.total_pixels.saturating_mul(3)
            && self.bottom_dark_pixels.saturating_mul(100) >= bottom_band_pixels.saturating_mul(5)
    }

    fn has_minor_bottom_caption_band(self) -> bool {
        if self.width == 0 || self.height == 0 || self.total_pixels == 0 {
            return false;
        }
        let bottom_band_pixels = self.width.saturating_mul((self.height / 5).max(1));
        bottom_band_pixels > 0
            && self.bottom_caption_pixels.saturating_mul(100)
                <= bottom_band_pixels.saturating_mul(6)
    }

    fn has_minor_bottom_caption_title_mark(self) -> bool {
        self.has_minor_bottom_caption_band()
            && (self.right_title_red_pixels > 0
                || self.warm_pixels.saturating_mul(1_000) >= self.total_pixels)
    }

    fn has_large_bottom_caption_video_band(self) -> bool {
        if !self.has_bottom_caption_band() || self.width == 0 || self.height == 0 {
            return false;
        }
        let bottom_band_pixels = self.width.saturating_mul((self.height / 5).max(1));
        bottom_band_pixels > 0
            && self.bottom_dark_pixels.saturating_mul(100) >= bottom_band_pixels.saturating_mul(45)
            && self.bottom_caption_pixels.saturating_mul(100)
                >= bottom_band_pixels.saturating_mul(5)
    }

    fn has_intro_caption_band(self) -> bool {
        if !self.has_bottom_caption_band() {
            return false;
        }
        let bottom_band_pixels = self.width.saturating_mul((self.height / 5).max(1));
        let dark_subtitle_band = self.bottom_dark_pixels.saturating_mul(100)
            >= bottom_band_pixels.saturating_mul(30)
            && self.bottom_caption_pixels.saturating_mul(100)
                <= bottom_band_pixels.saturating_mul(35);
        let bright_caption_panel = self.bottom_caption_pixels.saturating_mul(100)
            >= bottom_band_pixels.saturating_mul(70)
            && self.bottom_dark_pixels.saturating_mul(100) >= bottom_band_pixels.saturating_mul(4);

        dark_subtitle_band || bright_caption_panel
    }

    fn json(self) -> String {
        format!(
            "{{\"width\":{},\"height\":{},\"total_pixels\":{},\"nonzero_pixels\":{},\"black_pixels\":{},\"warm_pixels\":{},\"red_dominant_pixels\":{},\"magenta_dominant_pixels\":{},\"unique_colors\":{},\"dominant_color_bucket_pixels\":{},\"horizontal_color_changes\":{},\"periodic_horizontal_match_per_mille\":{},\"periodic_horizontal_ghosting\":{},\"title_screen_frame\":{},\"low_palette_noise\":{},\"low_palette_logo_transition\":{},\"texture_page_fragment_artifact\":{},\"letterboxed_stage_playfield_over_dark_band\":{},\"texture_page_fragment_blocking\":{},\"stage_texture_field_smear\":{},\"sparse_texture_fragment_handoff_artifact\":{},\"corrupt_title_texture_fragment\":{},\"repeating_tile_grid_artifact\":{},\"context_live_stage_hud\":{},\"saturated_transition_planes\":{},\"warm_dominant_fill_frame\":{},\"title_logo_overlay\":{},\"title_logo_overlay_blocking\":{},\"occupied_rows\":{},\"occupied_row_span\":{},\"right_title_red_pixels\":{},\"right_title_bright_pixels\":{},\"bottom_caption_pixels\":{},\"bottom_dark_pixels\":{},\"bottom_caption_band\":{},\"large_bottom_caption_video_band\":{},\"intro_caption_band\":{},\"visible_content\":{},\"scene_detail\":{},\"render_ready_scene\":{},\"handoff_scene\":{},\"gameplay_scene\":{}}}",
            self.width,
            self.height,
            self.total_pixels,
            self.nonzero_pixels,
            self.black_pixels,
            self.warm_pixels,
            self.red_dominant_pixels,
            self.magenta_dominant_pixels,
            self.unique_colors,
            self.dominant_color_bucket_pixels,
            self.horizontal_color_changes,
            self.periodic_horizontal_match_per_mille,
            self.has_periodic_horizontal_ghosting(),
            self.has_title_screen_frame(),
            self.has_low_palette_noise(),
            self.has_low_palette_logo_transition(),
            self.has_texture_page_fragment_artifact(),
            self.has_letterboxed_stage_playfield_over_dark_band(),
            self.has_blocking_texture_page_fragment_artifact(),
            self.has_stage_texture_field_smear(),
            self.has_sparse_texture_fragment_handoff_artifact(),
            self.has_corrupt_title_texture_fragment(),
            self.has_repeating_tile_grid_artifact(),
            self.has_context_live_stage_hud(),
            self.has_saturated_transition_planes(),
            self.has_warm_dominant_fill_frame(),
            self.has_title_logo_overlay(),
            self.has_blocking_title_logo_overlay(),
            self.occupied_rows,
            self.occupied_row_span,
            self.right_title_red_pixels,
            self.right_title_bright_pixels,
            self.bottom_caption_pixels,
            self.bottom_dark_pixels,
            self.has_bottom_caption_band(),
            self.has_large_bottom_caption_video_band(),
            self.has_intro_caption_band(),
            self.has_visible_content(),
            self.has_scene_detail(),
            self.has_render_ready_scene(),
            self.has_handoff_scene(),
            self.has_gameplay_scene()
        )
    }
}

fn native_frame_checksum(frame: &NativeDisplayFrame) -> u32 {
    let mut checksum = 0x811c_9dc5_u32;
    checksum ^= frame.width as u32;
    checksum = checksum.wrapping_mul(16_777_619);
    checksum ^= frame.height as u32;
    checksum = checksum.wrapping_mul(16_777_619);
    for &pixel in &frame.pixels {
        checksum ^= pixel & 0x00ff_ffff;
        checksum = checksum.wrapping_mul(16_777_619);
    }
    checksum
}

fn native_ram_diff_json(
    before: &[u8],
    after: &[u8],
    max_range_samples: usize,
    max_word_samples: usize,
) -> String {
    native_memory_diff_json(before, after, 0, max_range_samples, max_word_samples)
}

fn native_scratchpad_diff_json(
    before: &[u8],
    after: &[u8],
    max_range_samples: usize,
    max_word_samples: usize,
) -> String {
    native_memory_diff_json(
        before,
        after,
        NATIVE_SCRATCHPAD_BASE_ADDRESS,
        max_range_samples,
        max_word_samples,
    )
}

fn native_memory_diff_json(
    before: &[u8],
    after: &[u8],
    base_address: usize,
    max_range_samples: usize,
    max_word_samples: usize,
) -> String {
    let compared_len = before.len().min(after.len());
    let before_checksum = native_bytes_checksum(before);
    let after_checksum = native_bytes_checksum(after);
    let mut changed_bytes = 0usize;
    let mut changed_ranges = 0usize;
    let mut first_changed = None;
    let mut last_changed = None;
    let mut range_samples = Vec::new();
    let mut offset = 0usize;

    while offset < compared_len {
        if before[offset] == after[offset] {
            offset += 1;
            continue;
        }

        let start = offset;
        while offset < compared_len && before[offset] != after[offset] {
            offset += 1;
        }
        let end = offset;
        changed_ranges = changed_ranges.saturating_add(1);
        changed_bytes = changed_bytes.saturating_add(end.saturating_sub(start));
        first_changed.get_or_insert(start);
        last_changed = Some(end.saturating_sub(1));

        if range_samples.len() < max_range_samples {
            let sample_end = end.min(start.saturating_add(16));
            let start_address = base_address.saturating_add(start);
            let end_address = base_address.saturating_add(end);
            range_samples.push(format!(
                "{{\"start\":{},\"start_hex\":\"0x{:06x}\",\"address\":{},\"address_hex\":\"0x{:08x}\",\"end_exclusive\":{},\"end_exclusive_hex\":\"0x{:06x}\",\"end_address_exclusive\":{},\"end_address_exclusive_hex\":\"0x{:08x}\",\"len\":{},\"sample_len\":{},\"before_hex\":\"{}\",\"after_hex\":\"{}\"}}",
                start,
                start,
                start_address,
                start_address,
                end,
                end,
                end_address,
                end_address,
                end.saturating_sub(start),
                sample_end.saturating_sub(start),
                native_bytes_hex(&before[start..sample_end]),
                native_bytes_hex(&after[start..sample_end])
            ));
        }
    }

    let mut changed_words = 0usize;
    let mut word_samples = Vec::new();
    let mut word_offset = 0usize;
    while word_offset < compared_len {
        let word_end = (word_offset + 4).min(compared_len);
        if before[word_offset..word_end] != after[word_offset..word_end] {
            changed_words = changed_words.saturating_add(1);
            if word_samples.len() < max_word_samples {
                let word_address = base_address.saturating_add(word_offset);
                word_samples.push(format!(
                    "{{\"offset\":{},\"offset_hex\":\"0x{:06x}\",\"address\":{},\"address_hex\":\"0x{:08x}\",\"before\":{},\"before_hex\":\"0x{:08x}\",\"after\":{},\"after_hex\":\"0x{:08x}\"}}",
                    word_offset,
                    word_offset,
                    word_address,
                    word_address,
                    native_le_u32_prefix(&before[word_offset..word_end]),
                    native_le_u32_prefix(&before[word_offset..word_end]),
                    native_le_u32_prefix(&after[word_offset..word_end]),
                    native_le_u32_prefix(&after[word_offset..word_end])
                ));
            }
        }
        word_offset = word_offset.saturating_add(4);
    }

    format!(
        "{{\"before_len\":{},\"after_len\":{},\"compared_len\":{},\"length_mismatch\":{},\"base_address\":{},\"base_address_hex\":\"0x{:08x}\",\"before_checksum\":{},\"before_checksum_hex\":\"0x{:08x}\",\"after_checksum\":{},\"after_checksum_hex\":\"0x{:08x}\",\"changed_bytes\":{},\"changed_words\":{},\"changed_ranges\":{},\"first_changed\":{},\"first_changed_hex\":{},\"first_changed_address\":{},\"first_changed_address_hex\":{},\"last_changed\":{},\"last_changed_hex\":{},\"last_changed_address\":{},\"last_changed_address_hex\":{},\"range_samples\":[{}],\"word_samples\":[{}]}}",
        before.len(),
        after.len(),
        compared_len,
        before.len() != after.len(),
        base_address,
        base_address,
        before_checksum,
        before_checksum,
        after_checksum,
        after_checksum,
        changed_bytes,
        changed_words,
        changed_ranges,
        optional_usize_json(first_changed),
        optional_usize_hex_json(first_changed),
        optional_usize_address_json(base_address, first_changed),
        optional_usize_address_hex_json(base_address, first_changed),
        optional_usize_json(last_changed),
        optional_usize_hex_json(last_changed),
        optional_usize_address_json(base_address, last_changed),
        optional_usize_address_hex_json(base_address, last_changed),
        range_samples.join(","),
        word_samples.join(",")
    )
}

fn native_bytes_checksum(bytes: &[u8]) -> u32 {
    let mut checksum = 0x811c_9dc5_u32;
    for &byte in bytes {
        checksum ^= u32::from(byte);
        checksum = checksum.wrapping_mul(16_777_619);
    }
    checksum
}

fn native_bytes_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn native_le_u32_prefix(bytes: &[u8]) -> u32 {
    let mut word = [0u8; 4];
    let len = bytes.len().min(word.len());
    word[..len].copy_from_slice(&bytes[..len]);
    u32::from_le_bytes(word)
}

fn optional_usize_json(value: Option<usize>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn optional_usize_hex_json(value: Option<usize>) -> String {
    value.map_or_else(|| "null".to_string(), |value| format!("\"0x{value:06x}\""))
}

fn optional_usize_address_json(base_address: usize, value: Option<usize>) -> String {
    value.map_or_else(
        || "null".to_string(),
        |value| base_address.saturating_add(value).to_string(),
    )
}

fn optional_usize_address_hex_json(base_address: usize, value: Option<usize>) -> String {
    value.map_or_else(
        || "null".to_string(),
        |value| format!("\"0x{:08x}\"", base_address.saturating_add(value)),
    )
}

fn native_frame_periodic_horizontal_match_per_mille(frame: &NativeDisplayFrame) -> usize {
    if frame.width < 128
        || frame.height == 0
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
    {
        return 0;
    }

    let periods = [64usize, 96, 128, 160, 192];
    periods
        .into_iter()
        .filter(|period| *period < frame.width)
        .map(|period| {
            let mut matches = 0usize;
            let mut samples = 0usize;
            for y in (0..frame.height).step_by(2) {
                let row = y.saturating_mul(frame.width);
                for x in (0..frame.width - period).step_by(2) {
                    samples = samples.saturating_add(1);
                    let left = frame.pixels[row + x] & 0x00ff_ffff;
                    let right = frame.pixels[row + x + period] & 0x00ff_ffff;
                    if left != 0 && left == right {
                        matches = matches.saturating_add(1);
                    }
                }
            }
            matches
                .saturating_mul(1000)
                .checked_div(samples)
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(0)
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

#[derive(Clone, Debug, Default)]
struct NativeDirectInputProbe {
    activity: NativeInputActivity,
    action_read_count: usize,
    executed_steps: u64,
    skipped_reason: Option<&'static str>,
    probes: Vec<String>,
}

impl NativeDirectInputProbe {
    fn skipped(reason: &'static str) -> Self {
        Self {
            skipped_reason: Some(reason),
            ..Self::default()
        }
    }

    fn json(&self) -> String {
        format!(
            "{{\"skipped_reason\":{},\"action_read_count\":{},\"executed_steps\":{},\"activity\":{},\"probes\":[{}]}}",
            self.skipped_reason.map_or_else(
                || "null".to_string(),
                |reason| format!("\"{}\"", escape_json(reason))
            ),
            self.action_read_count,
            self.executed_steps,
            self.activity.json(),
            self.probes.join(",")
        )
    }
}

fn run_native_direct_input_probe(
    emulator: &mut NativeEmulator,
    actions: &[Action],
) -> NativeDirectInputProbe {
    let baseline = emulator.input_activity();
    let start_steps = emulator.executed_steps();
    let mut action_read_count = 0usize;
    let mut probes = Vec::new();
    let poll_instruction_slice = native_play_gui_poll_instruction_slice();

    for &action in actions {
        let action_baseline = emulator.input_activity();
        emulator.set_input(action.buttons());
        let mut poll_slices = 0u8;
        let mut action_activity_observed = false;
        while poll_slices < NATIVE_PLAY_INPUT_LATCH_POLLS && !emulator.is_terminal() {
            emulator.step_instructions(poll_instruction_slice);
            poll_slices = poll_slices.saturating_add(1);
            let delta = emulator
                .input_activity()
                .saturating_subtracted(action_baseline);
            if native_action_activity_observed(delta, action) {
                action_activity_observed = true;
                break;
            }
        }

        emulator.set_input(ActionButtons::default());
        emulator.step_instructions(poll_instruction_slice);

        let action_delta = emulator
            .input_activity()
            .saturating_subtracted(action_baseline);
        if action_activity_observed {
            action_read_count = action_read_count.saturating_add(1);
        }
        probes.push(format!(
            "{{\"action_index\":{},\"action\":\"{}\",\"poll_slices\":{},\"action_activity_observed\":{},\"input_delta\":{}}}",
            action.index(),
            action.name(),
            poll_slices,
            action_activity_observed,
            action_delta.json()
        ));
    }

    NativeDirectInputProbe {
        activity: emulator.input_activity().saturating_subtracted(baseline),
        action_read_count,
        executed_steps: emulator.executed_steps().saturating_sub(start_steps),
        skipped_reason: None,
        probes,
    }
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
        && (!buttons.service || activity.system_service_active_reads > 0)
        && (!buttons.start || activity.system_start_active_reads > 0)
}

fn native_combined_play_controls_active(
    _boot_activity: NativeInputActivity,
    sweep_activity: NativeInputActivity,
) -> bool {
    sweep_activity.p1_punch_active_reads > 0
        && sweep_activity.p1_kick_active_reads > 0
        && sweep_activity.p1_beast_active_reads > 0
        && sweep_activity.p3_guard_active_reads > 0
}

fn native_combined_full_controls_active(
    boot_activity: NativeInputActivity,
    sweep_activity: NativeInputActivity,
) -> bool {
    native_combined_play_controls_active(boot_activity, sweep_activity)
        && sweep_activity.p1_up_active_reads > 0
        && sweep_activity.p1_down_active_reads > 0
        && sweep_activity.p1_left_active_reads > 0
        && sweep_activity.p1_right_active_reads > 0
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct NativeInputLatch {
    buttons: ActionButtons,
    polls_remaining: u8,
}

impl NativeInputLatch {
    fn buttons(&mut self, current: ActionButtons) -> ActionButtons {
        if action_buttons_any(current) {
            self.buttons = current;
            self.polls_remaining = NATIVE_PLAY_INPUT_LATCH_POLLS;
            return current;
        }

        if self.polls_remaining > 0 {
            self.polls_remaining -= 1;
            return self.buttons;
        }

        self.buttons = ActionButtons::default();
        self.buttons
    }
}

fn action_buttons_any(buttons: ActionButtons) -> bool {
    buttons.start
        || buttons.coin
        || buttons.service
        || buttons.up
        || buttons.down
        || buttons.left
        || buttons.right
        || buttons.punch
        || buttons.kick
        || buttons.beast
        || buttons.guard
}

fn native_play_effective_buttons(
    manual_buttons: ActionButtons,
    scripted_action: Option<Action>,
) -> ActionButtons {
    if action_buttons_any(manual_buttons) {
        return manual_buttons;
    }

    if let Some(scripted_action) = scripted_action {
        return scripted_action.buttons();
    }

    manual_buttons
}

fn native_play_runtime_title(
    autoplay_enabled: bool,
    scripted_action_active: bool,
    manual_override: bool,
    buttons: ActionButtons,
    native_playable: bool,
    gpu_playable: bool,
    gte_signal: bool,
) -> String {
    let mode = if autoplay_enabled && scripted_action_active {
        "autoplay/layered input"
    } else if autoplay_enabled {
        "manual after entry script"
    } else {
        "manual boot"
    };
    let input_mode = if manual_override {
        "manual"
    } else if scripted_action_active {
        "script"
    } else {
        "manual"
    };
    format!(
        "Bloody Roar 2 native Rust - {mode} - input_mode={input_mode} input={} - gpu={} gte={} playable={} - Esc quit",
        action_buttons_label(buttons),
        gpu_playable,
        gte_signal,
        native_playable
    )
}

fn action_buttons_label(buttons: ActionButtons) -> String {
    let mut labels = Vec::new();
    if buttons.coin {
        labels.push("coin");
    }
    if buttons.service {
        labels.push("service");
    }
    if buttons.start {
        labels.push("start");
    }
    if buttons.up {
        labels.push("up");
    }
    if buttons.down {
        labels.push("down");
    }
    if buttons.left {
        labels.push("left");
    }
    if buttons.right {
        labels.push("right");
    }
    if buttons.punch {
        labels.push("punch");
    }
    if buttons.kick {
        labels.push("kick");
    }
    if buttons.beast {
        labels.push("beast");
    }
    if buttons.guard {
        labels.push("guard");
    }
    if labels.is_empty() {
        "noop".to_string()
    } else {
        labels.join("+")
    }
}

fn native_window_buttons(window: &Window) -> ActionButtons {
    merge_action_buttons(
        native_keys_to_buttons(window.get_keys()),
        native_keys_to_buttons(window.get_keys_pressed(KeyRepeat::Yes)),
    )
}

fn native_keys_to_buttons<I>(keys: I) -> ActionButtons
where
    I: IntoIterator<Item = Key>,
{
    let mut buttons = ActionButtons::default();
    for key in keys {
        match key {
            Key::Enter => {
                buttons.start = true;
                buttons.coin = true;
            }
            Key::P => buttons.start = true,
            Key::C => buttons.coin = true,
            Key::V => buttons.service = true,
            Key::Up | Key::W => buttons.up = true,
            Key::Down | Key::S => buttons.down = true,
            Key::Left | Key::A => buttons.left = true,
            Key::Right | Key::D => buttons.right = true,
            Key::Z | Key::Space | Key::J | Key::F => buttons.punch = true,
            Key::X | Key::K | Key::H => buttons.kick = true,
            Key::Q | Key::L | Key::B => buttons.beast = true,
            Key::E | Key::I | Key::G => buttons.guard = true,
            _ => {}
        }
    }
    buttons
}

fn merge_action_buttons(a: ActionButtons, b: ActionButtons) -> ActionButtons {
    ActionButtons {
        start: a.start || b.start,
        coin: a.coin || b.coin,
        service: a.service || b.service,
        up: a.up || b.up,
        down: a.down || b.down,
        left: a.left || b.left,
        right: a.right || b.right,
        punch: a.punch || b.punch,
        kick: a.kick || b.kick,
        beast: a.beast || b.beast,
        guard: a.guard || b.guard,
    }
}

fn parse_native_window_scale(value: Option<String>) -> Result<Scale, String> {
    match value
        .as_deref()
        .unwrap_or("1")
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
        "{}",
        "bloodyroar2-gym\n\nCommands:\n  info\n  action-space\n  observation-space\n  reset\n  step <action_index> [frames]\n  serve [address]\n  serve-native [address] [rom_zip_or_dir] [instructions_per_frame]\n  prepare-assets <archive.zip> [rom_dir]\n  mame-required [rom_dir]\n  rom-ident [rom_dir]\n  mame-check [rom_dir]\n  doctor [rom_dir]\n  play [rom_dir] [extra_mame_args...]\n  prepare-zinc <archive.zip> [extract_dir]\n  zinc-check [bundle_dir]\n  zinc-play [bundle_dir] [extra_zinc_args...]\n  native-inspect [rom_zip_or_dir]\n  native-rom-summary [rom_zip_or_dir]\n  native-cache-prepare [rom_zip_or_dir]\n  native-cache-path [rom_zip_or_dir]\n  native-step [rom_zip_or_dir] [instruction_count]\n  native-screenshot [rom_zip_or_dir] [instruction_count] [output.png]\n  native-display-screenshot [rom_zip_or_dir] [instruction_count] [output.png]\n  native-vram-screenshot [rom_zip_or_dir] [instruction_count] [output.png]\n  native-screen-dump [rom_zip_or_dir] [instruction_count] [output_prefix]\n  native-window-snapshot [rom_zip_or_dir] [instruction_count] [output.png]\n  native-play-snapshot [rom_zip_or_dir] [instructions_per_frame] [output_prefix] [--complete-script] [--match-script] [--fast-forward-frames n] [--draw-capture-predicate key=value,...] [action:frames...]\n  native-play [rom_zip_or_dir] [instructions_per_frame] [scale] [max_frames]\n  native-manual [rom_zip_or_dir] [instructions_per_frame] [scale] [max_frames]\n  native-autoplay [rom_zip_or_dir] [instructions_per_frame] [scale] [max_frames] [action:frames...]\n  native-input-check [rom_zip_or_dir] [instructions_per_frame]\n  native-health-check [rom_zip_or_dir] [instructions_per_frame] [branch_frames] [settle_frames] [wall_timeout_secs]\n  native-credit-mapping-probe [rom_zip_or_dir] [instructions_per_frame] [max_frames] [mapping...]\n  native-credit-bit-probe [rom_zip_or_dir] [instructions_per_frame] [max_frames] [mapping] [bit...]\n  native-credit-projection-probe [rom_zip_or_dir] [instructions_per_frame] [max_frames] [mapping] [bit] [projection...]\n  native-scripted-step <rom_zip_or_dir> <instructions_per_frame> <output.png> <action:frames>...\n  native-scripted-dump <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>...\n  native-scripted-candidates <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>...\n  native-scripted-summary <rom_zip_or_dir> <instructions_per_frame> <action:frames>...\n  native-scripted-probe <rom_zip_or_dir> <instructions_per_frame> <action:frames>...\n  native-scripted-frame-probe <rom_zip_or_dir> <instructions_per_frame> <probe_stride_frames> <action:frames>...\n  native-scripted-compact-probe <rom_zip_or_dir> <instructions_per_frame> <probe_stride_frames> <action:frames>...\n  native-scripted-live-probe <rom_zip_or_dir> <instructions_per_frame> <emit_stride_frames> <action:frames>...\n  native-scripted-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <action:frames>... [-- <trace options>]\n  native-scripted-vblank-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <warmup_action:frames>... --trace <trace_action:frames>... [-- <trace options>]\n  native-scripted-vblank-watch-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <warmup_action:frames>... --trace <trace_action:frames>... -- <trace options>\n  native-scripted-timeline <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>...\n  native-scripted-branch <rom_zip_or_dir> <instructions_per_frame> <branch_frames> <settle_frames> <warmup_action:frames>...\n  native-scripted-branch-summary <rom_zip_or_dir> <instructions_per_frame> <branch_frames> <settle_frames> <warmup_action:frames>...\n  native-scripted-ram-diff <rom_zip_or_dir> <instructions_per_frame> <branch_frames> <settle_frames> <warmup_action:frames>... [--actions action...]\n  native-draw-snapshot <rom_zip_or_dir> <instruction_count> <sequence_start> <sequence_end> <output_prefix>\n  native-scripted-draw-snapshot <rom_zip_or_dir> <instructions_per_frame> <sequence_start> <sequence_end> <output_prefix> <action:frames>...\n  native-trace [rom_zip_or_dir] [instruction_count] [hot_limit] [recent_limit] [stop_pc] [stop_below_pc] [--stop-above-pc address] [--stop-unmapped-pc] [--stop-watch-write] [--watch address [len]] [--watch-only]\n  native-env-step <rom_zip_or_dir> <action_index> [frames] [instructions_per_frame]\n  asset-check <path>\n\nnative-cache-prepare materializes ZIP assets once and records the latest cache directory; native-cache-path prints that reusable ROM directory. If a native command omits rom_zip_or_dir, it uses the latest cache dir first, then assets/BloodRoar2-combined.zip, then assets/roms. native-play fast-forwards warning/title/entry to the first verified playable render before opening the macOS window, with manual keys active: arrows/WASD move, Z/Space/J confirm or punch, X/K kick, Q/L/B beast, E/I/G guard, C coin, V service, Enter coin+start, P start, Esc quit. native-manual opens the cold-boot keyboard-controlled GUI with no entry script. native-window-snapshot writes the exact 640x480 GUI frame without opening a window. native-autoplay keeps the bounded pre-window fast-forward path for diagnostics and custom scripts. native-play-snapshot writes the bounded fast-forwarded frame without opening a window and reports stop_reason in JSON; --draw-capture-predicate accepts filters like kind=textured_rect,texture_page=0x0299,clut=0x7c80,min_area=50000,overlap=0:240:511:479 and arms after boot. native-credit-mapping-probe compares coin/start mappings without opening a window; native-credit-bit-probe reuses one booted title state to compare adapter bit masks; native-credit-projection-probe compares scratchpad projection targets such as default/current/edge/previous/copies/wide or 0x1f800080/4=0x8. max_frames is optional and intended for smoke tests. Set BR2_NATIVE_SCRIPT_TIMED_WALL_TIMEOUT_SECS to override timed native scripted diagnostic wall timeouts; set BR2_NATIVE_PLAY_SNAPSHOT_WALL_TIMEOUT_SECS to cap native-play-snapshot wall time; set BR2_NATIVE_GAP_PROBE_WALL_TIMEOUT_SECS to override native-gap-probe wall time during long macOS smoke tests.\nThis project never ships ROMs, BIOS files, Windows EXEs, or DLLs. Configure legally obtained assets outside Git."
    );
    println!(
        "  native-scripted-fast-summary <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
    );
    println!(
        "  native-credit-ram-probe [rom_zip_or_dir] [instructions_per_frame] [branch_frames] [settle_frames] [mapping] [projection] [bit...]"
    );
    println!(
        "  native-credit-state-probe [rom_zip_or_dir] [instructions_per_frame] [branch_frames] [settle_frames] [action...]"
    );
    println!(
        "  native-scripted-watch-followup-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <followup_steps> <warmup_action:frames>... --trace <trace_action:frames>... -- <trace options> [--followup-watch address [len]] [--followup-stop-watch-write]"
    );
    println!(
        "  native-scripted-input-probe <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
    );
    println!(
        "  native-scripted-code-dump <rom_zip_or_dir> <instructions_per_frame> <address> <words> <action:frames>..."
    );
    println!(
        "  native-scripted-mips-imm-search <rom_zip_or_dir> <instructions_per_frame> [limit] [imm...] -- [action:frames...]"
    );
    println!(
        "  native-scripted-lean-probe <rom_zip_or_dir> <instructions_per_frame> <probe_stride_frames> <action:frames>..."
    );
    println!(
        "  native-display-diagnose <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
    );
    println!(
        "  native-scripted-playability-diagnose <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
    );
    println!(
        "  native-scripted-transition-diagnose <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
    );
    println!(
        "  native-security-probe <rom_zip_or_dir> <instructions_per_frame> <action:frames>..."
    );
    println!(
        "  native-play-snapshot supports --draw-capture-predicate-from-boot to arm predicate captures before boot instead of after boot"
    );
    println!(
        "  native-gap-probe [rom_zip_or_dir] [instructions_per_frame] [--match-script|--handoff-only] [--max-frames n]"
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

fn parse_native_scripted_draw_snapshot_args(
    values: Vec<String>,
) -> Result<(u64, Vec<String>), String> {
    let mut arm_after_frames = 0_u64;
    let mut segments = Vec::new();
    let mut iter = values.into_iter();
    while let Some(value) = iter.next() {
        if value == "--arm-after-frames" {
            let frames = iter.next().ok_or_else(|| {
                "--arm-after-frames requires a non-negative frame count".to_string()
            })?;
            arm_after_frames = frames
                .parse::<u64>()
                .map_err(|_| "--arm-after-frames must be a non-negative integer".to_string())?;
        } else {
            segments.push(value);
        }
    }
    Ok((arm_after_frames, segments))
}

fn parse_native_draw_capture_predicate(
    value: &str,
) -> Result<NativeGpuDrawCapturePredicate, String> {
    let mut predicate = NativeGpuDrawCapturePredicate::default();
    let mut fields = 0usize;
    for raw_item in value.split(',') {
        let item = raw_item.trim();
        if item.is_empty() {
            continue;
        }
        let (raw_key, raw_value) = item.split_once('=').ok_or_else(|| {
            format!("--draw-capture-predicate item '{item}' must use key=value syntax")
        })?;
        let key = raw_key.trim();
        let raw_value = raw_value.trim();
        match key {
            "kind" => predicate.kind = Some(raw_value.to_string()),
            "texture_page" | "tp" | "page" => {
                predicate.texture_page = Some(parse_u16_auto_label(
                    raw_value,
                    "--draw-capture-predicate texture_page",
                )?)
            }
            "clut" => {
                predicate.clut = Some(parse_u16_auto_label(
                    raw_value,
                    "--draw-capture-predicate clut",
                )?)
            }
            "min_area" => {
                predicate.min_area = Some(parse_i64_auto_label(
                    raw_value,
                    "--draw-capture-predicate min_area",
                )?)
            }
            "max_area" => {
                predicate.max_area = Some(parse_i64_auto_label(
                    raw_value,
                    "--draw-capture-predicate max_area",
                )?)
            }
            "min_written" | "min_written_pixels" => {
                predicate.min_written_pixels = Some(parse_u64_auto_label(
                    raw_value,
                    "--draw-capture-predicate min_written_pixels",
                )?)
            }
            "overlap" | "overlaps" => {
                predicate.overlap = Some(parse_native_draw_capture_overlap(raw_value)?)
            }
            _ => return Err(format!("unknown --draw-capture-predicate key '{key}'")),
        }
        fields = fields.saturating_add(1);
    }
    if fields == 0 {
        return Err(
            "--draw-capture-predicate must contain at least one key=value item".to_string(),
        );
    }
    Ok(predicate)
}

fn parse_native_draw_capture_overlap(value: &str) -> Result<(i32, i32, i32, i32), String> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 4 {
        return Err("--draw-capture-predicate overlap must be left:top:right:bottom".to_string());
    }
    let left = parse_i32_auto_label(parts[0], "--draw-capture-predicate overlap left")?;
    let top = parse_i32_auto_label(parts[1], "--draw-capture-predicate overlap top")?;
    let right = parse_i32_auto_label(parts[2], "--draw-capture-predicate overlap right")?;
    let bottom = parse_i32_auto_label(parts[3], "--draw-capture-predicate overlap bottom")?;
    if left > right || top > bottom {
        return Err("--draw-capture-predicate overlap bounds are inverted".to_string());
    }
    Ok((left, top, right, bottom))
}

fn default_native_play_script() -> Vec<NativeScriptSegment> {
    native_snapshot_match_entry_script()
}

fn native_snapshot_handoff_script(match_script: bool) -> Vec<NativeScriptSegment> {
    if match_script {
        native_snapshot_match_entry_script()
    } else {
        default_native_play_script()
    }
}

fn native_snapshot_match_entry_script() -> Vec<NativeScriptSegment> {
    let mut segments = native_warning_skip_to_title_script();
    segments.extend([
        NativeScriptSegment {
            action: Action::Coin,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Coin,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Service,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 36,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 90,
        },
        NativeScriptSegment {
            action: Action::Punch,
            frames: 36,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 60,
        },
        NativeScriptSegment {
            action: Action::CoinStart,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 30,
        },
        NativeScriptSegment {
            action: Action::Punch,
            frames: 30,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 45,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 45,
        },
        NativeScriptSegment {
            action: Action::Right,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Down,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Left,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Up,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Punch,
            frames: 36,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 60,
        },
        NativeScriptSegment {
            action: Action::Kick,
            frames: 30,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 45,
        },
        NativeScriptSegment {
            action: Action::Beast,
            frames: 30,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 45,
        },
        NativeScriptSegment {
            action: Action::Guard,
            frames: 30,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 45,
        },
        NativeScriptSegment {
            action: Action::Punch,
            frames: 30,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 240,
        },
    ]);
    segments
}

fn native_manual_entry_script() -> Vec<NativeScriptSegment> {
    let mut segments = native_warning_skip_to_title_script();
    segments.extend([
        NativeScriptSegment {
            action: Action::Coin,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Coin,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 36,
        },
        NativeScriptSegment {
            action: Action::Service,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 96,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 420,
        },
    ]);
    segments
}

fn native_warning_skip_to_title_script() -> Vec<NativeScriptSegment> {
    vec![
        NativeScriptSegment {
            action: Action::Noop,
            frames: NATIVE_WARNING_SKIP_INITIAL_FRAMES,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: NATIVE_WARNING_SKIP_PULSE_FRAMES,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: NATIVE_WARNING_SKIP_GAP_FRAMES,
        },
        NativeScriptSegment {
            action: Action::Punch,
            frames: NATIVE_WARNING_SKIP_PULSE_FRAMES,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: NATIVE_WARNING_SKIP_GAP_FRAMES,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: NATIVE_WARNING_SKIP_PULSE_FRAMES,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: NATIVE_PLAY_TITLE_ENTRY_FRAMES,
        },
    ]
}

fn native_script_segment_is_credit_attempt(segment: NativeScriptSegment) -> bool {
    matches!(
        segment.action,
        Action::Coin | Action::CoinStart | Action::Service
    )
}

fn native_play_boot_wait_segments(segments: &[NativeScriptSegment]) -> Vec<NativeScriptSegment> {
    segments
        .iter()
        .copied()
        .take_while(|segment| !native_script_segment_is_credit_attempt(*segment))
        .collect()
}

fn native_select_entry_script() -> Vec<NativeScriptSegment> {
    let mut segments = native_manual_entry_script();
    segments.push(NativeScriptSegment {
        action: Action::Punch,
        frames: 60,
    });
    segments.push(NativeScriptSegment {
        action: Action::Noop,
        frames: 360,
    });
    segments
}

fn native_health_checkpoint_script() -> Vec<NativeScriptSegment> {
    native_snapshot_match_entry_script()
}

#[cfg(test)]
fn native_input_check_checkpoint_script() -> Vec<NativeScriptSegment> {
    let mut segments = native_warning_skip_to_title_script();
    segments.extend([
        NativeScriptSegment {
            action: Action::Coin,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Coin,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 72,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 90,
        },
    ]);
    segments
}

fn native_health_match_entry_tail_script() -> Vec<NativeScriptSegment> {
    vec![
        NativeScriptSegment {
            action: Action::Coin,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Coin,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 36,
        },
        NativeScriptSegment {
            action: Action::Service,
            frames: 12,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 96,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 1200,
        },
    ]
}

fn native_match_entry_script() -> Vec<NativeScriptSegment> {
    let mut segments = native_select_entry_script();
    segments.extend(native_health_match_entry_tail_script());
    segments
}

fn native_aggressive_match_entry_script() -> Vec<NativeScriptSegment> {
    let mut segments = native_match_entry_script();
    segments.extend([
        NativeScriptSegment {
            action: Action::CoinStart,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 36,
        },
        NativeScriptSegment {
            action: Action::Punch,
            frames: 30,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 60,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 60,
        },
        NativeScriptSegment {
            action: Action::Right,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Down,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Left,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Up,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 18,
        },
        NativeScriptSegment {
            action: Action::Punch,
            frames: 36,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 90,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: 24,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 120,
        },
        NativeScriptSegment {
            action: Action::Punch,
            frames: 30,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 60,
        },
        NativeScriptSegment {
            action: Action::Kick,
            frames: 30,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 60,
        },
        NativeScriptSegment {
            action: Action::Beast,
            frames: 30,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 60,
        },
        NativeScriptSegment {
            action: Action::Guard,
            frames: 30,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 60,
        },
        NativeScriptSegment {
            action: Action::Punch,
            frames: 30,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 900,
        },
    ]);
    segments
}

fn parse_native_autoplay_tail(
    values: Vec<String>,
) -> Result<(Option<u64>, Vec<NativeScriptSegment>), String> {
    if values.is_empty() {
        return Ok((None, native_match_entry_script()));
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
        (Some(max_frames), values[1..].to_vec())
    };
    let segments = if segment_values.is_empty() {
        native_match_entry_script()
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

fn write_output_file(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    std::fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
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

    write_output_file(&actual_display_output, emulator.actual_display_png())?;
    write_output_file(
        &raw_actual_display_output,
        emulator.raw_actual_display_png(),
    )?;
    write_output_file(&display_output, emulator.display_png())?;
    write_output_file(&observation_output, emulator.screenshot_png())?;
    write_output_file(&vram_output, emulator.vram_png())?;

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
        write_output_file(&output, &candidate.png)?;
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
        let actual_display_output = suffixed_path(
            output_prefix,
            &format!("seq-{:06}.actual-display.png", capture.sequence),
        );
        let raw_actual_display_output = suffixed_path(
            output_prefix,
            &format!("seq-{:06}.raw-actual-display.png", capture.sequence),
        );
        let gui_display_output = suffixed_path(
            output_prefix,
            &format!("seq-{:06}.gui.png", capture.sequence),
        );
        let bounds_output = suffixed_path(
            output_prefix,
            &format!("seq-{:06}.bounds.png", capture.sequence),
        );
        let texture_output = capture.texture_png.as_ref().map(|_| {
            suffixed_path(
                output_prefix,
                &format!("seq-{:06}.texture.png", capture.sequence),
            )
        });
        let raw_texture_output = capture.raw_texture_png.as_ref().map(|_| {
            suffixed_path(
                output_prefix,
                &format!("seq-{:06}.raw-texture.png", capture.sequence),
            )
        });
        let palette_output = capture.palette_png.as_ref().map(|_| {
            suffixed_path(
                output_prefix,
                &format!("seq-{:06}.palette.png", capture.sequence),
            )
        });
        let resolved_palette_output = capture.resolved_palette_png.as_ref().map(|_| {
            suffixed_path(
                output_prefix,
                &format!("seq-{:06}.resolved-palette.png", capture.sequence),
            )
        });
        let mut texture_candidate_outputs = Vec::new();
        for candidate in &capture.texture_candidate_images {
            let label = filename_token(&candidate.label);
            let decoded_output = suffixed_path(
                output_prefix,
                &format!(
                    "seq-{:06}.texture-candidate-{}-x{}-y{}-w{}.decoded.png",
                    capture.sequence, label, candidate.x, candidate.y, candidate.raw_width
                ),
            );
            let raw_output = suffixed_path(
                output_prefix,
                &format!(
                    "seq-{:06}.texture-candidate-{}-x{}-y{}-w{}.raw.png",
                    capture.sequence, label, candidate.x, candidate.y, candidate.raw_width
                ),
            );
            write_output_file(&decoded_output, &candidate.decoded_png)?;
            write_output_file(&raw_output, &candidate.raw_png)?;
            texture_candidate_outputs.push(format!(
                "{{\"label\":\"{}\",\"x\":{},\"y\":{},\"raw_width\":{},\"raw_height\":{},\"decoded_output\":\"{}\",\"raw_output\":\"{}\"}}",
                escape_json(&candidate.label),
                candidate.x,
                candidate.y,
                candidate.raw_width,
                candidate.raw_height,
                escape_json(&decoded_output.display().to_string()),
                escape_json(&raw_output.display().to_string())
            ));
        }
        let texture_candidate_outputs_json = texture_candidate_outputs.join(",");
        write_output_file(&display_output, &capture.display_png)?;
        write_output_file(&actual_display_output, &capture.actual_display_png)?;
        write_output_file(&raw_actual_display_output, &capture.raw_actual_display_png)?;
        write_output_file(&gui_display_output, &capture.gui_display_png)?;
        write_output_file(&bounds_output, &capture.bounds_png)?;
        if let (Some(output), Some(png)) = (&texture_output, &capture.texture_png) {
            write_output_file(output, png)?;
        }
        if let (Some(output), Some(png)) = (&raw_texture_output, &capture.raw_texture_png) {
            write_output_file(output, png)?;
        }
        if let (Some(output), Some(png)) = (&palette_output, &capture.palette_png) {
            write_output_file(output, png)?;
        }
        if let (Some(output), Some(png)) = (&resolved_palette_output, &capture.resolved_palette_png)
        {
            write_output_file(output, png)?;
        }
        captures.push(
            capture.json(
                &display_output.display().to_string(),
                &actual_display_output.display().to_string(),
                &raw_actual_display_output.display().to_string(),
                &gui_display_output.display().to_string(),
                &bounds_output.display().to_string(),
                texture_output
                    .as_ref()
                    .map(|output| output.display().to_string())
                    .as_deref(),
                raw_texture_output
                    .as_ref()
                    .map(|output| output.display().to_string())
                    .as_deref(),
                palette_output
                    .as_ref()
                    .map(|output| output.display().to_string())
                    .as_deref(),
                resolved_palette_output
                    .as_ref()
                    .map(|output| output.display().to_string())
                    .as_deref(),
                &texture_candidate_outputs_json,
            ),
        );
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

fn native_code_words_json(emulator: &NativeEmulator, base_address: u32, words: usize) -> String {
    let ram = emulator.ram_snapshot();
    let scratchpad = emulator.scratchpad_snapshot();
    (0..words)
        .map(|index| {
            let address = base_address.wrapping_add((index as u32).saturating_mul(4));
            native_debug_word_json(emulator, address, &ram, &scratchpad)
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn native_debug_word_json(
    emulator: &NativeEmulator,
    address: u32,
    ram: &[u8],
    scratchpad: &[u8],
) -> String {
    let physical = native_debug_physical_address(address);
    if native_debug_ram_offset(physical, ram.len(), 4).is_some() {
        let value = emulator.debug_read_u32(address);
        return native_debug_word_entry_json(address, physical, "ram", Some(value));
    }

    if native_debug_scratchpad_offset(physical, scratchpad.len(), 4).is_some() {
        let value = emulator.debug_read_u32(address);
        return native_debug_word_entry_json(address, physical, "scratchpad", Some(value));
    }

    native_debug_word_entry_json(address, physical, "unmapped", None)
}

fn native_debug_word_entry_json(
    address: u32,
    physical: u32,
    region: &str,
    value: Option<u32>,
) -> String {
    format!(
        "{{\"address\":{},\"address_hex\":\"0x{:08x}\",\"physical_address\":{},\"physical_address_hex\":\"0x{:08x}\",\"region\":\"{}\",\"word\":{},\"word_hex\":{}}}",
        address,
        address,
        physical,
        physical,
        region,
        optional_u32_json(value),
        optional_u32_hex_json(value)
    )
}

fn native_debug_physical_address(address: u32) -> u32 {
    match address {
        0x8000_0000..=0xbfff_ffff => address & 0x1fff_ffff,
        _ => address,
    }
}

fn native_debug_ram_offset(physical: u32, len: usize, width: usize) -> Option<usize> {
    let offset = physical as usize;
    offset
        .checked_add(width)
        .is_some_and(|end| end <= len)
        .then_some(offset)
}

fn native_debug_scratchpad_offset(physical: u32, len: usize, width: usize) -> Option<usize> {
    let offset = physical.checked_sub(NATIVE_SCRATCHPAD_BASE_ADDRESS as u32)? as usize;
    offset
        .checked_add(width)
        .is_some_and(|end| end <= len)
        .then_some(offset)
}

#[derive(Clone, Debug)]
struct NativeMipsImmediateHit {
    region: &'static str,
    region_base_physical: u32,
    offset: usize,
    physical_address: u32,
    virtual_address: u32,
    word: u32,
    opcode: u8,
    mnemonic: &'static str,
    rs: u8,
    rt: u8,
    immediate: u16,
    runtime_code: bool,
}

fn parse_native_mips_imm_search_values(
    values: Vec<String>,
) -> Result<(Vec<u16>, Vec<NativeScriptSegment>), String> {
    let Some(split_at) = values.iter().position(|value| value == "--") else {
        if values.is_empty() {
            return Ok((
                native_mips_imm_search_default_immediates(),
                native_warning_skip_to_title_script(),
            ));
        }
        if values.first().is_some_and(|value| value.contains(':')) {
            return Ok((
                native_mips_imm_search_default_immediates(),
                parse_native_script_segments(values)?,
            ));
        }
        return Ok((
            parse_native_mips_immediate_values(values)?,
            native_warning_skip_to_title_script(),
        ));
    };

    let immediate_values = values[..split_at].to_vec();
    let segment_values = values[split_at + 1..].to_vec();
    let immediates = if immediate_values.is_empty() {
        native_mips_imm_search_default_immediates()
    } else {
        parse_native_mips_immediate_values(immediate_values)?
    };
    let segments = if segment_values.is_empty() {
        native_warning_skip_to_title_script()
    } else {
        parse_native_script_segments(segment_values)?
    };

    Ok((immediates, segments))
}

fn native_mips_imm_search_default_immediates() -> Vec<u16> {
    vec![
        0x0008, 0x0010, 0x0020, 0x0030, 0x0080, 0x0400, 0x0800, 0x0808, 0xffcf, 0xffdf, 0xffef,
        0xfff7,
    ]
}

fn parse_native_mips_immediate_values(values: Vec<String>) -> Result<Vec<u16>, String> {
    let mut immediates = Vec::new();
    for value in values {
        let parsed = parse_trace_u32(&value, "MIPS immediate")?;
        if parsed <= 0xffff {
            push_unique_u16(&mut immediates, parsed as u16);
            continue;
        }

        let low = (parsed & 0xffff) as u16;
        let high = (parsed >> 16) as u16;
        push_unique_u16(&mut immediates, low);
        if high != 0 {
            push_unique_u16(&mut immediates, high);
        }
    }
    if immediates.is_empty() {
        return Err("at least one non-empty MIPS immediate is required".to_string());
    }
    Ok(immediates)
}

fn push_unique_u16(values: &mut Vec<u16>, value: u16) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn run_native_scripted_mips_imm_search(
    rom: PathBuf,
    instructions_per_frame: u64,
    limit: usize,
    immediates: Vec<u16>,
    segments: Vec<NativeScriptSegment>,
) -> Result<(), String> {
    let mut emulator = native_emulator_from_rom_source(rom)
        .map_err(|error| format!("failed to create MIPS immediate search emulator: {error}"))?;
    let script_run =
        run_native_script_quiet_vblank(&mut emulator, instructions_per_frame, &segments);
    let ram = emulator.ram_snapshot();
    let scratchpad = emulator.scratchpad_snapshot();
    let (total_hits, emitted_hits, hit_json) =
        native_mips_immediate_search_json(&ram, &scratchpad, &immediates, limit);
    let immediate_json = immediates
        .iter()
        .map(|immediate| {
            format!(
                "{{\"value\":{},\"value_hex\":\"0x{immediate:04x}\"}}",
                immediate
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    println!(
        "{{\"instructions_per_frame\":{},\"limit\":{},\"total_hits\":{},\"emitted_hits\":{},\"truncated\":{},\"immediates\":[{}],\"segments\":[{}],\"script_run\":{},\"hits\":[{}],\"state\":{}}}",
        instructions_per_frame,
        limit,
        total_hits,
        emitted_hits,
        total_hits > emitted_hits,
        immediate_json,
        native_script_segments_json(&segments),
        script_run.json(),
        hit_json,
        emulator.input_compact_probe_json()
    );
    Ok(())
}

fn native_mips_immediate_search_json(
    ram: &[u8],
    scratchpad: &[u8],
    immediates: &[u16],
    limit: usize,
) -> (usize, usize, String) {
    let mut hits = Vec::new();
    collect_native_mips_immediate_hits(&mut hits, "ram", 0, ram, immediates);
    collect_native_mips_immediate_hits(
        &mut hits,
        "scratchpad",
        NATIVE_SCRATCHPAD_BASE_ADDRESS as u32,
        scratchpad,
        immediates,
    );
    hits.sort_by_key(native_mips_immediate_hit_sort_key);

    let total_hits = hits.len();
    let emitted_hits = total_hits.min(limit);
    let hit_json = hits
        .iter()
        .take(limit)
        .map(|hit| {
            let region_bytes = if hit.region == "scratchpad" {
                scratchpad
            } else {
                ram
            };
            native_mips_immediate_hit_json(hit, region_bytes)
        })
        .collect::<Vec<_>>()
        .join(",");

    (total_hits, emitted_hits, hit_json)
}

fn collect_native_mips_immediate_hits(
    hits: &mut Vec<NativeMipsImmediateHit>,
    region: &'static str,
    region_base_physical: u32,
    bytes: &[u8],
    immediates: &[u16],
) {
    for word_index in 0..(bytes.len() / 4) {
        let offset = word_index * 4;
        let word = native_read_le_u32(bytes, offset);
        if !native_mips_instruction_has_immediate(word) {
            continue;
        }
        let immediate = (word & 0xffff) as u16;
        if !immediates.contains(&immediate) {
            continue;
        }

        let physical_address = region_base_physical.wrapping_add(offset as u32);
        let opcode = ((word >> 26) & 0x3f) as u8;
        hits.push(NativeMipsImmediateHit {
            region,
            region_base_physical,
            offset,
            physical_address,
            virtual_address: native_mips_debug_virtual_address(physical_address),
            word,
            opcode,
            mnemonic: native_mips_immediate_mnemonic(opcode, word),
            rs: ((word >> 21) & 0x1f) as u8,
            rt: ((word >> 16) & 0x1f) as u8,
            immediate,
            runtime_code: (NATIVE_BR2_RUNTIME_CODE_START..NATIVE_BR2_RUNTIME_CODE_END)
                .contains(&physical_address),
        });
    }
}

fn native_mips_immediate_hit_sort_key(hit: &NativeMipsImmediateHit) -> (u8, u8, u16, u32, u32) {
    let region_rank = if hit.runtime_code {
        0
    } else if hit.region == "scratchpad" {
        2
    } else {
        1
    };
    (
        region_rank,
        native_mips_opcode_interest_rank(hit.opcode),
        hit.immediate,
        hit.physical_address,
        hit.word,
    )
}

fn native_mips_opcode_interest_rank(opcode: u8) -> u8 {
    match opcode {
        0x0c => 0,        // andi
        0x0d | 0x0e => 1, // ori/xori
        0x0f => 2,        // lui
        0x04 | 0x05 => 3, // beq/bne
        0x08 | 0x09 => 4, // addi/addiu
        0x20..=0x2e => 5, // load/store offsets
        _ => 6,
    }
}

fn native_mips_immediate_hit_json(hit: &NativeMipsImmediateHit, region_bytes: &[u8]) -> String {
    format!(
        "{{\"region\":\"{}\",\"address\":{},\"address_hex\":\"0x{:08x}\",\"physical_address\":{},\"physical_address_hex\":\"0x{:08x}\",\"runtime_code\":{},\"word\":{},\"word_hex\":\"0x{:08x}\",\"opcode\":{},\"mnemonic\":\"{}\",\"rs\":{},\"rs_name\":\"{}\",\"rt\":{},\"rt_name\":\"{}\",\"immediate\":{},\"immediate_hex\":\"0x{:04x}\",\"signed_immediate\":{},\"context\":[{}]}}",
        hit.region,
        hit.virtual_address,
        hit.virtual_address,
        hit.physical_address,
        hit.physical_address,
        hit.runtime_code,
        hit.word,
        hit.word,
        hit.opcode,
        hit.mnemonic,
        hit.rs,
        native_mips_register_name(hit.rs),
        hit.rt,
        native_mips_register_name(hit.rt),
        hit.immediate,
        hit.immediate,
        hit.immediate as i16,
        native_mips_context_json(hit.region_base_physical, region_bytes, hit.offset)
    )
}

fn native_mips_context_json(region_base_physical: u32, bytes: &[u8], hit_offset: usize) -> String {
    let word_count = bytes.len() / 4;
    if word_count == 0 {
        return String::new();
    }

    let hit_word_index = hit_offset / 4;
    let first = hit_word_index.saturating_sub(3);
    let last = (hit_word_index + 3).min(word_count - 1);
    (first..=last)
        .map(|word_index| {
            let offset = word_index * 4;
            let physical_address = region_base_physical.wrapping_add(offset as u32);
            let word = native_read_le_u32(bytes, offset);
            let opcode = ((word >> 26) & 0x3f) as u8;
            format!(
                "{{\"address\":{},\"address_hex\":\"0x{:08x}\",\"physical_address\":{},\"physical_address_hex\":\"0x{:08x}\",\"word\":{},\"word_hex\":\"0x{:08x}\",\"mnemonic\":\"{}\",\"is_hit\":{}}}",
                native_mips_debug_virtual_address(physical_address),
                native_mips_debug_virtual_address(physical_address),
                physical_address,
                physical_address,
                word,
                word,
                native_mips_immediate_mnemonic(opcode, word),
                offset == hit_offset
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn native_read_le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn native_mips_debug_virtual_address(physical: u32) -> u32 {
    if physical < 0x0080_0000 {
        0x8000_0000 | physical
    } else {
        physical
    }
}

fn native_mips_instruction_has_immediate(word: u32) -> bool {
    let opcode = ((word >> 26) & 0x3f) as u8;
    !matches!(opcode, 0x00 | 0x02 | 0x03)
}

fn native_mips_immediate_mnemonic(opcode: u8, word: u32) -> &'static str {
    match opcode {
        0x00 => native_mips_special_mnemonic(word),
        0x01 => "regimm",
        0x02 => "j",
        0x03 => "jal",
        0x04 => "beq",
        0x05 => "bne",
        0x06 => "blez",
        0x07 => "bgtz",
        0x08 => "addi",
        0x09 => "addiu",
        0x0a => "slti",
        0x0b => "sltiu",
        0x0c => "andi",
        0x0d => "ori",
        0x0e => "xori",
        0x0f => "lui",
        0x10 => "cop0",
        0x11 => "cop1",
        0x12 => "cop2",
        0x13 => "cop3",
        0x20 => "lb",
        0x21 => "lh",
        0x22 => "lwl",
        0x23 => "lw",
        0x24 => "lbu",
        0x25 => "lhu",
        0x26 => "lwr",
        0x28 => "sb",
        0x29 => "sh",
        0x2a => "swl",
        0x2b => "sw",
        0x2e => "swr",
        0x30 => "lwc0",
        0x31 => "lwc1",
        0x32 => "lwc2",
        0x33 => "lwc3",
        0x38 => "swc0",
        0x39 => "swc1",
        0x3a => "swc2",
        0x3b => "swc3",
        _ => "unknown",
    }
}

fn native_mips_special_mnemonic(word: u32) -> &'static str {
    match word & 0x3f {
        0x00 => "sll",
        0x02 => "srl",
        0x03 => "sra",
        0x04 => "sllv",
        0x06 => "srlv",
        0x07 => "srav",
        0x08 => "jr",
        0x09 => "jalr",
        0x0c => "syscall",
        0x0d => "break",
        0x10 => "mfhi",
        0x11 => "mthi",
        0x12 => "mflo",
        0x13 => "mtlo",
        0x18 => "mult",
        0x19 => "multu",
        0x1a => "div",
        0x1b => "divu",
        0x20 => "add",
        0x21 => "addu",
        0x22 => "sub",
        0x23 => "subu",
        0x24 => "and",
        0x25 => "or",
        0x26 => "xor",
        0x27 => "nor",
        0x2a => "slt",
        0x2b => "sltu",
        _ => "special",
    }
}

fn native_mips_register_name(register: u8) -> &'static str {
    match register {
        0 => "$zero",
        1 => "$at",
        2 => "$v0",
        3 => "$v1",
        4 => "$a0",
        5 => "$a1",
        6 => "$a2",
        7 => "$a3",
        8 => "$t0",
        9 => "$t1",
        10 => "$t2",
        11 => "$t3",
        12 => "$t4",
        13 => "$t5",
        14 => "$t6",
        15 => "$t7",
        16 => "$s0",
        17 => "$s1",
        18 => "$s2",
        19 => "$s3",
        20 => "$s4",
        21 => "$s5",
        22 => "$s6",
        23 => "$s7",
        24 => "$t8",
        25 => "$t9",
        26 => "$k0",
        27 => "$k1",
        28 => "$gp",
        29 => "$sp",
        30 => "$fp",
        31 => "$ra",
        _ => "$invalid",
    }
}

fn run_native_script(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    segments: &[NativeScriptSegment],
) -> u64 {
    run_native_script_observed(emulator, instructions_per_frame, segments).total_frames
}

fn run_native_script_quiet_vblank(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    segments: &[NativeScriptSegment],
) -> NativeScriptRunSummary {
    let instructions_per_frame = instructions_per_frame.max(1);
    let mut total_frames = 0u64;
    let mut frame_attempts = 0u64;
    let mut missed_vblank_frames = 0u64;
    let mut stop_reason = "script_completed";

    'script: for segment in segments {
        emulator.set_input(segment.action.buttons());
        for _ in 0..segment.frames {
            let vblank_advanced = step_until_next_vblank_checked(emulator, instructions_per_frame);
            frame_attempts = frame_attempts.saturating_add(1);
            if !vblank_advanced {
                missed_vblank_frames = missed_vblank_frames.saturating_add(1);
            }
            total_frames = total_frames.saturating_add(1);
            if emulator.is_terminal() {
                stop_reason = "terminal";
                break 'script;
            }
        }
    }

    NativeScriptRunSummary {
        total_frames,
        frame_attempts,
        missed_vblank_frames,
        observed_native_playable_candidate: false,
        first_native_playable_frame: None,
        last_native_playable_frame: None,
        stop_reason,
    }
}

fn run_native_script_observed(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    segments: &[NativeScriptSegment],
) -> NativeScriptRunSummary {
    let max_frames = native_script_max_frames_for_segments(segments);
    run_native_script_observed_with_stop(
        emulator,
        instructions_per_frame,
        segments,
        NativeScriptStopMode::None,
        Some(max_frames),
    )
    .summary
}

fn native_script_max_frames_for_segments(segments: &[NativeScriptSegment]) -> u64 {
    segments
        .iter()
        .map(|segment| segment.frames)
        .sum::<u64>()
        .max(1)
        .saturating_add(NATIVE_PLAY_HANDOFF_SETTLE_FRAMES)
}

fn native_script_progress_position_json(
    segments: &[NativeScriptSegment],
    segment_index: usize,
    segment_frame: u64,
) -> String {
    let completed = segment_index >= segments.len();
    let current_segment = segments.get(segment_index);
    let remaining_frames = current_segment
        .map(|segment| segment.frames.saturating_sub(segment_frame))
        .unwrap_or(0);
    let current_json = current_segment.map_or_else(
        || "null".to_string(),
        |segment| {
            format!(
                "{{\"index\":{},\"action_index\":{},\"action\":\"{}\",\"frame\":{},\"frames\":{},\"remaining_frames\":{}}}",
                segment_index,
                segment.action.index(),
                segment.action.name(),
                segment_frame,
                segment.frames,
                remaining_frames
            )
        },
    );
    let next_json = segments.get(segment_index.saturating_add(1)).map_or_else(
        || "null".to_string(),
        |segment| {
            format!(
                "{{\"index\":{},\"action_index\":{},\"action\":\"{}\",\"frames\":{}}}",
                segment_index.saturating_add(1),
                segment.action.index(),
                segment.action.name(),
                segment.frames
            )
        },
    );

    format!(
        "{{\"completed\":{},\"segment_index\":{},\"segment_frame\":{},\"current\":{},\"next\":{},\"remaining_frames\":{}}}",
        completed, segment_index, segment_frame, current_json, next_json, remaining_frames
    )
}

#[allow(dead_code)]
fn run_native_script_observed_until_playable_with_limit(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    segments: &[NativeScriptSegment],
    max_frames: u64,
) -> NativeScriptProgress {
    run_native_script_observed_with_stop_and_timeout(
        emulator,
        instructions_per_frame,
        segments,
        NativeScriptStopMode::PlayableCandidate,
        Some(max_frames),
        None,
    )
}

fn run_native_script_observed_until_playable_with_limit_and_timeout(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    segments: &[NativeScriptSegment],
    max_frames: u64,
    wall_timeout: Duration,
) -> NativeScriptProgress {
    run_native_script_observed_with_stop_and_timeout(
        emulator,
        instructions_per_frame,
        segments,
        NativeScriptStopMode::PlayableCandidate,
        Some(max_frames),
        Some(wall_timeout),
    )
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeScriptStopMode {
    None,
    Timed,
    FastTimed,
    InputCheck,
    InputProbe,
    TitleScreen,
    HandoffFrame,
    PlayableCandidate,
    DrawCaptureRange { sequence_end: u64 },
}

fn run_native_script_observed_with_stop(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    segments: &[NativeScriptSegment],
    stop_mode: NativeScriptStopMode,
    max_frames: Option<u64>,
) -> NativeScriptProgress {
    run_native_script_observed_with_stop_and_timeout(
        emulator,
        instructions_per_frame,
        segments,
        stop_mode,
        max_frames,
        None,
    )
}

#[derive(Clone, Copy, Debug)]
struct NativeScriptDiagnosticsSettings {
    vblank_presentation_capture_interval: Option<u64>,
    unlinked_primitive_replay_interval: Option<u64>,
}

fn apply_native_script_diagnostics(
    emulator: &mut NativeEmulator,
) -> NativeScriptDiagnosticsSettings {
    let settings = NativeScriptDiagnosticsSettings {
        vblank_presentation_capture_interval: emulator.vblank_presentation_capture_interval(),
        unlinked_primitive_replay_interval: emulator.unlinked_primitive_replay_interval(),
    };
    emulator.set_vblank_presentation_capture_interval(Some(NATIVE_SCRIPT_VBLANK_CAPTURE_INTERVAL));
    emulator
        .set_unlinked_primitive_replay_interval(NATIVE_SCRIPT_UNLINKED_PRIMITIVE_REPLAY_INTERVAL);
    settings
}

fn restore_native_script_diagnostics(
    emulator: &mut NativeEmulator,
    settings: NativeScriptDiagnosticsSettings,
) {
    emulator
        .set_vblank_presentation_capture_interval(settings.vblank_presentation_capture_interval);
    emulator.set_unlinked_primitive_replay_interval(settings.unlinked_primitive_replay_interval);
}

fn run_native_script_observed_with_stop_and_timeout(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    segments: &[NativeScriptSegment],
    stop_mode: NativeScriptStopMode,
    max_frames: Option<u64>,
    wall_timeout_override: Option<Duration>,
) -> NativeScriptProgress {
    let instructions_per_frame = instructions_per_frame.max(1);
    let mut total_frames = 0u64;
    let mut frame_attempts = 0u64;
    let mut missed_vblank_frames = 0u64;
    let mut observed_native_playable_candidate = false;
    let mut first_native_playable_frame = None;
    let mut last_native_playable_frame = None;
    let mut last_native_playable_snapshot = None;
    let mut last_native_playable_segment_index = 0usize;
    let mut last_native_playable_segment_frame = 0u64;
    let mut segment_index = 0usize;
    let mut segment_frame = 0u64;
    let mut stop_reason = "script_completed";
    let mut stopped = false;
    let started_at = Instant::now();
    let wall_timeout = wall_timeout_override.or_else(|| native_script_wall_timeout(stop_mode));
    let diagnostics_enabled = native_script_stop_mode_uses_diagnostics(stop_mode);
    let original_diagnostics_settings =
        diagnostics_enabled.then(|| apply_native_script_diagnostics(emulator));
    let target_script_frames = segments
        .iter()
        .map(|segment| segment.frames)
        .sum::<u64>()
        .max(1);
    let frame_attempt_budget = max_frames
        .unwrap_or(target_script_frames)
        .saturating_mul(NATIVE_SCRIPT_FRAME_ATTEMPT_MULTIPLIER)
        .max(target_script_frames);

    'script: for (index, segment) in segments.iter().enumerate() {
        segment_index = index;
        segment_frame = 0;
        emulator.set_input(segment.action.buttons());
        while segment_frame < segment.frames {
            if wall_timeout.is_some_and(|wall_timeout| started_at.elapsed() >= wall_timeout) {
                stop_reason = "wall_timeout";
                stopped = true;
                break 'script;
            }
            let step = step_until_next_vblank_checked_with_wall_timeout(
                emulator,
                instructions_per_frame,
                started_at,
                wall_timeout,
            );
            if step.wall_timeout {
                stop_reason = "wall_timeout";
                stopped = true;
                break 'script;
            }
            if native_script_draw_capture_range_ready(emulator, stop_mode) {
                stop_reason = "draw_capture_range_complete";
                stopped = true;
                break 'script;
            }
            let vblank_advanced = step.vblank_advanced;
            frame_attempts = frame_attempts.saturating_add(1);
            if !vblank_advanced {
                missed_vblank_frames = missed_vblank_frames.saturating_add(1);
                if frame_attempts >= frame_attempt_budget {
                    stop_reason = "max_frame_attempts";
                    stopped = true;
                    break 'script;
                }
                if emulator.is_terminal() {
                    stop_reason = "terminal";
                    stopped = true;
                    break 'script;
                }
                continue;
            }

            total_frames += 1;
            segment_frame += 1;
            if native_script_should_sample_stop_observation(total_frames, stop_mode) {
                emulator.capture_vblank_presented_frame();
                if native_script_stop_observation_ready(emulator, stop_mode) {
                    let native_candidate_observed = emulator.native_playable_candidate();
                    if native_candidate_observed {
                        observed_native_playable_candidate = true;
                        last_native_playable_snapshot = Some(emulator.clone());
                        last_native_playable_segment_index = segment_index;
                        last_native_playable_segment_frame = segment_frame;
                        first_native_playable_frame.get_or_insert(total_frames);
                        last_native_playable_frame = Some(total_frames);
                    }
                    let first_playable_frame = first_native_playable_frame.unwrap_or(total_frames);
                    let settle_frames = match stop_mode {
                        NativeScriptStopMode::None => u64::MAX,
                        NativeScriptStopMode::Timed => u64::MAX,
                        NativeScriptStopMode::FastTimed => u64::MAX,
                        NativeScriptStopMode::InputCheck => u64::MAX,
                        NativeScriptStopMode::InputProbe => u64::MAX,
                        NativeScriptStopMode::DrawCaptureRange { .. } => u64::MAX,
                        NativeScriptStopMode::TitleScreen => 0,
                        NativeScriptStopMode::HandoffFrame => 0,
                        NativeScriptStopMode::PlayableCandidate => {
                            NATIVE_PLAY_HANDOFF_SETTLE_FRAMES
                        }
                    };
                    let settled =
                        total_frames.saturating_sub(first_playable_frame) >= settle_frames;
                    let should_sample_stop = settled
                        && total_frames.is_multiple_of(NATIVE_PLAY_HANDOFF_CHECK_STRIDE_FRAMES);
                    let stop_ready = match stop_mode {
                        NativeScriptStopMode::TitleScreen => true,
                        NativeScriptStopMode::HandoffFrame => true,
                        NativeScriptStopMode::PlayableCandidate => should_sample_stop,
                        NativeScriptStopMode::None
                        | NativeScriptStopMode::Timed
                        | NativeScriptStopMode::FastTimed
                        | NativeScriptStopMode::InputCheck
                        | NativeScriptStopMode::InputProbe
                        | NativeScriptStopMode::DrawCaptureRange { .. } => false,
                    };
                    if stop_ready {
                        stop_reason = match stop_mode {
                            NativeScriptStopMode::None => "script_completed",
                            NativeScriptStopMode::Timed => "script_completed",
                            NativeScriptStopMode::FastTimed => "script_completed",
                            NativeScriptStopMode::InputCheck => "script_completed",
                            NativeScriptStopMode::InputProbe => "script_completed",
                            NativeScriptStopMode::TitleScreen => "title_screen_ready",
                            NativeScriptStopMode::HandoffFrame => "handoff_frame_ready",
                            NativeScriptStopMode::PlayableCandidate => "playable_candidate_settled",
                            NativeScriptStopMode::DrawCaptureRange { .. } => "script_completed",
                        };
                        stopped = true;
                        break 'script;
                    }
                }
            }
            if stop_mode == NativeScriptStopMode::InputCheck
                && native_script_should_sample_playable_observation(total_frames)
            {
                emulator.capture_vblank_presented_frame();
                if native_script_input_check_observation_ready(emulator) {
                    stop_reason = "input_check_ready";
                    stopped = true;
                    break 'script;
                }
            }
            if stop_mode == NativeScriptStopMode::InputProbe
                && native_script_input_probe_ready(emulator)
            {
                stop_reason = "input_probe_ready";
                stopped = true;
                break 'script;
            }
            if native_script_title_wait_ready_to_advance(
                emulator,
                *segment,
                segment_frame,
                total_frames,
                stop_mode,
            ) {
                if stop_mode == NativeScriptStopMode::TitleScreen {
                    stop_reason = "title_screen_ready";
                    stopped = true;
                    break 'script;
                }
                segment_frame = segment.frames;
                continue;
            }
            if max_frames.is_some_and(|max_frames| total_frames >= max_frames) {
                stop_reason = "max_frames";
                stopped = true;
                break 'script;
            }
            if emulator.is_terminal() {
                stop_reason = "terminal";
                stopped = true;
                break 'script;
            }
        }
        if segment_frame >= segment.frames {
            segment_index = index + 1;
            segment_frame = 0;
        }
    }

    if !stopped
        && stop_mode == NativeScriptStopMode::PlayableCandidate
        && max_frames.is_some_and(|max_frames| total_frames < max_frames)
        && !emulator.is_terminal()
    {
        segment_index = segments.len();
        segment_frame = 0;
        emulator.set_input(ActionButtons::default());
        loop {
            if wall_timeout.is_some_and(|wall_timeout| started_at.elapsed() >= wall_timeout) {
                stop_reason = "wall_timeout";
                break;
            }
            let step = step_until_next_vblank_checked_with_wall_timeout(
                emulator,
                instructions_per_frame,
                started_at,
                wall_timeout,
            );
            if step.wall_timeout {
                stop_reason = "wall_timeout";
                break;
            }
            if native_script_draw_capture_range_ready(emulator, stop_mode) {
                stop_reason = "draw_capture_range_complete";
                break;
            }
            let vblank_advanced = step.vblank_advanced;
            frame_attempts = frame_attempts.saturating_add(1);
            if !vblank_advanced {
                missed_vblank_frames = missed_vblank_frames.saturating_add(1);
                if frame_attempts >= frame_attempt_budget {
                    stop_reason = "max_frame_attempts";
                    break;
                }
                if emulator.is_terminal() {
                    stop_reason = "terminal";
                    break;
                }
                continue;
            }

            total_frames += 1;
            segment_frame += 1;
            if native_script_should_sample_stop_observation(total_frames, stop_mode) {
                emulator.capture_vblank_presented_frame();
                if native_script_stop_observation_ready(emulator, stop_mode) {
                    let native_candidate_observed = emulator.native_playable_candidate();
                    if native_candidate_observed {
                        observed_native_playable_candidate = true;
                        last_native_playable_snapshot = Some(emulator.clone());
                        last_native_playable_segment_index = segment_index;
                        last_native_playable_segment_frame = segment_frame;
                        first_native_playable_frame.get_or_insert(total_frames);
                        last_native_playable_frame = Some(total_frames);
                    }
                    let first_playable_frame = first_native_playable_frame.unwrap_or(total_frames);
                    let settle_frames = match stop_mode {
                        NativeScriptStopMode::None => u64::MAX,
                        NativeScriptStopMode::Timed => u64::MAX,
                        NativeScriptStopMode::FastTimed => u64::MAX,
                        NativeScriptStopMode::InputCheck => u64::MAX,
                        NativeScriptStopMode::InputProbe => u64::MAX,
                        NativeScriptStopMode::DrawCaptureRange { .. } => u64::MAX,
                        NativeScriptStopMode::TitleScreen => 0,
                        NativeScriptStopMode::HandoffFrame => 0,
                        NativeScriptStopMode::PlayableCandidate => {
                            NATIVE_PLAY_HANDOFF_SETTLE_FRAMES
                        }
                    };
                    let settled =
                        total_frames.saturating_sub(first_playable_frame) >= settle_frames;
                    let should_sample_stop = settled
                        && total_frames.is_multiple_of(NATIVE_PLAY_HANDOFF_CHECK_STRIDE_FRAMES);
                    let stop_ready = match stop_mode {
                        NativeScriptStopMode::TitleScreen => true,
                        NativeScriptStopMode::HandoffFrame => true,
                        NativeScriptStopMode::PlayableCandidate => should_sample_stop,
                        NativeScriptStopMode::None
                        | NativeScriptStopMode::Timed
                        | NativeScriptStopMode::FastTimed
                        | NativeScriptStopMode::InputCheck
                        | NativeScriptStopMode::InputProbe
                        | NativeScriptStopMode::DrawCaptureRange { .. } => false,
                    };
                    if stop_ready {
                        stop_reason = match stop_mode {
                            NativeScriptStopMode::None => "script_completed",
                            NativeScriptStopMode::Timed => "script_completed",
                            NativeScriptStopMode::FastTimed => "script_completed",
                            NativeScriptStopMode::InputCheck => "script_completed",
                            NativeScriptStopMode::InputProbe => "script_completed",
                            NativeScriptStopMode::TitleScreen => "title_screen_ready",
                            NativeScriptStopMode::HandoffFrame => "handoff_frame_ready",
                            NativeScriptStopMode::PlayableCandidate => "playable_candidate_settled",
                            NativeScriptStopMode::DrawCaptureRange { .. } => "script_completed",
                        };
                        break;
                    }
                }
            }
            if max_frames.is_some_and(|max_frames| total_frames >= max_frames) {
                stop_reason = "max_frames";
                break;
            }
            if emulator.is_terminal() {
                stop_reason = "terminal";
                break;
            }
        }
    }

    if stop_mode == NativeScriptStopMode::PlayableCandidate
        && observed_native_playable_candidate
        && !native_play_emulator_handoff_ready(emulator)
        && let Some(snapshot) = last_native_playable_snapshot
    {
        *emulator = snapshot;
        segment_index = last_native_playable_segment_index;
        segment_frame = last_native_playable_segment_frame;
        if stop_reason == "script_completed" || stop_reason == "max_frames" {
            stop_reason = "playable_candidate_snapshot";
        }
    }

    emulator.capture_vblank_presented_frame();
    if let Some(original_diagnostics_settings) = original_diagnostics_settings {
        restore_native_script_diagnostics(emulator, original_diagnostics_settings);
    }

    NativeScriptProgress {
        summary: NativeScriptRunSummary {
            total_frames,
            frame_attempts,
            missed_vblank_frames,
            observed_native_playable_candidate,
            first_native_playable_frame,
            last_native_playable_frame,
            stop_reason,
        },
        segment_index,
        segment_frame,
    }
}

fn native_script_playable_observation_ready(
    emulator: &NativeEmulator,
    stop_mode: NativeScriptStopMode,
) -> bool {
    if !emulator.native_playable_candidate() {
        return false;
    }
    let _ = stop_mode;
    native_play_emulator_handoff_ready(emulator)
}

fn native_script_stop_observation_ready(
    emulator: &NativeEmulator,
    stop_mode: NativeScriptStopMode,
) -> bool {
    match stop_mode {
        NativeScriptStopMode::TitleScreen => {
            native_play_emulator_title_screen_input_ready(emulator)
        }
        NativeScriptStopMode::HandoffFrame => native_play_emulator_handoff_ready(emulator),
        NativeScriptStopMode::PlayableCandidate => {
            native_script_playable_observation_ready(emulator, stop_mode)
        }
        NativeScriptStopMode::None
        | NativeScriptStopMode::Timed
        | NativeScriptStopMode::FastTimed
        | NativeScriptStopMode::InputCheck
        | NativeScriptStopMode::InputProbe
        | NativeScriptStopMode::DrawCaptureRange { .. } => false,
    }
}

fn native_script_draw_capture_range_ready(
    emulator: &NativeEmulator,
    stop_mode: NativeScriptStopMode,
) -> bool {
    let NativeScriptStopMode::DrawCaptureRange { sequence_end } = stop_mode else {
        return false;
    };

    !emulator.draw_captures().is_empty() && emulator.draw_sequence() > sequence_end
}

fn native_script_input_check_observation_ready(emulator: &NativeEmulator) -> bool {
    if !emulator.input_activity().has_credit_probe_activity() {
        return false;
    }

    let frame = native_play_window_frame(&emulator.display_frame());
    native_play_frame_ready_with_context(emulator.native_playable_candidate(), &frame)
}

fn native_script_input_probe_ready(emulator: &NativeEmulator) -> bool {
    let activity = emulator.input_activity();
    activity.has_credit_probe_activity() && activity.has_any_control_activity()
}

fn native_script_should_sample_playable_observation(total_frames: u64) -> bool {
    total_frames == 1 || total_frames.is_multiple_of(NATIVE_PLAY_HANDOFF_CHECK_STRIDE_FRAMES)
}

fn native_script_should_sample_stop_observation(
    total_frames: u64,
    stop_mode: NativeScriptStopMode,
) -> bool {
    match stop_mode {
        NativeScriptStopMode::TitleScreen => {
            total_frames == 1
                || total_frames.is_multiple_of(NATIVE_PLAY_HANDOFF_RENDER_CHECK_STRIDE_FRAMES)
        }
        NativeScriptStopMode::HandoffFrame => {
            total_frames == 1
                || total_frames.is_multiple_of(NATIVE_PLAY_HANDOFF_RENDER_CHECK_STRIDE_FRAMES)
        }
        NativeScriptStopMode::PlayableCandidate => {
            native_script_should_sample_playable_observation(total_frames)
        }
        NativeScriptStopMode::None
        | NativeScriptStopMode::Timed
        | NativeScriptStopMode::FastTimed
        | NativeScriptStopMode::InputCheck
        | NativeScriptStopMode::InputProbe
        | NativeScriptStopMode::DrawCaptureRange { .. } => false,
    }
}

fn native_script_stop_mode_uses_diagnostics(stop_mode: NativeScriptStopMode) -> bool {
    !matches!(stop_mode, NativeScriptStopMode::FastTimed)
}

fn native_script_title_wait_ready_to_advance(
    emulator: &mut NativeEmulator,
    segment: NativeScriptSegment,
    segment_frame: u64,
    total_frames: u64,
    stop_mode: NativeScriptStopMode,
) -> bool {
    if !native_script_can_early_advance_title_wait(segment, segment_frame, stop_mode)
        || !total_frames.is_multiple_of(NATIVE_PLAY_HANDOFF_RENDER_CHECK_STRIDE_FRAMES)
    {
        return false;
    }

    emulator.capture_vblank_presented_frame();
    native_play_emulator_title_screen_input_ready(emulator)
        || native_play_emulator_handoff_ready(emulator)
}

fn native_script_can_early_advance_title_wait(
    segment: NativeScriptSegment,
    segment_frame: u64,
    stop_mode: NativeScriptStopMode,
) -> bool {
    if segment.action != Action::Noop
        || segment.frames < NATIVE_PLAY_TITLE_EARLY_ADVANCE_MIN_SEGMENT_FRAMES
        || segment_frame < NATIVE_WARNING_SKIP_GAP_FRAMES
    {
        return false;
    }

    matches!(
        stop_mode,
        NativeScriptStopMode::Timed
            | NativeScriptStopMode::FastTimed
            | NativeScriptStopMode::InputCheck
            | NativeScriptStopMode::InputProbe
            | NativeScriptStopMode::TitleScreen
            | NativeScriptStopMode::HandoffFrame
            | NativeScriptStopMode::PlayableCandidate
    )
}

fn native_play_emulator_title_screen_input_ready(emulator: &NativeEmulator) -> bool {
    let frame = native_play_window_frame_for_emulator(emulator);
    native_play_title_screen_input_ready_from_frame(&frame)
}

fn native_play_title_screen_input_ready_from_frame(frame: &NativeDisplayFrame) -> bool {
    let stats = NativeFrameStats::from_frame(frame);
    stats.has_title_screen_frame()
}

fn native_play_title_screen_ready_from_frame(frame: &NativeDisplayFrame) -> bool {
    let stats = NativeFrameStats::from_frame(frame);
    stats.has_title_screen_frame() && !stats.has_corrupt_title_texture_fragment()
}

fn native_play_emulator_handoff_ready(emulator: &NativeEmulator) -> bool {
    let frame = native_play_window_frame_for_emulator(emulator);
    native_play_frame_ready_with_context(emulator.native_playable_candidate(), &frame)
}

#[cfg(test)]
fn native_play_gui_handoff_frame_ready(frame: &NativeDisplayFrame) -> bool {
    native_play_frame_ready_with_context(false, frame)
}

fn native_play_render_ready_frame(frame: &NativeDisplayFrame) -> bool {
    NativeFrameStats::from_frame(frame).has_render_ready_scene()
}

fn native_play_presentable_frame(frame: &NativeDisplayFrame) -> bool {
    NativeFrameStats::from_frame(frame).has_presentable_scene()
}

fn native_play_visible_window_frame(
    candidate: NativeDisplayFrame,
    last_safe: &Option<NativeDisplayFrame>,
) -> NativeDisplayFrame {
    let stats = NativeFrameStats::from_frame(&candidate);
    if let Some(last_safe) = last_safe {
        if stats.has_blocking_display_artifact() || !stats.has_visible_content() {
            return last_safe.clone();
        }
    }

    candidate
}

fn native_play_visible_window_frame_with_current(
    candidate: NativeDisplayFrame,
    last_safe: &Option<NativeDisplayFrame>,
    current: &NativeDisplayFrame,
) -> NativeDisplayFrame {
    let candidate_stats = NativeFrameStats::from_frame(&candidate);
    if last_safe.is_none() && candidate_stats.has_blocking_display_artifact() {
        return current.clone();
    }

    native_play_visible_window_frame(candidate, last_safe)
}

fn native_play_select_visible_window_frame_for_emulator(
    emulator: &NativeEmulator,
    candidate: NativeDisplayFrame,
    candidate_stats: NativeFrameStats,
    last_safe: &Option<NativeDisplayFrame>,
    current: &NativeDisplayFrame,
) -> NativeDisplayFrame {
    let gpu_verified_actual = native_play_gpu_verified_actual_window_frame(emulator);
    let candidate_gpu_verified =
        native_play_gpu_verified_window_frame(emulator, &candidate, candidate_stats);
    if native_play_candidate_should_replace_stale_title(
        emulator.native_playable_candidate(),
        &candidate,
        candidate_stats,
        current,
    ) {
        return candidate;
    }
    native_play_select_visible_window_frame(
        candidate,
        candidate_gpu_verified,
        gpu_verified_actual,
        last_safe,
        current,
    )
}

fn native_play_select_visible_window_frame(
    candidate: NativeDisplayFrame,
    candidate_gpu_verified: bool,
    gpu_verified_actual: Option<NativeDisplayFrame>,
    last_safe: &Option<NativeDisplayFrame>,
    current: &NativeDisplayFrame,
) -> NativeDisplayFrame {
    if let Some(actual_window) = gpu_verified_actual {
        return actual_window;
    }

    if candidate_gpu_verified {
        candidate
    } else {
        native_play_visible_window_frame_with_current(candidate, last_safe, current)
    }
}

fn native_play_candidate_should_replace_stale_title(
    native_playable_candidate: bool,
    candidate: &NativeDisplayFrame,
    candidate_stats: NativeFrameStats,
    current: &NativeDisplayFrame,
) -> bool {
    if !native_playable_candidate {
        return false;
    }
    let current_stats = NativeFrameStats::from_frame(current);
    if !current_stats.has_title_screen_frame()
        && !current_stats.has_corrupt_title_texture_fragment()
    {
        return false;
    }

    let candidate_context_scene = native_play_frame_ready_with_context(true, candidate)
        || native_play_gameplay_scene_with_context(true, candidate_stats);
    let candidate_texture_blocks = candidate_stats.has_texture_page_fragment_artifact()
        && !candidate_stats.has_letterboxed_live_stage_scene()
        && !candidate_context_scene;
    let candidate_caption_blocks = candidate_stats.has_bottom_caption_band()
        && !candidate_stats.has_native_context_caption_ui()
        && !candidate_context_scene;

    candidate_stats.has_visible_content()
        && candidate_stats.has_scene_detail()
        && !candidate_stats.has_title_screen_frame()
        && !candidate_stats.has_corrupt_title_texture_fragment()
        && !candidate_stats.has_sparse_texture_fragment_handoff_artifact()
        && !candidate_stats.has_low_palette_noise()
        && !candidate_stats.has_low_palette_logo_transition()
        && !candidate_texture_blocks
        && !candidate_stats.has_repeating_tile_grid_artifact()
        && !candidate_stats.has_saturated_transition_planes()
        && !candidate_stats.has_warm_dominant_fill_frame()
        && !candidate_caption_blocks
        && !candidate_stats.has_periodic_horizontal_ghosting()
        && (candidate_context_scene
            || candidate_stats.has_letterboxed_live_stage_scene()
            || candidate_stats.has_native_context_scene()
            || native_play_frame_ready_with_context(true, candidate)
            || native_play_gameplay_scene_with_context(true, candidate_stats))
}

fn native_play_gameplay_scene_with_context(
    native_playable_candidate: bool,
    stats: NativeFrameStats,
) -> bool {
    !stats.has_title_screen_frame()
        && !stats.has_sparse_texture_fragment_handoff_artifact()
        && (stats.has_gameplay_scene()
            || (native_playable_candidate
                && stats.has_handoff_scene()
                && stats.has_render_ready_scene())
            || (native_playable_candidate
                && stats.has_native_context_scene()
                && stats.has_context_stage_letterbox())
            || (native_playable_candidate && stats.has_letterboxed_live_stage_scene())
            || native_play_live_field_scene_with_context(native_playable_candidate, stats))
}

fn native_play_live_field_scene_with_context(
    native_playable_candidate: bool,
    stats: NativeFrameStats,
) -> bool {
    if !native_playable_candidate
        || stats.width < NATIVE_PLAY_MIN_WINDOW_WIDTH
        || stats.height < NATIVE_PLAY_MIN_WINDOW_HEIGHT
        || !stats.has_visible_content()
        || !stats.has_scene_detail()
        || stats.has_title_screen_frame()
        || stats.has_sparse_texture_fragment_handoff_artifact()
        || stats.has_blocking_texture_page_fragment_artifact()
        || stats.has_corrupt_title_texture_fragment()
        || stats.has_low_palette_noise()
        || stats.has_low_palette_logo_transition()
        || stats.has_repeating_tile_grid_artifact()
        || stats.has_saturated_transition_planes()
        || stats.has_warm_dominant_fill_frame()
        || stats.has_periodic_horizontal_ghosting()
        || stats.has_bottom_caption_band()
    {
        return false;
    }

    let bottom_band_pixels = stats.width.saturating_mul((stats.height / 5).max(1));
    bottom_band_pixels > 0
        && stats.nonzero_pixels.saturating_mul(100) >= stats.total_pixels.saturating_mul(30)
        && stats.black_pixels.saturating_mul(100) <= stats.total_pixels.saturating_mul(70)
        && stats.dominant_color_bucket_pixels.saturating_mul(100)
            <= stats.total_pixels.saturating_mul(70)
        && stats.occupied_row_span.saturating_mul(100) >= stats.height.saturating_mul(50)
        && stats.bottom_dark_pixels.saturating_mul(100) >= bottom_band_pixels.saturating_mul(60)
}

fn native_play_gpu_verified_window_frame(
    emulator: &NativeEmulator,
    frame: &NativeDisplayFrame,
    stats: NativeFrameStats,
) -> bool {
    let native_playable_candidate = emulator.native_playable_candidate();
    native_play_gpu_verified_actual_window_frame(emulator).is_some_and(|actual_window| {
        native_frame_checksum(frame) == native_frame_checksum(&actual_window)
            && (native_play_gpu_verified_frame_stats_with_context(native_playable_candidate, stats)
                || native_play_runtime_verified_actual_window_stats(stats))
    })
}

fn native_play_gpu_verified_actual_window_frame(
    emulator: &NativeEmulator,
) -> Option<NativeDisplayFrame> {
    let native_playable_candidate = emulator.native_playable_candidate();
    let gpu_gameplay_context = native_play_gpu_gameplay_context(emulator);
    if !native_playable_candidate
        || !emulator.actual_display_supports_playable_candidate()
        || !gpu_gameplay_context
    {
        return None;
    }

    let actual_window = native_play_actual_window_frame_for_emulator(emulator);
    let actual_stats = NativeFrameStats::from_frame(&actual_window);
    (native_play_gpu_verified_frame_stats_with_context(native_playable_candidate, actual_stats)
        || native_play_runtime_verified_actual_window_stats(actual_stats))
    .then_some(actual_window)
}

#[cfg(test)]
fn native_play_gpu_verified_actual_window_frame_with_context(
    actual: &NativeDisplayFrame,
    native_playable_candidate: bool,
    gpu_actual_supports_playable: bool,
    gpu_gameplay_context: bool,
) -> Option<NativeDisplayFrame> {
    if !native_playable_candidate || !gpu_actual_supports_playable || !gpu_gameplay_context {
        return None;
    }

    let actual_window = native_play_window_frame_with_context(
        actual,
        native_playable_candidate,
        gpu_gameplay_context,
    );
    let actual_stats = NativeFrameStats::from_frame(&actual_window);
    native_play_gpu_verified_frame_stats_with_context(native_playable_candidate, actual_stats)
        .then_some(actual_window)
}

fn native_play_gpu_verified_frame_stats_with_context(
    native_playable_candidate: bool,
    stats: NativeFrameStats,
) -> bool {
    native_play_gpu_verified_frame_stats(stats)
        || native_play_gpu_verified_context_stage_frame_stats(native_playable_candidate, stats)
        || (native_playable_candidate && stats.has_letterboxed_live_stage_scene())
}

fn native_play_runtime_verified_actual_window_stats(stats: NativeFrameStats) -> bool {
    stats.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
        && stats.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT
        && stats.has_visible_content()
        && stats.has_scene_detail()
        && !stats.has_title_screen_frame()
        && !stats.has_corrupt_title_texture_fragment()
        && !stats.has_low_palette_noise()
        && !stats.has_low_palette_logo_transition()
        && !stats.has_sparse_texture_fragment_handoff_artifact()
        && !stats.has_repeating_tile_grid_artifact()
        && !stats.has_saturated_transition_planes()
        && !stats.has_warm_dominant_fill_frame()
        && !stats.has_periodic_horizontal_ghosting()
        && stats.nonzero_pixels.saturating_mul(100) >= stats.total_pixels.saturating_mul(18)
        && stats.occupied_row_span.saturating_mul(100) >= stats.height.saturating_mul(35)
        && stats.dominant_color_bucket_pixels.saturating_mul(100)
            <= stats.total_pixels.saturating_mul(84)
}

fn native_play_gpu_verified_context_stage_frame_stats(
    native_playable_candidate: bool,
    stats: NativeFrameStats,
) -> bool {
    native_playable_candidate
        && stats.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
        && stats.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT
        && stats.has_visible_content()
        && stats.has_scene_detail()
        && stats.has_context_live_stage_hud()
        && stats.has_context_stage_letterbox()
        && !stats.has_title_screen_frame()
        && !stats.has_sparse_texture_fragment_handoff_artifact()
        && !stats.has_blocking_texture_page_fragment_artifact()
        && !stats.has_stage_texture_field_smear()
        && !stats.has_corrupt_title_texture_fragment()
        && !stats.has_low_palette_noise()
        && !stats.has_low_palette_logo_transition()
        && !stats.has_repeating_tile_grid_artifact()
        && !stats.has_saturated_transition_planes()
        && !stats.has_warm_dominant_fill_frame()
        && !stats.has_blocking_title_logo_overlay()
        && !stats.has_periodic_horizontal_ghosting()
        && stats.nonzero_pixels.saturating_mul(100) >= stats.total_pixels.saturating_mul(35)
        && stats.black_pixels.saturating_mul(100) <= stats.total_pixels.saturating_mul(55)
        && stats.dominant_color_bucket_pixels.saturating_mul(100)
            <= stats.total_pixels.saturating_mul(55)
        && stats.occupied_row_span.saturating_mul(100) >= stats.height.saturating_mul(75)
}

fn native_play_gpu_verified_frame_stats(stats: NativeFrameStats) -> bool {
    stats.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
        && stats.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT
        && stats.has_visible_content()
        && stats.has_scene_detail()
        && !stats.has_title_screen_frame()
        && !stats.has_sparse_texture_fragment_handoff_artifact()
        && !stats.has_blocking_texture_page_fragment_artifact()
        && !stats.has_corrupt_title_texture_fragment()
        && !stats.has_low_palette_noise()
        && !stats.has_low_palette_logo_transition()
        && !stats.has_repeating_tile_grid_artifact()
        && !stats.has_saturated_transition_planes()
        && !stats.has_warm_dominant_fill_frame()
        && !stats.has_blocking_title_logo_overlay()
        && !stats.has_periodic_horizontal_ghosting()
}

fn native_play_inferred_playable_candidate(emulator: &NativeEmulator) -> bool {
    if emulator.native_playable_candidate() {
        return true;
    }

    if !native_play_gpu_gameplay_context(emulator) {
        return false;
    }

    let frame = native_play_window_frame_for_emulator_with_context(emulator, true);
    let stats = NativeFrameStats::from_frame(&frame);
    native_play_frame_ready_with_context(true, &frame)
        || native_play_gameplay_scene_with_context(true, stats)
}

fn native_play_frame_ready_with_context(
    native_playable_candidate: bool,
    frame: &NativeDisplayFrame,
) -> bool {
    let stats = NativeFrameStats::from_frame(frame);
    let letterboxed_live_stage =
        native_playable_candidate && stats.has_letterboxed_live_stage_scene();
    let periodic_ghosting_blocks =
        stats.has_periodic_horizontal_ghosting() && !native_playable_candidate;
    let full_visible_scene = stats.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
        && stats.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT
        && stats.has_visible_content()
        && stats.has_scene_detail()
        && !stats.has_title_screen_frame()
        && !stats.has_blocking_texture_page_fragment_artifact()
        && !stats.has_corrupt_title_texture_fragment()
        && !stats.has_low_palette_noise()
        && !stats.has_low_palette_logo_transition()
        && !stats.has_repeating_tile_grid_artifact()
        && !stats.has_saturated_transition_planes()
        && !stats.has_warm_dominant_fill_frame()
        && !periodic_ghosting_blocks;
    let native_context_scene = native_playable_candidate
        && (stats.has_native_context_scene() || stats.has_periodic_horizontal_ghosting());
    let live_field_scene =
        native_play_live_field_scene_with_context(native_playable_candidate, stats);
    let handoff_scene = stats.has_handoff_scene()
        && (!stats.has_bottom_caption_band()
            || (native_playable_candidate && stats.has_native_context_caption_ui()));
    letterboxed_live_stage
        || (full_visible_scene
            && (handoff_scene
                || stats.has_gameplay_scene()
                || native_context_scene
                || live_field_scene))
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
            "--stop-above-pc" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "--stop-above-pc requires an address".to_string())?;
                options.stop_above_pc = Some(parse_trace_u32(&raw, "--stop-above-pc")?);
            }
            "--stop-unmapped-pc" => {
                options.stop_unmapped_pc = true;
            }
            "--stop-unaligned-pc" => {
                options.stop_unaligned_pc = true;
            }
            "--stop-watch-access" => {
                options.stop_watch_access = true;
            }
            "--stop-watch-data" => {
                options.stop_watch_data = true;
            }
            "--stop-watch-write" => {
                options.stop_watch_write = true;
            }
            "--stop-watch-write-skip" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "--stop-watch-write-skip requires a count".to_string())?;
                options.stop_watch_write_skip = raw.parse::<u64>().map_err(|_| {
                    "--stop-watch-write-skip must be a non-negative integer".to_string()
                })?;
            }
            "--stop-cycles-elapsed" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "--stop-cycles-elapsed requires a cycle count".to_string())?;
                let limit = raw
                    .parse::<u64>()
                    .map_err(|_| "--stop-cycles-elapsed must be a positive integer".to_string())?;
                if limit == 0 {
                    return Err("--stop-cycles-elapsed must be greater than zero".to_string());
                }
                options.stop_cycles_elapsed = Some(limit);
            }
            "--stop-excode" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "--stop-excode requires an exception code".to_string())?;
                let excode = parse_trace_u32(&raw, "--stop-excode")?;
                if excode > 31 {
                    return Err("--stop-excode must be between 0 and 31".to_string());
                }
                options.stop_excode = Some(excode);
            }
            "--stop-pc-skip" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "--stop-pc-skip requires a count".to_string())?;
                options.stop_pc_skip = raw
                    .parse::<u64>()
                    .map_err(|_| "--stop-pc-skip must be a non-negative integer".to_string())?;
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

fn parse_native_watch_followup_trace_options(
    values: Vec<String>,
) -> Result<(NativeTraceConfig, NativeTraceConfig), String> {
    let mut trace_values = Vec::new();
    let mut followup_options = NativeTraceConfig::default();
    let mut explicit_followup_watch_only = false;
    let mut args = values.into_iter().peekable();

    while let Some(value) = args.next() {
        match value.as_str() {
            "--followup-watch" => {
                let raw_address = args
                    .next()
                    .ok_or_else(|| "--followup-watch requires an address".to_string())?;
                let address = parse_trace_u32(&raw_address, "--followup-watch address")?;
                let len = if args.peek().is_some_and(|next| !next.starts_with("--")) {
                    let raw_len = args.next().expect("peeked followup watch length");
                    parse_watch_len(&raw_len)?
                } else {
                    4
                };
                followup_options.watch_ranges.push((address, len));
            }
            "--followup-watch-only" => {
                followup_options.watch_only = true;
                explicit_followup_watch_only = true;
            }
            "--followup-no-watch-only" => {
                followup_options.watch_only = false;
                explicit_followup_watch_only = true;
            }
            "--followup-stop-watch-access" => {
                followup_options.stop_watch_access = true;
            }
            "--followup-stop-watch-data" => {
                followup_options.stop_watch_data = true;
            }
            "--followup-stop-watch-write" => {
                followup_options.stop_watch_write = true;
            }
            "--followup-stop-watch-write-skip" => {
                let raw = args.next().ok_or_else(|| {
                    "--followup-stop-watch-write-skip requires a count".to_string()
                })?;
                followup_options.stop_watch_write_skip = raw.parse::<u64>().map_err(|_| {
                    "--followup-stop-watch-write-skip must be a non-negative integer".to_string()
                })?;
            }
            "--followup-stop-pc" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "--followup-stop-pc requires an address".to_string())?;
                followup_options.stop_pc = Some(parse_trace_u32(&raw, "--followup-stop-pc")?);
            }
            _ => trace_values.push(value),
        }
    }

    let trace_options = parse_native_trace_options(trace_values)?;
    if followup_options.watch_ranges.is_empty() {
        followup_options.watch_ranges = trace_options.watch_ranges.clone();
    }
    if !explicit_followup_watch_only {
        followup_options.watch_only = true;
    }
    Ok((trace_options, followup_options))
}

fn native_trace_watch_ranges_json(ranges: &[(u32, u32)]) -> String {
    let items = ranges
        .iter()
        .map(|(address, len)| {
            format!(
                "{{\"address\":{},\"address_hex\":\"0x{:08x}\",\"len\":{}}}",
                address, address, len
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{}]", items)
}

fn parse_trace_u32(value: &str, label: &str) -> Result<u32, String> {
    parse_u32_auto(value)
        .map_err(|_| format!("{label} must be a u32 integer, optionally prefixed with 0x"))
}

fn parse_u16_auto_label(value: &str, label: &str) -> Result<u16, String> {
    let parsed = parse_u32_auto(value)
        .map_err(|_| format!("{label} must be a u16 integer, optionally prefixed with 0x"))?;
    u16::try_from(parsed)
        .map_err(|_| format!("{label} must fit in a u16 integer, optionally prefixed with 0x"))
}

fn parse_i32_auto_label(value: &str, label: &str) -> Result<i32, String> {
    let parsed = parse_i64_auto(value)
        .map_err(|_| format!("{label} must be an i32 integer, optionally prefixed with 0x"))?;
    i32::try_from(parsed)
        .map_err(|_| format!("{label} must fit in an i32 integer, optionally prefixed with 0x"))
}

fn parse_i64_auto_label(value: &str, label: &str) -> Result<i64, String> {
    parse_i64_auto(value)
        .map_err(|_| format!("{label} must be an i64 integer, optionally prefixed with 0x"))
}

fn parse_u64_auto_label(value: &str, label: &str) -> Result<u64, String> {
    parse_u64_auto(value)
        .map_err(|_| format!("{label} must be a u64 integer, optionally prefixed with 0x"))
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

fn parse_i64_auto(value: &str) -> Result<i64, std::num::ParseIntError> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        i64::from_str_radix(hex, 16)
    } else {
        value.parse::<i64>()
    }
}

fn parse_u64_auto(value: &str) -> Result<u64, std::num::ParseIntError> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16)
    } else {
        value.parse::<u64>()
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

fn doctor_report(config: MameConfig) -> String {
    let native_rom_source_is_file = config.rom_dir.is_file();
    let native_report = match NativeRomSet::scan(&config.rom_dir) {
        Ok(romset) => romset.compatibility_report().summary_json(),
        Err(error) => format!(
            "{{\"scan_ok\":false,\"error\":\"{}\"}}",
            escape_json(&error.to_string())
        ),
    };
    let mame_report = MameRuntime::new(config).doctor();

    format!(
        "{{\"mame\":{},\"native\":{{\"zip_supported\":{},\"rom_source_is_file\":{},\"rom_summary\":{}}}}}\n",
        mame_report.trim(),
        native_rom_source_is_file,
        native_rom_source_is_file,
        native_report
    )
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

fn native_fast_forward_instructions_per_frame(requested: u64) -> u64 {
    if requested == 0 {
        NATIVE_PLAY_FAST_FORWARD_INSTRUCTIONS_PER_FRAME
    } else {
        requested.max(NATIVE_PLAY_FAST_FORWARD_INSTRUCTIONS_PER_FRAME)
    }
}

fn native_snapshot_instructions_per_frame(requested: u64) -> u64 {
    native_fast_forward_instructions_per_frame(requested)
}

fn native_play_gui_instructions_per_frame(requested: u64) -> u64 {
    if requested == 0 {
        NATIVE_PLAY_GUI_DEFAULT_INSTRUCTIONS_PER_FRAME
    } else {
        requested
            .max(1)
            .min(NATIVE_PLAY_GUI_DEFAULT_INSTRUCTIONS_PER_FRAME)
    }
}

fn native_play_gui_poll_instruction_slice() -> u64 {
    env::var(NATIVE_PLAY_GUI_POLL_INSTRUCTION_SLICE_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(NATIVE_PLAY_GUI_POLL_INSTRUCTION_SLICE)
}

fn native_script_wall_timeout(stop_mode: NativeScriptStopMode) -> Option<Duration> {
    match stop_mode {
        NativeScriptStopMode::None => None,
        NativeScriptStopMode::Timed => {
            let timeout_secs = native_script_timed_wall_timeout_secs();
            (timeout_secs > 0).then(|| Duration::from_secs(timeout_secs))
        }
        NativeScriptStopMode::FastTimed => {
            let timeout_secs = native_script_timed_wall_timeout_secs();
            (timeout_secs > 0).then(|| Duration::from_secs(timeout_secs))
        }
        NativeScriptStopMode::InputProbe => {
            let timeout_secs = native_script_timed_wall_timeout_secs();
            (timeout_secs > 0).then(|| Duration::from_secs(timeout_secs))
        }
        NativeScriptStopMode::InputCheck => (NATIVE_INPUT_CHECK_WALL_TIMEOUT_SECS > 0)
            .then(|| Duration::from_secs(NATIVE_INPUT_CHECK_WALL_TIMEOUT_SECS)),
        NativeScriptStopMode::TitleScreen => (NATIVE_PLAYABLE_SCRIPT_WALL_TIMEOUT_SECS > 0)
            .then(|| Duration::from_secs(NATIVE_PLAYABLE_SCRIPT_WALL_TIMEOUT_SECS)),
        NativeScriptStopMode::HandoffFrame => (NATIVE_PLAYABLE_SCRIPT_WALL_TIMEOUT_SECS > 0)
            .then(|| Duration::from_secs(NATIVE_PLAYABLE_SCRIPT_WALL_TIMEOUT_SECS)),
        NativeScriptStopMode::PlayableCandidate => (NATIVE_PLAYABLE_SCRIPT_WALL_TIMEOUT_SECS > 0)
            .then(|| Duration::from_secs(NATIVE_PLAYABLE_SCRIPT_WALL_TIMEOUT_SECS)),
        NativeScriptStopMode::DrawCaptureRange { .. } => {
            let timeout_secs = native_script_timed_wall_timeout_secs();
            (timeout_secs > 0).then(|| Duration::from_secs(timeout_secs))
        }
    }
}

fn native_script_timed_wall_timeout_secs() -> u64 {
    env::var(NATIVE_SCRIPT_TIMED_WALL_TIMEOUT_SECS_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(NATIVE_SCRIPT_TIMED_WALL_TIMEOUT_SECS)
}

fn native_play_snapshot_wall_timeout(max_frames: u64) -> Duration {
    if let Some(timeout_secs) = env::var(NATIVE_PLAY_SNAPSHOT_WALL_TIMEOUT_SECS_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
    {
        return Duration::from_secs(timeout_secs);
    }

    let frame_budget_secs = max_frames
        .div_ceil(20)
        .max(1)
        .min(NATIVE_PLAY_SNAPSHOT_WALL_TIMEOUT_CAP_SECS);
    Duration::from_secs(NATIVE_PLAY_SNAPSHOT_TAIL_WALL_TIMEOUT_SECS.max(frame_budget_secs))
}

fn native_gap_probe_wall_timeout(max_frames: u64) -> Duration {
    if let Some(timeout_secs) = env::var(NATIVE_GAP_PROBE_WALL_TIMEOUT_SECS_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
    {
        return Duration::from_secs(timeout_secs);
    }

    let frame_budget_secs = max_frames
        .div_ceil(7)
        .max(NATIVE_PLAY_SNAPSHOT_TAIL_WALL_TIMEOUT_SECS)
        .min(NATIVE_GAP_PROBE_WALL_TIMEOUT_CAP_SECS);
    Duration::from_secs(frame_budget_secs)
}

fn native_match_snapshot_boot_wall_timeout(remaining: Duration) -> Duration {
    remaining.min(Duration::from_secs(
        NATIVE_PLAY_SNAPSHOT_MATCH_BOOT_WALL_TIMEOUT_SECS,
    ))
}

fn native_credit_mapping_probe_wall_timeout() -> Duration {
    let timeout_secs = env::var(NATIVE_CREDIT_MAPPING_PROBE_WALL_TIMEOUT_SECS_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(NATIVE_CREDIT_MAPPING_PROBE_WALL_TIMEOUT_SECS);
    Duration::from_secs(timeout_secs)
}

fn native_live_probe_wall_timeout() -> Option<Duration> {
    (NATIVE_LIVE_PROBE_WALL_TIMEOUT_SECS > 0)
        .then(|| Duration::from_secs(NATIVE_LIVE_PROBE_WALL_TIMEOUT_SECS))
}

#[allow(clippy::too_many_arguments)]
fn native_play_blocker_report_json(
    required_playable: bool,
    assets_complete: bool,
    exact_mame_assets: bool,
    input_activity: NativeInputActivity,
    handoff_playable: bool,
    handoff_render_verified: bool,
    final_stage_required: bool,
    final_native_playable_candidate: bool,
    final_render_verified: bool,
    final_frame_full_size: bool,
    final_frame_scene_accepted: bool,
    final_frame_gameplay_scene: bool,
    final_frame_handoff_scene: bool,
    require_gameplay_scene: bool,
    final_frame_stats: &NativeFrameStats,
    br2_credit_hle_accepted_seen: bool,
) -> String {
    let input_seen = input_activity.has_any_control_activity();
    let full_controls_seen = input_activity.has_full_control_activity();
    let credit_probe_seen = input_activity.has_credit_probe_activity();
    let credit_register_seen = input_activity.has_coin_register_active_activity();
    let coin_counter_seen =
        input_activity.coin_counter_0_edges > 0 || input_activity.coin_counter_1_edges > 0;
    let native_credit_adapter_seen = input_activity.has_native_credit_adapter_activity();
    let credit_scratchpad_projected = native_credit_adapter_seen;
    let credit_transition_seen = required_playable
        || handoff_playable
        || final_native_playable_candidate
        || final_frame_gameplay_scene;
    let credit_accepted_seen = br2_credit_hle_accepted_seen
        || credit_register_seen
        || coin_counter_seen
        || credit_transition_seen;
    let visible_content = final_frame_stats.has_visible_content();
    let render_ready_scene = final_frame_stats.has_render_ready_scene();
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();

    if !assets_complete {
        blockers.push("native_runtime_assets_incomplete");
    } else if !exact_mame_assets {
        warnings.push("noncanonical_runtime_assets");
    }

    if !input_seen {
        blockers.push("no_input_reads_observed");
    } else if final_stage_required && !credit_probe_seen {
        blockers.push("credit_coin_start_not_confirmed");
    }

    if final_stage_required && credit_probe_seen && !credit_accepted_seen {
        blockers.push("credit_input_observed_but_not_accepted");
    }
    if final_stage_required && !final_native_playable_candidate {
        blockers.push("native_playability_heuristic_not_met");
    }
    if final_stage_required && !final_frame_full_size {
        blockers.push("final_window_not_full_size");
    }
    if final_stage_required && !final_render_verified {
        blockers.push("final_render_not_verified");
    }
    if require_gameplay_scene && !final_frame_gameplay_scene {
        blockers.push("gameplay_scene_not_rendered");
    } else if final_stage_required && !final_frame_scene_accepted {
        blockers.push("scene_not_accepted");
    }
    if !final_stage_required && !handoff_playable {
        blockers.push("handoff_not_playable");
    }
    if input_seen && credit_probe_seen && !required_playable {
        blockers.push("input_observed_but_no_gameplay_transition");
    }
    if visible_content && !render_ready_scene && !final_frame_gameplay_scene {
        blockers.push("visible_but_sparse_or_title_only_frame");
    }
    if !visible_content {
        blockers.push("no_visible_frame_content");
    }

    let status = if required_playable {
        "pass"
    } else if input_seen || visible_content || handoff_render_verified {
        "partial"
    } else {
        "fail"
    };
    let primary_blocker = blockers.first().copied().unwrap_or("none");

    format!(
        "{{\"status\":\"{}\",\"primary_blocker\":\"{}\",\"blockers\":[{}],\"warnings\":[{}],\"credit_acceptance\":{{\"coin_counter_0_edges\":{},\"coin_counter_1_edges\":{},\"coin_register_reads\":{},\"coin_register_active_reads\":{},\"coin_register_writes\":{},\"native_credit_adapter_writes\":{},\"native_credit_adapter_edges\":{},\"credit_probe_seen\":{},\"credit_register_seen\":{},\"coin_counter_seen\":{},\"native_credit_adapter_seen\":{},\"credit_scratchpad_projected\":{},\"br2_credit_hle_accepted_seen\":{},\"credit_transition_seen\":{},\"game_transition_seen\":{},\"credit_accepted_seen\":{}}},\"gates\":{{\"required_playable\":{},\"assets_complete\":{},\"exact_mame_assets\":{},\"input_seen\":{},\"full_controls_seen\":{},\"credit_probe_seen\":{},\"handoff_playable\":{},\"handoff_render_verified\":{},\"final_stage_required\":{},\"final_native_playable_candidate\":{},\"final_render_verified\":{},\"final_frame_full_size\":{},\"final_frame_scene_accepted\":{},\"final_frame_gameplay_scene\":{},\"final_frame_handoff_scene\":{},\"require_gameplay_scene\":{},\"visible_content\":{},\"render_ready_scene\":{}}}}}",
        status,
        primary_blocker,
        native_json_string_array(&blockers),
        native_json_string_array(&warnings),
        input_activity.coin_counter_0_edges,
        input_activity.coin_counter_1_edges,
        input_activity.coin_register_reads,
        input_activity.coin_register_active_reads,
        input_activity.coin_register_writes,
        input_activity.native_credit_adapter_writes,
        input_activity.native_credit_adapter_edges,
        credit_probe_seen,
        credit_register_seen,
        coin_counter_seen,
        native_credit_adapter_seen,
        credit_scratchpad_projected,
        br2_credit_hle_accepted_seen,
        credit_transition_seen,
        credit_transition_seen,
        credit_accepted_seen,
        required_playable,
        assets_complete,
        exact_mame_assets,
        input_seen,
        full_controls_seen,
        credit_probe_seen,
        handoff_playable,
        handoff_render_verified,
        final_stage_required,
        final_native_playable_candidate,
        final_render_verified,
        final_frame_full_size,
        final_frame_scene_accepted,
        final_frame_gameplay_scene,
        final_frame_handoff_scene,
        require_gameplay_scene,
        visible_content,
        render_ready_scene
    )
}

fn native_json_string_array(values: &[&str]) -> String {
    values
        .iter()
        .map(|value| format!("\"{}\"", escape_json(value)))
        .collect::<Vec<_>>()
        .join(",")
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
        NATIVE_PLAY_TITLE_ENTRY_FRAMES, NativeFrameStats, NativeInputActivity, NativeInputLatch,
        NativeScriptSegment, NativeScriptStopMode, default_native_play_script,
        native_control_sweep_script, native_credit_adapter_ready, native_credit_gap_reason,
        native_credit_probe_ready, native_credit_ready, native_credit_register_ready,
        native_credit_system_port_ready, native_credit_transition_verified,
        native_fast_forward_instructions_per_frame, native_frame_checksum,
        native_gap_observed_frame_score, native_gap_probe_wall_timeout,
        native_health_checkpoint_script, native_health_match_entry_tail_script,
        native_health_stage_wall_timeout_with_reserve_for_elapsed,
        native_input_check_checkpoint_script, native_keys_to_buttons, native_match_entry_script,
        native_match_snapshot_boot_wall_timeout, native_play_blocker_report_json,
        native_play_boot_wait_segments, native_play_candidate_should_replace_stale_title,
        native_play_effective_buttons, native_play_frame_ready_with_context,
        native_play_gameplay_scene_with_context,
        native_play_gpu_verified_actual_window_frame_with_context,
        native_play_gpu_verified_frame_stats, native_play_gpu_verified_frame_stats_with_context,
        native_play_gui_handoff_frame_ready, native_play_gui_instructions_per_frame,
        native_play_repair_context_live_stage_window_frame,
        native_play_select_visible_window_frame, native_play_should_prefer_actual_display,
        native_play_should_prefer_gpu_verified_actual_display,
        native_play_should_prefer_raw_actual_display, native_play_snapshot_wall_timeout,
        native_play_title_screen_input_ready_from_frame, native_play_title_screen_ready_from_frame,
        native_play_window_frame, native_play_window_frame_with_context,
        native_remaining_wall_timeout_for_elapsed, native_script_completed,
        native_script_max_frames_for_segments, native_script_segment_is_credit_attempt,
        native_script_wall_timeout, native_snapshot_handoff_script,
        native_snapshot_instructions_per_frame, next_native_rom_source_or_default,
        next_scripted_action, parse_action_token, parse_native_autoplay_tail,
        parse_native_script_segments, parse_native_watch_followup_trace_options,
        parse_native_window_scale, remaining_native_script_segments,
        remaining_native_script_segments_after_title_ready,
    };
    use bloodyroar2_gym::{Action, ActionButtons, NativeDisplayFrame};
    use minifb::{Key, Scale};

    fn test_solid_frame(color: u32) -> NativeDisplayFrame {
        NativeDisplayFrame {
            width: 640,
            height: 480,
            pixels: vec![color; 640 * 480],
        }
    }

    fn test_gradient_playfield_frame() -> NativeDisplayFrame {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                let red = 24 + ((x * 17 + y * 5) % 176) as u32;
                let green = 40 + ((x * 7 + y * 19) % 184) as u32;
                let blue = 32 + ((x * 13 + y * 11) % 192) as u32;
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    fn test_texture_page_fragment_frame() -> NativeDisplayFrame {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 24..366 {
            for x in 0..width {
                if (x / 17 + y / 11) % 7 == 0 {
                    continue;
                }
                let palette_index = ((x / 13 + y / 9 + (x * y) / 2048) % 96) as u32;
                let red = 16 + ((palette_index * 5) % 160);
                let green = 24 + ((palette_index * 7) % 144);
                let blue = 32 + ((palette_index * 11) % 176);
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }

        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    fn test_contextual_live_stage_field_frame() -> NativeDisplayFrame {
        let width = 512;
        let height = 240;
        let mut pixels = vec![0; width * height];
        for y in 0..183 {
            for x in 0..width {
                if (x / 11 + y / 7 + (x * y) / 4096) % 10 < 3 {
                    continue;
                }
                let palette_index = ((x / 2 + y + (x * y) / 4096) % 96) as u32;
                let red = 16 + ((palette_index * 5) % 160);
                let green = 24 + ((palette_index * 7) % 144);
                let blue = 32 + ((palette_index * 11) % 176);
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        for y in 116..188 {
            for x in 324..506 {
                if (x + y) % 5 == 0 {
                    continue;
                }
                pixels[y * width + x] = if (x / 8 + y / 5) % 3 == 0 {
                    0x00e0_1810
                } else {
                    0x00d8_d8_d8
                };
            }
        }

        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    fn test_high_vertical_single_field_handoff_frame() -> NativeDisplayFrame {
        let width = 512;
        let height = 480;
        let mut pixels = vec![0; width * height];

        for y in 240..423 {
            for x in 0..width {
                if (x / 8 + y / 8) % 4 == 0 {
                    continue;
                }
                let palette_index = ((x / 13 + y / 9 + (x * y) / 8192) % 112) as u32;
                let red = 16 + ((palette_index * 5) % 160);
                let green = 24 + ((palette_index * 7) % 144);
                let blue = 32 + ((palette_index * 11) % 176);
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        for y in 355..410 {
            for x in 390..506 {
                pixels[y * width + x] = if (x + y) % 17 == 0 {
                    0x00f0_f0_f0
                } else {
                    0x00d8_2010
                };
            }
        }

        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    fn test_live_stage_field_with_title_like_hud_frame() -> NativeDisplayFrame {
        let width = 512;
        let height = 240;
        let mut pixels = vec![0; width * height];
        for y in 0..126 {
            for x in 0..width {
                if (x / 19 + y / 13 + (x * y) / 2048) % 17 == 0 {
                    continue;
                }
                let red = 8 + ((x * 17 + y * 13 + (x * y) / 97) % 192) as u32;
                let green = 24 + ((x * 11 + y * 23 + (x * y) / 131) % 184) as u32;
                let blue = 32 + ((x * 29 + y * 7 + (x * y) / 173) % 200) as u32;
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        for y in 70..122 {
            for x in 28..300 {
                if (x + y) % 7 != 0 {
                    let red = 16 + ((x * 5 + y * 3) % 96) as u32;
                    let green = 64 + ((x * 7 + y * 11) % 152) as u32;
                    let blue = 96 + ((x * 13 + y * 17) % 144) as u32;
                    pixels[y * width + x] = (red << 16) | (green << 8) | blue;
                }
            }
        }
        for y in 108..126 {
            for x in 268..506 {
                if (x / 5 + y / 4) % 5 == 0 {
                    pixels[y * width + x] = 0x00f0_f0_f0;
                } else if (x + y) % 11 == 0 {
                    pixels[y * width + x] = 0x00ff_e8_c0;
                } else {
                    pixels[y * width + x] = 0x00d8_1810;
                }
            }
        }

        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    fn test_repeating_tile_grid_artifact_frame() -> NativeDisplayFrame {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                if y >= 384 && (160..480).contains(&x) {
                    continue;
                }
                let tile_edge = x % 10 == 0 || y % 10 == 0;
                let palette_index = ((x / 10 + y / 10 + (x * y) / 8192) % 48) as u32;
                let color = if tile_edge {
                    if (x / 10 + y / 10) % 2 == 0 {
                        0x00e8_e8_dc
                    } else {
                        0x00b8_c8_d0
                    }
                } else {
                    let red = 96 + ((palette_index * 5) % 112);
                    let green = 72 + ((palette_index * 7) % 104);
                    let blue = 48 + ((palette_index * 11) % 96);
                    if (x + y) % 2 == 0 {
                        (red << 16) | (green << 8) | blue
                    } else {
                        (blue << 16) | (red << 8) | green
                    }
                };
                pixels[y * width + x] = color;
            }
        }
        for y in 232..392 {
            for x in 356..604 {
                if (x / 8 + y / 8) % 3 != 0 {
                    pixels[y * width + x] = 0x00f0_f0_e8;
                }
            }
        }

        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    fn test_dark_native_match_frame() -> NativeDisplayFrame {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..366 {
            for x in (32..608).step_by(23) {
                let blue = 32 + ((x * 5 + y * 3) % 112) as u32;
                pixels[y * width + x] = blue;
                if x + 1 < width {
                    pixels[y * width + x + 1] = blue << 1;
                }
            }
        }
        for y in 264..366 {
            for x in 40..596 {
                if (x + y) % 4 <= 2 {
                    let right_title_region = x >= width.saturating_mul(52) / 100;
                    let red = if right_title_region {
                        24 + ((x * 7 + y * 3) % 72) as u32
                    } else {
                        48 + ((x * 7 + y * 3) % 176) as u32
                    };
                    let green = 40 + ((x * 11 + y * 5) % 160) as u32;
                    let blue = 24 + ((x * 13 + y * 17) % 144) as u32;
                    pixels[y * width + x] = (red << 16) | (green << 8) | blue;
                }
            }
        }
        for y in 292..350 {
            for x in 160..326 {
                if (x * 3 + y * 7) % 4 != 0 {
                    let red = 160 + ((x + y) % 80) as u32;
                    let green = 92 + ((x * 5 + y) % 96) as u32;
                    let blue = 16 + ((x + y * 3) % 80) as u32;
                    pixels[y * width + x] = (red << 16) | (green << 8) | blue;
                }
            }
        }
        for y in 96..130 {
            for x in 256..384 {
                if (x + y) % 3 != 0 {
                    pixels[y * width + x] = 0x00f0_f0_f0;
                }
            }
        }
        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    fn test_letterboxed_native_stage_frame() -> NativeDisplayFrame {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];

        for y in 244..414 {
            for x in 76..566 {
                if (x / 17 + y / 13 + (x * y) / 4096) % 3 == 0 {
                    continue;
                }
                let title_region = x >= width * 52 / 100 && y >= height * 48 / 100;
                let red_base = if title_region { 8 } else { 24 };
                let red_span = if title_region { 40 } else { 96 };
                let red = red_base + ((x * 7 + y * 3 + (x * y) / 257) % red_span) as u32;
                let green = 44 + ((x * 11 + y * 5 + (x * y) / 193) % 160) as u32;
                let blue = 72 + ((x * 13 + y * 17 + (x * y) / 149) % 160) as u32;
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        for y in 254..286 {
            for x in 92..238 {
                if (x + y) % 4 != 0 {
                    let red = 16 + ((x * 3 + y * 5) % 64) as u32;
                    let green = 72 + ((x * 7 + y * 11) % 144) as u32;
                    let blue = 112 + ((x * 13 + y * 17) % 128) as u32;
                    pixels[y * width + x] = (red << 16) | (green << 8) | blue;
                }
            }
        }
        for y in 336..408 {
            for x in 88..552 {
                if (x / 8 + y / 5) % 5 <= 1 {
                    let title_region = x >= width * 52 / 100 && y >= height * 48 / 100;
                    let red = if title_region {
                        24 + ((x * 5 + y * 7) % 48) as u32
                    } else {
                        96 + ((x * 5 + y * 7) % 96) as u32
                    };
                    let green = 64 + ((x * 11 + y * 3) % 96) as u32;
                    let blue = 24 + ((x * 13 + y * 9) % 80) as u32;
                    pixels[y * width + x] = (red << 16) | (green << 8) | blue;
                }
            }
        }

        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    fn test_letterboxed_texture_like_live_stage_frame() -> NativeDisplayFrame {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];

        for y in 0..366 {
            for x in 18..622 {
                if (x / 18 + y / 14 + (x * y) / 8192) % 6 == 0 {
                    continue;
                }
                let palette_index = ((x / 14 + y / 11 + (x * y) / 8192) % 72) as u32;
                let red = 12 + ((palette_index * 3) % 96);
                let green = 40 + ((palette_index * 5) % 160);
                let blue = 56 + ((palette_index * 7) % 160);
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        for y in 176..318 {
            for x in 48..430 {
                if (x / 12 + y / 8) % 4 <= 1 {
                    let palette_index = ((x / 10 + y / 8 + (x * y) / 8192) % 48) as u32;
                    let red = 16 + ((palette_index * 3) % 88);
                    let green = 68 + ((palette_index * 5) % 144);
                    let blue = 96 + ((palette_index * 7) % 128);
                    pixels[y * width + x] = (red << 16) | (green << 8) | blue;
                }
            }
        }
        for y in 230..370 {
            for x in 486..624 {
                if (x / 6 + y / 5) % 3 == 0 {
                    pixels[y * width + x] = 0x00f0_f0_f0;
                } else if (x + y) % 3 != 0 {
                    pixels[y * width + x] = 0x00d8_2010;
                }
            }
        }

        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    fn test_letterboxed_stage_field_smear_frame() -> NativeDisplayFrame {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];

        for y in 0..366 {
            for x in 0..width {
                if (x / 8 + y / 8) % 4 == 0 {
                    continue;
                }
                let palette_index = ((x / 32 + y / 16 + (x * y) / 8192) % 72) as u32;
                let red = 12 + ((palette_index * 3) % 80);
                let green = 28 + ((palette_index * 5) % 112);
                let blue = 48 + ((palette_index * 7) % 128);
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        for y in 226..336 {
            for x in 40..430 {
                if (x / 20 + y / 12) % 5 <= 2 {
                    let palette_index = ((x / 18 + y / 11 + (x * y) / 16384) % 48) as u32;
                    let red = 16 + ((palette_index * 3) % 88);
                    let green = 68 + ((palette_index * 5) % 144);
                    let blue = 96 + ((palette_index * 7) % 128);
                    pixels[y * width + x] = (red << 16) | (green << 8) | blue;
                }
            }
        }
        for y in 250..292 {
            for x in 486..624 {
                if (x + y) % 2 == 0 {
                    pixels[y * width + x] = 0x00f0_f0_f0;
                } else {
                    pixels[y * width + x] = 0x00d8_2010;
                }
            }
        }

        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    fn test_sparse_texture_fragment_handoff_artifact_frame() -> NativeDisplayFrame {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];

        for y in 241..424 {
            for x in 24..520 {
                if (x / 11 + y / 7) % 5 == 0 {
                    continue;
                }
                let palette_index = ((x / 5 + y / 3 + (x * y) / 4096) % 96) as u32;
                let red = 4 + ((palette_index * 13) % 244);
                let green = 8 + ((palette_index * 7) % 112);
                let blue = 18 + ((palette_index * 17) % 224);
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        for y in 272..334 {
            for x in 338..526 {
                let band = ((x - 338) / 12) + ((y - 272) / 8);
                pixels[y * width + x] = match band % 5 {
                    0 => 0x00ff_5800,
                    1 => 0x0000_5cff,
                    2 => 0x0000_d020,
                    3 => 0x0000_1840,
                    _ => 0x0000_0000,
                };
            }
        }
        for y in 344..386 {
            for x in 508..628 {
                pixels[y * width + x] = if (x + y) % 3 == 0 {
                    0x00f0_f0_f0
                } else {
                    0x00c8_2010
                };
            }
        }
        for y in 392..424 {
            for x in 40..460 {
                pixels[y * width + x] = if (x / 8 + y / 4) % 2 == 0 {
                    0x0048_4040
                } else {
                    0x00d8_a010
                };
            }
        }

        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    fn test_title_screen_frame() -> NativeDisplayFrame {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        let colors = [
            0x00d0_2010,
            0x00ff_5030,
            0x00b8_1808,
            0x00ff_c080,
            0x00e0_4020,
            0x00ff_8040,
            0x00a0_1010,
            0x00f0_6028,
        ];
        for y in 245..305 {
            for x in 355..635 {
                let index = ((x / 7) + (y / 5)) % colors.len();
                pixels[y * width + x] = colors[index];
            }
        }
        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    fn test_sparse_bright_br2_title_frame() -> NativeDisplayFrame {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        let title_colors = [0x00ff_ffff, 0x00f0_f0_f0, 0x00d8_d8_d8, 0x00c8_c8_c8];
        for y in 245..269 {
            for x in 500..580 {
                let index = ((x / 3) + (y / 2)) % title_colors.len();
                pixels[y * width + x] = title_colors[index];
            }
        }
        let logo_colors = [
            0x00ff_ffff,
            0x00f0_8010,
            0x0030_50ff,
            0x00f8_f8_c0,
            0x00d8_d8_d8,
            0x00ff_a040,
            0x0040_70ff,
            0x00c0_c0_c0,
        ];
        for y in 300..330 {
            for x in 170..470 {
                if (x + y) % 5 != 0 {
                    let index = ((x / 5) + (y / 3)) % logo_colors.len();
                    pixels[y * width + x] = logo_colors[index];
                }
            }
        }
        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    fn test_corrupt_sparse_title_texture_fragment_frame() -> NativeDisplayFrame {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..120 {
                let palette_index = ((x / 3 + y / 2 + (x * y) / 1024) % 96) as u32;
                let red = 96 + ((palette_index * 7) % 160);
                let green = 8 + ((palette_index * 5) % 80);
                let blue = 16 + ((palette_index * 11) % 144);
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        for y in 245..269 {
            for x in 500..580 {
                let color = match (x / 3 + y / 2) % 4 {
                    0 => 0x00ff_ffff,
                    1 => 0x00f0_f0_f0,
                    2 => 0x00d8_d8_d8,
                    _ => 0x00c8_c8_c8,
                };
                pixels[y * width + x] = color;
            }
        }

        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    fn test_caption_warning_with_right_bright_text_frame() -> NativeDisplayFrame {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 245..269 {
            for x in 500..580 {
                pixels[y * width + x] = 0x00f0_f0_f0;
            }
        }
        for y in 390..414 {
            for x in (80..560).step_by(3) {
                pixels[y * width + x] = 0x00ff_ffff;
                pixels[y * width + x + 1] = 0x00d8_d8_d8;
            }
        }
        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

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
    fn watch_followup_trace_options_inherit_primary_watch_by_default() {
        let (trace, followup) = parse_native_watch_followup_trace_options(vec![
            "--watch".to_string(),
            "0x1fa00000".to_string(),
            "4".to_string(),
            "--stop-watch-access".to_string(),
        ])
        .expect("options parse");

        assert_eq!(trace.watch_ranges, vec![(0x1fa0_0000, 4)]);
        assert!(trace.stop_watch_access);
        assert_eq!(followup.watch_ranges, vec![(0x1fa0_0000, 4)]);
        assert!(followup.watch_only);
        assert!(!followup.stop_watch_access);
        assert!(!followup.stop_watch_write);
    }

    #[test]
    fn watch_followup_trace_options_allow_write_only_followup_watch() {
        let (trace, followup) = parse_native_watch_followup_trace_options(vec![
            "--watch".to_string(),
            "0x1fa00000".to_string(),
            "4".to_string(),
            "--followup-watch".to_string(),
            "0x1f800068".to_string(),
            "0x20".to_string(),
            "--followup-stop-watch-write".to_string(),
            "--followup-stop-watch-write-skip".to_string(),
            "2".to_string(),
        ])
        .expect("options parse");

        assert_eq!(trace.watch_ranges, vec![(0x1fa0_0000, 4)]);
        assert_eq!(followup.watch_ranges, vec![(0x1f80_0068, 0x20)]);
        assert!(followup.watch_only);
        assert!(followup.stop_watch_write);
        assert_eq!(followup.stop_watch_write_skip, 2);
    }

    #[test]
    fn rejects_invalid_script_segments() {
        assert!(parse_native_script_segments(Vec::new()).is_err());
        assert!(parse_native_script_segments(vec!["coin".to_string()]).is_err());
        assert!(parse_native_script_segments(vec!["coin:0".to_string()]).is_err());
        assert!(parse_action_token("999").is_err());
    }

    #[test]
    fn optional_native_rom_source_leaves_numeric_first_args_for_command_parsers() {
        let mut args = vec![
            "500000".to_string(),
            "tmp/native-validation/smoke".to_string(),
            "coin:20".to_string(),
        ]
        .into_iter()
        .peekable();

        let _rom = next_native_rom_source_or_default(&mut args);

        assert_eq!(args.next().as_deref(), Some("500000"));
        assert_eq!(args.next().as_deref(), Some("tmp/native-validation/smoke"));
    }

    #[test]
    fn parses_native_autoplay_tail_with_default_and_custom_script() {
        let (max_frames, default_segments) =
            parse_native_autoplay_tail(Vec::new()).expect("default autoplay parses");
        assert_eq!(max_frames, None);
        assert_eq!(default_segments, native_match_entry_script());

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
    fn native_window_scale_defaults_to_uncropped_x1() {
        assert!(matches!(parse_native_window_scale(None), Ok(Scale::X1)));
        assert!(matches!(
            parse_native_window_scale(Some("fit".to_string())),
            Ok(Scale::FitScreen)
        ));
    }

    #[test]
    fn native_fast_forward_uses_stable_vblank_instruction_budget() {
        assert_eq!(
            native_fast_forward_instructions_per_frame(0),
            super::NATIVE_PLAY_FAST_FORWARD_INSTRUCTIONS_PER_FRAME
        );
        assert_eq!(
            native_fast_forward_instructions_per_frame(10_000),
            super::NATIVE_PLAY_FAST_FORWARD_INSTRUCTIONS_PER_FRAME
        );
        assert_eq!(
            native_fast_forward_instructions_per_frame(120_000),
            super::NATIVE_PLAY_FAST_FORWARD_INSTRUCTIONS_PER_FRAME
        );
        assert_eq!(native_fast_forward_instructions_per_frame(500_000), 500_000);
        assert_eq!(native_fast_forward_instructions_per_frame(600_000), 600_000);
    }

    #[test]
    fn native_snapshot_uses_stable_fast_forward_instruction_budget() {
        assert_eq!(
            native_snapshot_instructions_per_frame(0),
            super::NATIVE_PLAY_FAST_FORWARD_INSTRUCTIONS_PER_FRAME
        );
        assert_eq!(
            native_snapshot_instructions_per_frame(120_000),
            super::NATIVE_PLAY_FAST_FORWARD_INSTRUCTIONS_PER_FRAME
        );
        assert_eq!(native_snapshot_instructions_per_frame(500_000), 500_000);
    }

    #[test]
    fn native_play_gui_caps_requested_instruction_budget_for_responsiveness() {
        assert_eq!(
            native_play_gui_instructions_per_frame(0),
            super::NATIVE_PLAY_GUI_DEFAULT_INSTRUCTIONS_PER_FRAME
        );
        assert_eq!(native_play_gui_instructions_per_frame(1), 1);
        assert_eq!(native_play_gui_instructions_per_frame(10_000), 10_000);
        assert_eq!(native_play_gui_instructions_per_frame(120_000), 120_000);
        assert_eq!(
            native_play_gui_instructions_per_frame(600_000),
            super::NATIVE_PLAY_GUI_DEFAULT_INSTRUCTIONS_PER_FRAME
        );
    }

    #[test]
    fn native_play_boot_wait_fast_forwards_warning_skip_before_credit() {
        let segments = super::native_manual_entry_script();
        let boot_wait = native_play_boot_wait_segments(&segments);
        let warning_skip = super::native_warning_skip_to_title_script();

        assert_eq!(boot_wait, warning_skip);
        assert!(
            boot_wait
                .iter()
                .any(|segment| segment.action == Action::Start)
        );
        assert!(
            boot_wait
                .iter()
                .any(|segment| segment.action == Action::Punch)
        );
        assert_eq!(
            boot_wait.last().copied(),
            Some(NativeScriptSegment {
                action: Action::Noop,
                frames: NATIVE_PLAY_TITLE_ENTRY_FRAMES
            })
        );
        assert_eq!(segments[boot_wait.len()].action, Action::Coin);
    }

    #[test]
    fn native_title_screen_ready_accepts_sparse_insert_coin_title_frame() {
        let title = test_title_screen_frame();
        let blank = test_solid_frame(0);
        let gameplay = test_gradient_playfield_frame();

        assert!(super::native_play_title_screen_ready_from_frame(&title));
        assert!(!super::native_play_title_screen_ready_from_frame(&blank));
        assert!(!super::native_play_title_screen_ready_from_frame(&gameplay));
    }

    #[test]
    fn native_title_screen_frame_is_not_render_or_gameplay_ready() {
        let title = test_title_screen_frame();
        let stats = NativeFrameStats::from_frame(&title);

        assert!(stats.has_title_screen_frame(), "{stats:?}");
        assert!(super::native_play_title_screen_ready_from_frame(&title));
        assert!(!stats.has_render_ready_scene(), "{stats:?}");
        assert!(!stats.has_handoff_scene(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
        assert!(
            !super::native_play_frame_ready_with_context(true, &title),
            "{stats:?}"
        );
        assert!(
            !super::native_play_gameplay_scene_with_context(true, stats),
            "{stats:?}"
        );
    }

    #[test]
    fn native_title_screen_ready_accepts_sparse_bright_br2_title_frame() {
        let title = test_sparse_bright_br2_title_frame();
        let stats = NativeFrameStats::from_frame(&title);

        assert!(
            stats.right_title_bright_pixels.saturating_mul(200) >= stats.total_pixels,
            "{stats:?}"
        );
        assert!(super::native_play_title_screen_ready_from_frame(&title));
    }

    #[test]
    fn native_title_screen_ready_rejects_corrupt_sparse_texture_fragment() {
        let safe = test_title_screen_frame();
        let corrupt = test_corrupt_sparse_title_texture_fragment_frame();
        let stats = NativeFrameStats::from_frame(&corrupt);

        assert!(stats.has_title_screen_frame(), "{stats:?}");
        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(stats.has_corrupt_title_texture_fragment(), "{stats:?}");
        assert!(stats.has_blocking_display_artifact(), "{stats:?}");
        assert!(
            native_play_title_screen_input_ready_from_frame(&corrupt),
            "{stats:?}"
        );
        assert!(
            !super::native_play_title_screen_ready_from_frame(&corrupt),
            "{stats:?}"
        );
        assert!(!super::native_play_presentable_frame(&corrupt), "{stats:?}");

        let visible = super::native_play_visible_window_frame(corrupt.clone(), &Some(safe.clone()));
        assert_eq!(visible.pixels, safe.pixels);
    }

    #[test]
    fn native_title_screen_ready_rejects_caption_warning_frame() {
        let warning = test_caption_warning_with_right_bright_text_frame();
        let stats = NativeFrameStats::from_frame(&warning);

        assert!(
            stats.right_title_bright_pixels.saturating_mul(200) >= stats.total_pixels,
            "{stats:?}"
        );
        assert!(stats.has_intro_caption_band(), "{stats:?}");
        assert!(!super::native_play_title_screen_ready_from_frame(&warning));
    }

    #[test]
    fn native_title_wait_early_advance_only_applies_to_long_noop_timed_modes() {
        let long_noop = NativeScriptSegment {
            action: Action::Noop,
            frames: super::NATIVE_PLAY_TITLE_EARLY_ADVANCE_MIN_SEGMENT_FRAMES,
        };
        let short_noop = NativeScriptSegment {
            action: Action::Noop,
            frames: super::NATIVE_PLAY_TITLE_EARLY_ADVANCE_MIN_SEGMENT_FRAMES - 1,
        };
        let long_start = NativeScriptSegment {
            action: Action::Start,
            frames: super::NATIVE_PLAY_TITLE_EARLY_ADVANCE_MIN_SEGMENT_FRAMES,
        };

        assert!(super::native_script_can_early_advance_title_wait(
            long_noop,
            super::NATIVE_WARNING_SKIP_GAP_FRAMES,
            NativeScriptStopMode::Timed,
        ));
        assert!(super::native_script_can_early_advance_title_wait(
            long_noop,
            super::NATIVE_WARNING_SKIP_GAP_FRAMES,
            NativeScriptStopMode::TitleScreen,
        ));
        assert!(!super::native_script_can_early_advance_title_wait(
            long_noop,
            super::NATIVE_WARNING_SKIP_GAP_FRAMES - 1,
            NativeScriptStopMode::Timed,
        ));
        assert!(!super::native_script_can_early_advance_title_wait(
            short_noop,
            super::NATIVE_WARNING_SKIP_GAP_FRAMES,
            NativeScriptStopMode::Timed,
        ));
        assert!(!super::native_script_can_early_advance_title_wait(
            long_start,
            super::NATIVE_WARNING_SKIP_GAP_FRAMES,
            NativeScriptStopMode::Timed,
        ));
        assert!(!super::native_script_can_early_advance_title_wait(
            long_noop,
            super::NATIVE_WARNING_SKIP_GAP_FRAMES,
            NativeScriptStopMode::None,
        ));
        assert!(!super::native_script_can_early_advance_title_wait(
            long_noop,
            super::NATIVE_WARNING_SKIP_GAP_FRAMES,
            NativeScriptStopMode::DrawCaptureRange { sequence_end: 1 },
        ));
    }

    #[test]
    fn native_health_rendering_gap_reason_prioritizes_input_ok_render_gap() {
        assert_eq!(
            super::native_health_rendering_gap_reason(true, false, false, false, true),
            "gpu_scene_not_rendered"
        );
        assert_eq!(
            super::native_health_rendering_gap_reason(true, true, false, false, true),
            "full_scene_not_composed"
        );
        assert_eq!(
            super::native_health_rendering_gap_reason(false, false, false, false, true),
            "no_rendering_present"
        );
    }

    #[test]
    fn native_credit_ready_requires_register_or_transition_signal() {
        let probed = NativeInputActivity {
            system_coin_active_reads: 1,
            system_start_active_reads: 1,
            coin_insert_edges: 1,
            legacy_system_coin_latch_edges: 1,
            legacy_system_start_latch_edges: 1,
            ..NativeInputActivity::default()
        };

        assert!(native_credit_probe_ready(probed));
        assert!(!native_credit_register_ready(probed));
        assert!(!native_credit_transition_verified(true, false, false));
        assert!(native_credit_system_port_ready(true, probed, true));
        assert!(!native_credit_ready(true, false, false, false));
        assert_eq!(
            native_credit_gap_reason(true, probed, true, false, false, true),
            "system_port_seen_but_credit_transition_not_verified"
        );

        let system_port_ready = NativeInputActivity {
            coin_register_writes: 1,
            ..probed
        };
        assert!(native_credit_system_port_ready(
            true,
            system_port_ready,
            true
        ));
        assert_eq!(
            native_credit_gap_reason(true, system_port_ready, true, false, false, true),
            "system_port_seen_but_credit_transition_not_verified"
        );
        assert!(!native_credit_ready(true, false, false, false));
        assert_eq!(
            native_credit_gap_reason(true, system_port_ready, true, false, false, false),
            "coin_register_or_transition_not_verified"
        );

        let adapter_ready = NativeInputActivity {
            native_credit_adapter_writes: 1,
            native_credit_adapter_edges: 1,
            ..probed
        };
        assert!(native_credit_adapter_ready(adapter_ready));
        assert!(!native_credit_ready(true, false, false, true));
        assert_eq!(
            native_credit_gap_reason(true, adapter_ready, true, false, false, true),
            "credit_scratchpad_projected_but_not_accepted"
        );

        let register_ready = NativeInputActivity {
            coin_register_reads: 1,
            coin_register_active_reads: 1,
            ..probed
        };
        assert!(native_credit_register_ready(register_ready));
        assert!(native_credit_ready(true, true, false, false));
        assert!(native_credit_ready(true, false, true, false));
        assert_eq!(
            native_credit_gap_reason(true, register_ready, true, true, false, false),
            "none"
        );
        assert_eq!(
            native_credit_gap_reason(true, register_ready, true, true, true, false),
            "none"
        );
    }

    #[test]
    fn native_play_blocker_report_marks_input_without_gameplay_transition() {
        let frame = test_solid_frame(0x0030_1008);
        let stats = NativeFrameStats::from_frame(&frame);
        let input_activity = NativeInputActivity {
            system_coin_active_reads: 4,
            system_start_active_reads: 4,
            coin_insert_edges: 1,
            legacy_system_coin_latch_edges: 1,
            legacy_system_start_latch_edges: 1,
            ..NativeInputActivity::default()
        };

        let report = native_play_blocker_report_json(
            false,
            true,
            false,
            input_activity,
            false,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            false,
            true,
            &stats,
            false,
        );

        assert!(report.contains("\"status\":\"partial\""));
        assert!(report.contains("\"primary_blocker\":\"credit_input_observed_but_not_accepted\""));
        assert!(report.contains("\"noncanonical_runtime_assets\""));
        assert!(report.contains("\"credit_acceptance\""));
        assert!(report.contains("\"credit_accepted_seen\":false"));
        assert!(report.contains("\"input_observed_but_no_gameplay_transition\""));
    }

    #[test]
    fn native_play_blocker_report_does_not_treat_adapter_projection_as_credit_acceptance() {
        let frame = test_solid_frame(0x0030_1008);
        let stats = NativeFrameStats::from_frame(&frame);
        let input_activity = NativeInputActivity {
            system_coin_active_reads: 4,
            system_start_active_reads: 4,
            coin_insert_edges: 1,
            native_credit_adapter_writes: 1,
            native_credit_adapter_edges: 1,
            ..NativeInputActivity::default()
        };

        let report = native_play_blocker_report_json(
            false,
            true,
            false,
            input_activity,
            false,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            false,
            true,
            &stats,
            false,
        );

        assert!(report.contains("\"native_credit_adapter_seen\":true"));
        assert!(report.contains("\"credit_scratchpad_projected\":true"));
        assert!(report.contains("\"game_transition_seen\":false"));
        assert!(report.contains("\"credit_accepted_seen\":false"));
        assert!(report.contains("\"credit_input_observed_but_not_accepted\""));
        assert!(report.contains("\"native_playability_heuristic_not_met\""));
    }

    #[test]
    fn native_play_blocker_report_treats_br2_credit_hle_as_credit_acceptance() {
        let frame = test_solid_frame(0x0030_1008);
        let stats = NativeFrameStats::from_frame(&frame);
        let input_activity = NativeInputActivity {
            system_coin_active_reads: 4,
            system_start_active_reads: 4,
            coin_insert_edges: 1,
            native_credit_adapter_writes: 1,
            native_credit_adapter_edges: 1,
            ..NativeInputActivity::default()
        };

        let report = native_play_blocker_report_json(
            false,
            true,
            false,
            input_activity,
            false,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            false,
            true,
            &stats,
            true,
        );

        assert!(report.contains("\"br2_credit_hle_accepted_seen\":true"));
        assert!(report.contains("\"credit_accepted_seen\":true"));
        assert!(!report.contains("credit_input_observed_but_not_accepted"));
    }

    #[test]
    fn native_play_blocker_report_passes_when_required_gates_pass() {
        let frame = test_gradient_playfield_frame();
        let stats = NativeFrameStats::from_frame(&frame);
        let input_activity = NativeInputActivity {
            p1_up_active_reads: 1,
            p1_down_active_reads: 1,
            p1_left_active_reads: 1,
            p1_right_active_reads: 1,
            p1_punch_active_reads: 1,
            p1_kick_active_reads: 1,
            p1_beast_active_reads: 1,
            p3_guard_active_reads: 1,
            system_coin_active_reads: 1,
            system_start_active_reads: 1,
            coin_insert_edges: 1,
            legacy_system_coin_latch_edges: 1,
            legacy_system_start_latch_edges: 1,
            ..NativeInputActivity::default()
        };

        let report = native_play_blocker_report_json(
            true,
            true,
            true,
            input_activity,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            false,
            true,
            &stats,
            false,
        );

        assert!(report.contains("\"status\":\"pass\""));
        assert!(report.contains("\"primary_blocker\":\"none\""));
        assert!(report.contains("\"blockers\":[]"));
        assert!(report.contains("\"credit_accepted_seen\":true"));
        assert!(report.contains("\"full_controls_seen\":true"));
    }

    #[test]
    fn explicit_native_script_runs_without_wall_timeout() {
        assert_eq!(native_script_wall_timeout(NativeScriptStopMode::None), None);
        assert_eq!(
            native_script_wall_timeout(NativeScriptStopMode::Timed),
            Some(std::time::Duration::from_secs(
                super::NATIVE_SCRIPT_TIMED_WALL_TIMEOUT_SECS
            ))
        );
        assert_eq!(
            native_script_wall_timeout(NativeScriptStopMode::FastTimed),
            native_script_wall_timeout(NativeScriptStopMode::Timed)
        );
        assert_eq!(
            native_script_wall_timeout(NativeScriptStopMode::InputProbe),
            native_script_wall_timeout(NativeScriptStopMode::Timed)
        );
        assert!(super::native_script_stop_mode_uses_diagnostics(
            NativeScriptStopMode::Timed
        ));
        assert!(!super::native_script_stop_mode_uses_diagnostics(
            NativeScriptStopMode::FastTimed
        ));
        assert_eq!(
            native_script_wall_timeout(NativeScriptStopMode::InputCheck),
            Some(std::time::Duration::from_secs(
                super::NATIVE_INPUT_CHECK_WALL_TIMEOUT_SECS
            ))
        );
        assert_eq!(
            native_script_wall_timeout(NativeScriptStopMode::TitleScreen),
            Some(std::time::Duration::from_secs(
                super::NATIVE_PLAYABLE_SCRIPT_WALL_TIMEOUT_SECS
            ))
        );
        assert_eq!(
            native_script_wall_timeout(NativeScriptStopMode::HandoffFrame),
            Some(std::time::Duration::from_secs(
                super::NATIVE_PLAYABLE_SCRIPT_WALL_TIMEOUT_SECS
            ))
        );
        assert_eq!(
            native_script_wall_timeout(NativeScriptStopMode::PlayableCandidate),
            Some(std::time::Duration::from_secs(
                super::NATIVE_PLAYABLE_SCRIPT_WALL_TIMEOUT_SECS
            ))
        );
    }

    #[test]
    fn native_health_match_entry_timeout_extends_playability_probe_only() {
        const {
            assert!(
                super::NATIVE_HEALTH_MATCH_ENTRY_WALL_TIMEOUT_SECS
                    > super::NATIVE_PLAYABLE_SCRIPT_WALL_TIMEOUT_SECS
            );
        }
        assert_eq!(
            native_script_wall_timeout(NativeScriptStopMode::PlayableCandidate),
            Some(std::time::Duration::from_secs(
                super::NATIVE_PLAYABLE_SCRIPT_WALL_TIMEOUT_SECS
            ))
        );
    }

    #[test]
    fn native_health_match_entry_timeout_reserves_control_sweep_budget() {
        let timeout = native_health_stage_wall_timeout_with_reserve_for_elapsed(
            std::time::Duration::from_secs(120),
            std::time::Duration::from_secs(super::NATIVE_HEALTH_CHECK_WALL_TIMEOUT_SECS),
            std::time::Duration::from_secs(super::NATIVE_HEALTH_MATCH_ENTRY_WALL_TIMEOUT_SECS),
            std::time::Duration::from_secs(super::NATIVE_HEALTH_CONTROL_SWEEP_RESERVE_SECS),
        );
        assert_eq!(
            timeout,
            Some(std::time::Duration::from_secs(
                super::NATIVE_HEALTH_MATCH_ENTRY_WALL_TIMEOUT_SECS
            ))
        );

        let exhausted = native_health_stage_wall_timeout_with_reserve_for_elapsed(
            std::time::Duration::from_secs(
                super::NATIVE_HEALTH_CHECK_WALL_TIMEOUT_SECS
                    - super::NATIVE_HEALTH_CONTROL_SWEEP_RESERVE_SECS,
            ),
            std::time::Duration::from_secs(super::NATIVE_HEALTH_CHECK_WALL_TIMEOUT_SECS),
            std::time::Duration::from_secs(super::NATIVE_HEALTH_MATCH_ENTRY_WALL_TIMEOUT_SECS),
            std::time::Duration::from_secs(super::NATIVE_HEALTH_CONTROL_SWEEP_RESERVE_SECS),
        );
        assert_eq!(exhausted, None);
    }

    #[test]
    fn native_play_snapshot_wall_timeout_scales_with_frame_budget() {
        assert_eq!(
            native_play_snapshot_wall_timeout(1),
            std::time::Duration::from_secs(super::NATIVE_PLAY_SNAPSHOT_TAIL_WALL_TIMEOUT_SECS)
        );
        assert_eq!(
            native_play_snapshot_wall_timeout(6_000),
            std::time::Duration::from_secs(300)
        );
        assert_eq!(
            native_play_snapshot_wall_timeout(12_000),
            std::time::Duration::from_secs(super::NATIVE_PLAY_SNAPSHOT_WALL_TIMEOUT_CAP_SECS)
        );
    }

    #[test]
    fn native_match_snapshot_boot_timeout_reserves_tail_budget() {
        assert_eq!(
            native_match_snapshot_boot_wall_timeout(std::time::Duration::from_secs(360)),
            std::time::Duration::from_secs(
                super::NATIVE_PLAY_SNAPSHOT_MATCH_BOOT_WALL_TIMEOUT_SECS
            )
        );
        assert!(
            super::NATIVE_PLAY_SNAPSHOT_MATCH_BOOT_WALL_TIMEOUT_SECS
                < super::NATIVE_PLAY_SNAPSHOT_TAIL_WALL_TIMEOUT_SECS
        );
    }

    #[test]
    fn native_gap_probe_wall_timeout_uses_wider_runtime_budget() {
        assert_eq!(
            native_gap_probe_wall_timeout(1),
            std::time::Duration::from_secs(super::NATIVE_PLAY_SNAPSHOT_TAIL_WALL_TIMEOUT_SECS)
        );
        assert_eq!(
            native_gap_probe_wall_timeout(2_200),
            std::time::Duration::from_secs(315)
        );
        assert_eq!(
            native_gap_probe_wall_timeout(20_000),
            std::time::Duration::from_secs(super::NATIVE_GAP_PROBE_WALL_TIMEOUT_CAP_SECS)
        );
    }

    #[test]
    fn match_snapshot_boot_timeout_leaves_tail_budget() {
        let long_remaining = std::time::Duration::from_secs(240);
        assert_eq!(
            native_match_snapshot_boot_wall_timeout(long_remaining),
            std::time::Duration::from_secs(
                super::NATIVE_PLAY_SNAPSHOT_MATCH_BOOT_WALL_TIMEOUT_SECS
            )
        );

        let short_remaining = std::time::Duration::from_secs(7);
        assert_eq!(
            native_match_snapshot_boot_wall_timeout(short_remaining),
            short_remaining
        );
    }

    #[test]
    fn native_credit_mapping_probe_wall_timeout_uses_default_budget() {
        assert_eq!(
            super::native_credit_mapping_probe_wall_timeout(),
            std::time::Duration::from_secs(super::NATIVE_CREDIT_MAPPING_PROBE_WALL_TIMEOUT_SECS)
        );
    }

    #[test]
    fn native_script_replay_interval_is_throttled_for_snapshot_runtime() {
        assert_eq!(
            super::NATIVE_SCRIPT_UNLINKED_PRIMITIVE_REPLAY_INTERVAL,
            Some(5)
        );
    }

    #[test]
    fn native_remaining_wall_timeout_returns_none_at_or_after_deadline() {
        assert_eq!(
            native_remaining_wall_timeout_for_elapsed(
                std::time::Duration::from_secs(2),
                std::time::Duration::from_secs(5)
            ),
            Some(std::time::Duration::from_secs(3))
        );
        assert_eq!(
            native_remaining_wall_timeout_for_elapsed(
                std::time::Duration::from_secs(5),
                std::time::Duration::from_secs(5)
            ),
            None
        );
        assert_eq!(
            native_remaining_wall_timeout_for_elapsed(
                std::time::Duration::from_secs(6),
                std::time::Duration::from_secs(5)
            ),
            None
        );
    }

    #[test]
    fn native_script_frame_budget_covers_full_match_entry_script() {
        let segments = native_match_entry_script();
        let script_frames = segments.iter().map(|segment| segment.frames).sum::<u64>();

        assert!(script_frames <= super::NATIVE_PLAY_FAST_FORWARD_MAX_FRAMES);
        assert_eq!(
            native_script_max_frames_for_segments(&segments),
            script_frames + super::NATIVE_PLAY_HANDOFF_SETTLE_FRAMES
        );
    }

    #[test]
    fn default_native_play_script_reaches_match_handoff() {
        let segments = default_native_play_script();
        let snapshot = super::native_snapshot_match_entry_script();

        assert_eq!(segments, snapshot);
        assert!(segments.starts_with(&super::native_warning_skip_to_title_script()));
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
        assert_eq!(
            segments
                .iter()
                .filter(|segment| segment.action == Action::Punch)
                .count(),
            5
        );
        assert!(
            segments
                .iter()
                .any(|segment| matches!(segment.action, Action::CoinStart | Action::Kick))
        );
        assert!(
            segments
                .iter()
                .any(|segment| matches!(segment.action, Action::Right | Action::Down))
        );
        for action in [Action::Up, Action::Down, Action::Left, Action::Right] {
            assert!(
                segments.iter().any(|segment| segment.action == action),
                "missing direction action {action:?}"
            );
        }
        for action in [Action::Punch, Action::Kick, Action::Beast, Action::Guard] {
            assert!(
                segments.iter().any(|segment| segment.action == action),
                "missing attack action {action:?}"
            );
        }
        assert_eq!(segments.last().expect("last segment").action, Action::Noop);
        assert_eq!(segments.last().expect("last segment").frames, 240);
        assert!(
            segments.iter().map(|segment| segment.frames).sum::<u64>()
                < native_match_entry_script()
                    .iter()
                    .map(|segment| segment.frames)
                    .sum::<u64>()
        );
        assert_eq!(
            segments.last().copied(),
            Some(NativeScriptSegment {
                action: Action::Noop,
                frames: 240,
            })
        );
    }

    #[test]
    fn native_snapshot_match_script_uses_bounded_entry_controls() {
        let segments = native_snapshot_handoff_script(true);
        let baseline = native_match_entry_script();
        let snapshot = super::native_snapshot_match_entry_script();

        assert_eq!(segments, snapshot);
        assert!(segments.starts_with(&super::native_warning_skip_to_title_script()));
        assert!(
            native_script_max_frames_for_segments(&segments)
                < native_script_max_frames_for_segments(&baseline)
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.action == Action::Right)
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.action == Action::Down)
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.action == Action::Left)
        );
        assert!(segments.iter().any(|segment| segment.action == Action::Up));
        assert!(
            segments
                .iter()
                .any(|segment| segment.action == Action::Beast)
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.action == Action::Guard)
        );
        assert!(
            segments
                .iter()
                .filter(|segment| segment.action == Action::Punch)
                .count()
                >= baseline
                    .iter()
                    .filter(|segment| segment.action == Action::Punch)
                    .count()
        );
        assert_eq!(
            native_snapshot_handoff_script(false),
            default_native_play_script()
        );
    }

    #[test]
    fn native_warning_skip_delays_credit_until_title_handoff_window() {
        let segments = super::native_manual_entry_script();
        let boot_wait_frames = native_play_boot_wait_segments(&segments)
            .iter()
            .map(|segment| segment.frames)
            .sum::<u64>();

        assert!(
            (1_800..=2_100).contains(&boot_wait_frames),
            "warning skip should avoid sending coin/start before the title handoff window without burning the match-entry wall budget"
        );
    }

    #[test]
    fn native_match_entry_script_reproduces_fight_start_sequence() {
        let match_segments = native_match_entry_script();

        assert_eq!(match_segments.len(), 29);
        assert_eq!(
            match_segments.last().expect("last segment").action,
            Action::Noop
        );
        assert_eq!(match_segments.last().expect("last segment").frames, 1200);
        assert!(
            match_segments
                .iter()
                .filter(|segment| segment.action == Action::Start)
                .count()
                >= 2
        );
        assert_eq!(
            match_segments
                .iter()
                .map(|segment| segment.frames)
                .sum::<u64>(),
            super::native_warning_skip_to_title_script()
                .iter()
                .map(|segment| segment.frames)
                .sum::<u64>()
                + 2544
        );
        assert_eq!(
            match_segments
                .iter()
                .filter(|segment| segment.action == Action::Coin)
                .count(),
            4
        );
        assert_eq!(
            match_segments
                .iter()
                .filter(|segment| segment.action == Action::Service)
                .count(),
            2
        );
        assert_eq!(
            match_segments
                .iter()
                .filter(|segment| segment.action == Action::Punch)
                .count(),
            2
        );
    }

    #[test]
    fn native_health_checkpoint_script_reaches_playable_match_entry() {
        let checkpoint_segments = native_health_checkpoint_script();
        let checkpoint_frames = checkpoint_segments
            .iter()
            .map(|segment| segment.frames)
            .sum::<u64>();

        assert_eq!(
            checkpoint_segments,
            super::native_snapshot_match_entry_script()
        );
        assert!(checkpoint_frames <= 3_600);
        assert!(
            checkpoint_segments
                .iter()
                .filter(|segment| segment.action == Action::Coin && segment.frames >= 12)
                .count()
                >= 2
        );
        assert!(
            checkpoint_segments
                .iter()
                .filter(|segment| segment.action == Action::Start && segment.frames >= 18)
                .count()
                >= 2
        );
        assert!(
            checkpoint_segments
                .iter()
                .filter(|segment| {
                    matches!(
                        segment.action,
                        Action::Up
                            | Action::Down
                            | Action::Left
                            | Action::Right
                            | Action::Punch
                            | Action::Kick
                            | Action::Beast
                            | Action::Guard
                    )
                })
                .count()
                >= super::native_health_branch_actions().len()
        );
    }

    #[test]
    fn native_input_check_checkpoint_script_is_bounded_smoke_path() {
        let input_segments = native_input_check_checkpoint_script();
        let input_frames = input_segments
            .iter()
            .map(|segment| segment.frames)
            .sum::<u64>();
        let health_frames = native_health_checkpoint_script()
            .iter()
            .map(|segment| segment.frames)
            .sum::<u64>();

        assert!(health_frames >= input_frames);
        assert!(input_frames <= 2_200);
        assert!(health_frames <= 3_600);
        assert!(
            input_segments
                .iter()
                .filter(|segment| segment.action == Action::Coin && segment.frames >= 12)
                .count()
                >= 2
        );
        assert!(
            input_segments
                .iter()
                .filter(|segment| segment.action == Action::Start && segment.frames >= 18)
                .count()
                >= 2
        );
    }

    #[test]
    fn native_health_match_tail_keeps_extra_settle_after_playable_checkpoint() {
        let mut combined = native_health_checkpoint_script();
        combined.extend(native_health_match_entry_tail_script());
        let checkpoint_len = native_health_checkpoint_script().len();

        assert_eq!(
            combined[..checkpoint_len],
            super::native_snapshot_match_entry_script()
        );
        assert_eq!(
            combined.last().copied(),
            Some(NativeScriptSegment {
                action: Action::Noop,
                frames: 1200,
            })
        );
        assert!(
            combined.iter().map(|segment| segment.frames).sum::<u64>()
                > native_health_checkpoint_script()
                    .iter()
                    .map(|segment| segment.frames)
                    .sum::<u64>()
        );
    }

    #[test]
    fn remaining_native_script_segments_resume_partial_segment() {
        let segments = vec![
            NativeScriptSegment {
                action: Action::Noop,
                frames: 10,
            },
            NativeScriptSegment {
                action: Action::Start,
                frames: 5,
            },
            NativeScriptSegment {
                action: Action::Punch,
                frames: 3,
            },
        ];

        assert_eq!(
            remaining_native_script_segments(&segments, 1, 2),
            vec![
                NativeScriptSegment {
                    action: Action::Start,
                    frames: 3,
                },
                NativeScriptSegment {
                    action: Action::Punch,
                    frames: 3,
                }
            ]
        );
        assert!(remaining_native_script_segments(&segments, 3, 0).is_empty());
    }

    #[test]
    fn title_ready_remaining_script_skips_leading_wait_before_credit() {
        let segments = native_snapshot_handoff_script(true);
        let boot_wait_len = native_play_boot_wait_segments(&segments).len();
        assert!(boot_wait_len > 0);

        let tail = remaining_native_script_segments_after_title_ready(
            &segments,
            boot_wait_len - 1,
            NATIVE_PLAY_TITLE_ENTRY_FRAMES / 2,
        );

        assert_eq!(
            tail.first().map(|segment| segment.action),
            Some(Action::Coin)
        );
        assert!(
            tail.iter()
                .position(|segment| native_script_segment_is_credit_attempt(*segment))
                == Some(0)
        );
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
    fn native_combined_controls_do_not_require_credit_ports() {
        let sweep_activity = NativeInputActivity {
            p1_up_active_reads: 1,
            p1_down_active_reads: 1,
            p1_left_active_reads: 1,
            p1_right_active_reads: 1,
            p1_punch_active_reads: 1,
            p1_kick_active_reads: 1,
            p1_beast_active_reads: 1,
            p3_guard_active_reads: 1,
            ..NativeInputActivity::default()
        };

        assert!(super::native_combined_play_controls_active(
            NativeInputActivity::default(),
            sweep_activity
        ));
        assert!(super::native_combined_full_controls_active(
            NativeInputActivity::default(),
            sweep_activity
        ));
    }

    #[test]
    fn native_play_window_frame_scales_partial_boot_frame_to_full_window() {
        let source = NativeDisplayFrame {
            width: 512,
            height: 240,
            pixels: vec![0x00ff_ffff; 512 * 240],
        };

        let scaled = native_play_window_frame(&source);

        assert_eq!((scaled.width, scaled.height), (640, 480));
        assert_eq!(scaled.pixels.len(), 640 * 480);
        assert_eq!(scaled.pixels[0], 0x00ff_ffff);
        assert_eq!(scaled.pixels[639], 0x00ff_ffff);
        assert_eq!(scaled.pixels[640 * 479], 0x00ff_ffff);
        assert_eq!(scaled.pixels[640 * 480 - 1], 0x00ff_ffff);
    }

    #[test]
    fn native_play_window_frame_scales_narrow_frame_to_full_window() {
        let mut pixels = vec![0; 320 * 240];
        pixels[319] = 0x00ff_0000;
        pixels[319 + 239 * 320] = 0x0000_ff00;
        let source = NativeDisplayFrame {
            width: 320,
            height: 240,
            pixels,
        };

        let scaled = native_play_window_frame(&source);

        assert_eq!((scaled.width, scaled.height), (640, 480));
        assert_eq!(scaled.pixels[638], 0x00ff_0000);
        assert_eq!(scaled.pixels[639], 0x00ff_0000);
        assert_eq!(scaled.pixels[638 + 478 * 640], 0x0000_ff00);
        assert_eq!(scaled.pixels[639 + 479 * 640], 0x0000_ff00);
    }

    #[test]
    fn native_play_window_frame_preserves_full_narrow_480_line_frame() {
        let mut pixels = vec![0; 320 * 480];
        for y in 0..480 {
            let color = if y < 240 { 0x00ff_0000 } else { 0x0000_00ff };
            for x in 0..320 {
                pixels[y * 320 + x] = color;
            }
        }
        let source = NativeDisplayFrame {
            width: 320,
            height: 480,
            pixels,
        };

        let scaled = native_play_window_frame(&source);

        assert_eq!((scaled.width, scaled.height), (640, 480));
        assert_eq!(scaled.pixels[0], 0x00ff_0000);
        assert_eq!(scaled.pixels[639], 0x00ff_0000);
        assert_eq!(scaled.pixels[640 * 479], 0x0000_00ff);
        assert_eq!(scaled.pixels[640 * 480 - 1], 0x0000_00ff);
    }

    #[test]
    fn native_play_window_frame_preserves_asymmetric_visible_480_line_frame() {
        let mut pixels = vec![0; 512 * 480];
        for y in 16..48 {
            for x in 16..48 {
                pixels[y * 512 + x] = 0x00ff_0000;
            }
        }
        for y in 240..480 {
            for x in 0..512 {
                pixels[y * 512 + x] = 0x0000_ff00;
            }
        }
        let source = NativeDisplayFrame {
            width: 512,
            height: 480,
            pixels,
        };

        let scaled = native_play_window_frame(&source);

        assert_eq!((scaled.width, scaled.height), (640, 480));
        assert_eq!(scaled.pixels[20 + 16 * 640], 0x00ff_0000);
        assert_eq!(scaled.pixels[320 + 300 * 640], 0x0000_ff00);
    }

    #[test]
    fn native_play_window_frame_preserves_sparse_top_band_without_full_height_stretch() {
        let mut pixels = vec![0; 640 * 480];
        for y in 20..80 {
            for x in 160..480 {
                pixels[y * 640 + x] = 0x0000_ff00;
            }
        }
        let source = NativeDisplayFrame {
            width: 640,
            height: 480,
            pixels,
        };

        let scaled = native_play_window_frame(&source);
        let stats = NativeFrameStats::from_frame(&scaled);

        assert_eq!((scaled.width, scaled.height), (640, 480));
        assert!(
            stats.occupied_row_span < 100,
            "sparse top-only GUI frames should not be stretched into fake full-screen content: {stats:?}"
        );
        assert_eq!(scaled.pixels[320 + 240 * 640], 0);
        assert_eq!(scaled.pixels[320 + 40 * 640], 0x0000_ff00);
    }

    #[test]
    fn native_play_window_frame_preserves_sparse_warning_text_layout() {
        let mut pixels = vec![0; 320 * 240];
        for y in (24..96).step_by(3) {
            for glyph in 0..16 {
                let x0 = 40 + glyph * 14;
                for x in x0..x0 + 5 {
                    pixels[y * 320 + x] = 0x00ff_ffff;
                    pixels[(y + 1) * 320 + x] = 0x00ff_ffff;
                }
            }
        }
        let source = NativeDisplayFrame {
            width: 320,
            height: 240,
            pixels,
        };

        let scaled = native_play_window_frame(&source);
        let stats = NativeFrameStats::from_frame(&scaled);

        assert_eq!((scaled.width, scaled.height), (640, 480));
        assert!(stats.occupied_row_span < 240, "{stats:?}");
        assert_eq!(scaled.pixels[320 + 240 * 640], 0);
    }

    #[test]
    fn native_play_window_frame_preserves_sparse_title_logo_artifact_layout() {
        let mut pixels = vec![0; 512 * 240];
        for y in 132..212 {
            for x in 296..496 {
                let local_x = x - 296;
                let local_y = y - 132;
                if local_x < 150 && (local_x / 5 + local_y / 7) % 2 == 0 {
                    pixels[y * 512 + x] = 0x00d8_1010;
                } else if local_x >= 154 && (local_y / 4) % 3 == 0 {
                    pixels[y * 512 + x] = 0x00f0_f0_d8;
                }
            }
        }
        let source = NativeDisplayFrame {
            width: 512,
            height: 240,
            pixels,
        };

        let scaled = native_play_window_frame(&source);
        let stats = NativeFrameStats::from_frame(&scaled);

        assert_eq!((scaled.width, scaled.height), (640, 480));
        assert_eq!(scaled.pixels[420 + 16 * 640], 0);
        assert_eq!(scaled.pixels[420 + 464 * 640], 0);
        assert!(
            stats.occupied_row_span < 240,
            "sparse title/logo artifacts should not be vertically stretched: {stats:?}"
        );
        assert!(
            stats.right_title_red_pixels > 0 || stats.right_title_bright_pixels > 0,
            "{stats:?}"
        );
    }

    #[test]
    fn native_play_window_frame_deinterlaces_to_better_bottom_field() {
        let mut pixels = vec![0; 320 * 480];
        for y in 240..480 {
            for x in 0..320 {
                pixels[y * 320 + x] = 0x00ff_ffff;
            }
        }
        let source = NativeDisplayFrame {
            width: 320,
            height: 480,
            pixels,
        };

        let scaled = native_play_window_frame(&source);

        assert_eq!((scaled.width, scaled.height), (640, 480));
        assert_eq!(scaled.pixels[0], 0x00ff_ffff);
        assert_eq!(scaled.pixels[640 * 479], 0x00ff_ffff);
    }

    #[test]
    fn native_play_window_frame_deinterlaces_with_sparse_residual_other_field() {
        let mut pixels = vec![0; 512 * 480];
        for y in 32..44 {
            for x in (384..512).step_by(3) {
                pixels[y * 512 + x] = if x % 2 == 0 { 0x0020_1020 } else { 0x0008_1808 };
            }
        }
        for y in 240..480 {
            for x in 0..512 {
                pixels[y * 512 + x] = if (x + y) % 17 == 0 {
                    0x00ff_ffff
                } else {
                    0x0000_a040
                };
            }
        }
        let source = NativeDisplayFrame {
            width: 512,
            height: 480,
            pixels,
        };

        let scaled = native_play_window_frame(&source);

        assert_eq!((scaled.width, scaled.height), (640, 480));
        assert_ne!(scaled.pixels[0], 0);
        assert_ne!(scaled.pixels[640 * 479], 0);
    }

    #[test]
    fn native_play_window_frame_expands_wide_interlaced_mode_to_full_window() {
        let mut pixels = vec![0; 512 * 480];
        pixels[511] = 0x00ff_0000;
        pixels[511 + 479 * 512] = 0x0000_ff00;
        let source = NativeDisplayFrame {
            width: 512,
            height: 480,
            pixels,
        };

        let scaled = native_play_window_frame(&source);

        assert_eq!((scaled.width, scaled.height), (640, 480));
        assert_eq!(scaled.pixels[638], 0);
        assert_eq!(scaled.pixels[639], 0x00ff_0000);
        assert_eq!(scaled.pixels[638 + 479 * 640], 0);
        assert_eq!(scaled.pixels[639 + 479 * 640], 0x0000_ff00);
    }

    #[test]
    fn native_play_window_frame_expands_narrow_interlaced_frame_to_full_window() {
        let mut pixels = vec![0; 256 * 480];
        pixels[0] = 0x00ff_0000;
        pixels[255] = 0x0000_ff00;
        let source = NativeDisplayFrame {
            width: 256,
            height: 480,
            pixels,
        };

        let scaled = native_play_window_frame(&source);

        assert_eq!((scaled.width, scaled.height), (640, 480));
        assert_eq!(scaled.pixels[0], 0x00ff_0000);
        assert_eq!(scaled.pixels[1], 0x00ff_0000);
        assert_eq!(scaled.pixels[638], 0x0000_ff00);
        assert_eq!(scaled.pixels[639], 0x0000_ff00);
    }

    #[test]
    fn native_play_window_frame_deinterlace_preserves_field_pixels_without_blur() {
        let mut pixels = vec![0; 320 * 480];
        for y in 0..240 {
            for x in 0..320 {
                pixels[y * 320 + x] = if y % 2 == 0 { 0x00ff_0000 } else { 0x0000_00ff };
            }
        }
        let source = NativeDisplayFrame {
            width: 320,
            height: 480,
            pixels,
        };

        let scaled = native_play_window_frame(&source);

        assert_eq!((scaled.width, scaled.height), (640, 480));
        assert_eq!(scaled.pixels[0], 0x00ff_0000);
        assert_eq!(scaled.pixels[640], 0x00ff_0000);
        assert_eq!(scaled.pixels[640 * 2], 0x0000_00ff);
        assert_eq!(scaled.pixels[640 * 3], 0x0000_00ff);
    }

    #[test]
    fn native_play_display_prefers_actual_when_it_has_gameplay_scene() {
        let stable = test_solid_frame(0x0008_0808);
        let actual = test_gradient_playfield_frame();

        assert!(NativeFrameStats::from_frame(&actual).has_gameplay_scene());
        assert!(native_play_should_prefer_actual_display(
            &actual, &stable, true
        ));
    }

    #[test]
    fn native_play_display_keeps_deinterlaced_stable_over_low_field_actual() {
        let actual = test_live_stage_field_with_title_like_hud_frame();
        let mut stable_pixels = vec![0; actual.width * actual.height * 2];
        for y in 0..actual.height * 2 {
            let source_y = y / 2;
            let source = source_y * actual.width;
            let target = y * actual.width;
            stable_pixels[target..target + actual.width]
                .copy_from_slice(&actual.pixels[source..source + actual.width]);
        }
        let stable = NativeDisplayFrame {
            width: actual.width,
            height: actual.height * 2,
            pixels: stable_pixels,
        };
        let stable_window = native_play_window_frame(&stable);
        let stable_stats = NativeFrameStats::from_frame(&stable_window);

        assert!(stable_stats.has_scene_detail(), "{stable_stats:?}");
        assert!(
            !stable_stats.has_blocking_display_artifact(),
            "{stable_stats:?}"
        );
        assert!(!native_play_should_prefer_actual_display(
            &actual, &stable, true
        ));
    }

    #[test]
    fn native_play_display_prefers_resolved_title_frame_over_doubled_stable_frame() {
        let mut actual_pixels = vec![0; 512 * 240];
        for y in 104..124 {
            for x in 216..304 {
                let color = match (x + y) % 5 {
                    0 => 0x00ff_d800,
                    1 => 0x00f0_f0_f0,
                    2 => 0x0000_80ff,
                    3 => 0x00ff_6818,
                    _ => 0x0080_80a0,
                };
                actual_pixels[y * 512 + x] = color;
            }
        }
        for y in 134..216 {
            for x in 320..504 {
                let local_x = x - 320;
                let local_y = y - 134;
                let color = if local_x < 132 && (local_x / 4 + local_y / 5) % 3 != 0 {
                    0x00d8_1010
                } else if local_x >= 140 && (local_y / 3 + local_x / 11) % 2 == 0 {
                    0x00f0_f0_d8
                } else if (local_x + local_y) % 29 == 0 {
                    0x00ff_8030
                } else {
                    0
                };
                actual_pixels[y * 512 + x] = color;
            }
        }
        let actual = NativeDisplayFrame {
            width: 512,
            height: 240,
            pixels: actual_pixels,
        };
        let mut stable_pixels = vec![0; 512 * 480];
        for y in 0..480 {
            let source_y = y / 2;
            let source = source_y * 512;
            let target = y * 512;
            stable_pixels[target..target + 512]
                .copy_from_slice(&actual.pixels[source..source + 512]);
        }
        let stable = NativeDisplayFrame {
            width: 512,
            height: 480,
            pixels: stable_pixels,
        };

        let actual_window = native_play_window_frame(&actual);
        let stable_window = native_play_window_frame(&stable);
        let actual_stats = NativeFrameStats::from_frame(&actual_window);
        let stable_stats = NativeFrameStats::from_frame(&stable_window);

        assert!(native_play_title_screen_ready_from_frame(&actual_window));
        assert!(
            actual_stats.occupied_row_span < stable_stats.occupied_row_span,
            "resolved title frame should not be vertically stretched"
        );
        assert!(native_play_should_prefer_actual_display(
            &actual, &stable, false
        ));
    }

    #[test]
    fn native_play_display_prefers_raw_actual_title_over_sparse_output_fragment() {
        let raw = test_title_screen_frame();
        let mut fragment_pixels = vec![0; 320 * 240];
        for y in 0..28 {
            for x in 248..320 {
                fragment_pixels[y * 320 + x] = if (x + y) % 5 == 0 {
                    0x00f0_f0_f0
                } else {
                    0x00d8_1010
                };
            }
        }
        let fragment = NativeDisplayFrame {
            width: 320,
            height: 240,
            pixels: fragment_pixels,
        };
        let raw_window = native_play_window_frame_with_context(&raw, false, false);
        let fragment_window = native_play_window_frame_with_context(&fragment, false, false);

        assert!(native_play_title_screen_ready_from_frame(&raw_window));
        assert!(!native_play_title_screen_ready_from_frame(&fragment_window));
        assert!(native_play_should_prefer_raw_actual_display(
            &raw, &fragment, &fragment, false, false
        ));
        assert!(!native_play_should_prefer_raw_actual_display(
            &raw, &fragment, &raw, false, false
        ));
    }

    #[test]
    fn native_play_window_context_preserves_raw_512_title_with_minor_footer() {
        let width = 512;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..30 {
            for x in 412..512 {
                pixels[y * width + x] = if (x + y) % 4 == 0 {
                    0x00f0_f0_f0
                } else {
                    0x00d8_1010
                };
            }
        }
        for y in 248..314 {
            for x in 282..508 {
                let colors = [
                    0x00f0_f0_f0,
                    0x00d8_1810,
                    0x00ff_5030,
                    0x00b8_1808,
                    0x00ff_c080,
                    0x00e0_4020,
                    0x00ff_8040,
                    0x00a0_1010,
                ];
                pixels[y * width + x] = colors[(x / 5 + y / 3) % colors.len()];
            }
        }
        for y in 320..348 {
            for x in 310..500 {
                if (x + y) % 7 != 0 {
                    let red = 96 + ((x * 3 + y * 5) % 120) as u32;
                    let green = 24 + ((x + y * 3) % 56) as u32;
                    let blue = 16 + ((x * 5 + y) % 48) as u32;
                    pixels[y * width + x] = (red << 16) | (green << 8) | blue;
                };
            }
        }
        for y in 386..414 {
            for x in (96..406).step_by(5) {
                pixels[y * width + x] = 0x00f0_f0_f0;
                pixels[y * width + x + 1] = 0x00d8_d8_d8;
            }
        }
        let raw = NativeDisplayFrame {
            width,
            height,
            pixels,
        };
        let fragment = NativeDisplayFrame {
            width: 320,
            height: 240,
            pixels: vec![0; 320 * 240],
        };
        let window = native_play_window_frame_with_context(&raw, false, false);
        let stats = NativeFrameStats::from_frame(&window);

        assert!(stats.has_minor_bottom_caption_band(), "{stats:?}");
        assert!(stats.has_title_screen_frame(), "{stats:?}");
        assert!(native_play_title_screen_ready_from_frame(&window));
        assert!(native_play_should_prefer_raw_actual_display(
            &raw, &fragment, &fragment, false, false
        ));
        assert!(
            stats.nonzero_pixels > 20_000,
            "raw title should not collapse to top/right fragment: {stats:?}"
        );
    }

    #[test]
    fn native_play_display_keeps_richer_stable_title_over_sparse_actual_title() {
        let mut actual_pixels = vec![0; 512 * 240];
        for y in 104..124 {
            for x in 216..304 {
                let color = match (x + y) % 5 {
                    0 => 0x00ff_d800,
                    1 => 0x00f0_f0_f0,
                    2 => 0x0000_80ff,
                    3 => 0x00ff_6818,
                    _ => 0x0080_80a0,
                };
                actual_pixels[y * 512 + x] = color;
            }
        }
        for y in 132..214 {
            for x in 316..504 {
                let local_x = x - 316;
                let local_y = y - 132;
                if local_x < 136 && (local_x / 4 + local_y / 5) % 3 != 0 {
                    actual_pixels[y * 512 + x] = 0x00d8_1010;
                } else if local_x >= 142 && (local_x / 5 + local_y / 3) % 2 == 0 {
                    actual_pixels[y * 512 + x] = 0x00f0_f0_d8;
                }
            }
        }
        let actual = NativeDisplayFrame {
            width: 512,
            height: 240,
            pixels: actual_pixels,
        };
        let mut stable_pixels = vec![0; 512 * 480];
        for y in 208..470 {
            for x in 0..260 {
                if (x + y) % 4 != 0 {
                    let red = 56 + ((x * 3 + y * 7) % 176) as u32;
                    let green = 24 + ((x * 11 + y * 5) % 104) as u32;
                    let blue = 16 + ((x * 13 + y * 3) % 96) as u32;
                    stable_pixels[y * 512 + x] = (red << 16) | (green << 8) | blue;
                }
            }
        }
        for y in 250..400 {
            for x in 286..508 {
                if (x / 6 + y / 4) % 3 != 0 {
                    let red = 128 + ((x * 5 + y * 3) % 112) as u32;
                    let green = 72 + ((x * 7 + y * 11) % 96) as u32;
                    let blue = 40 + ((x * 13 + y * 5) % 96) as u32;
                    stable_pixels[y * 512 + x] = (red << 16) | (green << 8) | blue;
                }
            }
        }
        let stable = NativeDisplayFrame {
            width: 512,
            height: 480,
            pixels: stable_pixels,
        };
        let actual_window = native_play_window_frame(&actual);
        let stable_window = native_play_window_frame(&stable);
        let stable_stats = NativeFrameStats::from_frame(&stable_window);

        assert!(native_play_title_screen_ready_from_frame(&actual_window));
        assert!(stable_stats.has_presentable_scene(), "{stable_stats:?}");
        assert!(stable_stats.has_scene_detail(), "{stable_stats:?}");
        assert!(!native_play_should_prefer_actual_display(
            &actual, &stable, false
        ));
    }

    #[test]
    fn native_play_display_rejects_contextual_texture_fragment_over_stale_title() {
        let stable = test_title_screen_frame();
        let actual = test_contextual_live_stage_field_frame();
        let actual_window = native_play_window_frame(&actual);
        let actual_stats = NativeFrameStats::from_frame(&actual_window);

        assert!(
            actual_stats.has_texture_page_fragment_artifact(),
            "{actual_stats:?}"
        );
        assert!(
            actual_stats.has_context_live_stage_hud(),
            "{actual_stats:?}"
        );
        assert!(!actual_stats.has_gameplay_scene(), "{actual_stats:?}");
        assert!(!native_play_should_prefer_actual_display(
            &actual, &stable, true
        ));
    }

    #[test]
    fn native_play_display_rejects_gpu_verified_texture_fragment_over_stale_title() {
        let stable = test_title_screen_frame();
        let actual = test_contextual_live_stage_field_frame();
        let actual_window = native_play_window_frame(&actual);
        let actual_stats = NativeFrameStats::from_frame(&actual_window);

        assert!(
            actual_stats.has_blocking_texture_page_fragment_artifact(),
            "{actual_stats:?}"
        );

        assert!(!native_play_should_prefer_actual_display(
            &actual, &stable, true
        ));
        assert!(!native_play_should_prefer_gpu_verified_actual_display(
            &actual, &stable, true, false, true
        ));
        assert!(!native_play_should_prefer_gpu_verified_actual_display(
            &actual, &stable, true, true, true
        ));
    }

    #[test]
    fn native_play_display_allows_gpu_verified_clean_gameplay_over_stale_title() {
        let stable = test_title_screen_frame();
        let actual = test_gradient_playfield_frame();
        let actual_stats = NativeFrameStats::from_frame(&actual);

        assert!(actual_stats.has_gameplay_scene(), "{actual_stats:?}");
        assert!(native_play_should_prefer_gpu_verified_actual_display(
            &actual, &stable, true, true, true
        ));
    }

    #[test]
    fn native_play_display_selects_gpu_verified_actual_over_stale_title_fallback() {
        let stale_title = test_title_screen_frame();
        let candidate = test_texture_page_fragment_frame();
        let actual = test_gradient_playfield_frame();
        let actual_stats = NativeFrameStats::from_frame(&actual);

        assert!(actual_stats.has_gameplay_scene(), "{actual_stats:?}");
        assert!(native_play_gpu_verified_frame_stats(actual_stats));

        let selected = native_play_select_visible_window_frame(
            candidate,
            false,
            Some(actual.clone()),
            &Some(stale_title.clone()),
            &stale_title,
        );

        assert_eq!(
            native_frame_checksum(&selected),
            native_frame_checksum(&actual)
        );
    }

    #[test]
    fn native_play_display_rejects_gpu_verified_context_stage_texture_fragment() {
        let actual = test_contextual_live_stage_field_frame();
        let actual_window = native_play_window_frame(&actual);
        let actual_stats = NativeFrameStats::from_frame(&actual_window);

        assert!(
            actual_stats.has_blocking_texture_page_fragment_artifact(),
            "{actual_stats:?}"
        );
        assert!(
            native_play_gpu_verified_actual_window_frame_with_context(&actual, true, true, true)
                .is_none(),
            "{actual_stats:?}"
        );
    }

    #[test]
    fn native_play_display_prefers_live_field_gameplay_over_stale_title() {
        let stable = test_title_screen_frame();
        let actual = test_live_stage_field_with_title_like_hud_frame();
        let actual_window = native_play_window_frame(&actual);
        let actual_stats = NativeFrameStats::from_frame(&actual_window);

        assert!(actual_stats.has_scene_detail(), "{actual_stats:?}");
        assert!(actual_stats.has_title_logo_overlay(), "{actual_stats:?}");
        assert!(!actual_stats.has_gameplay_scene(), "{actual_stats:?}");
        assert!(native_play_frame_ready_with_context(true, &actual_window));
        assert!(native_play_gameplay_scene_with_context(true, actual_stats));
        assert!(!native_play_frame_ready_with_context(false, &actual_window));
        assert!(native_play_should_prefer_actual_display(
            &actual, &stable, true
        ));
    }

    #[test]
    fn native_play_candidate_replaces_stale_title_with_context_ready_live_field() {
        let stale_title = test_title_screen_frame();
        let candidate =
            native_play_window_frame(&test_live_stage_field_with_title_like_hud_frame());
        let candidate_stats = NativeFrameStats::from_frame(&candidate);

        assert!(candidate_stats.has_scene_detail(), "{candidate_stats:?}");
        assert!(
            !candidate_stats.has_title_screen_frame(),
            "{candidate_stats:?}"
        );
        assert!(
            native_play_frame_ready_with_context(true, &candidate)
                || native_play_gameplay_scene_with_context(true, candidate_stats),
            "{candidate_stats:?}"
        );
        assert!(
            native_play_candidate_should_replace_stale_title(
                true,
                &candidate,
                candidate_stats,
                &stale_title,
            ),
            "{candidate_stats:?}"
        );
        assert!(
            !native_play_candidate_should_replace_stale_title(
                false,
                &candidate,
                candidate_stats,
                &stale_title,
            ),
            "{candidate_stats:?}"
        );
    }

    #[test]
    fn native_play_display_keeps_stable_when_actual_is_saturated_transition() {
        let stable = test_gradient_playfield_frame();
        let mut actual = test_solid_frame(0x00e0_7058);
        for y in 0..96 {
            for x in 0..actual.width {
                actual.pixels[y * actual.width + x] = 0x0000_0000;
            }
        }

        assert!(NativeFrameStats::from_frame(&stable).has_gameplay_scene());
        assert!(NativeFrameStats::from_frame(&actual).has_saturated_transition_planes());
        assert!(!native_play_should_prefer_actual_display(
            &actual, &stable, true
        ));
    }

    #[test]
    fn native_frame_stats_rejects_letterboxed_transition_frame() {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..160 {
            for x in 0..width {
                let red = 32 + ((x * 11 + y * 3) % 192) as u32;
                let green = 24 + ((x * 7 + y * 13) % 160) as u32;
                let blue = 16 + ((x * 5 + y * 17) % 128) as u32;
                let color = (red << 16) | (green << 8) | blue;
                pixels[y * width + x] = color;
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "transition still has detail");
        assert!(
            !stats.has_gameplay_scene(),
            "large black void is not gameplay"
        );
    }

    #[test]
    fn native_frame_stats_rejects_bottom_caption_video_frame() {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                let red = 48 + ((x * 7 + y * 5) % 176) as u32;
                let green = 56 + ((x * 11 + y * 3) % 168) as u32;
                let blue = 64 + ((x * 13 + y * 17) % 160) as u32;
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        for y in 400..468 {
            for x in 0..width {
                pixels[y * width + x] = 0;
            }
        }
        for y in 430..454 {
            for x in (120..520).step_by(9) {
                for stroke_x in x..(x + 4).min(width) {
                    pixels[y * width + stroke_x] = 0x00ef_efef;
                }
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(stats.has_bottom_caption_band(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
        assert!(!native_play_gui_handoff_frame_ready(&frame), "{stats:?}");
        assert!(
            !native_play_frame_ready_with_context(true, &frame),
            "{stats:?}"
        );
    }

    #[test]
    fn native_play_frame_ready_accepts_small_caption_ui_with_core_context() {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                let red = 48 + ((x * 7 + y * 5) % 176) as u32;
                let green = 56 + ((x * 11 + y * 3) % 168) as u32;
                let blue = 64 + ((x * 13 + y * 17) % 160) as u32;
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        for y in 384..height {
            for x in 0..width {
                let green = 96 + ((x * 5 + y * 3) % 96) as u32;
                pixels[y * width + x] = green << 8;
            }
        }
        for y in 430..460 {
            for x in 0..width {
                pixels[y * width + x] = 0;
            }
        }
        for y in 442..450 {
            for x in (72..568).step_by(12) {
                for stroke_x in x..(x + 3).min(width) {
                    pixels[y * width + stroke_x] = 0x00ef_efef;
                }
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(stats.has_bottom_caption_band(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
        assert!(stats.has_native_context_caption_ui(), "{stats:?}");
        assert!(native_play_frame_ready_with_context(true, &frame));
    }

    #[test]
    fn native_frame_stats_rejects_red_or_salmon_corruption() {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                let red = 192 + ((x * 5 + y * 3) % 64) as u32;
                let green = 48 + ((x * 7 + y * 11) % 64) as u32;
                let blue = 32 + ((x * 13 + y * 17) % 48) as u32;
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "corruption can still be detailed");
        assert!(
            !stats.has_gameplay_scene(),
            "warm dominance is not gameplay"
        );
        assert!(stats.has_warm_dominant_fill_frame(), "{stats:?}");
        assert!(stats.has_blocking_display_artifact(), "{stats:?}");
        assert!(!super::native_play_presentable_frame(&frame), "{stats:?}");

        let safe = test_title_screen_frame();
        let visible = super::native_play_visible_window_frame(frame.clone(), &Some(safe.clone()));
        assert_eq!(visible.pixels, safe.pixels);
    }

    #[test]
    fn native_frame_stats_rejects_low_palette_logo_transition() {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                let color = if (x + y * 3) % 10 == 0 {
                    0
                } else if x < y.saturating_mul(width) / height / 2 {
                    0x0066_6600
                } else if x >= width / 2 {
                    let shade = ((x / 8 + y / 6) % 72) as u32;
                    ((64 + shade) << 16) | ((shade / 3) << 8)
                } else {
                    let shade = ((x / 9 + y / 7) % 56) as u32;
                    ((48 + shade) << 16) | ((shade / 4) << 8)
                };
                pixels[y * width + x] = color;
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(stats.has_low_palette_logo_transition(), "{stats:?}");
        assert!(!stats.has_handoff_scene(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
        assert!(
            !native_play_frame_ready_with_context(true, &frame),
            "{stats:?}"
        );
    }

    #[test]
    fn native_frame_stats_rejects_texture_page_fragment_as_gameplay() {
        let frame = test_texture_page_fragment_frame();
        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(stats.has_texture_page_fragment_artifact(), "{stats:?}");
        assert!(!stats.has_render_ready_scene(), "{stats:?}");
        assert!(!stats.has_handoff_scene(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
        assert!(
            !native_play_frame_ready_with_context(true, &frame),
            "{stats:?}"
        );
        assert!(
            !native_play_gameplay_scene_with_context(true, stats),
            "{stats:?}"
        );
        assert!(
            !native_play_gpu_verified_frame_stats_with_context(true, stats),
            "{stats:?}"
        );
    }

    #[test]
    fn native_frame_stats_rejects_stage_texture_field_smear_as_gameplay() {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 24..360 {
            for x in 0..width {
                let hash = (x as u32)
                    .wrapping_mul(1_103_515_245)
                    .wrapping_add((y as u32).wrapping_mul(12_345))
                    .rotate_left(((x ^ y) & 15) as u32);
                let color = if hash % 8 == 0 {
                    let red = 32 + ((hash >> 3) % 208);
                    let green = 24 + ((hash >> 11) % 200);
                    let blue = 32 + ((hash >> 19) % 192);
                    (red << 16) | (green << 8) | blue
                } else {
                    0x0010_1424
                };
                pixels[y * width + x] = color;
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };
        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(stats.has_stage_texture_field_smear(), "{stats:?}");
        assert!(
            stats.has_blocking_texture_page_fragment_artifact(),
            "{stats:?}"
        );
        assert!(stats.has_blocking_display_artifact(), "{stats:?}");
        assert!(!stats.has_render_ready_scene(), "{stats:?}");
        assert!(!stats.has_handoff_scene(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
        assert!(!super::native_play_presentable_frame(&frame), "{stats:?}");
        assert!(
            !native_play_frame_ready_with_context(true, &frame),
            "{stats:?}"
        );
        assert!(
            !native_play_gameplay_scene_with_context(true, stats),
            "{stats:?}"
        );
    }

    #[test]
    fn native_play_visible_frame_holds_last_safe_frame_for_texture_fragment() {
        let safe = test_title_screen_frame();
        let fragment = test_texture_page_fragment_frame();

        let visible =
            super::native_play_visible_window_frame(fragment.clone(), &Some(safe.clone()));
        assert_eq!(visible.pixels, safe.pixels);

        let visible_without_safe = super::native_play_visible_window_frame(fragment.clone(), &None);
        assert_eq!(visible_without_safe.pixels, fragment.pixels);
    }

    #[test]
    fn native_play_visible_frame_holds_last_safe_frame_for_contextual_texture_fragment() {
        let safe = test_title_screen_frame();
        let frame = native_play_window_frame(&test_contextual_live_stage_field_frame());
        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_texture_page_fragment_artifact(), "{stats:?}");
        assert!(
            stats.has_blocking_texture_page_fragment_artifact(),
            "{stats:?}"
        );
        assert!(stats.has_context_live_stage_hud(), "{stats:?}");
        assert!(!stats.has_render_ready_scene(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");

        let visible = super::native_play_visible_window_frame(frame.clone(), &Some(safe.clone()));
        assert_eq!(visible.pixels, safe.pixels);
    }

    #[test]
    fn native_play_context_live_stage_repair_does_not_mask_texture_fragment() {
        let frame = native_play_window_frame(&test_contextual_live_stage_field_frame());
        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_context_live_stage_hud(), "{stats:?}");
        assert!(
            stats.has_blocking_texture_page_fragment_artifact(),
            "{stats:?}"
        );

        let repaired = native_play_repair_context_live_stage_window_frame(frame, true, true);
        let repaired_stats = NativeFrameStats::from_frame(&repaired);

        assert_eq!((repaired.width, repaired.height), (640, 480));
        assert!(
            repaired_stats.has_blocking_texture_page_fragment_artifact(),
            "{repaired_stats:?}"
        );
        assert!(
            !native_play_gpu_verified_frame_stats(repaired_stats),
            "{repaired_stats:?}"
        );
        assert!(
            repaired_stats.has_context_live_stage_hud(),
            "{repaired_stats:?}"
        );
    }

    #[test]
    fn native_play_window_context_preserves_single_field_handoff_over_smear() {
        let source = test_high_vertical_single_field_handoff_frame();
        let raw_stats = NativeFrameStats::from_frame(&source);
        let deinterlaced = native_play_window_frame(&source);
        let deinterlaced_stats = NativeFrameStats::from_frame(&deinterlaced);
        let selected = native_play_window_frame_with_context(&source, true, true);
        let selected_stats = NativeFrameStats::from_frame(&selected);

        assert!(raw_stats.has_handoff_scene(), "{raw_stats:?}");
        assert!(
            deinterlaced_stats.has_stage_texture_field_smear()
                || deinterlaced_stats.has_blocking_texture_page_fragment_artifact(),
            "{deinterlaced_stats:?}"
        );
        assert_ne!(
            native_frame_checksum(&selected),
            native_frame_checksum(&deinterlaced)
        );
        assert!(
            !selected_stats.has_stage_texture_field_smear(),
            "{selected_stats:?}"
        );
        assert!(selected_stats.has_visible_content(), "{selected_stats:?}");
    }

    #[test]
    fn native_play_visible_frame_holds_last_safe_frame_for_repeating_tile_grid_artifact() {
        let safe = native_play_window_frame(&test_contextual_live_stage_field_frame());
        let artifact = test_repeating_tile_grid_artifact_frame();
        let stats = NativeFrameStats::from_frame(&artifact);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(stats.has_repeating_tile_grid_artifact(), "{stats:?}");
        assert!(!stats.has_render_ready_scene(), "{stats:?}");
        assert!(!stats.has_handoff_scene(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");

        let visible =
            super::native_play_visible_window_frame(artifact.clone(), &Some(safe.clone()));
        assert_eq!(visible.pixels, safe.pixels);
    }

    #[test]
    fn native_play_presentable_frame_accepts_title_screen_as_safe_gui_frame() {
        let title = test_title_screen_frame();
        let stats = NativeFrameStats::from_frame(&title);

        assert!(stats.has_title_screen_frame(), "{stats:?}");
        assert!(!stats.has_render_ready_scene(), "{stats:?}");
        assert!(super::native_play_presentable_frame(&title), "{stats:?}");
    }

    #[test]
    fn native_play_visible_frame_holds_last_safe_frame_for_low_information_fill() {
        let safe = test_title_screen_frame();
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            if y % 3 != 0 {
                for x in 0..width {
                    pixels[y * width + x] = 0x0044_4444;
                }
            }
        }
        let flat = NativeDisplayFrame {
            width,
            height,
            pixels,
        };
        let stats = NativeFrameStats::from_frame(&flat);

        assert!(stats.has_low_information_fill_frame(), "{stats:?}");
        assert!(!super::native_play_presentable_frame(&flat), "{stats:?}");

        let visible = super::native_play_visible_window_frame(flat.clone(), &Some(safe.clone()));
        assert_eq!(visible.pixels, safe.pixels);

        let visible_without_safe = super::native_play_visible_window_frame(flat.clone(), &None);
        assert_eq!(visible_without_safe.pixels, flat.pixels);
    }

    #[test]
    fn native_play_visible_frame_keeps_current_when_no_safe_exists_for_low_information_fill() {
        let current = test_title_screen_frame();
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..(width / 2) {
                pixels[y * width + x] = 0x0044_4444;
            }
        }
        let flat = NativeDisplayFrame {
            width,
            height,
            pixels,
        };
        let stats = NativeFrameStats::from_frame(&flat);

        assert!(stats.has_low_information_fill_frame(), "{stats:?}");

        let visible = super::native_play_visible_window_frame_with_current(flat, &None, &current);
        assert_eq!(visible.pixels, current.pixels);
    }

    #[test]
    fn native_frame_stats_rejects_title_logo_overlay_without_native_context() {
        let width = 640;
        let height = 480;
        let mut frame = test_gradient_playfield_frame();
        for y in 0..height {
            for x in 0..224 {
                frame.pixels[y * width + x] = 0;
            }
        }
        for y in 286..364 {
            for x in (372..560).step_by(12) {
                for stroke_x in x..(x + 7).min(width) {
                    frame.pixels[y * width + stroke_x] = 0x00d0_1010;
                }
            }
        }
        for y in 242..400 {
            for x in 574..626 {
                let shade = 144 + ((x + y) % 96) as u32;
                frame.pixels[y * width + x] = (shade << 16) | (shade << 8) | shade;
            }
        }

        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(stats.has_title_logo_overlay(), "{stats:?}");
        assert!(!stats.has_render_ready_scene(), "{stats:?}");
        assert!(!stats.has_handoff_scene(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
        assert!(
            !native_play_frame_ready_with_context(false, &frame),
            "{stats:?}"
        );
        assert!(native_play_frame_ready_with_context(true, &frame));
    }

    #[test]
    fn native_frame_stats_rejects_saturated_transition_planes() {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                let diagonal = x + y / 2;
                let color = if (x + y * 5) % 11 == 0 {
                    0
                } else if diagonal < width / 2 {
                    0x00ff_0088
                } else if y < height / 3 {
                    let shade = ((x / 16 + y / 4) % 96) as u32;
                    ((176 + shade / 2) << 16) | (shade << 8) | (224 + shade / 4)
                } else if x > width / 3 {
                    let shade = ((x / 7 + y / 5) % 92) as u32;
                    ((144 + shade) << 16) | (shade / 3) | ((96 + shade / 2) << 8)
                } else {
                    0x0088_8888
                };
                pixels[y * width + x] = color;
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(stats.has_saturated_transition_planes(), "{stats:?}");
        assert!(!stats.has_handoff_scene(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
        assert!(
            !native_play_frame_ready_with_context(true, &frame),
            "{stats:?}"
        );
    }

    #[test]
    fn native_frame_stats_rejects_periodic_fmv_ghosting() {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                let source_x = x % 64;
                let red = 24 + ((source_x * 7 + y * 3) % 208) as u32;
                let green = 40 + ((source_x * 11 + y * 5) % 176) as u32;
                let blue = 56 + ((source_x * 13 + y * 17) % 160) as u32;
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(stats.has_periodic_horizontal_ghosting(), "{stats:?}");
        assert!(!stats.has_handoff_scene(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
        assert!(!native_play_gui_handoff_frame_ready(&frame), "{stats:?}");
    }

    #[test]
    fn native_frame_stats_rejects_low_change_periodic_fmv_ghosting() {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                let phase = (x % 64) / 8;
                let red = 24 + ((phase * 19 + y * 3) % 208) as u32;
                let green = 40 + ((phase * 23 + y * 5) % 176) as u32;
                let blue = 56 + ((phase * 29 + y * 7) % 160) as u32;
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        let stats = NativeFrameStats::from_frame(&frame);

        assert!(
            stats.periodic_horizontal_match_per_mille >= 500,
            "{stats:?}"
        );
        assert!(
            stats.horizontal_color_changes.saturating_mul(100)
                < stats.total_pixels.saturating_mul(20),
            "{stats:?}"
        );
        assert!(stats.has_periodic_horizontal_ghosting(), "{stats:?}");
        assert!(!native_play_gui_handoff_frame_ready(&frame), "{stats:?}");
    }

    #[test]
    fn native_play_frame_ready_accepts_periodic_frame_only_with_core_playable_context() {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                let source_x = x % 64;
                let red = 24 + ((source_x * 7 + y * 3) % 208) as u32;
                let green = 40 + ((source_x * 11 + y * 5) % 176) as u32;
                let blue = 56 + ((source_x * 13 + y * 17) % 160) as u32;
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };
        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_periodic_horizontal_ghosting(), "{stats:?}");
        assert!(!native_play_frame_ready_with_context(false, &frame));
        assert!(native_play_frame_ready_with_context(true, &frame));
    }

    #[test]
    fn native_frame_stats_rejects_dark_character_versus_scene_without_playfield_density() {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 64..430 {
            for x in 40..260 {
                let red = 64 + ((x * 7 + y * 3) % 160) as u32;
                let green = 32 + ((x * 5 + y * 11) % 112) as u32;
                let blue = 16 + ((x * 13 + y * 17) % 96) as u32;
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
            for x in 360..590 {
                let red = 32 + ((x * 3 + y * 5) % 96) as u32;
                let green = 48 + ((x * 11 + y * 7) % 144) as u32;
                let blue = 72 + ((x * 17 + y * 13) % 168) as u32;
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
    }

    #[test]
    fn native_frame_stats_rejects_noisy_select_screen_with_dominant_black_bucket() {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                if y < 212 || (x + y) % 9 < 5 {
                    continue;
                }
                let red = 24 + ((x * 17 + y * 3) % 208) as u32;
                let green = 16 + ((x * 5 + y * 11) % 160) as u32;
                let blue = 32 + ((x * 13 + y * 7) % 176) as u32;
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
    }

    #[test]
    fn native_play_gameplay_context_accepts_dark_match_frame() {
        let frame = test_dark_native_match_frame();
        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_render_ready_scene(), "{stats:?}");
        assert!(stats.has_handoff_scene(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
        assert!(!native_play_gameplay_scene_with_context(false, stats));
        assert!(native_play_gameplay_scene_with_context(true, stats));
    }

    #[test]
    fn native_play_context_accepts_letterboxed_stage_over_title_fallback() {
        let frame = test_letterboxed_native_stage_frame();
        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(!stats.has_title_screen_frame(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
        assert!(stats.has_letterboxed_live_stage_scene(), "{stats:?}");
        assert!(!native_play_frame_ready_with_context(false, &frame));
        assert!(native_play_frame_ready_with_context(true, &frame));
        assert!(!native_play_gameplay_scene_with_context(false, stats));
        assert!(native_play_gameplay_scene_with_context(true, stats));

        let stale_title = test_title_screen_frame();
        assert!(native_play_candidate_should_replace_stale_title(
            true,
            &frame,
            stats,
            &stale_title,
        ));
        assert!(!native_play_candidate_should_replace_stale_title(
            false,
            &frame,
            stats,
            &stale_title,
        ));
        let selected = native_play_select_visible_window_frame(
            frame.clone(),
            true,
            None,
            &Some(stale_title.clone()),
            &stale_title,
        );
        assert_eq!(
            native_frame_checksum(&selected),
            native_frame_checksum(&frame)
        );
    }

    #[test]
    fn native_play_context_accepts_texture_like_letterboxed_stage_over_title_fallback() {
        let frame = test_letterboxed_texture_like_live_stage_frame();
        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(stats.has_texture_page_fragment_artifact(), "{stats:?}");
        assert!(
            stats.has_letterboxed_stage_playfield_over_dark_band(),
            "{stats:?}"
        );
        assert!(stats.has_letterboxed_live_stage_scene(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
        assert!(!native_play_frame_ready_with_context(false, &frame));
        assert!(native_play_frame_ready_with_context(true, &frame));
        assert!(!native_play_gameplay_scene_with_context(false, stats));
        assert!(native_play_gameplay_scene_with_context(true, stats));

        let stale_title = test_title_screen_frame();
        assert!(native_play_candidate_should_replace_stale_title(
            true,
            &frame,
            stats,
            &stale_title,
        ));
    }

    #[test]
    fn native_play_context_rejects_letterboxed_stage_field_smear() {
        let frame = test_letterboxed_stage_field_smear_frame();
        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(stats.has_texture_page_fragment_artifact(), "{stats:?}");
        assert!(stats.has_stage_texture_field_smear(), "{stats:?}");
        assert!(stats.has_context_stage_letterbox(), "{stats:?}");
        assert!(stats.has_context_live_stage_hud(), "{stats:?}");
        assert!(
            !stats.has_letterboxed_stage_playfield_over_dark_band(),
            "{stats:?}"
        );
        assert!(!stats.has_letterboxed_live_stage_scene(), "{stats:?}");
        assert!(!stats.has_render_ready_scene(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
        assert!(
            !native_play_frame_ready_with_context(true, &frame),
            "{stats:?}"
        );
        assert!(
            !native_play_gameplay_scene_with_context(true, stats),
            "{stats:?}"
        );

        let stale_title = test_title_screen_frame();
        assert!(!native_play_candidate_should_replace_stale_title(
            true,
            &frame,
            stats,
            &stale_title,
        ));
        let visible =
            super::native_play_visible_window_frame(frame.clone(), &Some(stale_title.clone()));
        assert_eq!(
            native_frame_checksum(&visible),
            native_frame_checksum(&stale_title)
        );
    }

    #[test]
    fn native_play_context_rejects_sparse_texture_fragment_handoff_artifact() {
        let frame = test_sparse_texture_fragment_handoff_artifact_frame();
        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(
            stats.has_sparse_texture_fragment_handoff_artifact(),
            "{stats:?}"
        );
        assert!(!stats.has_render_ready_scene(), "{stats:?}");
        assert!(!stats.has_handoff_scene(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
        assert!(
            !native_play_frame_ready_with_context(true, &frame),
            "{stats:?}"
        );
        assert!(
            !native_play_gameplay_scene_with_context(true, stats),
            "{stats:?}"
        );
    }

    #[test]
    fn native_frame_stats_rejects_low_palette_high_frequency_corruption() {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                let index = ((x + y * 5) % 96) as u32;
                let red = 16 + (index.wrapping_mul(29) % 224);
                let green = 16 + (index.wrapping_mul(47) % 224);
                let blue = 16 + (index.wrapping_mul(61) % 224);
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_scene_detail(), "{stats:?}");
        assert!(stats.has_low_palette_noise(), "{stats:?}");
        assert!(!stats.has_handoff_scene(), "{stats:?}");
        assert!(!stats.has_gameplay_scene(), "{stats:?}");
        assert!(!native_play_gui_handoff_frame_ready(&frame), "{stats:?}");
    }

    #[test]
    fn native_frame_stats_accepts_varied_full_playfield() {
        let width = 640;
        let height = 480;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                let red = 16 + ((x * 3 + y * 5) % 112) as u32;
                let green = 64 + ((x * 7 + y * 11) % 160) as u32;
                let blue = 48 + ((x * 13 + y * 17) % 176) as u32;
                pixels[y * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        let stats = NativeFrameStats::from_frame(&frame);

        assert!(stats.has_gameplay_scene(), "{stats:?}");
        assert!(native_play_gui_handoff_frame_ready(&frame), "{stats:?}");
    }

    #[test]
    fn native_gap_observed_frame_score_prefers_playfield_over_sparse_final_frame() {
        let playfield_stats = NativeFrameStats::from_frame(&test_gradient_playfield_frame());
        let sparse_stats = NativeFrameStats::from_frame(&test_solid_frame(0));

        assert!(playfield_stats.has_gameplay_scene(), "{playfield_stats:?}");
        assert!(!sparse_stats.has_visible_content(), "{sparse_stats:?}");
        assert!(
            native_gap_observed_frame_score(playfield_stats, true, true)
                > native_gap_observed_frame_score(sparse_stats, true, false),
            "gap probe best-frame tracking must not let a sparse final frame hide a valid observed playfield"
        );
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

    #[test]
    fn native_input_latch_preserves_brief_button_polls() {
        let mut latch = NativeInputLatch::default();
        let start = ActionButtons {
            start: true,
            ..ActionButtons::default()
        };

        assert_eq!(latch.buttons(start), start);
        assert_eq!(latch.buttons(ActionButtons::default()), start);

        for _ in 0..super::NATIVE_PLAY_INPUT_LATCH_POLLS {
            latch.buttons(ActionButtons::default());
        }

        assert_eq!(
            latch.buttons(ActionButtons::default()),
            ActionButtons::default()
        );
    }

    #[test]
    fn native_play_effective_buttons_prioritizes_manual_keyboard_over_script() {
        let manual = ActionButtons {
            left: true,
            punch: true,
            ..ActionButtons::default()
        };

        assert_eq!(
            native_play_effective_buttons(manual, Some(Action::Start)),
            manual
        );
        assert_eq!(
            native_play_effective_buttons(ActionButtons::default(), Some(Action::Start)),
            Action::Start.buttons()
        );
        assert_eq!(
            native_play_effective_buttons(ActionButtons::default(), None),
            ActionButtons::default()
        );
    }

    #[test]
    fn native_pressed_key_events_map_to_play_buttons() {
        let buttons =
            native_keys_to_buttons([Key::Enter, Key::C, Key::Up, Key::Z, Key::X, Key::Q, Key::E]);

        assert!(buttons.start);
        assert!(buttons.coin);
        assert!(buttons.up);
        assert!(buttons.punch);
        assert!(buttons.kick);
        assert!(buttons.beast);
        assert!(buttons.guard);

        let enter = native_keys_to_buttons([Key::Enter]);
        assert!(enter.start);
        assert!(enter.coin);

        let pure_start = native_keys_to_buttons([Key::P]);
        assert!(pure_start.start);
        assert!(!pure_start.coin);
    }
}
