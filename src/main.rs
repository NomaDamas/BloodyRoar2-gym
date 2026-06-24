use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use bloodyroar2_gym::{
    ACTION_SPACE, Action, ActionButtons, BloodyRoar2Env, MameConfig, MameRuntime,
    NativeDisplayFrame, NativeEmulator, NativeInputActivity, NativeRomSet, NativeTraceConfig,
    NullBackend, ZincConfig, ZincRuntime, action_space_json, api_index_json,
    observation_space_json, png_from_rgb888_pixels,
};
use minifb::{Key, Scale, ScaleMode, Window, WindowOptions};

const NATIVE_PLAY_MIN_WINDOW_WIDTH: usize = 640;
const NATIVE_PLAY_MIN_WINDOW_HEIGHT: usize = 480;
const NATIVE_PLAY_FAST_FORWARD_MAX_FRAMES: u64 = 2_400;
const NATIVE_PLAY_SCRIPT_INSTRUCTIONS_PER_FRAME: u64 = 500_000;
const NATIVE_PLAY_GUI_MIN_INSTRUCTIONS_PER_FRAME: u64 = 500_000;
const NATIVE_PLAY_FAST_FORWARD_INSTRUCTIONS_PER_FRAME: u64 =
    NATIVE_PLAY_SCRIPT_INSTRUCTIONS_PER_FRAME;
const NATIVE_PLAY_HANDOFF_SETTLE_FRAMES: u64 = 120;
const NATIVE_PLAY_HANDOFF_CHECK_STRIDE_FRAMES: u64 = 6;
const NATIVE_PLAY_GUI_INPUT_SLICE_INSTRUCTIONS: u64 = 5_000;
const NATIVE_PLAY_INPUT_LATCH_POLLS: u8 = 45;
const NATIVE_VBLANK_TRACE_MAX_INSTRUCTIONS: u64 = 100_000;

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
        "native-cache-prepare" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms/bldyror2.zip"));
            let romset = NativeRomSet::scan_cached(rom).map_err(|error| error.to_string())?;
            println!("{}", romset.compatibility_report().summary_json());
            Ok(())
        }
        "native-cache-path" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms/bldyror2.zip"));
            let romset = NativeRomSet::scan_cached(rom).map_err(|error| error.to_string())?;
            println!("{}", romset.path.display());
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
            write_output_file(&output, emulator.screenshot_png())?;
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
            write_output_file(&output, emulator.display_png())?;
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
            write_output_file(&output, emulator.vram_png())?;
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
        "native-play-snapshot" => {
            let rom = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("assets/roms"));
            let instructions_per_frame = args
                .next()
                .unwrap_or_else(|| "500000".to_string())
                .parse::<u64>()
                .map_err(|_| "instructions_per_frame must be a positive integer".to_string())?;
            let output_prefix = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("native-play-snapshot"));
            let mut complete_script = false;
            let mut fast_forward_max_frames = NATIVE_PLAY_FAST_FORWARD_MAX_FRAMES;
            let mut tail_values = Vec::new();
            let mut tail_args = args.peekable();
            while let Some(value) = tail_args.next() {
                match value.as_str() {
                    "--complete-script" => complete_script = true,
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
                fast_forward_max_frames,
                tail_segments,
            )
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
                true,
                true,
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
                false,
                false,
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
                true,
                false,
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
            let instructions_per_frame =
                native_fast_forward_instructions_per_frame(instructions_per_frame);
            let checkpoint_run =
                run_native_script_observed(&mut emulator, instructions_per_frame, &segments);
            let total_frames = checkpoint_run.total_frames;
            let observed_native_playable_candidate =
                checkpoint_run.observed_native_playable_candidate;
            let mut first_native_playable_frame = checkpoint_run.first_native_playable_frame;
            let mut last_native_playable_frame = checkpoint_run.last_native_playable_frame;
            let checkpoint = emulator.clone();
            let checkpoint_frame = checkpoint.display_frame();
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
                        NativeScriptSegment { action, frames: 18 },
                        NativeScriptSegment {
                            action: Action::Noop,
                            frames: 18,
                        },
                    ];
                    let branch_run = run_native_script_observed(
                        &mut branch,
                        instructions_per_frame,
                        &branch_segments,
                    );
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
                    control_sweep_native_playable_candidate |=
                        control_sweep.native_playable_candidate();
                    let branch_activity = control_sweep.input_activity();
                    let branch_delta = branch_activity.saturating_subtracted(checkpoint_activity);
                    let action_read = native_action_activity_observed(branch_delta, action);
                    let branch_frame = control_sweep.display_frame();
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
                        "{{\"action_index\":{},\"action\":\"{}\",\"action_activity_observed\":{},\"visual_changed\":{},\"frame_checksum\":{},\"native_playable_candidate\":{},\"run\":{},\"input_delta\":{},\"frame\":{}}}",
                        action.index(),
                        action.name(),
                        action_read,
                        visual_changed,
                        branch_frame_checksum,
                        control_sweep.native_playable_candidate(),
                        branch_run.json(),
                        branch_delta.json(),
                        branch_frame_stats.json()
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
            let final_native_playable_candidate = checkpoint.native_playable_candidate();
            let input_controls_active = input_activity.has_play_control_activity();
            let full_controls_active = input_activity.has_full_control_activity();
            let all_branch_actions_read =
                branch_action_reads == native_health_branch_actions().len();
            let branch_visual_response = branch_visual_changes > 0;
            let manual_checkpoint_ready =
                checkpoint_frame_stats.has_visible_content() && !checkpoint.is_terminal();
            let manual_controls_operable = manual_checkpoint_ready
                && all_branch_actions_read
                && branch_visual_response
                && full_controls_active;
            let native_playability_confirmed =
                final_native_playable_candidate || control_sweep_native_playable_candidate;
            let playable = manual_controls_operable && native_playability_confirmed;
            let first_native_playable_frame = optional_u64_json(first_native_playable_frame);
            let last_native_playable_frame = optional_u64_json(last_native_playable_frame);
            println!(
                "{{\"instructions_per_frame\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"control_sweep_frames\":{},\"control_sweep_missed_vblank_frames\":{},\"checkpoint_executed_steps\":{},\"control_sweep_executed_steps\":{},\"executed_steps\":{},\"input_activity\":{},\"native_playable_candidate\":{},\"observed_native_playable_candidate\":{},\"first_native_playable_frame\":{},\"last_native_playable_frame\":{},\"final_native_playable_candidate\":{},\"control_sweep_native_playable_candidate\":{},\"input_controls_active\":{},\"full_controls_active\":{},\"all_branch_actions_read\":{},\"branch_action_reads\":{},\"branch_count\":{},\"branch_visual_response\":{},\"branch_visual_changes\":{},\"manual_checkpoint_ready\":{},\"manual_controls_operable\":{},\"native_playability_confirmed\":{},\"checkpoint_frame_checksum\":{},\"checkpoint_frame\":{},\"playable\":{},\"branches\":[{}],\"state\":{}}}",
                instructions_per_frame,
                total_frames_with_sweep,
                checkpoint_run.missed_vblank_frames,
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
                manual_checkpoint_ready,
                manual_controls_operable,
                native_playability_confirmed,
                checkpoint_frame_checksum,
                checkpoint_frame_stats.json(),
                playable,
                branches.join(","),
                checkpoint.compact_probe_json()
            );
            if manual_controls_operable {
                Ok(())
            } else {
                Err(
                    "native input check failed: manual checkpoint, mapped controls, or visual branch response was not observed"
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
            let script_run =
                run_native_script_observed(&mut emulator, instructions_per_frame, &segments);
            let total_frames = script_run.total_frames;

            write_output_file(&output, emulator.screenshot_png())?;
            println!(
                "{{\"output\":\"{}\",\"instructions_per_frame\":{},\"total_frames\":{},\"script_run\":{},\"executed_steps\":{},\"segments\":[{}],\"state\":{}}}",
                escape_json(&output.display().to_string()),
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
            let script_run =
                run_native_script_observed(&mut emulator, instructions_per_frame, &segments);
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
                "{{\"actual_display_output\":\"{}\",\"raw_actual_display_output\":\"{}\",\"display_output\":\"{}\",\"observation_output\":\"{}\",\"vram_output\":\"{}\",\"window_output\":\"{}\",\"window_frame\":{},\"instructions_per_frame\":{},\"total_frames\":{},\"script_run\":{},\"executed_steps\":{},\"segments\":[{}],\"state\":{}}}",
                escape_json(&actual_display_output.display().to_string()),
                escape_json(&raw_actual_display_output.display().to_string()),
                escape_json(&display_output.display().to_string()),
                escape_json(&observation_output.display().to_string()),
                escape_json(&vram_output.display().to_string()),
                escape_json(&window_output.display().to_string()),
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
            let script_run =
                run_native_script_observed(&mut emulator, instructions_per_frame, &segments);
            let total_frames = script_run.total_frames;
            let candidates = write_native_display_candidates(&emulator, &output_prefix)?;
            println!(
                "{{\"candidate_outputs\":[{}],\"instructions_per_frame\":{},\"total_frames\":{},\"script_run\":{},\"executed_steps\":{},\"segments\":[{}],\"state\":{}}}",
                candidates.join(","),
                instructions_per_frame,
                total_frames,
                script_run.json(),
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
            let script_run =
                run_native_script_observed(&mut emulator, instructions_per_frame, &segments);
            let total_frames = script_run.total_frames;
            println!(
                "{{\"instructions_per_frame\":{},\"total_frames\":{},\"script_run\":{},\"executed_steps\":{},\"segments\":[{}],\"state\":{}}}",
                instructions_per_frame,
                total_frames,
                script_run.json(),
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
            let mut missed_vblank_frames = 0u64;
            let mut probes = Vec::new();

            for (index, segment) in segments.iter().enumerate() {
                let segment_run =
                    run_native_script_observed(&mut emulator, instructions_per_frame, &[*segment]);
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
            let rom = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-compact-probe <rom_zip_or_dir> <instructions_per_frame> <probe_stride_frames> <action:frames>..."
                    .to_string()
            })?;
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
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
            let instructions_per_frame = instructions_per_frame.max(1);
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
        "native-scripted-live-probe" => {
            let rom = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-live-probe <rom_zip_or_dir> <instructions_per_frame> <emit_stride_frames> <action:frames>..."
                    .to_string()
            })?;
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
        "native-scripted-vblank-trace" => {
            let rom = args.next().map(PathBuf::from).ok_or_else(|| {
                "usage: bloodyroar2-gym native-scripted-vblank-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <warmup_action:frames>... --trace <trace_action:frames>... [-- <trace options>]"
                    .to_string()
            })?;
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
            let mut emulator =
                NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
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

            let warmup_run = run_native_script_observed(
                &mut emulator,
                instructions_per_frame.max(1),
                &warmup_segments,
            );
            writeln!(
                stdout,
                "{{\"event\":\"warmup\",\"warmup_run\":{},\"state\":{}}}",
                warmup_run.json(),
                emulator.compact_probe_json()
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
                    writeln!(
                        stdout,
                        "{{\"event\":\"trace_frame\",\"segment_index\":{},\"action_index\":{},\"action\":\"{}\",\"frame_in_segment\":{},\"segment_frames\":{},\"traced_frames\":{},\"start_vblank\":{},\"end_vblank\":{},\"vblank_advanced\":{},\"trace\":{},\"state\":{}}}",
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
                        emulator.compact_probe_json()
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
                emulator.compact_probe_json()
            )
            .map_err(|error| format!("failed to write vblank trace finish: {error}"))?;
            stdout
                .flush()
                .map_err(|error| format!("failed to flush vblank trace finish: {error}"))?;
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
                    branch.compact_probe_json()
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
                checkpoint.compact_probe_json(),
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
    fast_forward_script: bool,
    stop_script_at_first_playable: bool,
) -> Result<(), String> {
    let mut emulator = NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
    let autoplay_enabled = !script_segments.is_empty();
    let mut rendered_frames = 0u64;
    let mut gui_rendered_frames = 0u64;
    let mut scripted_frames = 0u64;
    let mut scripted_missed_vblank_frames = 0u64;
    let initial_raw_candidate_frame = emulator.display_frame();
    let mut observed_native_playable_candidate = emulator.native_playable_candidate()
        && native_play_gui_handoff_frame_ready(&native_play_window_frame(
            &initial_raw_candidate_frame,
        ));
    let mut first_native_playable_frame =
        observed_native_playable_candidate.then_some(rendered_frames);
    let mut last_native_playable_frame = first_native_playable_frame;
    let mut script_segment_index = 0usize;
    let mut script_segment_frame = 0u64;

    if autoplay_enabled && fast_forward_script {
        let fast_forward_instructions_per_frame =
            native_fast_forward_instructions_per_frame(instructions_per_frame);
        let script_progress = if stop_script_at_first_playable {
            run_native_script_observed_until_playable_with_limit(
                &mut emulator,
                fast_forward_instructions_per_frame,
                &script_segments,
                NATIVE_PLAY_FAST_FORWARD_MAX_FRAMES,
            )
        } else {
            run_native_script_observed_with_stop(
                &mut emulator,
                fast_forward_instructions_per_frame,
                &script_segments,
                NativeScriptStopMode::None,
                Some(NATIVE_PLAY_FAST_FORWARD_MAX_FRAMES),
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
    let gui_instructions_per_frame = native_play_gui_instructions_per_frame(instructions_per_frame);

    let initial_raw_frame = emulator.display_frame();
    let initial_frame = native_play_window_frame(&initial_raw_frame);
    let title = if autoplay_enabled {
        "Bloody Roar 2 native Rust - autoplay to playable, keys override on stable render, Esc quit"
    } else {
        "Bloody Roar 2 native Rust - arrows/WASD move, Z/Space/J punch, X/K kick, Q/L beast, E/I guard, C coin, Enter/P start, Esc quit"
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
    window.set_target_fps(60);
    let mut current_window_frame = initial_frame.clone();
    let mut input_latch = NativeInputLatch::default();
    window
        .update_with_buffer(
            &current_window_frame.pixels,
            current_window_frame.width,
            current_window_frame.height,
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
            &current_window_frame,
            scripted_action,
            manual_override_enabled,
            &mut input_latch,
        )?;
        if step.window_closed_or_escape {
            break;
        }
        window.set_title(&native_play_runtime_title(
            autoplay_enabled,
            step.buttons,
            emulator.native_playable_candidate(),
            emulator.gpu_native_playable_candidate(),
            emulator.native_3d_gameplay_signal(),
        ));
        if step.manual_override {
            script_segment_index = script_segments.len();
            script_segment_frame = 0;
        }
        if scripted_action.is_some() && !step.manual_override {
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
        let raw_frame = emulator.display_frame();
        let frame = native_play_window_frame(&raw_frame);
        window
            .update_with_buffer(&frame.pixels, frame.width, frame.height)
            .map_err(|error| format!("failed to update native play window: {error:?}"))?;
        rendered_frames += 1;
        gui_rendered_frames += 1;
        let handoff_frame_ready = native_play_gui_handoff_frame_ready(&frame);
        current_window_frame = frame;
        if emulator.native_playable_candidate() && handoff_frame_ready {
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
    let final_frame_visible_content = final_frame_stats.has_visible_content();
    let final_frame_scene_detail = final_frame_stats.has_scene_detail();
    let final_frame_gameplay_scene = final_frame_stats.has_gameplay_scene();
    let final_frame_handoff_scene = final_frame_stats.has_handoff_scene();
    let final_frame_render_verified =
        final_frame_full_size && (final_frame_gameplay_scene || final_frame_handoff_scene);
    let final_window_size = window.get_size();
    let input_activity = emulator.input_activity();
    let input_controls_active = input_activity.has_play_control_activity();
    let full_controls_active = input_activity.has_full_control_activity();
    let native_play_input_verified = observed_native_playable_candidate && input_controls_active;
    let native_play_full_input_verified =
        observed_native_playable_candidate && full_controls_active;
    let playable = observed_native_playable_candidate
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
    println!(
        "{{\"rendered_frames\":{},\"gui_rendered_frames\":{},\"executed_steps\":{},\"autoplay_enabled\":{},\"autoplay_fast_forwarded\":{},\"autoplay_stop_at_first_playable\":{},\"autoplay_fast_forward_max_frames\":{},\"autoplay_fast_forward_instructions_per_frame\":{},\"gui_instructions_per_frame\":{},\"autoplay_script_completed\":{},\"autoplay_scripted_frames\":{},\"autoplay_missed_vblank_frames\":{},\"autoplay_segments\":[{}],\"initial_raw_frame\":{},\"initial_window_frame\":{},\"final_raw_frame\":{},\"final_window_size\":{{\"width\":{},\"height\":{}}},\"input_activity\":{},\"native_playable_candidate\":{},\"observed_native_playable_candidate\":{},\"first_native_playable_frame\":{},\"last_native_playable_frame\":{},\"final_native_playable_candidate\":{},\"input_controls_active\":{},\"full_controls_active\":{},\"native_play_input_verified\":{},\"native_play_full_input_verified\":{},\"final_frame_full_size\":{},\"final_frame_visible_content\":{},\"final_frame_scene_detail\":{},\"final_frame_gameplay_scene\":{},\"final_frame_handoff_scene\":{},\"final_frame_render_verified\":{},\"final_frame\":{},\"playable\":{},\"state\":{}}}",
        rendered_frames,
        gui_rendered_frames,
        emulator.executed_steps(),
        autoplay_enabled,
        autoplay_enabled && fast_forward_script,
        autoplay_enabled && fast_forward_script && stop_script_at_first_playable,
        if autoplay_enabled && fast_forward_script {
            NATIVE_PLAY_FAST_FORWARD_MAX_FRAMES
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
        final_frame_visible_content,
        final_frame_scene_detail,
        final_frame_gameplay_scene,
        final_frame_handoff_scene,
        final_frame_render_verified,
        final_frame_stats.json(),
        playable,
        emulator.probe_json()
    );
    Ok(())
}

fn run_native_play_snapshot(
    rom: PathBuf,
    instructions_per_frame: u64,
    output_prefix: &Path,
    complete_script: bool,
    fast_forward_max_frames: u64,
    tail_segments: Vec<NativeScriptSegment>,
) -> Result<(), String> {
    let mut emulator = NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
    let handoff_segments = default_native_play_script();
    let fast_forward_instructions_per_frame =
        native_fast_forward_instructions_per_frame(instructions_per_frame);
    let progress = run_native_script_observed_until_playable_with_limit(
        &mut emulator,
        fast_forward_instructions_per_frame,
        &handoff_segments,
        fast_forward_max_frames,
    );
    let boot_summary = progress.summary;
    let boot_prefix = suffixed_path(output_prefix, "boot-playable");
    let (
        boot_actual_display_output,
        boot_raw_actual_display_output,
        boot_display_output,
        boot_observation_output,
        boot_vram_output,
    ) = write_native_snapshot(&emulator, &boot_prefix)?;
    let boot_raw_frame = emulator.display_frame();
    let boot_window_frame = native_play_window_frame(&boot_raw_frame);
    let boot_window_output = suffixed_path(&boot_prefix, "window.png");
    write_native_display_frame_png(&boot_window_frame, &boot_window_output)?;
    let boot_frame_stats = NativeFrameStats::from_frame(&boot_window_frame);

    let mut effective_tail_segments = if complete_script {
        remaining_native_script_segments(
            &handoff_segments,
            progress.segment_index,
            progress.segment_frame,
        )
    } else {
        Vec::new()
    };
    effective_tail_segments.extend(tail_segments);
    let tail_run_json = if effective_tail_segments.is_empty() {
        "null".to_string()
    } else {
        run_native_script_observed_with_stop(
            &mut emulator,
            fast_forward_instructions_per_frame,
            &effective_tail_segments,
            NativeScriptStopMode::None,
            Some(fast_forward_max_frames),
        )
        .summary
        .json()
    };

    let (
        actual_display_output,
        raw_actual_display_output,
        display_output,
        observation_output,
        vram_output,
    ) = write_native_snapshot(&emulator, output_prefix)?;
    let final_raw_frame = emulator.display_frame();
    let final_window_frame = native_play_window_frame(&final_raw_frame);
    let window_output = suffixed_path(output_prefix, "window.png");
    write_native_display_frame_png(&final_window_frame, &window_output)?;
    let final_frame_stats = NativeFrameStats::from_frame(&final_window_frame);
    let final_native_playable_candidate = emulator.native_playable_candidate();
    let final_frame_gameplay_scene = final_frame_stats.has_gameplay_scene();
    let final_frame_handoff_scene = final_frame_stats.has_handoff_scene();
    let playable = boot_summary.observed_native_playable_candidate
        && final_native_playable_candidate
        && final_frame_stats.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
        && final_frame_stats.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT
        && (final_frame_gameplay_scene || final_frame_handoff_scene);

    println!(
        "{{\"output_prefix\":\"{}\",\"instructions_per_frame\":{},\"fast_forward_instructions_per_frame\":{},\"fast_forward_max_frames\":{},\"complete_script\":{},\"boot\":{{\"run\":{},\"actual_display_output\":\"{}\",\"raw_actual_display_output\":\"{}\",\"display_output\":\"{}\",\"observation_output\":\"{}\",\"vram_output\":\"{}\",\"window_output\":\"{}\",\"window_frame\":{}}},\"tail_segments\":[{}],\"tail_run\":{},\"final\":{{\"actual_display_output\":\"{}\",\"raw_actual_display_output\":\"{}\",\"display_output\":\"{}\",\"observation_output\":\"{}\",\"vram_output\":\"{}\",\"window_output\":\"{}\",\"window_frame\":{},\"gameplay_scene\":{},\"handoff_scene\":{},\"native_playable_candidate\":{},\"playable\":{}}},\"executed_steps\":{},\"state\":{}}}",
        escape_json(&output_prefix.display().to_string()),
        instructions_per_frame,
        fast_forward_instructions_per_frame,
        fast_forward_max_frames,
        complete_script,
        boot_summary.json(),
        escape_json(&boot_actual_display_output.display().to_string()),
        escape_json(&boot_raw_actual_display_output.display().to_string()),
        escape_json(&boot_display_output.display().to_string()),
        escape_json(&boot_observation_output.display().to_string()),
        escape_json(&boot_vram_output.display().to_string()),
        escape_json(&boot_window_output.display().to_string()),
        boot_frame_stats.json(),
        native_script_segments_json(&effective_tail_segments),
        tail_run_json,
        escape_json(&actual_display_output.display().to_string()),
        escape_json(&raw_actual_display_output.display().to_string()),
        escape_json(&display_output.display().to_string()),
        escape_json(&observation_output.display().to_string()),
        escape_json(&vram_output.display().to_string()),
        escape_json(&window_output.display().to_string()),
        final_frame_stats.json(),
        final_frame_gameplay_scene,
        final_frame_handoff_scene,
        final_native_playable_candidate,
        playable,
        emulator.executed_steps(),
        emulator.probe_json()
    );

    if playable {
        Ok(())
    } else if !boot_summary.observed_native_playable_candidate {
        Err("native play snapshot failed: playable frame was not observed".into())
    } else {
        Err("native play snapshot failed: final frame did not meet gameplay render criteria".into())
    }
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

fn write_native_display_frame_png(frame: &NativeDisplayFrame, output: &Path) -> Result<(), String> {
    write_output_file(
        output,
        png_from_rgb888_pixels(frame.width, frame.height, &frame.pixels),
    )
}

fn native_play_window_frame(frame: &NativeDisplayFrame) -> NativeDisplayFrame {
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

    let deinterlace = native_play_should_deinterlace_frame(frame);
    let deinterlace_field_offset = if deinterlace {
        native_play_deinterlace_field_offset(frame)
    } else {
        0
    };
    for y in 0..height {
        let source_y = native_play_source_y(
            y,
            frame.height,
            height,
            deinterlace,
            deinterlace_field_offset,
        );
        let target_start = y.saturating_mul(width);
        for x in 0..width {
            let source_x = x.saturating_mul(frame.width) / width;
            pixels[target_start + x] = native_play_window_source_pixel(
                frame,
                source_x,
                source_y,
                deinterlace,
                deinterlace_field_offset,
            );
        }
    }

    NativeDisplayFrame {
        width,
        height,
        pixels,
    }
}

fn native_play_should_deinterlace_frame(frame: &NativeDisplayFrame) -> bool {
    frame.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT && frame.width <= 384
}

fn native_play_source_y(
    target_y: usize,
    source_height: usize,
    target_height: usize,
    deinterlace: bool,
    deinterlace_field_offset: usize,
) -> usize {
    if deinterlace {
        let field_height = (source_height / 2).max(1);
        let field_y = (target_y.saturating_mul(field_height) / target_height)
            .min(field_height.saturating_sub(1));
        return deinterlace_field_offset
            .saturating_add(field_y)
            .min(source_height.saturating_sub(1));
    }

    target_y
        .saturating_mul(source_height)
        .checked_div(target_height)
        .unwrap_or_default()
        .min(source_height.saturating_sub(1))
}

fn native_play_deinterlace_field_offset(frame: &NativeDisplayFrame) -> usize {
    let field_height = (frame.height / 2).max(1);
    let top_score = native_play_deinterlace_field_score(frame, 0, field_height);
    let bottom_score = native_play_deinterlace_field_score(frame, field_height, field_height);
    if bottom_score > top_score {
        field_height
    } else {
        0
    }
}

fn native_play_deinterlace_field_score(
    frame: &NativeDisplayFrame,
    start_y: usize,
    field_height: usize,
) -> u64 {
    let end_y = start_y.saturating_add(field_height).min(frame.height);
    let mut nonzero = 0u64;
    let mut color_changes = 0u64;
    let mut luma_sum = 0u64;
    for y in start_y..end_y {
        let row = y.saturating_mul(frame.width);
        let mut previous = None;
        for x in 0..frame.width {
            let color = frame.pixels[row + x];
            if color != 0 {
                nonzero += 1;
            }
            if previous.is_some_and(|previous| previous != color) {
                color_changes += 1;
            }
            previous = Some(color);
            let red = u64::from((color >> 16) & 0xff);
            let green = u64::from((color >> 8) & 0xff);
            let blue = u64::from(color & 0xff);
            luma_sum += red.saturating_mul(30) + green.saturating_mul(59) + blue.saturating_mul(11);
        }
    }

    nonzero
        .saturating_mul(4)
        .saturating_add(color_changes.saturating_mul(3))
        .saturating_add(luma_sum / 10_000)
}

fn native_play_window_source_pixel(
    frame: &NativeDisplayFrame,
    source_x: usize,
    source_y: usize,
    deinterlace: bool,
    deinterlace_field_offset: usize,
) -> u32 {
    if deinterlace {
        return native_play_filtered_field_pixel(
            frame,
            source_x,
            source_y,
            deinterlace_field_offset,
        );
    }
    native_play_source_pixel(frame, source_x, source_y)
}

fn native_play_filtered_field_pixel(
    frame: &NativeDisplayFrame,
    source_x: usize,
    source_y: usize,
    field_offset: usize,
) -> u32 {
    let field_height = (frame.height / 2).max(1);
    let field_end = field_offset.saturating_add(field_height).min(frame.height);
    let center = native_play_source_pixel(frame, source_x, source_y);
    let previous_y = source_y.saturating_sub(1).max(field_offset);
    let next_y = source_y.saturating_add(1).min(field_end.saturating_sub(1));
    if previous_y == source_y && next_y == source_y {
        return center;
    }

    let previous = native_play_source_pixel(frame, source_x, previous_y);
    let next = native_play_source_pixel(frame, source_x, next_y);
    native_play_weighted_rgb_average(previous, center, next)
}

fn native_play_weighted_rgb_average(previous: u32, center: u32, next: u32) -> u32 {
    let red =
        (((previous >> 16) & 0xff) + (((center >> 16) & 0xff) * 2) + ((next >> 16) & 0xff)) / 4;
    let green =
        (((previous >> 8) & 0xff) + (((center >> 8) & 0xff) * 2) + ((next >> 8) & 0xff)) / 4;
    let blue = ((previous & 0xff) + ((center & 0xff) * 2) + (next & 0xff)) / 4;
    (red << 16) | (green << 8) | blue
}

fn native_play_source_pixel(frame: &NativeDisplayFrame, source_x: usize, source_y: usize) -> u32 {
    frame.pixels[source_y.saturating_mul(frame.width) + source_x]
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
    visible_frame: &NativeDisplayFrame,
    scripted_action: Option<Action>,
    manual_override_enabled: bool,
    input_latch: &mut NativeInputLatch,
) -> Result<NativePlayWindowStep, String> {
    let start_vblank = emulator.vblank_count();
    let mut remaining = instructions_per_frame.max(1);

    let mut last_buttons = ActionButtons::default();
    let mut manual_override = false;
    while remaining > 0
        && emulator.vblank_count() == start_vblank
        && !emulator.is_terminal()
        && window.is_open()
        && !window.is_key_down(Key::Escape)
    {
        let manual_buttons = input_latch.buttons(native_window_buttons(window));
        manual_override |= manual_override_enabled && action_buttons_any(manual_buttons);
        let buttons = native_play_effective_buttons(manual_buttons, scripted_action);
        last_buttons = buttons;
        emulator.set_input(buttons);
        let slice = remaining.min(NATIVE_PLAY_GUI_INPUT_SLICE_INSTRUCTIONS);
        emulator.step_until_next_vblank(slice);
        remaining = remaining.saturating_sub(slice);

        if emulator.vblank_count() == start_vblank && !emulator.is_terminal() {
            window
                .update_with_buffer(
                    &visible_frame.pixels,
                    visible_frame.width,
                    visible_frame.height,
                )
                .map_err(|error| format!("failed to poll native play window input: {error:?}"))?;
        }
    }

    Ok(NativePlayWindowStep {
        vblank_advanced: emulator.vblank_count() != start_vblank,
        window_closed_or_escape: !window.is_open() || window.is_key_down(Key::Escape),
        buttons: last_buttons,
        manual_override,
    })
}

fn run_native_scripted_live_probe(
    rom: PathBuf,
    instructions_per_frame: u64,
    emit_stride: u64,
    segments: Vec<NativeScriptSegment>,
) -> Result<(), String> {
    let mut emulator = NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
    let mut total_frames = 0u64;
    let mut missed_vblank_frames = 0u64;
    let mut stdout = io::stdout();

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

    for (segment_index, segment) in segments.iter().enumerate() {
        emulator.set_input(segment.action.buttons());
        for frame_in_segment in 1..=segment.frames {
            let vblank_advanced =
                step_until_next_vblank_checked(&mut emulator, instructions_per_frame);
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
                break;
            }
        }
        if emulator.is_terminal() {
            break;
        }
    }

    writeln!(
        stdout,
        "{{\"event\":\"finish\",\"instructions_per_frame\":{},\"emit_stride_frames\":{},\"total_frames\":{},\"missed_vblank_frames\":{},\"executed_steps\":{},\"terminal\":{},\"state\":{}}}",
        instructions_per_frame,
        emit_stride,
        total_frames,
        missed_vblank_frames,
        emulator.executed_steps(),
        emulator.is_terminal(),
        emulator.compact_probe_json()
    )
    .map_err(|error| format!("failed to write live probe finish: {error}"))?;
    stdout
        .flush()
        .map_err(|error| format!("failed to flush live probe finish: {error}"))?;
    Ok(())
}

fn run_native_health_check(
    rom: PathBuf,
    instructions_per_frame: u64,
    branch_frames: u64,
    settle_frames: u64,
) -> Result<(), String> {
    let instructions_per_frame = native_fast_forward_instructions_per_frame(instructions_per_frame);
    let romset = NativeRomSet::scan(rom.clone()).map_err(|error| error.to_string())?;
    let rom_compatibility = romset.compatibility_report();
    let assets_complete = rom_compatibility.native_runtime_usable();
    let exact_mame_assets = rom_compatibility.compatible();

    let mut checkpoint =
        NativeEmulator::from_rom_zip(rom.clone()).map_err(|error| error.to_string())?;
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
    let mut branch_input_activity = NativeInputActivity::default();
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
        branch_input_activity = branch_input_activity
            .saturating_added(branch_activity.saturating_subtracted(checkpoint_activity));
        let branch_stats = NativeFrameStats::from_frame(&branch.display_frame());
        let branch_native_playable = branch.native_playable_candidate();
        all_branch_actions_read &= action_read;
        if branch_native_playable {
            branch_native_playable_count += 1;
        }
        if branch_native_playable && branch_stats.has_gameplay_scene() {
            branch_full_scene_count += 1;
        }
        branches.push(format!(
            "{{\"action_index\":{},\"action\":\"{}\",\"action_activity_observed\":{},\"native_playable_candidate\":{},\"gameplay_scene\":{},\"terminal\":{},\"run\":{},\"input_activity\":{},\"frame\":{},\"state\":{}}}",
            action.index(),
            action.name(),
            action_read,
            branch_native_playable,
            branch_stats.has_gameplay_scene(),
            branch.is_terminal(),
            branch_run.json(),
            branch_activity.json(),
            branch_stats.json(),
            branch.compact_probe_json()
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

    let mut match_entry = NativeEmulator::from_rom_zip(rom).map_err(|error| error.to_string())?;
    let match_entry_segments = native_match_entry_script();
    let match_entry_run = run_native_script_observed(
        &mut match_entry,
        instructions_per_frame,
        &match_entry_segments,
    );
    let match_entry_activity = match_entry.input_activity();
    let match_entry_raw_frame = match_entry.display_frame();
    let match_entry_stats = NativeFrameStats::from_frame(&match_entry_raw_frame);
    let match_entry_window_stats =
        NativeFrameStats::from_frame(&native_play_window_frame(&match_entry_raw_frame));
    let match_entry_native_playable = match_entry.native_playable_candidate();
    let match_entry_full_size = match_entry_window_stats.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
        && match_entry_window_stats.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT;
    let match_entry_full_scene = match_entry_full_size
        && match_entry_native_playable
        && match_entry_window_stats.has_gameplay_scene();
    let match_entry_known_rendering_gap =
        !match_entry_full_scene && match_entry_window_stats.has_visible_content();
    let match_entry_json = format!(
        "{{\"run\":{},\"segments\":[{}],\"executed_steps\":{},\"terminal\":{},\"native_playable_candidate\":{},\"input_activity\":{},\"frame\":{},\"raw_frame\":{},\"window_frame\":{},\"full_size\":{},\"full_scene\":{},\"known_rendering_gap\":{},\"state\":{}}}",
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
        match_entry.compact_probe_json()
    );

    let native_core_running = checkpoint.executed_steps() > 0 && !checkpoint.is_terminal();
    let combined_control_activity = control_sweep_activity.saturating_added(branch_input_activity);
    let play_controls_active =
        native_combined_play_controls_active(checkpoint_activity, combined_control_activity);
    let full_controls_active =
        native_combined_full_controls_active(checkpoint_activity, combined_control_activity);
    let all_branches_native_playable =
        branch_native_playable_count == native_health_branch_actions().len();
    let rendering_present =
        checkpoint_stats.has_visible_content() || control_sweep_stats.has_visible_content();
    let manual_checkpoint_ready =
        checkpoint_stats.has_visible_content() && !checkpoint.is_terminal();
    let checkpoint_full_scene = checkpoint_native_playable && checkpoint_stats.has_gameplay_scene();
    let control_sweep_full_scene =
        control_sweep_native_playable && control_sweep_stats.has_gameplay_scene();
    let display_detail_present = checkpoint_stats.has_scene_detail()
        || control_sweep_stats.has_scene_detail()
        || branch_full_scene_count > 0;
    let full_scene_rendering =
        checkpoint_full_scene || control_sweep_full_scene || branch_full_scene_count > 0;
    let control_sweep_reaches_playable = control_sweep_native_playable
        || branch_native_playable_count > 0
        || match_entry_native_playable;
    let known_rendering_gap = rendering_present
        && (!full_scene_rendering || !control_sweep_reaches_playable || !match_entry_full_scene);
    let overall_pass = native_core_running
        && assets_complete
        && manual_checkpoint_ready
        && play_controls_active
        && full_controls_active
        && all_branch_actions_read
        && control_sweep_reaches_playable
        && rendering_present
        && full_scene_rendering
        && match_entry_full_scene;
    let overall_status = if overall_pass {
        "pass"
    } else if native_core_running || rendering_present || play_controls_active {
        "partial"
    } else {
        "fail"
    };

    println!(
        "{{\"overall_status\":\"{}\",\"overall_pass\":{},\"instructions_per_frame\":{},\"branch_frames\":{},\"settle_frames\":{},\"assets_complete\":{},\"exact_mame_assets\":{},\"rom_compatibility\":{},\"native_core_running\":{},\"manual_checkpoint_ready\":{},\"play_controls_active\":{},\"full_controls_active\":{},\"all_branch_actions_read\":{},\"all_branches_native_playable\":{},\"control_sweep_reaches_playable\":{},\"rendering_present\":{},\"display_detail_present\":{},\"checkpoint_full_scene\":{},\"control_sweep_full_scene\":{},\"match_entry_full_scene\":{},\"match_entry_known_rendering_gap\":{},\"full_scene_rendering\":{},\"known_rendering_gap\":{},\"branch_native_playable_count\":{},\"branch_full_scene_count\":{},\"branch_count\":{},\"checkpoint\":{{\"run\":{},\"segments\":[{}],\"executed_steps\":{},\"terminal\":{},\"native_playable_candidate\":{},\"input_activity\":{},\"frame\":{},\"state\":{}}},\"control_sweep\":{{\"run\":{},\"segments\":[{}],\"executed_steps\":{},\"terminal\":{},\"native_playable_candidate\":{},\"input_activity\":{},\"frame\":{},\"state\":{}}},\"match_entry\":{},\"branches\":[{}]}}",
        overall_status,
        overall_pass,
        instructions_per_frame,
        branch_frames,
        settle_frames,
        assets_complete,
        exact_mame_assets,
        rom_compatibility.summary_json(),
        native_core_running,
        manual_checkpoint_ready,
        play_controls_active,
        full_controls_active,
        all_branch_actions_read,
        all_branches_native_playable,
        control_sweep_reaches_playable,
        rendering_present,
        display_detail_present,
        checkpoint_full_scene,
        control_sweep_full_scene,
        match_entry_full_scene,
        match_entry_known_rendering_gap,
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
        checkpoint.compact_probe_json(),
        control_sweep_run.json(),
        native_script_segments_json(&control_sweep_segments),
        control_sweep.executed_steps(),
        control_sweep.is_terminal(),
        control_sweep_native_playable,
        control_sweep_activity.json(),
        control_sweep_stats.json(),
        control_sweep.compact_probe_json(),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeScriptRunSummary {
    total_frames: u64,
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

impl NativeScriptRunSummary {
    fn json(self) -> String {
        format!(
            "{{\"total_frames\":{},\"missed_vblank_frames\":{},\"observed_native_playable_candidate\":{},\"first_native_playable_frame\":{},\"last_native_playable_frame\":{},\"stop_reason\":\"{}\"}}",
            self.total_frames,
            self.missed_vblank_frames,
            self.observed_native_playable_candidate,
            optional_u64_json(self.first_native_playable_frame),
            optional_u64_json(self.last_native_playable_frame),
            self.stop_reason
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeFrameStats {
    width: usize,
    height: usize,
    total_pixels: usize,
    nonzero_pixels: usize,
    black_pixels: usize,
    warm_pixels: usize,
    unique_colors: usize,
    dominant_color_bucket_pixels: usize,
    horizontal_color_changes: usize,
    occupied_rows: usize,
    occupied_row_span: usize,
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
        let row_content_cutoff = (frame.width / 20).max(1);
        let mut occupied_rows = 0usize;
        let mut first_occupied_row = None;
        let mut last_occupied_row = None;
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
                if color != 0 {
                    nonzero_pixels += 1;
                    row_nonzero_pixels += 1;
                } else {
                    black_pixels += 1;
                }
                if red > 128 && red > green.saturating_add(32) && red > blue.saturating_add(32) {
                    warm_pixels += 1;
                }
                if y >= bottom_band_start {
                    let max_channel = red.max(green).max(blue);
                    let min_channel = red.min(green).min(blue);
                    let luma = (red.saturating_mul(30)
                        + green.saturating_mul(59)
                        + blue.saturating_mul(11))
                        / 100;
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
            unique_colors: unique_colors.len(),
            dominant_color_bucket_pixels: color_buckets.into_iter().max().unwrap_or(0),
            horizontal_color_changes,
            occupied_rows,
            occupied_row_span,
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

    fn has_handoff_scene(self) -> bool {
        if !self.has_scene_detail()
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

    fn has_bottom_caption_band(self) -> bool {
        if self.width == 0 || self.height == 0 || self.total_pixels == 0 {
            return false;
        }
        let bottom_band_pixels = self.width.saturating_mul((self.height / 5).max(1));
        self.bottom_caption_pixels.saturating_mul(1_000) >= self.total_pixels.saturating_mul(3)
            && self.bottom_dark_pixels.saturating_mul(100) >= bottom_band_pixels.saturating_mul(5)
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
            "{{\"width\":{},\"height\":{},\"total_pixels\":{},\"nonzero_pixels\":{},\"black_pixels\":{},\"warm_pixels\":{},\"unique_colors\":{},\"dominant_color_bucket_pixels\":{},\"horizontal_color_changes\":{},\"occupied_rows\":{},\"occupied_row_span\":{},\"bottom_caption_pixels\":{},\"bottom_dark_pixels\":{},\"bottom_caption_band\":{},\"intro_caption_band\":{},\"visible_content\":{},\"scene_detail\":{},\"handoff_scene\":{},\"gameplay_scene\":{}}}",
            self.width,
            self.height,
            self.total_pixels,
            self.nonzero_pixels,
            self.black_pixels,
            self.warm_pixels,
            self.unique_colors,
            self.dominant_color_bucket_pixels,
            self.horizontal_color_changes,
            self.occupied_rows,
            self.occupied_row_span,
            self.bottom_caption_pixels,
            self.bottom_dark_pixels,
            self.has_bottom_caption_band(),
            self.has_intro_caption_band(),
            self.has_visible_content(),
            self.has_scene_detail(),
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

fn native_combined_play_controls_active(
    boot_activity: NativeInputActivity,
    sweep_activity: NativeInputActivity,
) -> bool {
    boot_activity.system_coin_active_reads > 0
        && (boot_activity.system_start_active_reads > 0 || boot_activity.p1_start_active_reads > 0)
        && (boot_activity.p1_punch_active_reads > 0 || sweep_activity.p1_punch_active_reads > 0)
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
        manual_buttons
    } else {
        scripted_action.map_or(ActionButtons::default(), Action::buttons)
    }
}

fn native_play_runtime_title(
    autoplay_enabled: bool,
    buttons: ActionButtons,
    native_playable: bool,
    gpu_playable: bool,
    gte_signal: bool,
) -> String {
    let mode = if autoplay_enabled {
        "autoplay/manual override"
    } else {
        "manual boot"
    };
    format!(
        "Bloody Roar 2 native Rust - {mode} - input={} - gpu={} gte={} playable={} - Esc quit",
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
    ActionButtons {
        start: window.is_key_down(Key::Enter) || window.is_key_down(Key::P),
        coin: window.is_key_down(Key::C),
        up: window.is_key_down(Key::Up) || window.is_key_down(Key::W),
        down: window.is_key_down(Key::Down) || window.is_key_down(Key::S),
        left: window.is_key_down(Key::Left) || window.is_key_down(Key::A),
        right: window.is_key_down(Key::Right) || window.is_key_down(Key::D),
        punch: window.is_key_down(Key::Z)
            || window.is_key_down(Key::Space)
            || window.is_key_down(Key::J)
            || window.is_key_down(Key::F),
        kick: window.is_key_down(Key::X)
            || window.is_key_down(Key::K)
            || window.is_key_down(Key::H),
        beast: window.is_key_down(Key::Q)
            || window.is_key_down(Key::L)
            || window.is_key_down(Key::B),
        guard: window.is_key_down(Key::E)
            || window.is_key_down(Key::I)
            || window.is_key_down(Key::G),
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
        "bloodyroar2-gym\n\nCommands:\n  info\n  action-space\n  observation-space\n  reset\n  step <action_index> [frames]\n  serve [address]\n  serve-native [address] [rom_zip] [instructions_per_frame]\n  prepare-assets <archive.zip> [rom_dir]\n  mame-required [rom_dir]\n  rom-ident [rom_dir]\n  mame-check [rom_dir]\n  doctor [rom_dir]\n  play [rom_dir] [extra_mame_args...]\n  prepare-zinc <archive.zip> [extract_dir]\n  zinc-check [bundle_dir]\n  zinc-play [bundle_dir] [extra_zinc_args...]\n  native-inspect [rom_zip_or_dir]\n  native-rom-summary [rom_zip_or_dir]\n  native-cache-prepare [rom_zip_or_dir]\n  native-cache-path [rom_zip_or_dir]\n  native-step [rom_zip] [instruction_count]\n  native-screenshot [rom_zip] [instruction_count] [output.png]\n  native-display-screenshot [rom_zip] [instruction_count] [output.png]\n  native-vram-screenshot [rom_zip] [instruction_count] [output.png]\n  native-screen-dump [rom_zip] [instruction_count] [output_prefix]\n  native-play-snapshot [rom_zip_or_dir] [instructions_per_frame] [output_prefix] [--complete-script] [--fast-forward-frames n] [action:frames...]\n  native-play [rom_zip_or_dir] [instructions_per_frame] [scale] [max_frames]\n  native-manual [rom_zip_or_dir] [instructions_per_frame] [scale] [max_frames]\n  native-autoplay [rom_zip_or_dir] [instructions_per_frame] [scale] [max_frames] [action:frames...]\n  native-input-check [rom_zip_or_dir] [instructions_per_frame]\n  native-health-check [rom_zip_or_dir] [instructions_per_frame] [branch_frames] [settle_frames]\n  native-scripted-step <rom_zip_or_dir> <instructions_per_frame> <output.png> <action:frames>...\n  native-scripted-dump <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>...\n  native-scripted-candidates <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>...\n  native-scripted-summary <rom_zip_or_dir> <instructions_per_frame> <action:frames>...\n  native-scripted-probe <rom_zip_or_dir> <instructions_per_frame> <action:frames>...\n  native-scripted-frame-probe <rom_zip_or_dir> <instructions_per_frame> <probe_stride_frames> <action:frames>...\n  native-scripted-compact-probe <rom_zip_or_dir> <instructions_per_frame> <probe_stride_frames> <action:frames>...\n  native-scripted-live-probe <rom_zip_or_dir> <instructions_per_frame> <emit_stride_frames> <action:frames>...\n  native-scripted-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <action:frames>... [-- <trace options>]\n  native-scripted-vblank-trace <rom_zip_or_dir> <instructions_per_frame> <hot_limit> <recent_limit> <warmup_action:frames>... --trace <trace_action:frames>... [-- <trace options>]\n  native-scripted-timeline <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <action:frames>...\n  native-scripted-branch <rom_zip_or_dir> <instructions_per_frame> <output_prefix> <branch_frames> <settle_frames> <warmup_action:frames>...\n  native-scripted-branch-summary <rom_zip_or_dir> <instructions_per_frame> <branch_frames> <settle_frames> <warmup_action:frames>...\n  native-draw-snapshot <rom_zip_or_dir> <instruction_count> <sequence_start> <sequence_end> <output_prefix>\n  native-scripted-draw-snapshot <rom_zip_or_dir> <instructions_per_frame> <sequence_start> <sequence_end> <output_prefix> <action:frames>...\n  native-trace [rom_zip] [instruction_count] [hot_limit] [recent_limit] [stop_pc] [stop_below_pc] [--watch address [len]] [--watch-only]\n  native-env-step [rom_zip] [action_index] [frames] [instructions_per_frame]\n  asset-check <path>\n\nnative-play fast-forwards to the first stable character-select handoff, opens a 640x480 uncropped window, and always sends manual key presses to the emulator; arrows/WASD move, Z/Space/J confirm or punch, X/K kick, Q/L/B beast, E/I/G guard, C coin, Enter/P start, Esc quit. native-autoplay keeps the full scripted path for diagnostics. native-play-snapshot writes the same bounded fast-forwarded frame without opening a window and reports stop_reason in JSON. native-manual preserves fully manual boot input. max_frames is optional and intended for smoke tests.\nThis project never ships ROMs, BIOS files, Windows EXEs, or DLLs. Configure legally obtained assets outside Git."
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
    native_select_entry_script()
}

fn native_manual_entry_script() -> Vec<NativeScriptSegment> {
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
    ]
}

fn native_select_entry_script() -> Vec<NativeScriptSegment> {
    let mut segments = native_manual_entry_script();
    segments.push(NativeScriptSegment {
        action: Action::Punch,
        frames: 3,
    });
    segments
}

fn native_match_entry_script() -> Vec<NativeScriptSegment> {
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
            frames: 300,
        },
        NativeScriptSegment {
            action: Action::Start,
            frames: 30,
        },
        NativeScriptSegment {
            action: Action::Noop,
            frames: 900,
        },
    ]
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
        let palette_output = capture.palette_png.as_ref().map(|_| {
            suffixed_path(
                output_prefix,
                &format!("seq-{:06}.palette.png", capture.sequence),
            )
        });
        write_output_file(&display_output, &capture.display_png)?;
        write_output_file(&bounds_output, &capture.bounds_png)?;
        if let (Some(output), Some(png)) = (&texture_output, &capture.texture_png) {
            write_output_file(output, png)?;
        }
        if let (Some(output), Some(png)) = (&palette_output, &capture.palette_png) {
            write_output_file(output, png)?;
        }
        captures.push(
            capture.json(
                &display_output.display().to_string(),
                &bounds_output.display().to_string(),
                texture_output
                    .as_ref()
                    .map(|output| output.display().to_string())
                    .as_deref(),
                palette_output
                    .as_ref()
                    .map(|output| output.display().to_string())
                    .as_deref(),
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

fn run_native_script(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    segments: &[NativeScriptSegment],
) -> u64 {
    run_native_script_observed(emulator, instructions_per_frame, segments).total_frames
}

fn run_native_script_observed(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    segments: &[NativeScriptSegment],
) -> NativeScriptRunSummary {
    run_native_script_observed_with_stop(
        emulator,
        instructions_per_frame,
        segments,
        NativeScriptStopMode::None,
        None,
    )
    .summary
}

fn run_native_script_observed_until_playable_with_limit(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    segments: &[NativeScriptSegment],
    max_frames: u64,
) -> NativeScriptProgress {
    run_native_script_observed_with_stop(
        emulator,
        instructions_per_frame,
        segments,
        NativeScriptStopMode::PlayableCandidate,
        Some(max_frames),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeScriptStopMode {
    None,
    PlayableCandidate,
}

fn run_native_script_observed_with_stop(
    emulator: &mut NativeEmulator,
    instructions_per_frame: u64,
    segments: &[NativeScriptSegment],
    stop_mode: NativeScriptStopMode,
    max_frames: Option<u64>,
) -> NativeScriptProgress {
    let instructions_per_frame = instructions_per_frame.max(1);
    let mut total_frames = 0u64;
    let mut missed_vblank_frames = 0u64;
    let mut observed_native_playable_candidate = false;
    let mut first_native_playable_frame = None;
    let mut last_native_playable_frame = None;
    let mut segment_index = 0usize;
    let mut segment_frame = 0u64;
    let mut stop_reason = "script_completed";
    let mut stopped = false;

    'script: for (index, segment) in segments.iter().enumerate() {
        segment_index = index;
        segment_frame = 0;
        emulator.set_input(segment.action.buttons());
        for _ in 0..segment.frames {
            let vblank_advanced = step_until_next_vblank_checked(emulator, instructions_per_frame);
            if !vblank_advanced {
                missed_vblank_frames = missed_vblank_frames.saturating_add(1);
            }

            total_frames += 1;
            segment_frame += 1;
            if native_script_should_sample_playable_observation(total_frames)
                && native_script_playable_observation_ready(emulator, stop_mode)
            {
                observed_native_playable_candidate = true;
                let first_playable_frame = *first_native_playable_frame.get_or_insert(total_frames);
                last_native_playable_frame = Some(total_frames);
                let settle_frames = match stop_mode {
                    NativeScriptStopMode::None => u64::MAX,
                    NativeScriptStopMode::PlayableCandidate => NATIVE_PLAY_HANDOFF_SETTLE_FRAMES,
                };
                let settled = total_frames.saturating_sub(first_playable_frame) >= settle_frames;
                let should_sample_stop =
                    settled && total_frames.is_multiple_of(NATIVE_PLAY_HANDOFF_CHECK_STRIDE_FRAMES);
                let stop_ready = should_sample_stop && stop_mode != NativeScriptStopMode::None;
                if stop_ready {
                    stop_reason = match stop_mode {
                        NativeScriptStopMode::None => "script_completed",
                        NativeScriptStopMode::PlayableCandidate => "playable_candidate_settled",
                    };
                    stopped = true;
                    break 'script;
                }
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
        && stop_mode != NativeScriptStopMode::None
        && max_frames.is_some_and(|max_frames| total_frames < max_frames)
        && !emulator.is_terminal()
    {
        segment_index = segments.len();
        segment_frame = 0;
        emulator.set_input(ActionButtons::default());
        loop {
            let vblank_advanced = step_until_next_vblank_checked(emulator, instructions_per_frame);
            if !vblank_advanced {
                missed_vblank_frames = missed_vblank_frames.saturating_add(1);
            }

            total_frames += 1;
            segment_frame += 1;
            if native_script_should_sample_playable_observation(total_frames)
                && native_script_playable_observation_ready(emulator, stop_mode)
            {
                observed_native_playable_candidate = true;
                let first_playable_frame = *first_native_playable_frame.get_or_insert(total_frames);
                last_native_playable_frame = Some(total_frames);
                let settle_frames = match stop_mode {
                    NativeScriptStopMode::None => u64::MAX,
                    NativeScriptStopMode::PlayableCandidate => NATIVE_PLAY_HANDOFF_SETTLE_FRAMES,
                };
                let settled = total_frames.saturating_sub(first_playable_frame) >= settle_frames;
                let should_sample_stop =
                    settled && total_frames.is_multiple_of(NATIVE_PLAY_HANDOFF_CHECK_STRIDE_FRAMES);
                let stop_ready = should_sample_stop && stop_mode != NativeScriptStopMode::None;
                if stop_ready {
                    stop_reason = match stop_mode {
                        NativeScriptStopMode::None => "script_completed",
                        NativeScriptStopMode::PlayableCandidate => "playable_candidate_settled",
                    };
                    break;
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

    NativeScriptProgress {
        summary: NativeScriptRunSummary {
            total_frames,
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
    native_play_gui_handoff_frame_ready(&native_play_window_frame(&emulator.display_frame()))
}

fn native_script_should_sample_playable_observation(total_frames: u64) -> bool {
    total_frames == 1 || total_frames.is_multiple_of(NATIVE_PLAY_HANDOFF_CHECK_STRIDE_FRAMES)
}

fn native_play_gui_handoff_frame_ready(frame: &NativeDisplayFrame) -> bool {
    let stats = NativeFrameStats::from_frame(frame);
    stats.width >= NATIVE_PLAY_MIN_WINDOW_WIDTH
        && stats.height >= NATIVE_PLAY_MIN_WINDOW_HEIGHT
        && stats.has_visible_content()
        && stats.has_handoff_scene()
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

fn native_fast_forward_instructions_per_frame(_requested: u64) -> u64 {
    NATIVE_PLAY_FAST_FORWARD_INSTRUCTIONS_PER_FRAME
}

fn native_play_gui_instructions_per_frame(requested: u64) -> u64 {
    requested.max(NATIVE_PLAY_GUI_MIN_INSTRUCTIONS_PER_FRAME)
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
        NativeFrameStats, NativeInputLatch, NativeScriptSegment, default_native_play_script,
        native_control_sweep_script, native_fast_forward_instructions_per_frame,
        native_match_entry_script, native_play_effective_buttons,
        native_play_gui_handoff_frame_ready, native_play_gui_instructions_per_frame,
        native_play_window_frame, native_script_completed, native_select_entry_script,
        next_scripted_action, parse_action_token, parse_native_autoplay_tail,
        parse_native_script_segments, parse_native_window_scale, remaining_native_script_segments,
    };
    use bloodyroar2_gym::{Action, ActionButtons, NativeDisplayFrame};
    use minifb::Scale;

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
    fn native_fast_forward_uses_fixed_fast_script_budget() {
        assert_eq!(
            native_fast_forward_instructions_per_frame(0),
            super::NATIVE_PLAY_FAST_FORWARD_INSTRUCTIONS_PER_FRAME
        );
        assert_eq!(
            native_fast_forward_instructions_per_frame(10_000),
            super::NATIVE_PLAY_FAST_FORWARD_INSTRUCTIONS_PER_FRAME
        );
        assert_eq!(
            native_fast_forward_instructions_per_frame(500_000),
            super::NATIVE_PLAY_FAST_FORWARD_INSTRUCTIONS_PER_FRAME
        );
        assert_eq!(
            native_fast_forward_instructions_per_frame(600_000),
            super::NATIVE_PLAY_FAST_FORWARD_INSTRUCTIONS_PER_FRAME
        );
    }

    #[test]
    fn native_play_gui_uses_stable_vblank_instruction_budget() {
        assert_eq!(
            native_play_gui_instructions_per_frame(0),
            super::NATIVE_PLAY_GUI_MIN_INSTRUCTIONS_PER_FRAME
        );
        assert_eq!(
            native_play_gui_instructions_per_frame(120_000),
            super::NATIVE_PLAY_GUI_MIN_INSTRUCTIONS_PER_FRAME
        );
        assert_eq!(native_play_gui_instructions_per_frame(600_000), 600_000);
    }

    #[test]
    fn default_native_play_script_reaches_character_select_handoff() {
        let segments = default_native_play_script();

        assert_eq!(segments, native_select_entry_script());
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
            1
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
    fn native_match_entry_script_reproduces_fight_start_sequence() {
        let match_segments = native_match_entry_script();

        assert_eq!(match_segments.len(), 11);
        assert_eq!(
            match_segments.last().expect("last segment").action,
            Action::Noop
        );
        assert_eq!(match_segments.last().expect("last segment").frames, 900);
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
            2220
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
    fn native_play_window_frame_deinterlaces_narrow_480_line_frame() {
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
        assert_eq!(scaled.pixels[640], 0x00ff_0000);
        assert_eq!(scaled.pixels[640 * 2], 0x00ff_0000);
        assert_eq!(scaled.pixels[640 * 479], 0x00ff_0000);
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
        assert_eq!(scaled.pixels[0], 0x00bf_0000);
        assert_eq!(scaled.pixels[1], 0x00bf_0000);
        assert_eq!(scaled.pixels[638], 0x0000_bf00);
        assert_eq!(scaled.pixels[639], 0x0000_bf00);
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
    fn native_play_effective_buttons_prefers_manual_keyboard_over_script() {
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
}
