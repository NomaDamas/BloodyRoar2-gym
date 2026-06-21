use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::action::ActionButtons;
use crate::backend::BackendError;
use crate::native::bus::{Bus, NativeInputActivity};
use crate::native::cpu::{Cpu, StepOutcome, StepReport};
use crate::native::io::{NativeGpuDisplayCandidate, NativeGpuDrawCapture};
use crate::native::platform::native_platform_json;
use crate::native::romset::{NativeRomCompatibilityReport, NativeRomSet};

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
        let romset = NativeRomSet::scan(path.into())?;
        let rom_compatibility = romset.compatibility_report();
        let boot_rom = romset.load_boot_rom()?;
        let banked_roms = romset.load_banked_roms()?;
        let board_assets = romset.load_board_assets();
        Ok(Self {
            cpu: Cpu::default(),
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

    pub fn trace_instructions(
        &mut self,
        count: u64,
        hot_limit: usize,
        recent_limit: usize,
        config: NativeTraceConfig,
    ) -> NativeTrace {
        self.bus.set_access_trace_limit(recent_limit.max(32));
        self.bus.set_access_trace_watch_ranges(config.watch_ranges);
        self.bus.set_access_trace_watch_only(config.watch_only);
        let mut pc_counts = BTreeMap::new();
        let mut unsupported = BTreeMap::new();
        let mut recent_steps = Vec::new();
        let mut stop_pc_hits_to_skip = config.stop_pc_skip;
        let start_steps = self.executed_steps;

        for _ in 0..count {
            let report = self.step_instruction();
            *pc_counts.entry(report.start_pc).or_insert(0) += 1;
            if let StepOutcome::Unsupported(instruction) = report.outcome {
                *unsupported.entry(instruction).or_insert(0) += 1;
            }
            let reached_stop_pc = if config.stop_pc.is_some_and(|pc| report.start_pc == pc) {
                if stop_pc_hits_to_skip > 0 {
                    stop_pc_hits_to_skip -= 1;
                    false
                } else {
                    true
                }
            } else {
                false
            };
            let reached_low_pc = config.stop_below_pc.is_some_and(|pc| report.start_pc < pc);

            if recent_limit > 0 {
                recent_steps.push(report);
                if recent_steps.len() > recent_limit {
                    recent_steps.remove(0);
                }
            }

            if report.outcome != StepOutcome::Continue || reached_stop_pc || reached_low_pc {
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
        self.bus.set_access_trace_watch_ranges(config.watch_ranges);
        self.bus.set_access_trace_watch_only(config.watch_only);
        let mut pc_counts = BTreeMap::new();
        let mut unsupported = BTreeMap::new();
        let mut recent_steps = Vec::new();
        let mut stop_pc_hits_to_skip = config.stop_pc_skip;
        let start_steps = self.executed_steps;
        let start_vblank = self.bus.vblank_count();
        let max_instructions = max_instructions.max(1);

        while self.executed_steps - start_steps < max_instructions
            && self.bus.vblank_count() == start_vblank
            && self.last_outcome == StepOutcome::Continue
        {
            let report = self.step_instruction();
            *pc_counts.entry(report.start_pc).or_insert(0) += 1;
            if let StepOutcome::Unsupported(instruction) = report.outcome {
                *unsupported.entry(instruction).or_insert(0) += 1;
            }
            let reached_stop_pc = if config.stop_pc.is_some_and(|pc| report.start_pc == pc) {
                if stop_pc_hits_to_skip > 0 {
                    stop_pc_hits_to_skip -= 1;
                    false
                } else {
                    true
                }
            } else {
                false
            };
            let reached_low_pc = config.stop_below_pc.is_some_and(|pc| report.start_pc < pc);

            if recent_limit > 0 {
                recent_steps.push(report);
                if recent_steps.len() > recent_limit {
                    recent_steps.remove(0);
                }
            }

            if report.outcome != StepOutcome::Continue || reached_stop_pc || reached_low_pc {
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
        self.bus.set_access_trace_watch_ranges(config.watch_ranges);
        self.bus.set_access_trace_watch_only(config.watch_only);
        let mut pc_counts = BTreeMap::new();
        let mut unsupported = BTreeMap::new();
        let mut recent_steps = Vec::new();
        let mut stop_pc_hits_to_skip = config.stop_pc_skip;
        let start_steps = self.executed_steps;
        let instructions_per_frame = instructions_per_frame.max(1);
        let requested_steps = segments.iter().fold(0u64, |total, (_, frames)| {
            total.saturating_add(frames.saturating_mul(instructions_per_frame))
        });

        'script: for (buttons, frames) in segments {
            self.set_input(*buttons);
            for _ in 0..*frames {
                for _ in 0..instructions_per_frame {
                    let report = self.step_instruction();
                    *pc_counts.entry(report.start_pc).or_insert(0) += 1;
                    if let StepOutcome::Unsupported(instruction) = report.outcome {
                        *unsupported.entry(instruction).or_insert(0) += 1;
                    }
                    let reached_stop_pc = if config.stop_pc.is_some_and(|pc| report.start_pc == pc)
                    {
                        if stop_pc_hits_to_skip > 0 {
                            stop_pc_hits_to_skip -= 1;
                            false
                        } else {
                            true
                        }
                    } else {
                        false
                    };
                    let reached_low_pc =
                        config.stop_below_pc.is_some_and(|pc| report.start_pc < pc);

                    if recent_limit > 0 {
                        recent_steps.push(report);
                        if recent_steps.len() > recent_limit {
                            recent_steps.remove(0);
                        }
                    }

                    if self.last_outcome != StepOutcome::Continue
                        || reached_stop_pc
                        || reached_low_pc
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
            bus_access_trace_json: self.bus.access_trace_json(),
            state_json: self.json(),
        })
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

    pub fn display_png(&self) -> Vec<u8> {
        self.bus.io.gpu.display_png()
    }

    pub fn display_frame(&self) -> NativeDisplayFrame {
        let (width, height, pixels) = self.bus.io.gpu.actual_display_rgb_frame();
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

    pub fn draw_captures(&self) -> &[NativeGpuDrawCapture] {
        self.bus.gpu_draw_captures()
    }

    pub fn display_candidates(&self) -> Vec<NativeGpuDisplayCandidate> {
        self.bus.gpu_display_candidates()
    }

    pub fn input_activity_json(&self) -> String {
        self.bus.input_activity().json()
    }

    pub fn input_activity(&self) -> NativeInputActivity {
        self.bus.input_activity()
    }

    pub fn has_play_control_activity(&self) -> bool {
        self.bus.input_activity().has_play_control_activity()
    }

    pub fn has_full_control_activity(&self) -> bool {
        self.bus.input_activity().has_full_control_activity()
    }

    pub fn native_playable_candidate(&self) -> bool {
        self.bus.native_playable_candidate()
    }

    pub fn json(&self) -> String {
        format!(
            "{{\"cpu\":{},\"gte\":{},\"io\":{},\"zn_board\":{},\"native_sync\":{},\"native_playability\":{},\"platform\":{},\"rom_compatibility\":{},\"rom_bytes\":{},\"banked_rom_bytes\":{},\"ram_bytes\":{},\"scratchpad_bytes\":{},\"executed_steps\":{},\"last_step\":{},\"last_outcome\":\"{:?}\",\"playable\":{},\"development_stage\":\"native_runtime_validation\"}}",
            self.cpu.json(),
            self.cpu.gte_json(),
            self.bus.io_json(),
            self.bus.zn_board_json(),
            self.bus.native_sync_json(),
            self.bus.native_playability_json(),
            native_platform_json(),
            self.rom_compatibility.summary_json(),
            self.bus.rom_len(),
            self.bus.banked_rom_len(),
            self.bus.ram_len(),
            self.bus.scratchpad_len(),
            self.executed_steps,
            optional_step_json(self.last_step),
            self.last_outcome,
            self.bus.native_playable_candidate()
        )
    }

    pub fn diagnostic_json(&self) -> String {
        format!(
            "{{\"cpu\":{},\"gte\":{},\"io\":{},\"zn_board\":{},\"native_sync\":{},\"native_playability\":{},\"platform\":{},\"rom_compatibility\":{},\"rom_bytes\":{},\"banked_rom_bytes\":{},\"ram_bytes\":{},\"scratchpad_bytes\":{},\"executed_steps\":{},\"last_step\":{},\"last_outcome\":\"{:?}\",\"playable\":{},\"development_stage\":\"native_runtime_validation\"}}",
            self.cpu.json(),
            self.cpu.gte_json(),
            self.bus.io_compact_json(),
            self.bus.zn_board_json(),
            self.bus.native_sync_json(),
            self.bus.native_playability_json(),
            native_platform_json(),
            self.rom_compatibility.summary_json(),
            self.bus.rom_len(),
            self.bus.banked_rom_len(),
            self.bus.ram_len(),
            self.bus.scratchpad_len(),
            self.executed_steps,
            optional_step_json(self.last_step),
            self.last_outcome,
            self.bus.native_playable_candidate()
        )
    }

    pub fn probe_json(&self) -> String {
        format!(
            "{{\"cpu\":{{\"pc\":{},\"pc_hex\":\"0x{:08x}\",\"cycles\":{},\"halted\":{},\"status\":{},\"status_hex\":\"0x{:08x}\",\"cause\":{},\"cause_hex\":\"0x{:08x}\",\"epc\":{},\"epc_hex\":\"0x{:08x}\"}},\"runtime\":{},\"native_playability\":{},\"rom_compatibility\":{},\"executed_steps\":{},\"last_outcome\":\"{:?}\",\"playable\":{},\"development_stage\":\"native_runtime_validation\"}}",
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
            self.bus.runtime_probe_json(),
            self.bus.native_playability_json(),
            self.rom_compatibility.summary_json(),
            self.executed_steps,
            self.last_outcome,
            self.bus.native_playable_candidate()
        )
    }

    pub fn compact_probe_json(&self) -> String {
        format!(
            "{{\"cpu\":{{\"pc\":{},\"pc_hex\":\"0x{:08x}\",\"cycles\":{},\"halted\":{},\"status\":{},\"status_hex\":\"0x{:08x}\",\"cause\":{},\"cause_hex\":\"0x{:08x}\",\"epc\":{},\"epc_hex\":\"0x{:08x}\",\"r2\":{},\"r3\":{},\"r4\":{},\"r5\":{},\"r6\":{}}},\"runtime\":{},\"input_activity\":{},\"rom_compatibility\":{},\"executed_steps\":{},\"last_outcome\":\"{:?}\",\"playable\":{},\"development_stage\":\"native_runtime_validation\"}}",
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
            self.cpu.regs[2],
            self.cpu.regs[3],
            self.cpu.regs[4],
            self.cpu.regs[5],
            self.cpu.regs[6],
            self.bus.runtime_compact_probe_json(),
            self.bus.input_activity().json(),
            self.rom_compatibility.summary_json(),
            self.executed_steps,
            self.last_outcome,
            self.bus.native_playable_candidate()
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct NativeTraceConfig {
    pub stop_pc: Option<u32>,
    pub stop_below_pc: Option<u32>,
    pub stop_pc_skip: u64,
    pub watch_ranges: Vec<(u32, u32)>,
    pub watch_only: bool,
}

#[derive(Clone, Debug)]
pub struct NativeTrace {
    requested_steps: u64,
    executed_steps: u64,
    unique_pcs: usize,
    hot_pcs: Vec<NativeTracePc>,
    unsupported_instructions: Vec<NativeTraceInstruction>,
    recent_steps: Vec<StepReport>,
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
            "{{\"requested_steps\":{},\"executed_steps\":{},\"unique_pcs\":{},\"hot_pcs\":[{}],\"unsupported_instructions\":[{}],\"recent_steps\":[{}],\"bus_access_trace\":[{}],\"state\":{}}}",
            self.requested_steps,
            self.executed_steps,
            self.unique_pcs,
            hot_pcs,
            unsupported_instructions,
            recent_steps,
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
}

#[derive(Debug)]
struct NativeTraceParts {
    requested_steps: u64,
    executed_steps: u64,
    pc_counts: BTreeMap<u32, u64>,
    unsupported: BTreeMap<u32, u64>,
    recent_steps: Vec<StepReport>,
    hot_limit: usize,
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

#[cfg(test)]
mod tests {
    use super::{Bus, Cpu, NativeEmulator, StepOutcome};
    use crate::native::io::GPU_GP0;
    use crate::native::romset::NativeRomCompatibilityReport;

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
    fn display_frame_uses_resolved_display_instead_of_sparse_raw_actual() {
        let mut emulator = test_emulator();
        let (width, height) = emulator.bus.io.gpu.display_dimensions();

        for x in (0..width as u32).step_by(4) {
            let color = if x % 8 == 0 { 0x00ff_ffff } else { 0x0000_40ff };
            gp0_fill_rect(&mut emulator, color, x, 0, 4, height as u32);
        }
        emulator.bus.io.gpu.capture_vblank_presented_frame();

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
        let frame = emulator.display_frame();

        assert_ne!(raw.2, resolved.2);
        assert_eq!(frame.width, resolved.0);
        assert_eq!(frame.height, resolved.1);
        assert_eq!(frame.pixels, resolved.2);
    }
}
