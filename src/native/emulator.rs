use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;

use crate::action::ActionButtons;
use crate::backend::BackendError;
use crate::native::bus::{Bus, NativeBoardAssets, NativeInputActivity};
use crate::native::cpu::{Cpu, StepOutcome, StepReport};
use crate::native::framebuffer::{bytes_base64, png_from_rgb888_pixels};
use crate::native::io::{NativeGpuDisplayCandidate, NativeGpuDrawCapture};
use crate::native::platform::native_platform_json;
use crate::native::romset::{NativeRomCacheReport, NativeRomCompatibilityReport, NativeRomSet};

const NATIVE_AT28C16_MODE_ENV: &str = "BLOODYROAR2_NATIVE_AT28C16_MODE";
const NATIVE_COIN_MAPPING_ENV: &str = "BLOODYROAR2_NATIVE_COIN_MAPPING";
const NATIVE_LEGACY_ZINC_INPUT_ENV: &str = "BLOODYROAR2_NATIVE_LEGACY_ZINC_INPUT";
const NATIVE_WINDOW_FRAME_WIDTH: usize = 512;
const NATIVE_WINDOW_FRAME_HEIGHT: usize = 480;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeDisplayFrame {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct NativeEmulator {
    pub cpu: Cpu,
    bus: Bus,
    rom_compatibility: NativeRomCompatibilityReport,
    last_outcome: StepOutcome,
    executed_steps: u64,
    last_step: Option<StepReport>,
}

impl NativeEmulator {
    pub fn from_rom_zip(path: impl Into<PathBuf>) -> Result<Self, BackendError> {
        let romset = NativeRomSet::scan_cached(path.into())?;
        Self::from_romset(romset)
    }

    pub fn from_rom_zip_with_cache_report(
        path: impl Into<PathBuf>,
    ) -> Result<(Self, NativeRomCacheReport), BackendError> {
        let scan = NativeRomSet::scan_cached_with_report(path.into())?;
        let cache = scan.cache;
        let emulator = Self::from_romset(scan.romset)?;
        Ok((emulator, cache))
    }

    pub fn from_romset(romset: NativeRomSet) -> Result<Self, BackendError> {
        let rom_compatibility = romset.compatibility_report();
        let boot_rom = romset.load_boot_rom()?;
        let banked_roms = romset.load_banked_roms()?;
        let board_assets = apply_runtime_board_asset_overrides(romset.load_board_assets());
        let mut cpu = Cpu::default();
        cpu.set_br2_corrupt_runtime_recovery_hles(
            rom_compatibility.has_known_bad_dump("flash1.024_bad_dump_4866dce3"),
        );
        Ok(Self {
            cpu,
            bus: Bus::with_board_assets(boot_rom, banked_roms, 4 * 1024 * 1024, board_assets),
            rom_compatibility,
            last_outcome: StepOutcome::Continue,
            executed_steps: 0,
            last_step: None,
        })
    }

    pub fn step_instruction(&mut self) -> StepReport {
        let report = self.cpu.step_report(&mut self.bus);
        self.last_outcome = report.outcome;
        if report.cycles_elapsed > 0 {
            self.executed_steps += 1;
        }
        self.last_step = Some(report);
        report
    }

    pub fn step_instructions(&mut self, count: u64) -> u64 {
        let start_steps = self.executed_steps;
        for _ in 0..count {
            self.step_instruction();
            if self.last_outcome != StepOutcome::Continue {
                break;
            }
        }
        self.executed_steps - start_steps
    }

    fn step_instruction_untracked(&mut self) {
        if self.cpu.halted {
            self.last_outcome = StepOutcome::Halted;
            return;
        }

        let cycles_before = self.cpu.cycles;
        self.last_outcome = self.cpu.step(&mut self.bus);
        if self.cpu.cycles > cycles_before {
            self.executed_steps = self.executed_steps.saturating_add(1);
        }
    }

    pub fn step_instructions_until_vblank_untracked(&mut self, count: u64) -> u64 {
        let start_steps = self.executed_steps;
        let start_vblank = self.bus.vblank_count();
        let max_instructions = count.max(1);
        while self.executed_steps - start_steps < max_instructions
            && self.bus.vblank_count() == start_vblank
            && self.last_outcome == StepOutcome::Continue
        {
            self.step_instruction_untracked();
        }
        self.executed_steps - start_steps
    }

    pub fn step_until_next_vblank(&mut self, max_instructions: u64) -> u64 {
        let start_steps = self.executed_steps;
        let start_vblank = self.bus.vblank_count();
        let max_instructions = max_instructions.max(1);
        while self.executed_steps - start_steps < max_instructions
            && self.bus.vblank_count() == start_vblank
            && self.last_outcome == StepOutcome::Continue
        {
            self.step_instruction();
        }
        self.executed_steps - start_steps
    }

    pub fn vblank_count(&self) -> u64 {
        self.bus.vblank_count()
    }

    pub fn vblank_presentation_capture_interval(&self) -> Option<u64> {
        self.bus.vblank_presentation_capture_interval()
    }

    pub fn set_vblank_presentation_capture_interval(&mut self, interval: Option<u64>) {
        self.bus.set_vblank_presentation_capture_interval(interval);
    }

    pub fn set_native_otc_dma_recovery_suppressed(&mut self, suppressed: bool) {
        self.bus.set_native_otc_dma_recovery_suppressed(suppressed);
    }

    pub fn native_otc_dma_recovery_suppressed(&self) -> bool {
        self.bus.native_otc_dma_recovery_suppressed()
    }

    pub fn set_native_otc_dma_recovery_interval(&mut self, interval: u64) {
        self.bus.set_native_otc_dma_recovery_interval(interval);
    }

    pub fn native_otc_dma_recovery_interval(&self) -> u64 {
        self.bus.native_otc_dma_recovery_interval()
    }

    pub fn capture_vblank_presented_frame(&mut self) {
        self.bus.capture_vblank_presented_frame();
    }

    pub fn unlinked_primitive_replay_interval(&self) -> Option<u64> {
        self.bus.unlinked_primitive_replay_interval()
    }

    pub fn set_unlinked_primitive_replay_interval(&mut self, interval: Option<u64>) {
        self.bus.set_unlinked_primitive_replay_interval(interval);
    }

    pub fn set_unlinked_primitive_replay_enabled(&mut self, enabled: bool) {
        self.bus.set_unlinked_primitive_replay_enabled(enabled);
    }

    pub fn unlinked_primitive_replay_enabled(&self) -> bool {
        self.bus.unlinked_primitive_replay_enabled()
    }

    pub fn set_coin_input_mapping_name(&mut self, value: &str) -> bool {
        self.bus.set_coin_input_mapping_name(value)
    }

    pub fn set_native_credit_adapter_input_bit(&mut self, value: u32) -> bool {
        self.bus.set_native_credit_adapter_input_bit(value)
    }

    pub fn set_native_credit_projection_name(&mut self, value: &str) -> bool {
        self.bus.set_native_credit_projection_name(value)
    }

    pub fn apply_auto_coin_input_mapping(&mut self) -> Option<&'static str> {
        if env::var_os(NATIVE_COIN_MAPPING_ENV).is_some() {
            return None;
        }
        if env::var_os(NATIVE_LEGACY_ZINC_INPUT_ENV).is_some() {
            return None;
        }

        let mapping = self.auto_coin_input_mapping_name()?;
        if self.set_coin_input_mapping_name(mapping) {
            Some(mapping)
        } else {
            None
        }
    }

    fn auto_coin_input_mapping_name(&self) -> Option<&'static str> {
        if self
            .rom_compatibility
            .has_known_variant("zinc_cfg_eeprom_variant")
        {
            // ZiNc ROM/config provenance does not change the ZN JAMMA wiring.
            // Bloody Roar 2 uses SYSTEM bit 0 for Start 1 and bit 4 for Coin 1;
            // the legacy compatibility mapping also asserts the P2 lines and
            // can make the game evaluate a simultaneous two-player start.
            Some("mame")
        } else {
            None
        }
    }

    pub fn rom_compatibility(&self) -> &NativeRomCompatibilityReport {
        &self.rom_compatibility
    }

    pub fn trace_instructions(
        &mut self,
        count: u64,
        hot_limit: usize,
        recent_limit: usize,
        config: NativeTraceConfig,
    ) -> NativeTrace {
        self.bus.set_access_trace_limit(recent_limit.max(32));
        let mut watch_armed = config
            .watch_after_vblank
            .is_none_or(|threshold| self.bus.vblank_count() >= threshold);
        self.bus.set_access_trace_watch_ranges(if watch_armed {
            config.watch_ranges.clone()
        } else {
            Vec::new()
        });
        self.bus.set_access_trace_watch_only(config.watch_only);
        let mut pc_counts = BTreeMap::new();
        let mut unsupported = BTreeMap::new();
        let mut recent_steps = Vec::new();
        let mut stop_pc_hits_to_skip = config.stop_pc_skip;
        let mut stop_watch_write_hits_to_skip = config.stop_watch_write_skip;
        let start_steps = self.executed_steps;

        for _ in 0..count {
            self.arm_trace_watch_if_ready(&config, &mut watch_armed);
            let packed_helper_accepted_before = self.cpu.br2_packed_vertex_helper_accepted_count();
            let report = self.step_instruction();
            *pc_counts.entry(report.start_pc).or_insert(0) += 1;
            if let StepOutcome::Unsupported(instruction) = report.outcome {
                *unsupported.entry(instruction).or_insert(0) += 1;
            }
            let mut watch_write_hit = self.bus.access_trace_watch_write_hit();
            if config.stop_watch_write && watch_write_hit && stop_watch_write_hits_to_skip > 0 {
                stop_watch_write_hits_to_skip -= 1;
                self.bus.clear_access_trace_watch_hits();
                watch_write_hit = false;
            }
            let reached_stop = config.reached_stop_condition(
                report.start_pc,
                &mut stop_pc_hits_to_skip,
                self.bus.executable_pc_mapped(report.start_pc),
                self.bus.access_trace_watch_access_hit(),
                self.bus.access_trace_watch_data_hit(),
                watch_write_hit,
            );
            let reached_slow_step = config
                .stop_cycles_elapsed
                .is_some_and(|limit| report.cycles_elapsed >= limit);
            let reached_stop_excode = config
                .stop_excode
                .is_some_and(|excode| self.cpu.cause_excode() == excode)
                || config.stop_excodes.contains(&self.cpu.cause_excode());
            let reached_packed_helper_accept = config.stop_br2_packed_helper_accept
                && self.cpu.br2_packed_vertex_helper_accepted_count()
                    > packed_helper_accepted_before;

            if recent_limit > 0 {
                recent_steps.push(report);
                if recent_steps.len() > recent_limit {
                    recent_steps.remove(0);
                }
            }

            if report.outcome != StepOutcome::Continue
                || reached_stop
                || reached_slow_step
                || reached_stop_excode
                || reached_packed_helper_accept
            {
                break;
            }
        }

        NativeTrace::new(NativeTraceParts {
            requested_steps: count,
            executed_steps: self.executed_steps - start_steps,
            pc_counts,
            unsupported,
            recent_steps,
            hot_limit,
            watch_write_count: self.bus.access_trace_watch_write_count(),
            bus_access_trace_json: self.bus.access_trace_json(),
            state_json: self.json(),
        })
    }

    pub fn trace_until_next_vblank(
        &mut self,
        max_instructions: u64,
        hot_limit: usize,
        recent_limit: usize,
        config: NativeTraceConfig,
    ) -> (NativeTrace, bool) {
        self.bus.set_access_trace_limit(recent_limit.max(32));
        let mut watch_armed = config
            .watch_after_vblank
            .is_none_or(|threshold| self.bus.vblank_count() >= threshold);
        self.bus.set_access_trace_watch_ranges(if watch_armed {
            config.watch_ranges.clone()
        } else {
            Vec::new()
        });
        self.bus.set_access_trace_watch_only(config.watch_only);
        let mut pc_counts = BTreeMap::new();
        let mut unsupported = BTreeMap::new();
        let mut recent_steps = Vec::new();
        let mut stop_pc_hits_to_skip = config.stop_pc_skip;
        let mut stop_watch_write_hits_to_skip = config.stop_watch_write_skip;
        let start_steps = self.executed_steps;
        let start_vblank = self.bus.vblank_count();
        let max_instructions = max_instructions.max(1);

        while self.executed_steps - start_steps < max_instructions
            && self.bus.vblank_count() == start_vblank
            && self.last_outcome == StepOutcome::Continue
        {
            self.arm_trace_watch_if_ready(&config, &mut watch_armed);
            let packed_helper_accepted_before = self.cpu.br2_packed_vertex_helper_accepted_count();
            let report = self.step_instruction();
            *pc_counts.entry(report.start_pc).or_insert(0) += 1;
            if let StepOutcome::Unsupported(instruction) = report.outcome {
                *unsupported.entry(instruction).or_insert(0) += 1;
            }
            let mut watch_write_hit = self.bus.access_trace_watch_write_hit();
            if config.stop_watch_write && watch_write_hit && stop_watch_write_hits_to_skip > 0 {
                stop_watch_write_hits_to_skip -= 1;
                self.bus.clear_access_trace_watch_hits();
                watch_write_hit = false;
            }
            let reached_stop = config.reached_stop_condition(
                report.start_pc,
                &mut stop_pc_hits_to_skip,
                self.bus.executable_pc_mapped(report.start_pc),
                self.bus.access_trace_watch_access_hit(),
                self.bus.access_trace_watch_data_hit(),
                watch_write_hit,
            );
            let reached_slow_step = config
                .stop_cycles_elapsed
                .is_some_and(|limit| report.cycles_elapsed >= limit);
            let reached_stop_excode = config
                .stop_excode
                .is_some_and(|excode| self.cpu.cause_excode() == excode)
                || config.stop_excodes.contains(&self.cpu.cause_excode());
            let reached_packed_helper_accept = config.stop_br2_packed_helper_accept
                && self.cpu.br2_packed_vertex_helper_accepted_count()
                    > packed_helper_accepted_before;

            if recent_limit > 0 {
                recent_steps.push(report);
                if recent_steps.len() > recent_limit {
                    recent_steps.remove(0);
                }
            }

            if report.outcome != StepOutcome::Continue
                || reached_stop
                || reached_slow_step
                || reached_stop_excode
                || reached_packed_helper_accept
            {
                break;
            }
        }

        let vblank_advanced = self.bus.vblank_count() != start_vblank;
        (
            NativeTrace::new(NativeTraceParts {
                requested_steps: max_instructions,
                executed_steps: self.executed_steps - start_steps,
                pc_counts,
                unsupported,
                recent_steps,
                hot_limit,
                watch_write_count: self.bus.access_trace_watch_write_count(),
                bus_access_trace_json: self.bus.access_trace_json(),
                state_json: self.json(),
            }),
            vblank_advanced,
        )
    }

    pub fn trace_scripted_frames(
        &mut self,
        instructions_per_frame: u64,
        segments: &[(ActionButtons, u64)],
        hot_limit: usize,
        recent_limit: usize,
        config: NativeTraceConfig,
    ) -> NativeTrace {
        self.bus.set_access_trace_limit(recent_limit.max(32));
        let mut watch_armed = config
            .watch_after_vblank
            .is_none_or(|threshold| self.bus.vblank_count() >= threshold);
        self.bus.set_access_trace_watch_ranges(if watch_armed {
            config.watch_ranges.clone()
        } else {
            Vec::new()
        });
        self.bus.set_access_trace_watch_only(config.watch_only);
        let mut pc_counts = BTreeMap::new();
        let mut unsupported = BTreeMap::new();
        let mut recent_steps = Vec::new();
        let mut stop_pc_hits_to_skip = config.stop_pc_skip;
        let mut stop_watch_write_hits_to_skip = config.stop_watch_write_skip;
        let start_steps = self.executed_steps;
        let instructions_per_frame = instructions_per_frame.max(1);
        let requested_steps = segments.iter().fold(0u64, |total, (_, frames)| {
            total.saturating_add(frames.saturating_mul(instructions_per_frame))
        });

        'script: for (buttons, frames) in segments {
            self.set_input(*buttons);
            for _ in 0..*frames {
                for _ in 0..instructions_per_frame {
                    self.arm_trace_watch_if_ready(&config, &mut watch_armed);
                    let packed_helper_accepted_before =
                        self.cpu.br2_packed_vertex_helper_accepted_count();
                    let report = self.step_instruction();
                    *pc_counts.entry(report.start_pc).or_insert(0) += 1;
                    if let StepOutcome::Unsupported(instruction) = report.outcome {
                        *unsupported.entry(instruction).or_insert(0) += 1;
                    }
                    let mut watch_write_hit = self.bus.access_trace_watch_write_hit();
                    if config.stop_watch_write
                        && watch_write_hit
                        && stop_watch_write_hits_to_skip > 0
                    {
                        stop_watch_write_hits_to_skip -= 1;
                        self.bus.clear_access_trace_watch_hits();
                        watch_write_hit = false;
                    }
                    let reached_stop = config.reached_stop_condition(
                        report.start_pc,
                        &mut stop_pc_hits_to_skip,
                        self.bus.executable_pc_mapped(report.start_pc),
                        self.bus.access_trace_watch_access_hit(),
                        self.bus.access_trace_watch_data_hit(),
                        watch_write_hit,
                    );
                    let reached_slow_step = config
                        .stop_cycles_elapsed
                        .is_some_and(|limit| report.cycles_elapsed >= limit);
                    let reached_stop_excode = config
                        .stop_excode
                        .is_some_and(|excode| self.cpu.cause_excode() == excode)
                        || config.stop_excodes.contains(&self.cpu.cause_excode());
                    let reached_packed_helper_accept = config.stop_br2_packed_helper_accept
                        && self.cpu.br2_packed_vertex_helper_accepted_count()
                            > packed_helper_accepted_before;

                    if recent_limit > 0 {
                        recent_steps.push(report);
                        if recent_steps.len() > recent_limit {
                            recent_steps.remove(0);
                        }
                    }

                    if self.last_outcome != StepOutcome::Continue
                        || reached_stop
                        || reached_slow_step
                        || reached_stop_excode
                        || reached_packed_helper_accept
                    {
                        break 'script;
                    }
                }
            }
        }

        NativeTrace::new(NativeTraceParts {
            requested_steps,
            executed_steps: self.executed_steps - start_steps,
            pc_counts,
            unsupported,
            recent_steps,
            hot_limit,
            watch_write_count: self.bus.access_trace_watch_write_count(),
            bus_access_trace_json: self.bus.access_trace_json(),
            state_json: self.json(),
        })
    }

    fn arm_trace_watch_if_ready(&mut self, config: &NativeTraceConfig, watch_armed: &mut bool) {
        if *watch_armed
            || config
                .watch_after_vblank
                .is_none_or(|threshold| self.bus.vblank_count() < threshold)
        {
            return;
        }

        self.bus
            .set_access_trace_watch_ranges(config.watch_ranges.clone());
        *watch_armed = true;
    }

    pub fn set_input(&mut self, buttons: ActionButtons) {
        self.bus.set_input(buttons);
    }

    pub fn is_terminal(&self) -> bool {
        self.last_outcome != StepOutcome::Continue || self.cpu.halted
    }

    pub fn progress_signal(&self) -> f32 {
        ((self.cpu.cycles % 1_000_000) as f32) / 1_000_000.0
    }

    pub fn executed_steps(&self) -> u64 {
        self.executed_steps
    }

    pub fn screenshot_png_base64(&self) -> String {
        self.bus.io.gpu.screenshot_png_base64()
    }

    pub fn screenshot_png(&self) -> Vec<u8> {
        self.bus.io.gpu.screenshot_png()
    }

    pub fn observation_frame(&self) -> NativeDisplayFrame {
        let (width, height, pixels) = self.bus.io.gpu.screenshot_rgb_frame();
        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    pub fn display_png(&self) -> Vec<u8> {
        self.bus.io.gpu.display_png()
    }

    pub fn display_frame(&self) -> NativeDisplayFrame {
        let (width, height, pixels) = self.bus.io.gpu.display_rgb_frame();
        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    pub fn display_window_frame(&self) -> NativeDisplayFrame {
        native_window_frame_from_display(&self.display_frame())
    }

    pub fn display_window_png(&self) -> Vec<u8> {
        let frame = self.display_window_frame();
        png_from_rgb888_pixels(frame.width, frame.height, &frame.pixels)
    }

    pub fn display_window_png_base64(&self) -> String {
        bytes_base64(&self.display_window_png())
    }

    pub fn actual_display_frame(&self) -> NativeDisplayFrame {
        let (width, height, pixels) = self.bus.io.gpu.actual_display_rgb_frame();
        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    pub fn raw_actual_display_frame(&self) -> NativeDisplayFrame {
        let (width, height, pixels) = self.bus.io.gpu.raw_actual_display_rgb_frame();
        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    pub fn fast_gameplay_display_frame(&self) -> NativeDisplayFrame {
        let (width, height, pixels) = self.bus.fast_gameplay_display_rgb_frame();
        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    pub fn actual_display_png(&self) -> Vec<u8> {
        self.bus.io.gpu.actual_display_png()
    }

    pub fn raw_actual_display_png(&self) -> Vec<u8> {
        self.bus.io.gpu.raw_actual_display_png()
    }

    pub fn vram_png(&self) -> Vec<u8> {
        self.bus.io.gpu.vram_png()
    }

    pub fn set_draw_capture_range(&mut self, start: u64, end: u64) {
        self.bus.set_gpu_draw_capture_range(start, end);
    }

    pub fn set_draw_capture_predicates(
        &mut self,
        predicates: Vec<crate::native::io::NativeGpuDrawCapturePredicate>,
    ) {
        self.bus.set_gpu_draw_capture_predicates(predicates);
    }

    pub fn draw_sequence(&self) -> u64 {
        self.bus.gpu_draw_sequence()
    }

    pub fn draw_captures(&self) -> &[NativeGpuDrawCapture] {
        self.bus.gpu_draw_captures()
    }

    pub fn display_candidates(&self) -> Vec<NativeGpuDisplayCandidate> {
        self.bus.gpu_display_candidates()
    }

    pub fn input_activity_json(&self) -> String {
        self.bus.input_activity().json()
    }

    pub fn gpu_diagnostic_json(&self) -> String {
        self.bus.io_compact_json()
    }

    pub fn input_activity(&self) -> NativeInputActivity {
        self.bus.input_activity()
    }

    pub fn recovery_raster_cache_stats_json(&self) -> String {
        self.bus.recovery_raster_cache_stats_json()
    }

    pub fn br2_native_credit_hle_accepted_seen(&self) -> bool {
        self.bus.br2_native_credit_hle_accepted_seen()
    }

    pub fn br2_native_credit_hle_json(&self) -> String {
        self.bus.br2_native_credit_hle_json()
    }

    pub fn ram_snapshot(&self) -> Vec<u8> {
        self.bus.ram_snapshot()
    }

    pub fn scratchpad_snapshot(&self) -> Vec<u8> {
        self.bus.scratchpad_snapshot()
    }

    pub fn debug_read_u32(&self, address: u32) -> u32 {
        self.bus.read_u32(address)
    }

    pub fn has_play_control_activity(&self) -> bool {
        self.bus.input_activity().has_play_control_activity()
    }

    pub fn has_full_control_activity(&self) -> bool {
        self.bus.input_activity().has_full_control_activity()
    }

    pub fn native_playable_candidate(&self) -> bool {
        let gpu_playable_candidate = self.gpu_native_playable_candidate();
        let gpu_rendered_scene_candidate = self.gpu_native_rendered_scene_candidate();
        let gte_gameplay_signal = self.native_3d_gameplay_signal();
        self.native_playable_candidate_with_render_context(
            gpu_playable_candidate,
            gpu_rendered_scene_candidate,
            gte_gameplay_signal,
        )
    }

    pub fn native_playable_candidate_with_render_context(
        &self,
        gpu_playable_candidate: bool,
        gpu_rendered_scene_candidate: bool,
        gte_gameplay_signal: bool,
    ) -> bool {
        let display_frame_guard = self
            .native_display_frame_supports_playable_candidate_with_context(
                gpu_playable_candidate,
                gpu_rendered_scene_candidate,
                gte_gameplay_signal,
            );
        let native_3d_gameplay_candidate =
            display_frame_guard && gpu_rendered_scene_candidate && gte_gameplay_signal;
        display_frame_guard && (gpu_playable_candidate || native_3d_gameplay_candidate)
    }

    pub fn gpu_native_playable_candidate(&self) -> bool {
        self.bus.native_playable_candidate()
    }

    pub fn gpu_native_rendered_scene_candidate(&self) -> bool {
        self.bus.native_rendered_scene_candidate()
    }

    pub fn native_3d_gameplay_signal(&self) -> bool {
        self.cpu.native_3d_gameplay_signal()
    }

    pub fn actual_display_supports_playable_candidate(&self) -> bool {
        let gpu_playable_candidate = self.gpu_native_playable_candidate();
        let gpu_rendered_scene_candidate = self.gpu_native_rendered_scene_candidate();
        let gte_gameplay_signal = self.native_3d_gameplay_signal();
        self.actual_display_supports_playable_candidate_with_render_context(
            gpu_playable_candidate,
            gpu_rendered_scene_candidate,
            gte_gameplay_signal,
        )
    }

    pub fn actual_display_supports_playable_candidate_with_render_context(
        &self,
        gpu_playable_candidate: bool,
        gpu_rendered_scene_candidate: bool,
        gte_gameplay_signal: bool,
    ) -> bool {
        let render_context_allows_periodic =
            gpu_playable_candidate || (gpu_rendered_scene_candidate && gte_gameplay_signal);
        native_display_frame_or_window_supports_playable_candidate_with_render_context(
            &self.actual_display_frame(),
            render_context_allows_periodic,
        )
    }

    fn native_display_frame_supports_playable_candidate_with_context(
        &self,
        gpu_playable_candidate: bool,
        gpu_rendered_scene_candidate: bool,
        gte_gameplay_signal: bool,
    ) -> bool {
        let render_context_allows_periodic =
            gpu_playable_candidate || (gpu_rendered_scene_candidate && gte_gameplay_signal);
        let frame = self.display_frame();
        if native_display_frame_or_window_supports_playable_candidate_with_render_context(
            &frame,
            render_context_allows_periodic,
        ) {
            return true;
        }

        let actual = self.actual_display_frame();
        native_display_frame_or_window_supports_playable_candidate_with_render_context(
            &actual,
            render_context_allows_periodic,
        )
    }

    fn native_playability_json(&self) -> String {
        let gpu_playable_candidate = self.gpu_native_playable_candidate();
        let gpu_rendered_scene_candidate = self.gpu_native_rendered_scene_candidate();
        let gte_gameplay_signal = self.native_3d_gameplay_signal();
        let display_frame_guard = self
            .native_display_frame_supports_playable_candidate_with_context(
                gpu_playable_candidate,
                gpu_rendered_scene_candidate,
                gte_gameplay_signal,
            );
        let render_context_allows_periodic =
            gpu_playable_candidate || (gpu_rendered_scene_candidate && gte_gameplay_signal);
        let stable_frame = self.display_frame();
        let actual_frame = self.actual_display_frame();
        let stable_frame_guard =
            native_display_frame_or_window_supports_playable_candidate_with_render_context(
                &stable_frame,
                render_context_allows_periodic,
            );
        let actual_frame_guard =
            native_display_frame_or_window_supports_playable_candidate_with_render_context(
                &actual_frame,
                render_context_allows_periodic,
            );
        let display_frame_periodic_ghosting =
            native_display_frame_has_periodic_horizontal_ghosting(&stable_frame)
                && !actual_frame_guard;
        let native_3d_gameplay_candidate =
            display_frame_guard && gpu_rendered_scene_candidate && gte_gameplay_signal;
        let playable_candidate =
            display_frame_guard && (gpu_playable_candidate || native_3d_gameplay_candidate);
        let classification = if playable_candidate && native_3d_gameplay_candidate {
            "native_3d_gameplay_candidate"
        } else if playable_candidate {
            "native_playable_candidate"
        } else if gpu_playable_candidate && display_frame_periodic_ghosting {
            "gpu_candidate_rejected_periodic_display"
        } else if gpu_playable_candidate && !display_frame_guard {
            "gpu_candidate_rejected_display_frame"
        } else if display_frame_periodic_ghosting {
            "display_frame_periodic_ghosting"
        } else if !display_frame_guard {
            "display_frame_not_gameplay"
        } else if !gpu_rendered_scene_candidate {
            "gpu_scene_not_rendered"
        } else {
            "missing_native_3d_gte_signal"
        };

        format!(
            "{{\"playable_candidate\":{},\"classification\":\"{}\",\"gpu_playable_candidate\":{},\"gpu_rendered_scene_candidate\":{},\"gte_gameplay_signal\":{},\"native_3d_gameplay_candidate\":{},\"display_frame_guard\":{},\"stable_frame_guard\":{},\"actual_frame_guard\":{},\"display_frame_periodic_ghosting\":{},\"projected_vertices\":{},\"gpu\":{}}}",
            playable_candidate,
            classification,
            gpu_playable_candidate,
            gpu_rendered_scene_candidate,
            gte_gameplay_signal,
            native_3d_gameplay_candidate,
            display_frame_guard,
            stable_frame_guard,
            actual_frame_guard,
            display_frame_periodic_ghosting,
            self.cpu.gte_projected_vertices(),
            self.bus.native_playability_json()
        )
    }

    fn native_playability_compact_json(&self) -> String {
        let gpu_playable_candidate = self.gpu_native_playable_candidate();
        let gpu_rendered_scene_candidate = self.gpu_native_rendered_scene_candidate();
        let gte_gameplay_signal = self.native_3d_gameplay_signal();
        let display_frame_guard = self
            .native_display_frame_supports_playable_candidate_with_context(
                gpu_playable_candidate,
                gpu_rendered_scene_candidate,
                gte_gameplay_signal,
            );
        let render_context_allows_periodic =
            gpu_playable_candidate || (gpu_rendered_scene_candidate && gte_gameplay_signal);
        let stable_frame = self.display_frame();
        let actual_frame = self.actual_display_frame();
        let stable_frame_guard =
            native_display_frame_or_window_supports_playable_candidate_with_render_context(
                &stable_frame,
                render_context_allows_periodic,
            );
        let actual_frame_guard =
            native_display_frame_or_window_supports_playable_candidate_with_render_context(
                &actual_frame,
                render_context_allows_periodic,
            );
        let display_frame_periodic_ghosting =
            native_display_frame_has_periodic_horizontal_ghosting(&stable_frame)
                && !actual_frame_guard;
        let native_3d_gameplay_candidate =
            display_frame_guard && gpu_rendered_scene_candidate && gte_gameplay_signal;
        let playable_candidate =
            display_frame_guard && (gpu_playable_candidate || native_3d_gameplay_candidate);
        let classification = if playable_candidate && native_3d_gameplay_candidate {
            "native_3d_gameplay_candidate"
        } else if playable_candidate {
            "native_playable_candidate"
        } else if gpu_playable_candidate && display_frame_periodic_ghosting {
            "gpu_candidate_rejected_periodic_display"
        } else if gpu_playable_candidate && !display_frame_guard {
            "gpu_candidate_rejected_display_frame"
        } else if display_frame_periodic_ghosting {
            "display_frame_periodic_ghosting"
        } else if !display_frame_guard {
            "display_frame_not_gameplay"
        } else if !gpu_rendered_scene_candidate {
            "gpu_scene_not_rendered"
        } else {
            "missing_native_3d_gte_signal"
        };

        format!(
            "{{\"playable_candidate\":{},\"classification\":\"{}\",\"gpu_playable_candidate\":{},\"gpu_rendered_scene_candidate\":{},\"gte_gameplay_signal\":{},\"native_3d_gameplay_candidate\":{},\"display_frame_guard\":{},\"stable_frame_guard\":{},\"actual_frame_guard\":{},\"display_frame_periodic_ghosting\":{},\"projected_vertices\":{},\"gpu\":{}}}",
            playable_candidate,
            classification,
            gpu_playable_candidate,
            gpu_rendered_scene_candidate,
            gte_gameplay_signal,
            native_3d_gameplay_candidate,
            display_frame_guard,
            stable_frame_guard,
            actual_frame_guard,
            display_frame_periodic_ghosting,
            self.cpu.gte_projected_vertices(),
            self.bus.native_playability_compact_json()
        )
    }

    pub fn json(&self) -> String {
        let playable = self.native_playable_candidate();
        format!(
            "{{\"cpu\":{},\"gte\":{},\"io\":{},\"zn_board\":{},\"native_sync\":{},\"write_watch\":[{}],\"native_playability\":{},\"platform\":{},\"rom_compatibility\":{},\"rom_bytes\":{},\"banked_rom_bytes\":{},\"ram_bytes\":{},\"scratchpad_bytes\":{},\"executed_steps\":{},\"last_step\":{},\"last_outcome\":\"{:?}\",\"gpu_playable_candidate\":{},\"gte_gameplay_signal\":{},\"playable\":{},\"development_stage\":\"native_runtime_validation\"}}",
            self.cpu.json(),
            self.cpu.gte_json(),
            self.bus.io_json(),
            self.bus.zn_board_json(),
            self.bus.native_sync_json(),
            self.bus.write_watch_json(),
            self.native_playability_json(),
            native_platform_json(),
            self.rom_compatibility.summary_json(),
            self.bus.rom_len(),
            self.bus.banked_rom_len(),
            self.bus.ram_len(),
            self.bus.scratchpad_len(),
            self.executed_steps,
            optional_step_json(self.last_step),
            self.last_outcome,
            self.gpu_native_playable_candidate(),
            self.native_3d_gameplay_signal(),
            playable
        )
    }

    pub fn diagnostic_json(&self) -> String {
        let playable = self.native_playable_candidate();
        format!(
            "{{\"cpu\":{},\"gte\":{},\"io\":{},\"zn_board\":{},\"native_sync\":{},\"write_watch\":[{}],\"native_playability\":{},\"platform\":{},\"rom_compatibility\":{},\"rom_bytes\":{},\"banked_rom_bytes\":{},\"ram_bytes\":{},\"scratchpad_bytes\":{},\"executed_steps\":{},\"last_step\":{},\"last_outcome\":\"{:?}\",\"gpu_playable_candidate\":{},\"gte_gameplay_signal\":{},\"playable\":{},\"development_stage\":\"native_runtime_validation\"}}",
            self.cpu.json(),
            self.cpu.gte_json(),
            self.bus.io_compact_json(),
            self.bus.zn_board_json(),
            self.bus.native_sync_json(),
            self.bus.write_watch_json(),
            self.native_playability_json(),
            native_platform_json(),
            self.rom_compatibility.summary_json(),
            self.bus.rom_len(),
            self.bus.banked_rom_len(),
            self.bus.ram_len(),
            self.bus.scratchpad_len(),
            self.executed_steps,
            optional_step_json(self.last_step),
            self.last_outcome,
            self.gpu_native_playable_candidate(),
            self.native_3d_gameplay_signal(),
            playable
        )
    }

    pub fn probe_json(&self) -> String {
        let playable = self.native_playable_candidate();
        format!(
            "{{\"cpu\":{{\"pc\":{},\"pc_hex\":\"0x{:08x}\",\"cycles\":{},\"halted\":{},\"status\":{},\"status_hex\":\"0x{:08x}\",\"cause\":{},\"cause_hex\":\"0x{:08x}\",\"epc\":{},\"epc_hex\":\"0x{:08x}\"}},\"gte\":{{\"projected_vertices\":{},\"gameplay_signal\":{}}},\"runtime\":{},\"native_playability\":{},\"rom_compatibility\":{},\"executed_steps\":{},\"last_outcome\":\"{:?}\",\"gpu_playable_candidate\":{},\"playable\":{},\"development_stage\":\"native_runtime_validation\"}}",
            self.cpu.pc,
            self.cpu.pc,
            self.cpu.cycles,
            self.cpu.halted,
            self.cpu.cp0[12],
            self.cpu.cp0[12],
            self.cpu.cp0[13],
            self.cpu.cp0[13],
            self.cpu.cp0[14],
            self.cpu.cp0[14],
            self.cpu.gte_projected_vertices(),
            self.native_3d_gameplay_signal(),
            self.bus.runtime_probe_json(),
            self.native_playability_json(),
            self.rom_compatibility.summary_json(),
            self.executed_steps,
            self.last_outcome,
            self.gpu_native_playable_candidate(),
            playable
        )
    }

    pub fn compact_probe_json(&self) -> String {
        let playable = self.native_playable_candidate();
        format!(
            "{{\"cpu\":{{\"pc\":{},\"pc_hex\":\"0x{:08x}\",\"next_pc\":{},\"next_pc_hex\":\"0x{:08x}\",\"cycles\":{},\"halted\":{},\"status\":{},\"status_hex\":\"0x{:08x}\",\"cause\":{},\"cause_hex\":\"0x{:08x}\",\"epc\":{},\"epc_hex\":\"0x{:08x}\",\"r2\":{},\"r3\":{},\"r4\":{},\"r5\":{},\"r6\":{},\"r7\":{},\"r8\":{},\"r9\":{},\"r10\":{},\"r11\":{},\"r20\":{},\"r20_hex\":\"0x{:08x}\",\"sp\":{},\"sp_hex\":\"0x{:08x}\",\"ra\":{},\"ra_hex\":\"0x{:08x}\",\"pending_load\":{},\"delay_slot_branch_pc\":{}}},\"gte\":{{\"projected_vertices\":{},\"gameplay_signal\":{},\"command_counts\":[{}]}},\"runtime\":{},\"native_playability\":{},\"input_activity\":{},\"rom_compatibility\":{},\"executed_steps\":{},\"last_outcome\":\"{:?}\",\"gpu_playable_candidate\":{},\"playable\":{},\"development_stage\":\"native_runtime_validation\"}}",
            self.cpu.pc,
            self.cpu.pc,
            self.cpu.next_pc,
            self.cpu.next_pc,
            self.cpu.cycles,
            self.cpu.halted,
            self.cpu.cp0[12],
            self.cpu.cp0[12],
            self.cpu.cp0[13],
            self.cpu.cp0[13],
            self.cpu.cp0[14],
            self.cpu.cp0[14],
            self.cpu.regs[2],
            self.cpu.regs[3],
            self.cpu.regs[4],
            self.cpu.regs[5],
            self.cpu.regs[6],
            self.cpu.regs[7],
            self.cpu.regs[8],
            self.cpu.regs[9],
            self.cpu.regs[10],
            self.cpu.regs[11],
            self.cpu.regs[20],
            self.cpu.regs[20],
            self.cpu.regs[29],
            self.cpu.regs[29],
            self.cpu.regs[31],
            self.cpu.regs[31],
            self.cpu.pending_load_json(),
            self.cpu.delay_slot_branch_pc_json(),
            self.cpu.gte_projected_vertices(),
            self.native_3d_gameplay_signal(),
            self.cpu.gte_command_counts_summary_json(),
            self.bus.runtime_compact_probe_json(),
            self.native_playability_json(),
            self.bus.input_activity().json(),
            self.rom_compatibility.summary_json(),
            self.executed_steps,
            self.last_outcome,
            self.gpu_native_playable_candidate(),
            playable
        )
    }

    pub fn display_diagnosis_json(&self) -> String {
        format!(
            "{{\"cpu\":{{\"pc\":{},\"pc_hex\":\"0x{:08x}\",\"next_pc\":{},\"next_pc_hex\":\"0x{:08x}\",\"cycles\":{},\"halted\":{}}},\"gte\":{{\"projected_vertices\":{},\"gameplay_signal\":{},\"command_counts\":[{}]}},\"native_playability\":{},\"input_activity\":{},\"rom_compatibility\":{},\"executed_steps\":{},\"last_outcome\":\"{:?}\",\"gpu_playable_candidate\":{},\"gpu_rendered_scene_candidate\":{},\"playable\":{}}}",
            self.cpu.pc,
            self.cpu.pc,
            self.cpu.next_pc,
            self.cpu.next_pc,
            self.cpu.cycles,
            self.cpu.halted,
            self.cpu.gte_projected_vertices(),
            self.native_3d_gameplay_signal(),
            self.cpu.gte_command_counts_summary_json(),
            self.native_playability_json(),
            self.bus.input_activity().json(),
            self.rom_compatibility.summary_json(),
            self.executed_steps,
            self.last_outcome,
            self.gpu_native_playable_candidate(),
            self.gpu_native_rendered_scene_candidate(),
            self.native_playable_candidate()
        )
    }

    pub fn full_playability_diagnosis_json(&self) -> String {
        self.native_playability_json()
    }

    pub fn lean_probe_json(&self) -> String {
        let playable = self.native_playable_candidate();
        format!(
            "{{\"cpu\":{{\"pc\":{},\"pc_hex\":\"0x{:08x}\",\"next_pc\":{},\"next_pc_hex\":\"0x{:08x}\",\"cycles\":{},\"halted\":{},\"r2\":{},\"r4\":{},\"r5\":{},\"r10\":{},\"r10_hex\":\"0x{:08x}\",\"r15\":{},\"r15_hex\":\"0x{:08x}\"}},\"gte\":{{\"projected_vertices\":{},\"gameplay_signal\":{},\"command_counts\":[{}]}},\"input\":{},\"native_playability\":{},\"display_candidates\":[{}],\"rom_compatibility\":{},\"executed_steps\":{},\"last_outcome\":\"{:?}\",\"gpu_playable_candidate\":{},\"playable\":{},\"development_stage\":\"native_runtime_validation\"}}",
            self.cpu.pc,
            self.cpu.pc,
            self.cpu.next_pc,
            self.cpu.next_pc,
            self.cpu.cycles,
            self.cpu.halted,
            self.cpu.regs[2],
            self.cpu.regs[4],
            self.cpu.regs[5],
            self.cpu.regs[10],
            self.cpu.regs[10],
            self.cpu.regs[15],
            self.cpu.regs[15],
            self.cpu.gte_projected_vertices(),
            self.native_3d_gameplay_signal(),
            self.cpu.gte_command_counts_summary_json(),
            self.bus.input_compact_probe_json(),
            self.native_playability_compact_json(),
            self.bus.display_candidate_compact_summary_json(6),
            self.rom_compatibility.summary_json(),
            self.executed_steps,
            self.last_outcome,
            self.gpu_native_playable_candidate(),
            playable
        )
    }

    pub fn input_check_probe_json(&self) -> String {
        let gpu_playable_candidate = self.gpu_native_playable_candidate();
        let gpu_rendered_scene_candidate = self.gpu_native_rendered_scene_candidate();
        let gte_gameplay_signal = self.native_3d_gameplay_signal();
        let display_frame_guard = self
            .native_display_frame_supports_playable_candidate_with_context(
                gpu_playable_candidate,
                gpu_rendered_scene_candidate,
                gte_gameplay_signal,
            );
        let render_context_allows_periodic =
            gpu_playable_candidate || (gpu_rendered_scene_candidate && gte_gameplay_signal);
        let stable_frame = self.display_frame();
        let actual_frame = self.actual_display_frame();
        let stable_frame_guard =
            native_display_frame_or_window_supports_playable_candidate_with_render_context(
                &stable_frame,
                render_context_allows_periodic,
            );
        let actual_frame_guard =
            native_display_frame_or_window_supports_playable_candidate_with_render_context(
                &actual_frame,
                render_context_allows_periodic,
            );
        let display_frame_periodic_ghosting =
            native_display_frame_has_periodic_horizontal_ghosting(&stable_frame)
                && !actual_frame_guard;
        let native_3d_gameplay_candidate =
            display_frame_guard && gpu_rendered_scene_candidate && gte_gameplay_signal;
        let playable_candidate =
            display_frame_guard && (gpu_playable_candidate || native_3d_gameplay_candidate);
        let classification = if playable_candidate && native_3d_gameplay_candidate {
            "native_3d_gameplay_candidate"
        } else if playable_candidate {
            "native_playable_candidate"
        } else if gpu_playable_candidate && display_frame_periodic_ghosting {
            "gpu_candidate_rejected_periodic_display"
        } else if gpu_playable_candidate && !display_frame_guard {
            "gpu_candidate_rejected_display_frame"
        } else if display_frame_periodic_ghosting {
            "display_frame_periodic_ghosting"
        } else if !display_frame_guard {
            "display_frame_not_gameplay"
        } else if !gpu_rendered_scene_candidate {
            "gpu_scene_not_rendered"
        } else {
            "missing_native_3d_gte_signal"
        };

        format!(
            "{{\"cpu\":{{\"pc\":{},\"pc_hex\":\"0x{:08x}\",\"next_pc\":{},\"next_pc_hex\":\"0x{:08x}\",\"cycles\":{},\"halted\":{},\"r10\":{},\"r10_hex\":\"0x{:08x}\",\"r15\":{},\"r15_hex\":\"0x{:08x}\"}},\"vblank_count\":{},\"gte\":{{\"projected_vertices\":{},\"gameplay_signal\":{},\"command_counts\":[{}]}},\"runtime\":{},\"input_activity\":{},\"playability\":{{\"playable_candidate\":{},\"classification\":\"{}\",\"gpu_playable_candidate\":{},\"gpu_rendered_scene_candidate\":{},\"gte_gameplay_signal\":{},\"display_frame_guard\":{},\"stable_frame_guard\":{},\"actual_frame_guard\":{},\"display_frame_periodic_ghosting\":{}}},\"rom_compatibility\":{},\"executed_steps\":{},\"last_outcome\":\"{:?}\"}}",
            self.cpu.pc,
            self.cpu.pc,
            self.cpu.next_pc,
            self.cpu.next_pc,
            self.cpu.cycles,
            self.cpu.halted,
            self.cpu.regs[10],
            self.cpu.regs[10],
            self.cpu.regs[15],
            self.cpu.regs[15],
            self.vblank_count(),
            self.cpu.gte_projected_vertices(),
            gte_gameplay_signal,
            self.cpu.gte_command_counts_summary_json(),
            self.bus.input_compact_probe_json(),
            self.bus.input_activity().json(),
            playable_candidate,
            classification,
            gpu_playable_candidate,
            gpu_rendered_scene_candidate,
            gte_gameplay_signal,
            display_frame_guard,
            stable_frame_guard,
            actual_frame_guard,
            display_frame_periodic_ghosting,
            self.rom_compatibility.summary_json(),
            self.executed_steps,
            self.last_outcome
        )
    }

    pub fn input_probe_json(&self) -> String {
        format!(
            "{{\"cpu\":{{\"pc\":{},\"pc_hex\":\"0x{:08x}\",\"next_pc\":{},\"next_pc_hex\":\"0x{:08x}\",\"cycles\":{},\"halted\":{}}},\"runtime\":{},\"executed_steps\":{},\"last_outcome\":\"{:?}\"}}",
            self.cpu.pc,
            self.cpu.pc,
            self.cpu.next_pc,
            self.cpu.next_pc,
            self.cpu.cycles,
            self.cpu.halted,
            self.bus.input_probe_json(),
            self.executed_steps,
            self.last_outcome
        )
    }

    pub fn input_compact_probe_json(&self) -> String {
        format!(
            "{{\"cpu\":{{\"pc\":{},\"pc_hex\":\"0x{:08x}\",\"next_pc\":{},\"next_pc_hex\":\"0x{:08x}\",\"cycles\":{},\"halted\":{}}},\"runtime\":{},\"executed_steps\":{},\"last_outcome\":\"{:?}\"}}",
            self.cpu.pc,
            self.cpu.pc,
            self.cpu.next_pc,
            self.cpu.next_pc,
            self.cpu.cycles,
            self.cpu.halted,
            self.bus.input_compact_probe_json(),
            self.executed_steps,
            self.last_outcome
        )
    }

    pub fn input_summary_json(&self) -> String {
        format!(
            "{{\"cpu\":{{\"pc\":{},\"pc_hex\":\"0x{:08x}\",\"next_pc\":{},\"next_pc_hex\":\"0x{:08x}\",\"cycles\":{},\"halted\":{},\"r2\":{},\"r4\":{},\"r5\":{},\"r10\":{},\"r10_hex\":\"0x{:08x}\",\"r15\":{},\"r15_hex\":\"0x{:08x}\"}},\"runtime\":{},\"rom_compatibility\":{},\"executed_steps\":{},\"last_outcome\":\"{:?}\",\"gpu_playable_candidate\":{},\"playable\":{}}}",
            self.cpu.pc,
            self.cpu.pc,
            self.cpu.next_pc,
            self.cpu.next_pc,
            self.cpu.cycles,
            self.cpu.halted,
            self.cpu.regs[2],
            self.cpu.regs[4],
            self.cpu.regs[5],
            self.cpu.regs[10],
            self.cpu.regs[10],
            self.cpu.regs[15],
            self.cpu.regs[15],
            self.bus.input_summary_json(),
            self.rom_compatibility.summary_json(),
            self.executed_steps,
            self.last_outcome,
            self.gpu_native_playable_candidate(),
            self.native_playable_candidate()
        )
    }

    pub fn security_probe_json(&self) -> String {
        format!(
            "{{\"cpu\":{{\"pc\":{},\"pc_hex\":\"0x{:08x}\",\"next_pc\":{},\"next_pc_hex\":\"0x{:08x}\",\"cycles\":{},\"halted\":{},\"r2\":{},\"r4\":{},\"r5\":{},\"r10\":{},\"r10_hex\":\"0x{:08x}\",\"r15\":{},\"r15_hex\":\"0x{:08x}\"}},\"runtime\":{},\"rom_compatibility\":{},\"executed_steps\":{},\"last_outcome\":\"{:?}\",\"gpu_playable_candidate\":{},\"playable\":{}}}",
            self.cpu.pc,
            self.cpu.pc,
            self.cpu.next_pc,
            self.cpu.next_pc,
            self.cpu.cycles,
            self.cpu.halted,
            self.cpu.regs[2],
            self.cpu.regs[4],
            self.cpu.regs[5],
            self.cpu.regs[10],
            self.cpu.regs[10],
            self.cpu.regs[15],
            self.cpu.regs[15],
            self.bus.security_probe_json(),
            self.rom_compatibility.summary_json(),
            self.executed_steps,
            self.last_outcome,
            self.gpu_native_playable_candidate(),
            self.native_playable_candidate()
        )
    }

    pub fn native_sync_compact_json(&self) -> String {
        self.bus.native_sync_compact_json()
    }
}

pub fn native_window_frame_from_display(frame: &NativeDisplayFrame) -> NativeDisplayFrame {
    let width = NATIVE_WINDOW_FRAME_WIDTH;
    let height = NATIVE_WINDOW_FRAME_HEIGHT;
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

    let deinterlace_candidate = native_window_should_deinterlace_frame(frame);
    let deinterlace_field_offset = if deinterlace_candidate {
        native_window_deinterlace_field_offset(frame)
    } else {
        0
    };
    let deinterlace = deinterlace_candidate
        && !native_window_deinterlace_field_is_texture_atlas_artifact(
            frame,
            deinterlace_field_offset,
        );
    let source_layout_height = if deinterlace {
        (frame.height / 2).max(1)
    } else {
        frame.height
    };
    let sparse_title_logo_artifact = native_display_frame_has_sparse_title_logo_artifact(frame);
    let (viewport_x, viewport_y, viewport_width, viewport_height) = if sparse_title_logo_artifact {
        native_window_letterbox_viewport_preserving_aspect(
            frame.width,
            source_layout_height,
            width,
            height,
        )
    } else {
        native_window_letterbox_viewport(frame.width, source_layout_height, width, height)
    };

    for y in 0..viewport_height {
        let source_y = native_window_source_y(
            y,
            frame.height,
            viewport_height,
            deinterlace,
            deinterlace_field_offset,
        );
        let target_start = viewport_y
            .saturating_add(y)
            .saturating_mul(width)
            .saturating_add(viewport_x);
        for x in 0..viewport_width {
            let source_x = x.saturating_mul(frame.width) / viewport_width;
            pixels[target_start + x] =
                frame.pixels[source_y.saturating_mul(frame.width) + source_x];
        }
    }

    let scaled = NativeDisplayFrame {
        width,
        height,
        pixels,
    };
    if sparse_title_logo_artifact {
        return scaled;
    }

    native_window_expand_sparse_vertical_frame(&scaled)
}

fn native_window_expand_sparse_vertical_frame(frame: &NativeDisplayFrame) -> NativeDisplayFrame {
    if frame.width == 0
        || frame.height == 0
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
        || native_window_sparse_frame_is_text_screen(frame)
    {
        return frame.clone();
    }

    let Some((first, last, occupied_rows)) = native_window_occupied_row_bounds(frame) else {
        return frame.clone();
    };
    let span = last.saturating_sub(first).saturating_add(1);
    if span >= frame.height.saturating_mul(45) / 100
        || occupied_rows >= frame.height / 2
        || span < 8
        || !native_window_sparse_vertical_frame_can_expand(frame, first, last, occupied_rows)
    {
        return frame.clone();
    }

    let margin = (span / 12).clamp(2, 16);
    let source_start = first.saturating_sub(margin);
    let source_end = last
        .saturating_add(margin)
        .min(frame.height.saturating_sub(1));
    let source_height = source_end.saturating_sub(source_start).saturating_add(1);
    if source_height == 0 || source_height >= frame.height.saturating_mul(3) / 4 {
        return frame.clone();
    }

    let mut pixels = vec![0; frame.width.saturating_mul(frame.height)];
    for y in 0..frame.height {
        let source_y = source_start
            .saturating_add(y.saturating_mul(source_height) / frame.height)
            .min(frame.height.saturating_sub(1));
        let source_offset = source_y.saturating_mul(frame.width);
        let target_offset = y.saturating_mul(frame.width);
        pixels[target_offset..target_offset + frame.width]
            .copy_from_slice(&frame.pixels[source_offset..source_offset + frame.width]);
    }

    NativeDisplayFrame {
        width: frame.width,
        height: frame.height,
        pixels,
    }
}

fn native_window_sparse_vertical_frame_can_expand(
    frame: &NativeDisplayFrame,
    first: usize,
    last: usize,
    occupied_rows: usize,
) -> bool {
    let height = frame.height.max(1);
    let span = last.saturating_sub(first).saturating_add(1);
    if first < height / 8 || last > height.saturating_mul(7) / 8 {
        return false;
    }
    if span < height / 5 || occupied_rows < height / 6 {
        return false;
    }
    if native_display_frame_has_sparse_title_logo_artifact(frame)
        || native_display_frame_has_title_logo_overlay(frame)
        || native_display_frame_has_glyph_texture_atlas_artifact(frame)
    {
        return false;
    }

    true
}

fn native_window_sparse_frame_is_text_screen(frame: &NativeDisplayFrame) -> bool {
    let mut unique_colors = Vec::new();
    let mut horizontal_color_changes = 0usize;
    let mut nonzero_pixels = 0usize;
    let mut occupied_rows = 0usize;
    let row_content_cutoff = (frame.width / 20).max(1);

    for y in 0..frame.height {
        let row = y.saturating_mul(frame.width);
        let mut row_nonzero_pixels = 0usize;
        for x in 0..frame.width {
            let color = frame.pixels.get(row + x).copied().unwrap_or_default() & 0x00ff_ffff;
            if color != 0 {
                nonzero_pixels = nonzero_pixels.saturating_add(1);
                row_nonzero_pixels = row_nonzero_pixels.saturating_add(1);
                if unique_colors.len() < 49 && !unique_colors.contains(&color) {
                    unique_colors.push(color);
                }
            }
            if x > 0 {
                let previous =
                    frame.pixels.get(row + x - 1).copied().unwrap_or_default() & 0x00ff_ffff;
                if previous != color {
                    horizontal_color_changes = horizontal_color_changes.saturating_add(1);
                }
            }
        }
        if row_nonzero_pixels >= row_content_cutoff {
            occupied_rows = occupied_rows.saturating_add(1);
        }
    }

    let total_pixels = frame.width.saturating_mul(frame.height);
    total_pixels > 0
        && nonzero_pixels > 0
        && nonzero_pixels.saturating_mul(100) <= total_pixels.saturating_mul(45)
        && horizontal_color_changes.saturating_mul(5) >= nonzero_pixels.max(1)
        && (occupied_rows >= 16 || occupied_rows >= frame.height.saturating_div(6))
        && unique_colors.len() <= 48
        && !native_display_frame_has_saturated_logo_transition(frame)
}

fn native_window_occupied_row_bounds(frame: &NativeDisplayFrame) -> Option<(usize, usize, usize)> {
    let mut first = None;
    let mut last = 0usize;
    let mut occupied_rows = 0usize;
    let meaningful_row_pixels = (frame.width / 20).max(1);
    for y in 0..frame.height {
        let row_start = y.saturating_mul(frame.width);
        let row_end = row_start.saturating_add(frame.width);
        let row_pixels = frame.pixels[row_start..row_end]
            .iter()
            .filter(|pixel| **pixel & 0x00ff_ffff != 0)
            .take(meaningful_row_pixels)
            .count();
        if row_pixels >= meaningful_row_pixels {
            first.get_or_insert(y);
            last = y;
            occupied_rows = occupied_rows.saturating_add(1);
        }
    }
    first.map(|first| (first, last, occupied_rows))
}

fn native_window_letterbox_viewport(
    source_width: usize,
    source_height: usize,
    target_width: usize,
    target_height: usize,
) -> (usize, usize, usize, usize) {
    if source_width == 0 || source_height == 0 || target_width == 0 || target_height == 0 {
        return (0, 0, 0, 0);
    }
    if native_window_should_stretch_video_frame(
        source_width,
        source_height,
        target_width,
        target_height,
    ) {
        return (0, 0, target_width, target_height);
    }

    let source_width_u128 = source_width as u128;
    let source_height_u128 = source_height as u128;
    let target_width_u128 = target_width as u128;
    let target_height_u128 = target_height as u128;
    let (viewport_width, viewport_height) = if source_width_u128.saturating_mul(target_height_u128)
        > target_width_u128.saturating_mul(source_height_u128)
    {
        let viewport_height = target_width_u128
            .saturating_mul(source_height_u128)
            .checked_div(source_width_u128)
            .unwrap_or(1)
            .max(1) as usize;
        (target_width, viewport_height.min(target_height))
    } else {
        let viewport_width = target_height_u128
            .saturating_mul(source_width_u128)
            .checked_div(source_height_u128)
            .unwrap_or(1)
            .max(1) as usize;
        (viewport_width.min(target_width), target_height)
    };

    (
        target_width.saturating_sub(viewport_width) / 2,
        target_height.saturating_sub(viewport_height) / 2,
        viewport_width,
        viewport_height,
    )
}

fn native_window_letterbox_viewport_preserving_aspect(
    source_width: usize,
    source_height: usize,
    target_width: usize,
    target_height: usize,
) -> (usize, usize, usize, usize) {
    if source_width == 0 || source_height == 0 || target_width == 0 || target_height == 0 {
        return (0, 0, 0, 0);
    }

    let source_width_u128 = source_width as u128;
    let source_height_u128 = source_height as u128;
    let target_width_u128 = target_width as u128;
    let target_height_u128 = target_height as u128;
    let (viewport_width, viewport_height) = if source_width_u128.saturating_mul(target_height_u128)
        > target_width_u128.saturating_mul(source_height_u128)
    {
        let viewport_height = target_width_u128
            .saturating_mul(source_height_u128)
            .checked_div(source_width_u128)
            .unwrap_or(1)
            .max(1) as usize;
        (target_width, viewport_height.min(target_height))
    } else {
        let viewport_width = target_height_u128
            .saturating_mul(source_width_u128)
            .checked_div(source_height_u128)
            .unwrap_or(1)
            .max(1) as usize;
        (viewport_width.min(target_width), target_height)
    };

    (
        target_width.saturating_sub(viewport_width) / 2,
        target_height.saturating_sub(viewport_height) / 2,
        viewport_width,
        viewport_height,
    )
}

fn native_window_should_stretch_video_frame(
    source_width: usize,
    source_height: usize,
    target_width: usize,
    target_height: usize,
) -> bool {
    if target_width != NATIVE_WINDOW_FRAME_WIDTH || target_height != NATIVE_WINDOW_FRAME_HEIGHT {
        return false;
    }

    let common_ps1_width = matches!(source_width, 256 | 320 | 368 | 384 | 512 | 640);
    let common_ps1_height = (224..=256).contains(&source_height) || source_height == 480;
    common_ps1_width && common_ps1_height
}

fn native_window_should_deinterlace_frame(frame: &NativeDisplayFrame) -> bool {
    if frame.height < NATIVE_WINDOW_FRAME_HEIGHT {
        return false;
    }
    if native_display_frame_has_caption_stalled_select_ui_artifact(frame) {
        return false;
    }

    let field_height = (frame.height / 2).max(1);
    let top = native_window_deinterlace_field_profile(frame, 0, field_height);
    let bottom = native_window_deinterlace_field_profile(frame, field_height, field_height);
    let top_score = top.score();
    let bottom_score = bottom.score();
    let stronger = top_score.max(bottom_score);
    let weaker = top_score.min(bottom_score);
    let field_pixels = frame.width.saturating_mul(field_height) as u64;
    if top.nonzero.max(bottom.nonzero).saturating_mul(100) < field_pixels.saturating_mul(18) {
        return false;
    }
    if stronger <= weaker.saturating_mul(2).saturating_add(1_024) {
        return false;
    }

    let (weaker_profile, stronger_profile) = if top_score <= bottom_score {
        (top, bottom)
    } else {
        (bottom, top)
    };
    let blank_field_nonzero_limit = (field_pixels / 400).max(64);
    let residual_field_nonzero_limit = (field_pixels / 32).max(blank_field_nonzero_limit);
    let weak_blank_field = weaker_profile.nonzero <= blank_field_nonzero_limit
        && weaker_profile.color_changes <= frame.width as u64;
    let weak_residual_artifact = weaker_profile.nonzero <= residual_field_nonzero_limit
        && weaker_profile.color_changes >= frame.width as u64
        && weaker_profile.color_changes.saturating_mul(4)
            <= stronger_profile.color_changes.max(frame.width as u64);
    let asymmetric_stacked_field = native_window_has_asymmetric_stacked_fields(
        frame,
        weaker_profile,
        stronger_profile,
        field_pixels,
    );

    asymmetric_stacked_field
        || ((weak_blank_field || weak_residual_artifact)
            && weaker_profile.nonzero.saturating_mul(8) <= stronger_profile.nonzero.max(1)
            && weaker.saturating_mul(8) <= stronger.max(1))
}

fn native_window_has_asymmetric_stacked_fields(
    frame: &NativeDisplayFrame,
    weaker: NativeWindowFieldProfile,
    stronger: NativeWindowFieldProfile,
    field_pixels: u64,
) -> bool {
    weaker.nonzero.saturating_mul(100) >= field_pixels.saturating_mul(5)
        && stronger.nonzero.saturating_mul(100) >= field_pixels.saturating_mul(18)
        && weaker.nonzero.saturating_mul(20) <= stronger.nonzero.saturating_mul(9)
        && weaker.color_changes.saturating_mul(4) <= stronger.color_changes.max(frame.width as u64)
}

pub fn native_window_prefers_stronger_stacked_field(frame: &NativeDisplayFrame) -> bool {
    if !native_window_should_deinterlace_frame(frame) {
        return false;
    }

    let field_height = (frame.height / 2).max(1);
    let top = native_window_deinterlace_field_profile(frame, 0, field_height);
    let bottom = native_window_deinterlace_field_profile(frame, field_height, field_height);
    let (weaker, stronger) = if top.score() <= bottom.score() {
        (top, bottom)
    } else {
        (bottom, top)
    };
    let field_pixels = frame.width.saturating_mul(field_height) as u64;
    let field_offset = native_window_deinterlace_field_offset(frame);

    native_window_has_asymmetric_stacked_fields(frame, weaker, stronger, field_pixels)
        && !native_window_deinterlace_field_is_texture_atlas_artifact(frame, field_offset)
}

fn native_window_source_y(
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

fn native_window_deinterlace_field_offset(frame: &NativeDisplayFrame) -> usize {
    let field_height = (frame.height / 2).max(1);
    let top_score = native_window_deinterlace_field_profile(frame, 0, field_height).score();
    let bottom_score =
        native_window_deinterlace_field_profile(frame, field_height, field_height).score();
    if bottom_score > top_score {
        field_height
    } else {
        0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct NativeWindowFieldProfile {
    nonzero: u64,
    color_changes: u64,
}

impl NativeWindowFieldProfile {
    fn score(self) -> u64 {
        self.nonzero
            .saturating_mul(4)
            .saturating_add(self.color_changes.saturating_mul(3))
    }
}

fn native_window_deinterlace_field_is_texture_atlas_artifact(
    frame: &NativeDisplayFrame,
    field_offset: usize,
) -> bool {
    if frame.width < 256
        || frame.height < 240
        || !frame.height.is_multiple_of(2)
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
    {
        return false;
    }

    let field_height = frame.height / 2;
    if field_offset.saturating_add(field_height) > frame.height {
        return false;
    }

    let other_offset = if field_offset == 0 { field_height } else { 0 };
    let active = native_window_deinterlace_field_profile(frame, field_offset, field_height);
    let other = native_window_deinterlace_field_profile(frame, other_offset, field_height);
    let field_pixels = frame.width.saturating_mul(field_height) as u64;
    let other_is_blank_or_residual = other.nonzero.saturating_mul(100) <= field_pixels.max(1) * 3
        && other.color_changes.saturating_mul(100) <= field_pixels.max(1) * 4;
    if !other_is_blank_or_residual || active.nonzero.saturating_mul(100) < field_pixels.max(1) * 15
    {
        return false;
    }

    let field = native_window_field_frame(frame, field_offset, field_height);
    if native_window_field_has_live_scene_density(&field) {
        return false;
    }

    native_display_frame_has_title_logo_overlay(&field)
        || native_display_frame_has_sparse_title_logo_artifact(&field)
        || native_display_frame_has_glyph_texture_atlas_artifact(&field)
}

fn native_window_field_has_live_scene_density(frame: &NativeDisplayFrame) -> bool {
    if frame.width < 256
        || frame.height < 200
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
    {
        return false;
    }

    let total = frame.width.saturating_mul(frame.height).max(1) as u64;
    let mut nonzero = 0u64;
    let mut bright = 0u64;
    let mut horizontal_changes = 0u64;
    let mut occupied_rows = 0u64;
    let mut unique_colors = Vec::new();
    let meaningful_row_pixels = (frame.width / 20).max(1);

    for y in 0..frame.height {
        let row_start = y.saturating_mul(frame.width);
        let mut row_nonzero = 0usize;
        let mut previous = None;
        for x in 0..frame.width {
            let color = frame.pixels[row_start + x] & 0x00ff_ffff;
            if color != 0 {
                nonzero = nonzero.saturating_add(1);
                row_nonzero = row_nonzero.saturating_add(1);
                let red = (color >> 16) & 0xff;
                let green = (color >> 8) & 0xff;
                let blue = color & 0xff;
                if red.max(green).max(blue) >= 0x80 {
                    bright = bright.saturating_add(1);
                }
                if unique_colors.len() < 97 && !unique_colors.contains(&color) {
                    unique_colors.push(color);
                }
            }
            if previous.is_some_and(|previous| previous != color) {
                horizontal_changes = horizontal_changes.saturating_add(1);
            }
            previous = Some(color);
        }
        if row_nonzero >= meaningful_row_pixels {
            occupied_rows = occupied_rows.saturating_add(1);
        }
    }

    nonzero.saturating_mul(100) >= total.saturating_mul(28)
        && bright.saturating_mul(100) >= total.saturating_mul(3)
        && horizontal_changes.saturating_mul(100) >= total.saturating_mul(4)
        && occupied_rows.saturating_mul(100) >= (frame.height as u64).saturating_mul(45)
        && unique_colors.len() >= 32
}

fn native_window_field_frame(
    frame: &NativeDisplayFrame,
    field_offset: usize,
    field_height: usize,
) -> NativeDisplayFrame {
    let mut pixels = Vec::with_capacity(frame.width.saturating_mul(field_height));
    for y in field_offset..field_offset.saturating_add(field_height).min(frame.height) {
        let row_start = y.saturating_mul(frame.width);
        let row_end = row_start.saturating_add(frame.width);
        pixels.extend_from_slice(&frame.pixels[row_start..row_end]);
    }

    NativeDisplayFrame {
        width: frame.width,
        height: field_height,
        pixels,
    }
}

fn native_window_deinterlace_field_profile(
    frame: &NativeDisplayFrame,
    start_y: usize,
    field_height: usize,
) -> NativeWindowFieldProfile {
    let end_y = start_y.saturating_add(field_height).min(frame.height);
    let mut nonzero = 0u64;
    let mut color_changes = 0u64;
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
        }
    }

    NativeWindowFieldProfile {
        nonzero,
        color_changes,
    }
}

fn apply_runtime_board_asset_overrides(mut assets: NativeBoardAssets) -> NativeBoardAssets {
    if let Ok(mode) = env::var(NATIVE_AT28C16_MODE_ENV) {
        assets = apply_at28c16_mode_override(assets, &mode);
    }

    assets
}

fn apply_at28c16_mode_override(mut assets: NativeBoardAssets, mode: &str) -> NativeBoardAssets {
    if native_at28c16_mode_is_blank(mode) {
        assets.at28c16 = Some(vec![0xff; 2048]);
        assets.at28c16_blank_default = true;
        assets.legacy_zinc_input_compat = false;
    }

    assets
}

fn native_at28c16_mode_is_blank(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "blank" | "default" | "empty" | "erase" | "erased" | "ff" | "ignore" | "none"
    )
}

#[derive(Clone, Debug, Default)]
pub struct NativeTraceConfig {
    pub stop_pc: Option<u32>,
    pub stop_below_pc: Option<u32>,
    pub stop_above_pc: Option<u32>,
    pub stop_unmapped_pc: bool,
    pub stop_unaligned_pc: bool,
    pub stop_watch_access: bool,
    pub stop_watch_data: bool,
    pub stop_watch_write: bool,
    pub stop_br2_packed_helper_accept: bool,
    pub stop_pc_skip: u64,
    pub stop_watch_write_skip: u64,
    pub stop_cycles_elapsed: Option<u64>,
    pub stop_excode: Option<u32>,
    pub stop_excodes: Vec<u32>,
    pub watch_ranges: Vec<(u32, u32)>,
    pub watch_only: bool,
    pub watch_after_vblank: Option<u64>,
}

impl NativeTraceConfig {
    fn reached_stop_condition(
        &self,
        pc: u32,
        stop_pc_hits_to_skip: &mut u64,
        pc_mapped_for_execution: bool,
        watch_access_hit: bool,
        watch_data_hit: bool,
        watch_write_hit: bool,
    ) -> bool {
        let reached_stop_pc = if self.stop_pc.is_some_and(|stop_pc| pc == stop_pc) {
            if *stop_pc_hits_to_skip > 0 {
                *stop_pc_hits_to_skip -= 1;
                false
            } else {
                true
            }
        } else {
            false
        };
        let reached_low_pc = self.stop_below_pc.is_some_and(|stop_pc| pc < stop_pc);
        let reached_high_pc = self.stop_above_pc.is_some_and(|stop_pc| pc > stop_pc);
        let reached_unmapped_pc = self.stop_unmapped_pc && !pc_mapped_for_execution;
        let reached_unaligned_pc = self.stop_unaligned_pc && pc & 0x03 != 0;
        let reached_watch_access = self.stop_watch_access && watch_access_hit;
        let reached_watch_data = self.stop_watch_data && watch_data_hit;
        let reached_watch_write = self.stop_watch_write && watch_write_hit;
        reached_stop_pc
            || reached_low_pc
            || reached_high_pc
            || reached_unmapped_pc
            || reached_unaligned_pc
            || reached_watch_access
            || reached_watch_data
            || reached_watch_write
    }
}

#[derive(Clone, Debug)]
pub struct NativeTrace {
    requested_steps: u64,
    executed_steps: u64,
    unique_pcs: usize,
    hot_pcs: Vec<NativeTracePc>,
    unsupported_instructions: Vec<NativeTraceInstruction>,
    recent_steps: Vec<StepReport>,
    watch_write_count: u64,
    bus_access_trace_json: String,
    state_json: String,
}

impl NativeTrace {
    fn new(parts: NativeTraceParts) -> Self {
        let unique_pcs = parts.pc_counts.len();
        let mut hot_pcs = parts
            .pc_counts
            .into_iter()
            .map(|(pc, count)| NativeTracePc { pc, count })
            .collect::<Vec<_>>();
        hot_pcs.sort_by(|left, right| right.count.cmp(&left.count).then(left.pc.cmp(&right.pc)));
        hot_pcs.truncate(parts.hot_limit);

        let mut unsupported_instructions = parts
            .unsupported
            .into_iter()
            .map(|(instruction, count)| NativeTraceInstruction { instruction, count })
            .collect::<Vec<_>>();
        unsupported_instructions.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then(left.instruction.cmp(&right.instruction))
        });

        Self {
            requested_steps: parts.requested_steps,
            executed_steps: parts.executed_steps,
            unique_pcs,
            hot_pcs,
            unsupported_instructions,
            recent_steps: parts.recent_steps,
            watch_write_count: parts.watch_write_count,
            bus_access_trace_json: parts.bus_access_trace_json,
            state_json: parts.state_json,
        }
    }

    pub fn json(&self) -> String {
        let hot_pcs = self
            .hot_pcs
            .iter()
            .map(NativeTracePc::json)
            .collect::<Vec<_>>()
            .join(",");
        let unsupported_instructions = self
            .unsupported_instructions
            .iter()
            .map(NativeTraceInstruction::json)
            .collect::<Vec<_>>()
            .join(",");
        let recent_steps = self
            .recent_steps
            .iter()
            .map(StepReport::json)
            .collect::<Vec<_>>()
            .join(",");

        format!(
            "{{\"requested_steps\":{},\"executed_steps\":{},\"unique_pcs\":{},\"hot_pcs\":[{}],\"unsupported_instructions\":[{}],\"recent_steps\":[{}],\"watch_write_count\":{},\"bus_access_trace\":[{}],\"state\":{}}}",
            self.requested_steps,
            self.executed_steps,
            self.unique_pcs,
            hot_pcs,
            unsupported_instructions,
            recent_steps,
            self.watch_write_count,
            self.bus_access_trace_json,
            self.state_json
        )
    }

    pub fn compact_json(&self) -> String {
        let hot_pcs = self
            .hot_pcs
            .iter()
            .map(NativeTracePc::json)
            .collect::<Vec<_>>()
            .join(",");
        let unsupported_instructions = self
            .unsupported_instructions
            .iter()
            .map(NativeTraceInstruction::json)
            .collect::<Vec<_>>()
            .join(",");
        let recent_steps = self
            .recent_steps
            .iter()
            .map(StepReport::json)
            .collect::<Vec<_>>()
            .join(",");

        format!(
            "{{\"requested_steps\":{},\"executed_steps\":{},\"unique_pcs\":{},\"hot_pcs\":[{}],\"unsupported_instructions\":[{}],\"recent_steps\":[{}]}}",
            self.requested_steps,
            self.executed_steps,
            self.unique_pcs,
            hot_pcs,
            unsupported_instructions,
            recent_steps
        )
    }

    pub fn bus_access_trace_json(&self) -> &str {
        &self.bus_access_trace_json
    }

    pub fn watch_write_count(&self) -> u64 {
        self.watch_write_count
    }
}

#[derive(Debug)]
struct NativeTraceParts {
    requested_steps: u64,
    executed_steps: u64,
    pc_counts: BTreeMap<u32, u64>,
    unsupported: BTreeMap<u32, u64>,
    recent_steps: Vec<StepReport>,
    hot_limit: usize,
    watch_write_count: u64,
    bus_access_trace_json: String,
    state_json: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeTracePc {
    pc: u32,
    count: u64,
}

impl NativeTracePc {
    fn json(&self) -> String {
        format!(
            "{{\"pc\":{},\"pc_hex\":\"0x{:08x}\",\"count\":{}}}",
            self.pc, self.pc, self.count
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeTraceInstruction {
    instruction: u32,
    count: u64,
}

impl NativeTraceInstruction {
    fn json(&self) -> String {
        format!(
            "{{\"instruction\":{},\"instruction_hex\":\"0x{:08x}\",\"count\":{}}}",
            self.instruction, self.instruction, self.count
        )
    }
}

fn optional_step_json(report: Option<StepReport>) -> String {
    report.map_or_else(|| "null".to_string(), |report| report.json())
}

fn native_display_frame_has_visible_detail(frame: &NativeDisplayFrame) -> bool {
    if frame.width == 0
        || frame.height == 0
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
    {
        return false;
    }

    let mut nonzero = 0usize;
    let mut horizontal_changes = 0usize;
    let mut sampled_colors = [0u32; 32];
    let mut sampled_color_count = 0usize;
    for y in 0..frame.height {
        let row = y.saturating_mul(frame.width);
        let mut previous = None;
        for x in 0..frame.width {
            let color = frame.pixels[row + x] & 0x00ff_ffff;
            if color != 0 {
                nonzero = nonzero.saturating_add(1);
            }
            if previous.is_some_and(|previous| previous != color) {
                horizontal_changes = horizontal_changes.saturating_add(1);
            }
            previous = Some(color);

            if color != 0
                && x.is_multiple_of((frame.width / 16).max(1))
                && y.is_multiple_of((frame.height / 16).max(1))
                && !sampled_colors[..sampled_color_count].contains(&color)
                && sampled_color_count < sampled_colors.len()
            {
                sampled_colors[sampled_color_count] = color;
                sampled_color_count += 1;
            }
        }
    }

    let total = frame.width.saturating_mul(frame.height);
    nonzero.saturating_mul(100) >= total.saturating_mul(10)
        && horizontal_changes.saturating_mul(100) >= total
        && sampled_color_count >= 4
}

fn native_display_frame_supports_playable_candidate_with_render_context(
    frame: &NativeDisplayFrame,
    render_context_allows_periodic: bool,
) -> bool {
    let contextual_live_stage =
        render_context_allows_periodic && native_display_frame_has_context_live_stage_hud(frame);
    let contextual_full_live_scene =
        render_context_allows_periodic && native_display_frame_has_context_full_live_scene(frame);
    let texture_page_blocks =
        native_display_frame_has_texture_page_fragment_artifact(frame) && !contextual_live_stage;
    let caption_stall_blocks = native_display_frame_has_caption_stalled_select_ui_artifact(frame);
    let title_logo_blocks = native_display_frame_has_title_logo_overlay(frame)
        && !contextual_live_stage
        && !contextual_full_live_scene;
    let saturated_logo_blocks =
        native_display_frame_has_saturated_logo_transition(frame) && !contextual_full_live_scene;
    native_display_frame_has_visible_detail(frame)
        && !texture_page_blocks
        && !caption_stall_blocks
        && !saturated_logo_blocks
        && !title_logo_blocks
        && !native_display_frame_has_periodic_horizontal_ghosting(frame)
}

fn native_display_frame_has_caption_stalled_select_ui_artifact(frame: &NativeDisplayFrame) -> bool {
    if frame.width < 512
        || frame.height < 240
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
    {
        return false;
    }

    let mut unique_colors = Vec::new();
    let mut color_buckets = [0usize; 4096];
    let mut horizontal_changes = 0usize;
    let mut nonzero_pixels = 0usize;
    let mut black_pixels = 0usize;
    let mut warm_pixels = 0usize;
    let mut red_dominant_pixels = 0usize;
    let row_content_cutoff = (frame.width / 20).max(1);
    let mut first_occupied_row = None;
    let mut last_occupied_row = None;
    let warning_region_x_start = frame.width / 20;
    let warning_region_x_end = frame.width.saturating_mul(48) / 100;
    let warning_region_y_start = frame.height / 12;
    let warning_region_y_end = frame.height.saturating_mul(38) / 100;
    let warning_row_cutoff = (frame.width / 80).max(6);
    let mut top_left_warning_text_rows = 0usize;
    let bottom_band_start = frame.height.saturating_mul(4) / 5;
    let mut bottom_caption_pixels = 0usize;
    let mut bottom_dark_pixels = 0usize;

    for y in 0..frame.height {
        let row = y.saturating_mul(frame.width);
        let mut row_nonzero_pixels = 0usize;
        let mut row_warning_text_pixels = 0usize;
        let mut previous = None;
        for x in 0..frame.width {
            let color = frame.pixels[row + x] & 0x00ff_ffff;
            let red = (color >> 16) & 0xff;
            let green = (color >> 8) & 0xff;
            let blue = color & 0xff;
            let max_channel = red.max(green).max(blue);
            let min_channel = red.min(green).min(blue);
            let luma =
                (red.saturating_mul(30) + green.saturating_mul(59) + blue.saturating_mul(11)) / 100;

            if color != 0 {
                nonzero_pixels = nonzero_pixels.saturating_add(1);
                row_nonzero_pixels = row_nonzero_pixels.saturating_add(1);
            } else {
                black_pixels = black_pixels.saturating_add(1);
            }
            if red > 128 && red > green.saturating_add(32) && red > blue.saturating_add(32) {
                warm_pixels = warm_pixels.saturating_add(1);
            }
            if red > green.saturating_add(32) && red > blue.saturating_add(32) {
                red_dominant_pixels = red_dominant_pixels.saturating_add(1);
            }
            if previous.is_some_and(|previous| previous != color) {
                horizontal_changes = horizontal_changes.saturating_add(1);
            }
            previous = Some(color);

            if x >= warning_region_x_start
                && x < warning_region_x_end
                && y >= warning_region_y_start
                && y < warning_region_y_end
                && luma >= 142
                && max_channel.saturating_sub(min_channel) <= 112
            {
                row_warning_text_pixels = row_warning_text_pixels.saturating_add(1);
            }
            if y >= bottom_band_start {
                if luma < 40 {
                    bottom_dark_pixels = bottom_dark_pixels.saturating_add(1);
                } else if luma > 156 && max_channel.saturating_sub(min_channel) <= 96 {
                    bottom_caption_pixels = bottom_caption_pixels.saturating_add(1);
                }
            }
            if unique_colors.len() < 257 && !unique_colors.contains(&color) {
                unique_colors.push(color);
            }
            let bucket = (((red >> 4) as usize) << 8)
                | (((green >> 4) as usize) << 4)
                | ((blue >> 4) as usize);
            color_buckets[bucket] = color_buckets[bucket].saturating_add(1);
        }

        if row_nonzero_pixels >= row_content_cutoff {
            first_occupied_row.get_or_insert(y);
            last_occupied_row = Some(y);
        }
        if y >= warning_region_y_start
            && y < warning_region_y_end
            && row_warning_text_pixels >= warning_row_cutoff
        {
            top_left_warning_text_rows = top_left_warning_text_rows.saturating_add(1);
        }
    }

    let total_pixels = frame.width.saturating_mul(frame.height);
    let bottom_band_pixels = frame.width.saturating_mul((frame.height / 5).max(1));
    let occupied_row_span = first_occupied_row
        .zip(last_occupied_row)
        .map_or(0usize, |(first, last)| {
            last.saturating_sub(first).saturating_add(1)
        });
    let dominant_color_bucket_pixels = color_buckets.into_iter().max().unwrap_or(0);
    let has_visible_content =
        nonzero_pixels.saturating_mul(100) >= total_pixels && unique_colors.len() >= 2;
    let has_scene_detail = has_visible_content
        && unique_colors.len() >= 64
        && horizontal_changes.saturating_mul(30) >= total_pixels;
    let has_bottom_caption_band = bottom_caption_pixels.saturating_mul(1_000)
        >= total_pixels.saturating_mul(3)
        && bottom_dark_pixels.saturating_mul(100) >= bottom_band_pixels.saturating_mul(5);
    let has_intro_caption_band = has_bottom_caption_band && {
        let dark_subtitle_band = bottom_dark_pixels.saturating_mul(100)
            >= bottom_band_pixels.saturating_mul(30)
            && bottom_caption_pixels.saturating_mul(100) <= bottom_band_pixels.saturating_mul(35);
        let bright_caption_panel = bottom_caption_pixels.saturating_mul(100)
            >= bottom_band_pixels.saturating_mul(70)
            && bottom_dark_pixels.saturating_mul(100) >= bottom_band_pixels.saturating_mul(4);
        dark_subtitle_band || bright_caption_panel
    };

    bottom_band_pixels > 0
        && has_scene_detail
        && has_bottom_caption_band
        && has_intro_caption_band
        && unique_colors.len() >= 192
        && unique_colors.len() <= 320
        && nonzero_pixels.saturating_mul(100) >= total_pixels.saturating_mul(55)
        && nonzero_pixels.saturating_mul(100) <= total_pixels.saturating_mul(66)
        && black_pixels.saturating_mul(100) >= total_pixels.saturating_mul(34)
        && black_pixels.saturating_mul(100) <= total_pixels.saturating_mul(45)
        && red_dominant_pixels.saturating_mul(100) >= total_pixels.saturating_mul(12)
        && red_dominant_pixels.saturating_mul(100) <= total_pixels.saturating_mul(22)
        && warm_pixels.saturating_mul(100) <= total_pixels.saturating_mul(8)
        && dominant_color_bucket_pixels.saturating_mul(100) >= total_pixels.saturating_mul(40)
        && dominant_color_bucket_pixels.saturating_mul(100) <= total_pixels.saturating_mul(50)
        && horizontal_changes.saturating_mul(100) >= total_pixels.saturating_mul(32)
        && horizontal_changes.saturating_mul(100) <= total_pixels.saturating_mul(46)
        && occupied_row_span.saturating_mul(100) >= frame.height.saturating_mul(95)
        && bottom_dark_pixels.saturating_mul(100) >= bottom_band_pixels.saturating_mul(80)
        && bottom_dark_pixels.saturating_mul(100) <= bottom_band_pixels.saturating_mul(95)
        && bottom_caption_pixels.saturating_mul(1_000) >= total_pixels.saturating_mul(4)
        && bottom_caption_pixels.saturating_mul(1_000) <= total_pixels.saturating_mul(10)
        && top_left_warning_text_rows <= (frame.height / 32).max(8)
}

fn native_display_frame_has_texture_page_fragment_artifact(frame: &NativeDisplayFrame) -> bool {
    if frame.width < 320
        || frame.height < 240
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
    {
        return false;
    }

    let bottom_start = frame.height.saturating_mul(4) / 5;
    let row_content_cutoff = (frame.width / 20).max(1);
    let mut unique_colors = Vec::new();
    let mut color_buckets = [0usize; 4096];
    let mut nonzero = 0usize;
    let mut black = 0usize;
    let mut bottom_dark = 0usize;
    let mut horizontal_changes = 0usize;
    let mut occupied_rows = 0usize;
    let mut first_occupied_row = None;
    let mut last_occupied_row = None;

    for y in 0..frame.height {
        let row = y.saturating_mul(frame.width);
        let mut row_nonzero = 0usize;
        let mut previous = None;
        for x in 0..frame.width {
            let color = frame.pixels[row + x] & 0x00ff_ffff;
            let red = (color >> 16) & 0xff;
            let green = (color >> 8) & 0xff;
            let blue = color & 0xff;
            let luma =
                (red.saturating_mul(30) + green.saturating_mul(59) + blue.saturating_mul(11)) / 100;

            if color != 0 {
                nonzero = nonzero.saturating_add(1);
                row_nonzero = row_nonzero.saturating_add(1);
            } else {
                black = black.saturating_add(1);
            }
            if y >= bottom_start && luma < 40 {
                bottom_dark = bottom_dark.saturating_add(1);
            }
            if previous.is_some_and(|previous| previous != color) {
                horizontal_changes = horizontal_changes.saturating_add(1);
            }
            previous = Some(color);
            if unique_colors.len() < 161 && !unique_colors.contains(&color) {
                unique_colors.push(color);
            }
            let bucket = (((red >> 4) as usize) << 8)
                | (((green >> 4) as usize) << 4)
                | ((blue >> 4) as usize);
            color_buckets[bucket] = color_buckets[bucket].saturating_add(1);
        }
        if row_nonzero >= row_content_cutoff {
            occupied_rows = occupied_rows.saturating_add(1);
            first_occupied_row.get_or_insert(y);
            last_occupied_row = Some(y);
        }
    }

    let total = frame.width.saturating_mul(frame.height);
    let bottom_pixels = frame.width.saturating_mul((frame.height / 5).max(1));
    let occupied_row_span = first_occupied_row
        .zip(last_occupied_row)
        .map_or(0, |(first, last)| {
            last.saturating_sub(first).saturating_add(1)
        });
    let dominant = color_buckets.into_iter().max().unwrap_or(0);
    let scene_detail =
        total > 0 && unique_colors.len() >= 64 && horizontal_changes.saturating_mul(30) >= total;

    scene_detail
        && bottom_pixels > 0
        && unique_colors.len() <= 160
        && bottom_dark.saturating_mul(100) >= bottom_pixels.saturating_mul(92)
        && black.saturating_mul(100) >= total.saturating_mul(35)
        && dominant.saturating_mul(100) >= total.saturating_mul(32)
        && occupied_rows.saturating_mul(100) >= frame.height.saturating_mul(45)
        && occupied_row_span.saturating_mul(100) <= frame.height.saturating_mul(85)
        && nonzero.saturating_mul(100) >= total.saturating_mul(35)
        && nonzero.saturating_mul(100) <= total.saturating_mul(70)
        && horizontal_changes.saturating_mul(100) <= total.saturating_mul(30)
}

fn native_display_frame_has_context_stage_letterbox(frame: &NativeDisplayFrame) -> bool {
    if frame.width < 320
        || frame.height < 240
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
    {
        return false;
    }

    let bottom_start = frame.height.saturating_mul(4) / 5;
    let row_content_cutoff = (frame.width / 20).max(1);
    let mut nonzero = 0usize;
    let mut black = 0usize;
    let mut bottom_dark = 0usize;
    let mut occupied_rows = 0usize;
    let mut first_occupied_row = None;
    let mut last_occupied_row = None;
    let mut color_buckets = [0usize; 4096];

    for y in 0..frame.height {
        let row = y.saturating_mul(frame.width);
        let mut row_nonzero = 0usize;
        for x in 0..frame.width {
            let color = frame.pixels[row + x] & 0x00ff_ffff;
            let red = (color >> 16) & 0xff;
            let green = (color >> 8) & 0xff;
            let blue = color & 0xff;
            let luma =
                (red.saturating_mul(30) + green.saturating_mul(59) + blue.saturating_mul(11)) / 100;
            if color != 0 {
                nonzero = nonzero.saturating_add(1);
                row_nonzero = row_nonzero.saturating_add(1);
            } else {
                black = black.saturating_add(1);
            }
            if y >= bottom_start && luma < 40 {
                bottom_dark = bottom_dark.saturating_add(1);
            }
            let bucket = (((red >> 4) as usize) << 8)
                | (((green >> 4) as usize) << 4)
                | ((blue >> 4) as usize);
            color_buckets[bucket] = color_buckets[bucket].saturating_add(1);
        }
        if row_nonzero >= row_content_cutoff {
            occupied_rows = occupied_rows.saturating_add(1);
            first_occupied_row.get_or_insert(y);
            last_occupied_row = Some(y);
        }
    }

    let total = frame.width.saturating_mul(frame.height);
    let bottom_pixels = frame.width.saturating_mul((frame.height / 5).max(1));
    let occupied_row_span = first_occupied_row
        .zip(last_occupied_row)
        .map_or(0, |(first, last)| {
            last.saturating_sub(first).saturating_add(1)
        });
    let dominant = color_buckets.into_iter().max().unwrap_or(0);

    total > 0
        && bottom_pixels > 0
        && nonzero.saturating_mul(100) >= total.saturating_mul(35)
        && black.saturating_mul(100) <= total.saturating_mul(55)
        && bottom_dark.saturating_mul(100) >= bottom_pixels.saturating_mul(70)
        && occupied_rows.saturating_mul(100) >= frame.height.saturating_mul(55)
        && occupied_row_span.saturating_mul(100) >= frame.height.saturating_mul(60)
        && dominant.saturating_mul(100) <= total.saturating_mul(55)
}

fn native_display_frame_has_context_live_stage_hud(frame: &NativeDisplayFrame) -> bool {
    native_display_frame_has_context_stage_letterbox(frame)
        && native_display_frame_has_title_logo_overlay(frame)
}

fn native_display_frame_has_context_full_live_scene(frame: &NativeDisplayFrame) -> bool {
    if frame.width < 512
        || frame.height < 480
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
    {
        return false;
    }

    let row_content_cutoff = (frame.width / 20).max(1);
    let mut unique_colors = Vec::new();
    let mut color_buckets = [0usize; 4096];
    let mut nonzero = 0usize;
    let mut black = 0usize;
    let mut horizontal_changes = 0usize;
    let mut occupied_rows = 0usize;
    let mut first_occupied_row = None;
    let mut last_occupied_row = None;

    for y in 0..frame.height {
        let row = y.saturating_mul(frame.width);
        let mut row_nonzero = 0usize;
        let mut previous = None;
        for x in 0..frame.width {
            let color = frame.pixels[row + x] & 0x00ff_ffff;
            let red = (color >> 16) & 0xff;
            let green = (color >> 8) & 0xff;
            let blue = color & 0xff;
            if color != 0 {
                nonzero = nonzero.saturating_add(1);
                row_nonzero = row_nonzero.saturating_add(1);
            } else {
                black = black.saturating_add(1);
            }
            if previous.is_some_and(|previous| previous != color) {
                horizontal_changes = horizontal_changes.saturating_add(1);
            }
            previous = Some(color);
            if unique_colors.len() < 65 && !unique_colors.contains(&color) {
                unique_colors.push(color);
            }
            let bucket = (((red >> 4) as usize) << 8)
                | (((green >> 4) as usize) << 4)
                | ((blue >> 4) as usize);
            color_buckets[bucket] = color_buckets[bucket].saturating_add(1);
        }
        if row_nonzero >= row_content_cutoff {
            occupied_rows = occupied_rows.saturating_add(1);
            first_occupied_row.get_or_insert(y);
            last_occupied_row = Some(y);
        }
    }

    let total = frame.width.saturating_mul(frame.height);
    let occupied_row_span = first_occupied_row
        .zip(last_occupied_row)
        .map_or(0, |(first, last)| {
            last.saturating_sub(first).saturating_add(1)
        });
    let dominant = color_buckets.into_iter().max().unwrap_or(0);

    total > 0
        && unique_colors.len() >= 24
        && nonzero.saturating_mul(100) >= total.saturating_mul(35)
        && black.saturating_mul(100) <= total.saturating_mul(45)
        && horizontal_changes.saturating_mul(30) >= total
        && occupied_rows.saturating_mul(100) >= frame.height.saturating_mul(55)
        && occupied_row_span.saturating_mul(100) >= frame.height.saturating_mul(60)
        && dominant.saturating_mul(100) <= total.saturating_mul(60)
        && native_display_frame_has_title_logo_overlay(frame)
        && !native_display_frame_has_texture_page_fragment_artifact(frame)
        && !native_display_frame_has_sparse_title_logo_artifact(frame)
        && !native_display_frame_has_periodic_horizontal_ghosting(frame)
}

fn native_display_frame_or_window_supports_playable_candidate_with_render_context(
    frame: &NativeDisplayFrame,
    render_context_allows_periodic: bool,
) -> bool {
    if native_display_frame_supports_playable_candidate_with_render_context(
        frame,
        render_context_allows_periodic,
    ) {
        return true;
    }

    let window = native_window_frame_from_display(frame);
    native_display_frame_supports_playable_candidate_with_render_context(
        &window,
        render_context_allows_periodic,
    )
}

fn native_display_frame_has_title_logo_overlay(frame: &NativeDisplayFrame) -> bool {
    if frame.width < 320
        || frame.height < 240
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
    {
        return false;
    }

    let x_start = frame.width.saturating_mul(52) / 100;
    let x_end = frame.width.saturating_mul(99) / 100;
    let y_start = frame.height.saturating_mul(48) / 100;
    let y_end = frame.height.saturating_mul(84) / 100;
    let mut red_logo_pixels = 0usize;
    let mut bright_logo_pixels = 0usize;
    let mut black_pixels = 0usize;
    let mut horizontal_changes = 0usize;

    for y in 0..frame.height {
        let row = y.saturating_mul(frame.width);
        let mut previous = None;
        for x in 0..frame.width {
            let color = frame.pixels[row + x] & 0x00ff_ffff;
            let red = (color >> 16) & 0xff;
            let green = (color >> 8) & 0xff;
            let blue = color & 0xff;
            let max_channel = red.max(green).max(blue);
            let min_channel = red.min(green).min(blue);
            let luma =
                (red.saturating_mul(30) + green.saturating_mul(59) + blue.saturating_mul(11)) / 100;

            if color == 0 || luma < 24 {
                black_pixels = black_pixels.saturating_add(1);
            }
            if previous.is_some_and(|previous| previous != color) {
                horizontal_changes = horizontal_changes.saturating_add(1);
            }
            previous = Some(color);

            if x < x_start || x >= x_end || y < y_start || y >= y_end {
                continue;
            }
            if red >= 128 && red > green.saturating_add(48) && red > blue.saturating_add(48) {
                red_logo_pixels = red_logo_pixels.saturating_add(1);
            }
            if luma >= 150 && max_channel.saturating_sub(min_channel) <= 104 {
                bright_logo_pixels = bright_logo_pixels.saturating_add(1);
            }
        }
    }

    let total = frame.width.saturating_mul(frame.height);
    let title_mark_pixels = red_logo_pixels.saturating_add(bright_logo_pixels);
    red_logo_pixels.saturating_mul(1_000) >= total.saturating_mul(8)
        && bright_logo_pixels.saturating_mul(1_000) >= total.saturating_mul(2)
        && title_mark_pixels.saturating_mul(100) >= total
        && black_pixels.saturating_mul(100) >= total.saturating_mul(10)
        && horizontal_changes.saturating_mul(100) >= total.saturating_mul(20)
}

fn native_display_frame_has_sparse_title_logo_artifact(frame: &NativeDisplayFrame) -> bool {
    if frame.width < 256
        || frame.height < 120
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
    {
        return false;
    }

    let x_start = frame.width.saturating_mul(50) / 100;
    let y_start = frame.height.saturating_mul(38) / 100;
    let mut total_nonzero = 0usize;
    let mut total_dark = 0usize;
    let mut region_samples = 0usize;
    let mut region_nonzero = 0usize;
    let mut region_red = 0usize;
    let mut region_bright = 0usize;
    let mut occupied_rows = 0usize;
    let row_content_cutoff = (frame.width / 32).max(1);

    for y in 0..frame.height {
        let row = y.saturating_mul(frame.width);
        let mut row_nonzero = 0usize;
        for x in 0..frame.width {
            let color = frame.pixels[row + x] & 0x00ff_ffff;
            let red = (color >> 16) & 0xff;
            let green = (color >> 8) & 0xff;
            let blue = color & 0xff;
            let max_channel = red.max(green).max(blue);
            let min_channel = red.min(green).min(blue);
            let luma =
                (red.saturating_mul(30) + green.saturating_mul(59) + blue.saturating_mul(11)) / 100;

            if color != 0 {
                total_nonzero = total_nonzero.saturating_add(1);
                row_nonzero = row_nonzero.saturating_add(1);
            }
            if color == 0 || max_channel <= 32 {
                total_dark = total_dark.saturating_add(1);
            }

            if x >= x_start && y >= y_start {
                region_samples = region_samples.saturating_add(1);
                if color != 0 {
                    region_nonzero = region_nonzero.saturating_add(1);
                }
                if red >= 128 && red > green.saturating_add(44) && red > blue.saturating_add(44) {
                    region_red = region_red.saturating_add(1);
                }
                if luma >= 148 && max_channel.saturating_sub(min_channel) <= 112 {
                    region_bright = region_bright.saturating_add(1);
                }
            }
        }
        if row_nonzero >= row_content_cutoff {
            occupied_rows = occupied_rows.saturating_add(1);
        }
    }

    let total = frame.width.saturating_mul(frame.height);
    if total == 0 || region_samples == 0 {
        return false;
    }

    let region_marks = region_red.saturating_add(region_bright);
    let sparse_frame = total_nonzero.saturating_mul(100) <= total.saturating_mul(24)
        && total_dark.saturating_mul(100) >= total.saturating_mul(55)
        && occupied_rows.saturating_mul(100) <= frame.height.saturating_mul(72);
    let logo_mark_density = region_nonzero.saturating_mul(100) >= region_samples.saturating_mul(3)
        && region_red.saturating_mul(100) >= region_samples.saturating_mul(1)
        && region_bright.saturating_mul(1_000) >= region_samples.saturating_mul(3)
        && region_marks.saturating_mul(100) >= region_samples.saturating_mul(2);

    sparse_frame && logo_mark_density
}

fn native_display_frame_has_glyph_texture_atlas_artifact(frame: &NativeDisplayFrame) -> bool {
    if frame.width < 256
        || frame.height < 120
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
    {
        return false;
    }

    let mut nonzero = 0usize;
    let mut dark = 0usize;
    let mut bright = 0usize;
    let mut horizontal_changes = 0usize;
    let mut busy_rows = 0usize;
    let mut unique_colors = Vec::new();
    let mut color_buckets = [0usize; 4096];
    let row_change_cutoff = (frame.width / 10).max(1);

    for y in 0..frame.height {
        let row = y.saturating_mul(frame.width);
        let mut row_changes = 0usize;
        let mut previous = None;
        for x in 0..frame.width {
            let color = frame.pixels[row + x] & 0x00ff_ffff;
            let red = (color >> 16) & 0xff;
            let green = (color >> 8) & 0xff;
            let blue = color & 0xff;
            let max_channel = red.max(green).max(blue);
            let luma =
                (red.saturating_mul(30) + green.saturating_mul(59) + blue.saturating_mul(11)) / 100;

            if color != 0 {
                nonzero = nonzero.saturating_add(1);
            }
            if color == 0 || max_channel <= 36 {
                dark = dark.saturating_add(1);
            }
            if luma >= 148 {
                bright = bright.saturating_add(1);
            }
            if previous.is_some_and(|previous| previous != color) {
                horizontal_changes = horizontal_changes.saturating_add(1);
                row_changes = row_changes.saturating_add(1);
            }
            previous = Some(color);

            if unique_colors.len() < 129 && !unique_colors.contains(&color) {
                unique_colors.push(color);
            }
            let bucket = (((red >> 4) as usize) << 8)
                | (((green >> 4) as usize) << 4)
                | ((blue >> 4) as usize);
            color_buckets[bucket] = color_buckets[bucket].saturating_add(1);
        }
        if row_changes >= row_change_cutoff {
            busy_rows = busy_rows.saturating_add(1);
        }
    }

    let total = frame.width.saturating_mul(frame.height).max(1);
    let dominant_bucket = color_buckets.into_iter().max().unwrap_or_default();
    let atlas_density = nonzero.saturating_mul(100) >= total.saturating_mul(18)
        && nonzero.saturating_mul(100) <= total.saturating_mul(96);
    let low_palette = unique_colors.len() <= 128;
    let glyph_contrast = bright.saturating_mul(100) >= total.saturating_mul(4)
        && dark.saturating_mul(100) >= total.saturating_mul(8);
    let grid_edges = horizontal_changes.saturating_mul(100) >= total.saturating_mul(8)
        && busy_rows.saturating_mul(100) >= frame.height.saturating_mul(20);
    let atlas_bucket_bias = dominant_bucket.saturating_mul(100) >= total.saturating_mul(12);

    atlas_density && low_palette && glyph_contrast && grid_edges && atlas_bucket_bias
}

fn native_display_frame_has_saturated_logo_transition(frame: &NativeDisplayFrame) -> bool {
    if frame.width == 0
        || frame.height == 0
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
    {
        return false;
    }

    if std::env::var_os("BR2_NATIVE_ALLOW_SATURATED_PLAYABLE_CANDIDATE").is_some() {
        return false;
    }

    let mut red_dominant = 0usize;
    let mut magenta_dominant = 0usize;
    let mut high_saturation = 0usize;
    for color in frame.pixels.iter().take(frame.width * frame.height) {
        let red = (color >> 16) & 0xff;
        let green = (color >> 8) & 0xff;
        let blue = color & 0xff;
        let max_channel = red.max(green).max(blue);
        let min_channel = red.min(green).min(blue);
        if max_channel >= 160 && max_channel.saturating_sub(min_channel) >= 96 {
            high_saturation = high_saturation.saturating_add(1);
        }
        if red >= 144 && green <= 96 && blue <= 96 {
            red_dominant = red_dominant.saturating_add(1);
        }
        if red >= 144 && blue >= 128 && green <= 112 {
            magenta_dominant = magenta_dominant.saturating_add(1);
        }
    }

    let total = frame.width.saturating_mul(frame.height);
    magenta_dominant.saturating_mul(100) >= total.saturating_mul(18)
        || red_dominant.saturating_mul(100) >= total.saturating_mul(18)
        || high_saturation.saturating_mul(100) >= total.saturating_mul(45)
}

fn native_display_frame_has_periodic_horizontal_ghosting(frame: &NativeDisplayFrame) -> bool {
    if frame.width < 128
        || frame.height == 0
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
    {
        return false;
    }

    let mut horizontal_changes = 0usize;
    for y in (0..frame.height).step_by(2) {
        let row = y.saturating_mul(frame.width);
        let mut previous = None;
        for x in (0..frame.width).step_by(2) {
            let color = frame.pixels[row + x] & 0x00ff_ffff;
            if previous.is_some_and(|previous| previous != color) {
                horizontal_changes = horizontal_changes.saturating_add(1);
            }
            previous = Some(color);
        }
    }

    if horizontal_changes.saturating_mul(100) < frame.width.saturating_mul(frame.height) / 2 {
        return false;
    }

    [64usize, 96, 128, 160, 192]
        .into_iter()
        .filter(|period| *period < frame.width)
        .any(|period| {
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

            samples > 0 && matches.saturating_mul(1000) / samples >= 350
        })
}

#[cfg(test)]
mod tests {
    use super::{
        Bus, Cpu, NativeDisplayFrame, NativeEmulator, StepOutcome, apply_at28c16_mode_override,
        native_at28c16_mode_is_blank, native_display_frame_has_context_full_live_scene,
        native_display_frame_has_context_stage_letterbox,
        native_display_frame_has_periodic_horizontal_ghosting,
        native_display_frame_has_saturated_logo_transition,
        native_display_frame_has_texture_page_fragment_artifact,
        native_display_frame_has_title_logo_overlay, native_display_frame_has_visible_detail,
        native_display_frame_supports_playable_candidate_with_render_context,
    };
    use crate::native::bus::NativeBoardAssets;
    use crate::native::io::GPU_GP0;
    use crate::native::romset::{NativeRomAssetMismatch, NativeRomCompatibilityReport};

    fn program(instructions: &[u32]) -> Vec<u8> {
        instructions
            .iter()
            .flat_map(|instruction| instruction.to_le_bytes())
            .collect()
    }

    fn i_type(opcode: u32, rs: u32, rt: u32, imm: i16) -> u32 {
        (opcode << 26) | (rs << 21) | (rt << 16) | (imm as u16 as u32)
    }

    fn r_type(rs: u32, rt: u32, rd: u32, shamt: u32, function: u32) -> u32 {
        (rs << 21) | (rt << 16) | (rd << 11) | (shamt << 6) | function
    }

    fn gp0_fill_rect(emulator: &mut NativeEmulator, color: u32, x: u32, y: u32, w: u32, h: u32) {
        emulator.bus.io.write_u32(GPU_GP0, 0x0200_0000 | color);
        emulator.bus.io.write_u32(GPU_GP0, (y << 16) | x);
        emulator.bus.io.write_u32(GPU_GP0, (h << 16) | w);
    }

    fn test_emulator() -> NativeEmulator {
        NativeEmulator {
            cpu: Cpu::default(),
            bus: Bus::new(Vec::new(), 2 * 1024 * 1024),
            rom_compatibility: NativeRomCompatibilityReport::missing_all_required_assets(),
            last_outcome: StepOutcome::Continue,
            executed_steps: 0,
            last_step: None,
        }
    }

    fn test_emulator_with_rom_compatibility(
        rom_compatibility: NativeRomCompatibilityReport,
    ) -> NativeEmulator {
        NativeEmulator {
            cpu: Cpu::default(),
            bus: Bus::new(Vec::new(), 2 * 1024 * 1024),
            rom_compatibility,
            last_outcome: StepOutcome::Continue,
            executed_steps: 0,
            last_step: None,
        }
    }

    #[test]
    fn at28c16_runtime_mode_parser_accepts_blank_aliases_only() {
        assert!(native_at28c16_mode_is_blank("blank"));
        assert!(native_at28c16_mode_is_blank(" default "));
        assert!(native_at28c16_mode_is_blank("FF"));
        assert!(native_at28c16_mode_is_blank("ignore"));
        assert!(!native_at28c16_mode_is_blank("asset"));
        assert!(!native_at28c16_mode_is_blank("cfg"));
    }

    #[test]
    fn auto_coin_input_mapping_ignores_known_bad_program_dump_without_zinc_cfg() {
        let mut report = NativeRomCompatibilityReport::missing_all_required_assets();
        report.mismatched_assets.push(NativeRomAssetMismatch {
            name: "flash1.024".to_string(),
            role: "program_flash",
            expected_size: 2 * 1024 * 1024,
            actual_size: 2 * 1024 * 1024,
            expected_crc32: Some(0x0346_5a69),
            actual_crc32: 0x4866_dce3,
        });
        let emulator = test_emulator_with_rom_compatibility(report);

        assert_eq!(emulator.auto_coin_input_mapping_name(), None);
    }

    #[test]
    fn auto_coin_input_mapping_keeps_mame_jamma_wiring_for_zinc_cfg() {
        let mut report = NativeRomCompatibilityReport::missing_all_required_assets();
        report.mismatched_assets.push(NativeRomAssetMismatch {
            name: "at28c16_world".to_string(),
            role: "settings_eeprom",
            expected_size: 128,
            actual_size: 2048,
            expected_crc32: Some(0x01b4_2397),
            actual_crc32: 0x4c5f_ea47,
        });
        let emulator = test_emulator_with_rom_compatibility(report);

        assert_eq!(emulator.auto_coin_input_mapping_name(), Some("mame"));
    }

    #[test]
    fn auto_coin_input_mapping_leaves_unknown_roms_unmodified() {
        let emulator = test_emulator();

        assert_eq!(emulator.auto_coin_input_mapping_name(), None);
    }

    #[test]
    fn blank_at28c16_runtime_mode_disables_legacy_zinc_input_compat() {
        let assets = NativeBoardAssets {
            at28c16: Some(vec![0x00; 2048]),
            at28c16_blank_default: false,
            legacy_zinc_input_compat: true,
            ..NativeBoardAssets::default()
        };

        let overridden = apply_at28c16_mode_override(assets.clone(), "blank");

        assert_eq!(overridden.at28c16, Some(vec![0xff; 2048]));
        assert!(overridden.at28c16_blank_default);
        assert!(!overridden.legacy_zinc_input_compat);
        assert_eq!(
            apply_at28c16_mode_override(assets, "asset").at28c16,
            Some(vec![0x00; 2048])
        );
    }

    #[test]
    fn lean_probe_keeps_input_and_playability_without_gpu_trace_payloads() {
        let emulator = test_emulator();
        let json = emulator.lean_probe_json();

        assert!(json.contains("\"input\""));
        assert!(json.contains("\"native_playability\""));
        assert!(json.contains("\"zn_input_read_summary\""));
        assert!(!json.contains("gpu_recent_draw_commands"));
        assert!(!json.contains("gpu_recent_transfer_commands"));
        assert!(!json.contains("gpu_top_draw_commands"));
        assert!(!json.contains("recent_security_transfers"));
    }

    #[test]
    fn batch_step_reports_actual_executed_steps_and_last_boundary() {
        let rom = program(&[
            i_type(0x09, 0, 2, 42),   // addiu v0, zero, 42
            r_type(0, 0, 0, 0, 0x0d), // break
            i_type(0x09, 0, 2, 99),   // must not execute after halt
        ]);
        let mut emulator = NativeEmulator {
            cpu: Cpu::default(),
            bus: Bus::new(rom, 2 * 1024 * 1024),
            rom_compatibility: NativeRomCompatibilityReport::missing_all_required_assets(),
            last_outcome: StepOutcome::Continue,
            executed_steps: 0,
            last_step: None,
        };

        let executed = emulator.step_instructions(10);
        let json = emulator.json();

        assert_eq!(executed, 2);
        assert_eq!(emulator.cpu.regs[2], 42);
        assert_eq!(emulator.cpu.cycles, 2);
        assert_eq!(emulator.last_outcome, StepOutcome::Halted);
        assert!(json.contains("\"platform\":{\"execution_path\":"));
        assert!(json.contains("\"generic_equivalent\":true"));
        assert!(json.contains("\"executed_steps\":2"));
        assert!(json.contains("\"last_step\":{\"start_pc\":532676612"));
        assert!(json.contains("\"cycles_elapsed\":1"));
        assert!(json.contains("\"last_outcome\":\"Halted\""));
    }

    #[test]
    fn untracked_step_until_vblank_preserves_last_step_boundary() {
        let rom = program(&[
            i_type(0x09, 0, 2, 42),   // addiu v0, zero, 42
            i_type(0x09, 0, 3, 7),    // addiu v1, zero, 7
            r_type(0, 0, 0, 0, 0x0d), // break
        ]);
        let mut emulator = NativeEmulator {
            cpu: Cpu::default(),
            bus: Bus::new(rom, 2 * 1024 * 1024),
            rom_compatibility: NativeRomCompatibilityReport::missing_all_required_assets(),
            last_outcome: StepOutcome::Continue,
            executed_steps: 0,
            last_step: None,
        };

        let first_report = emulator.step_instruction();
        let tracked_boundary = emulator.last_step;
        let executed = emulator.step_instructions_until_vblank_untracked(10);

        assert_eq!(first_report.outcome, StepOutcome::Continue);
        assert_eq!(executed, 2);
        assert_eq!(emulator.cpu.regs[2], 42);
        assert_eq!(emulator.cpu.regs[3], 7);
        assert_eq!(emulator.cpu.cycles, 3);
        assert_eq!(emulator.last_outcome, StepOutcome::Halted);
        assert_eq!(emulator.last_step, tracked_boundary);
    }

    #[test]
    fn display_frame_uses_stable_gui_display_instead_of_sparse_raw_actual() {
        let mut emulator = test_emulator();
        let (width, height) = emulator.bus.io.gpu.display_dimensions();

        for x in (0..width as u32).step_by(4) {
            let color = if x % 8 == 0 { 0x00ff_ffff } else { 0x0000_40ff };
            gp0_fill_rect(&mut emulator, color, x, 0, 4, height as u32);
        }
        emulator.bus.io.gpu.capture_vblank_presented_frame();
        emulator
            .bus
            .io
            .gpu
            .mark_live_textured_playfield_for_test(width, height);

        gp0_fill_rect(
            &mut emulator,
            0x0000_0000,
            0,
            0,
            width as u32,
            height as u32,
        );
        gp0_fill_rect(
            &mut emulator,
            0x00ff_ffff,
            (width - 64) as u32,
            (height - 20) as u32,
            56,
            12,
        );

        let raw = emulator.bus.io.gpu.raw_actual_display_rgb_frame();
        let resolved = emulator.bus.io.gpu.actual_display_rgb_frame();
        let stable = emulator.bus.io.gpu.display_rgb_frame();
        let frame = emulator.display_frame();

        assert_ne!(raw.2, resolved.2);
        assert_eq!(frame.width, stable.0);
        assert_eq!(frame.height, stable.1);
        assert_eq!(frame.pixels, stable.2);
    }

    #[test]
    fn native_window_keeps_sparse_title_logo_letterboxed_not_vertically_expanded() {
        let width = 512usize;
        let height = 240usize;
        let mut pixels = vec![0; width * height];

        for y in 148..190 {
            for x in 328..430 {
                if (x / 5) % 2 == 0 || (y / 4) % 2 == 0 {
                    pixels[y * width + x] = 0x00d8_1010;
                }
            }
        }
        for y in 192..202 {
            for x in 332..470 {
                if (x + y) % 3 != 0 {
                    pixels[y * width + x] = 0x00f8_f8f0;
                }
            }
        }
        for y in 129..208 {
            for x in 455..497 {
                let shade = 176 + ((x + y) % 72) as u32;
                pixels[y * width + x] = (shade << 16) | (shade << 8) | shade;
            }
        }

        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        assert!(super::native_display_frame_has_sparse_title_logo_artifact(
            &frame
        ));
        let window = super::native_window_frame_from_display(&frame);
        let (_, first_y, _, last_y) = nonzero_bounds(&window).expect("logo should remain visible");
        let occupied_height = last_y.saturating_sub(first_y).saturating_add(1);

        assert_eq!(window.width, 512);
        assert_eq!(window.height, 480);
        assert!(
            occupied_height <= 120,
            "sparse title logo should stay letterboxed instead of being expanded to {occupied_height}px"
        );
        assert!(
            first_y >= 240,
            "sparse title logo should remain in the lower display area after aspect-preserving scale"
        );
    }

    #[test]
    fn native_window_keeps_single_field_glyph_atlas_from_vertical_expansion() {
        let width = 512usize;
        let height = 480usize;
        let field_height = height / 2;
        let mut pixels = vec![0; width * height];
        const PALETTE: [u32; 8] = [
            0x0018_1010,
            0x0050_4008,
            0x00d8_1010,
            0x00f0_f0d8,
            0x0008_2038,
            0x00e0_d000,
            0x0070_7040,
            0x00ff_ffff,
        ];

        for local_y in (0..field_height).step_by(12) {
            for cell_x in (0..width).step_by(16) {
                let color = PALETTE[(cell_x / 16 + local_y / 12) % PALETTE.len()];
                let y = field_height + local_y;
                for x in cell_x..(cell_x + 10).min(width) {
                    pixels[y * width + x] = color;
                    if local_y + 4 < field_height {
                        pixels[(y + 4) * width + x] = color;
                    }
                }
                for glyph_y in local_y..(local_y + 9).min(field_height) {
                    pixels[(field_height + glyph_y) * width + cell_x] = color;
                    if cell_x + 5 < width && (cell_x / 16 + local_y / 12) % 2 == 0 {
                        pixels[(field_height + glyph_y) * width + cell_x + 5] = 0x00ff_ffff;
                    }
                }
            }
        }
        for y in 176..224 {
            for x in 332..500 {
                let local_x = x - 332;
                let local_y = y - 176;
                pixels[(field_height + y) * width + x] =
                    if local_x < 112 && (local_x / 4 + local_y / 3) % 2 == 0 {
                        0x00d8_1010
                    } else if local_x >= 120 && local_y % 5 != 0 {
                        0x00f0_f0d8
                    } else {
                        0
                    };
            }
        }

        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };
        let active = super::native_window_field_frame(&frame, field_height, field_height);

        assert!(super::native_window_should_deinterlace_frame(&frame));
        assert!(super::native_display_frame_has_glyph_texture_atlas_artifact(&active));
        assert!(
            super::native_window_deinterlace_field_is_texture_atlas_artifact(&frame, field_height)
        );
        let window = super::native_window_frame_from_display(&frame);
        let (_, first_y, _, last_y) =
            nonzero_bounds(&window).expect("active atlas field should remain visible");
        let occupied_height = last_y.saturating_sub(first_y).saturating_add(1);

        assert_eq!(window.width, 512);
        assert_eq!(window.height, 480);
        assert!(
            first_y >= 220,
            "single-field atlas should stay in the lower field instead of starting at y={first_y}"
        );
        assert!(
            occupied_height <= 260,
            "single-field atlas should not be expanded to full height: {occupied_height}px"
        );
    }

    #[test]
    fn native_window_deinterlaces_dense_bottom_live_scene_despite_title_like_hud() {
        let width = 512usize;
        let height = 480usize;
        let field_height = height / 2;
        let mut pixels = vec![0; width * height];

        for local_y in 0..216 {
            for x in 12..width {
                let red = 12 + ((x * 5 + local_y * 3) % 92) as u32;
                let green = 24 + ((x * 7 + local_y * 11) % 184) as u32;
                let blue = 40 + ((x * 13 + local_y * 17) % 176) as u32;
                pixels[(field_height + local_y) * width + x] = (red << 16) | (green << 8) | blue;
            }
        }
        for local_y in 70..126 {
            for x in 0..178 {
                if (x + local_y) % 5 != 0 {
                    pixels[(field_height + local_y) * width + x] = 0x00f8_f8f8;
                }
            }
        }
        for local_y in 150..205 {
            for x in 330..506 {
                let local_x = x - 330;
                let color = if local_x < 88 {
                    0x00d0_1818
                } else if (local_x + local_y) % 3 == 0 {
                    0x00f0_f0e0
                } else {
                    0x0018_2040
                };
                pixels[(field_height + local_y) * width + x] = color;
            }
        }

        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };
        let active = super::native_window_field_frame(&frame, field_height, field_height);

        assert!(super::native_window_should_deinterlace_frame(&frame));
        assert!(super::native_window_field_has_live_scene_density(&active));
        assert!(
            !super::native_window_deinterlace_field_is_texture_atlas_artifact(&frame, field_height)
        );

        let window = super::native_window_frame_from_display(&frame);
        let (_, first_y, _, last_y) =
            nonzero_bounds(&window).expect("live scene should remain visible");
        let occupied_height = last_y.saturating_sub(first_y).saturating_add(1);

        assert_eq!((window.width, window.height), (512, 480));
        assert!(
            first_y <= 16,
            "deinterlaced live scene should start near the top instead of y={first_y}"
        );
        assert!(
            occupied_height >= 360,
            "deinterlaced live scene should use most of the output height, got {occupied_height}px"
        );
        assert_ne!(window.pixels[256 + 40 * 512], 0);
        assert_ne!(window.pixels[256 + 420 * 512], 0);
    }

    #[test]
    fn native_window_deinterlaces_stronger_field_when_both_stacked_fields_are_visible() {
        let width = 512usize;
        let height = 480usize;
        let field_height = height / 2;
        let mut pixels = vec![0; width * height];

        // Model a stale, partially populated top field.
        for y in 54..126 {
            for x in 112..368 {
                pixels[y * width + x] = 0x00f0_1010;
            }
        }

        // Model the current field with substantially denser scene detail.
        for local_y in 8..232 {
            for x in 18..246 {
                let red = 24 + ((x * 3 + local_y * 5) % 96) as u32;
                let green = 48 + ((x * 7 + local_y * 11) % 176) as u32;
                let blue = 32 + ((x * 13 + local_y * 17) % 192) as u32;
                pixels[(field_height + local_y) * width + x] = (red << 16) | (green << 8) | blue;
            }
        }

        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        assert!(super::native_window_should_deinterlace_frame(&frame));
        assert!(super::native_window_prefers_stronger_stacked_field(&frame));
        assert_eq!(
            super::native_window_deinterlace_field_offset(&frame),
            field_height
        );
        assert!(
            !super::native_window_deinterlace_field_is_texture_atlas_artifact(&frame, field_height)
        );

        let window = super::native_window_frame_from_display(&frame);
        let (_, first_y, _, last_y) =
            nonzero_bounds(&window).expect("stronger current field should remain visible");
        assert_eq!((window.width, window.height), (512, 480));
        assert!(first_y <= 20);
        assert!(last_y >= 455);
        assert!(
            window
                .pixels
                .iter()
                .all(|pixel| *pixel & 0x00ff_ffff != 0x00f0_1010),
            "stale top-field pixels must not leak into the window frame"
        );
    }

    fn nonzero_bounds(frame: &NativeDisplayFrame) -> Option<(usize, usize, usize, usize)> {
        let mut min_x = usize::MAX;
        let mut min_y = usize::MAX;
        let mut max_x = 0usize;
        let mut max_y = 0usize;

        for y in 0..frame.height {
            for x in 0..frame.width {
                if frame.pixels[y * frame.width + x] & 0x00ff_ffff == 0 {
                    continue;
                }
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }

        (min_x != usize::MAX).then_some((min_x, min_y, max_x, max_y))
    }

    fn test_caption_stalled_select_artifact_frame() -> NativeDisplayFrame {
        let width = 512usize;
        let height = 480usize;
        let bottom_start = height * 4 / 5;
        let mut pixels = vec![0; width * height];

        for y in 0..bottom_start {
            for x in 0..width {
                let slot = x % 7;
                if slot < 2 {
                    continue;
                }
                let seed = (x / 7 + y * 11 + if slot < 5 { 0 } else { 97 }) as u32;
                pixels[y * width + x] = if seed.is_multiple_of(3) {
                    let red = 72 + (seed % 48);
                    let low = 12 + (seed % 20);
                    (red << 16) | (low << 8) | low
                } else if seed % 3 == 1 {
                    let green = 72 + (seed % 96);
                    let blue = 24 + (seed % 80);
                    (16 << 16) | (green << 8) | blue
                } else {
                    let red = 20 + (seed % 56);
                    let green = 36 + (seed % 72);
                    let blue = 88 + (seed % 96);
                    (red << 16) | (green << 8) | blue
                };
            }
        }

        for y in bottom_start..height {
            let local_y = y - bottom_start;
            for x in 0..width {
                let index = y * width + x;
                if x < 16 || (local_y < 64 && x == 26) {
                    pixels[index] = 0x00d0_d0d0;
                } else if (16..26).contains(&x) {
                    pixels[index] = 0x0040_4040;
                }
            }
        }

        NativeDisplayFrame {
            width,
            height,
            pixels,
        }
    }

    #[test]
    fn native_playable_candidate_rejects_periodic_fmv_ghosting_guard() {
        let width = 320usize;
        let height = 240usize;
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

        assert!(native_display_frame_has_periodic_horizontal_ghosting(
            &frame
        ));
        assert!(
            !native_display_frame_supports_playable_candidate_with_render_context(&frame, false)
        );
        assert!(
            !native_display_frame_supports_playable_candidate_with_render_context(&frame, true)
        );
    }

    #[test]
    fn native_playable_candidate_rejects_caption_stalled_select_guard() {
        let frame = test_caption_stalled_select_artifact_frame();

        assert!(native_display_frame_has_visible_detail(&frame));
        assert!(super::native_display_frame_has_caption_stalled_select_ui_artifact(&frame));
        assert!(
            !super::native_window_should_deinterlace_frame(&frame),
            "full-height select UI must not collapse to one field"
        );
        assert!(
            !native_display_frame_supports_playable_candidate_with_render_context(&frame, false)
        );
        assert!(
            !native_display_frame_supports_playable_candidate_with_render_context(&frame, true)
        );
    }

    #[test]
    fn native_playable_candidate_rejects_saturated_logo_transition_guard() {
        let width = 512usize;
        let height = 480usize;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                let color = if y < height / 3 {
                    0x00b0_10d8
                } else if x < width / 2 {
                    0x00d0_1010
                } else {
                    let shade = ((x + y) % 96) as u32;
                    ((80 + shade) << 16) | ((48 + shade) << 8) | (128 + shade / 2)
                };
                pixels[y * width + x] = color;
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        assert!(native_display_frame_has_visible_detail(&frame));
        assert!(native_display_frame_has_saturated_logo_transition(&frame));
        assert!(
            !native_display_frame_supports_playable_candidate_with_render_context(&frame, true)
        );
    }

    #[test]
    fn native_playable_candidate_rejects_texture_page_fragment_guard() {
        let width = 512usize;
        let height = 240usize;
        let mut pixels = vec![0; width * height];
        for y in 12..192 {
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
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        assert!(native_display_frame_has_visible_detail(&frame));
        assert!(native_display_frame_has_texture_page_fragment_artifact(
            &frame
        ));
        assert!(
            !native_display_frame_supports_playable_candidate_with_render_context(&frame, true)
        );
    }

    #[test]
    fn native_playable_candidate_allows_contextual_live_stage_field() {
        let width = 512usize;
        let height = 240usize;
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
                    0x00d8_d8d8
                };
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        assert!(native_display_frame_has_visible_detail(&frame));
        assert!(native_display_frame_has_context_stage_letterbox(&frame));
        assert!(native_display_frame_has_title_logo_overlay(&frame));
        assert!(native_display_frame_has_texture_page_fragment_artifact(
            &frame
        ));
        assert!(
            !native_display_frame_supports_playable_candidate_with_render_context(&frame, false)
        );
        assert!(native_display_frame_supports_playable_candidate_with_render_context(&frame, true));
    }

    #[test]
    fn native_playable_candidate_allows_full_live_scene_title_overlay_false_positive() {
        let width = 512usize;
        let height = 480usize;
        let mut pixels = vec![0; width * height];

        for y in 0..height {
            for x in 0..width {
                let color = if (194..226).contains(&y) {
                    0
                } else {
                    let red = 40 + ((x * 11 + y * 3 + (x * y) / 257) % 176) as u32;
                    let green = 32 + ((x * 7 + y * 13 + (x * y) / 509) % 168) as u32;
                    let blue = 24 + ((x * 17 + y * 5 + (x * y) / 383) % 160) as u32;
                    (red << 16) | (green << 8) | blue
                };
                pixels[y * width + x] = color;
            }
        }

        let title_x = width * 52 / 100;
        let title_y = height * 48 / 100;
        let title_width = width * 47 / 100;
        let title_height = height * 36 / 100;
        for y in title_y..title_y + title_height {
            for x in title_x..title_x + title_width {
                let local_x = x - title_x;
                let local_y = y - title_y;
                pixels[y * width + x] = if local_y % 18 < 10 && local_x < title_width * 2 / 3 {
                    0x00d8_1818
                } else if (local_x + local_y * 3) % 23 < 5 {
                    0x00f0_f0f0
                } else {
                    0
                };
            }
        }

        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        assert!(native_display_frame_has_visible_detail(&frame));
        assert!(native_display_frame_has_title_logo_overlay(&frame));
        assert!(native_display_frame_has_context_full_live_scene(&frame));
        assert!(
            !native_display_frame_supports_playable_candidate_with_render_context(&frame, false)
        );
        assert!(native_display_frame_supports_playable_candidate_with_render_context(&frame, true));
    }

    #[test]
    fn native_playable_candidate_rejects_title_logo_overlay_guard() {
        let width = 512usize;
        let height = 240usize;
        let mut pixels = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                let color = if x < 168 {
                    0
                } else {
                    let red = 24 + ((x * 17 + y * 5) % 176) as u32;
                    let green = 40 + ((x * 7 + y * 19) % 184) as u32;
                    let blue = 32 + ((x * 13 + y * 11) % 192) as u32;
                    (red << 16) | (green << 8) | blue
                };
                pixels[y * width + x] = color;
            }
        }
        for y in 144..188 {
            for x in (300..448).step_by(10) {
                for stroke_x in x..(x + 6).min(width) {
                    pixels[y * width + stroke_x] = 0x00d0_1010;
                }
            }
        }
        for y in 118..205 {
            for x in 450..505 {
                let shade = 152 + ((x + y) % 88) as u32;
                pixels[y * width + x] = (shade << 16) | (shade << 8) | shade;
            }
        }
        let frame = NativeDisplayFrame {
            width,
            height,
            pixels,
        };

        assert!(native_display_frame_has_visible_detail(&frame));
        assert!(native_display_frame_has_title_logo_overlay(&frame));
        assert!(
            !native_display_frame_supports_playable_candidate_with_render_context(&frame, true)
        );
    }
}
