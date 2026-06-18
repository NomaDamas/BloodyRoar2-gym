use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::action::ActionButtons;
use crate::backend::BackendError;
use crate::native::bus::Bus;
use crate::native::cpu::{Cpu, StepOutcome, StepReport};
use crate::native::platform::native_platform_json;
use crate::native::romset::{NativeRomCompatibilityReport, NativeRomSet};

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
        let start_steps = self.executed_steps;

        for _ in 0..count {
            let report = self.step_instruction();
            *pc_counts.entry(report.start_pc).or_insert(0) += 1;
            if let StepOutcome::Unsupported(instruction) = report.outcome {
                *unsupported.entry(instruction).or_insert(0) += 1;
            }
            let reached_stop_pc = config.stop_pc.is_some_and(|pc| report.start_pc == pc);
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

    pub fn vram_png(&self) -> Vec<u8> {
        self.bus.io.gpu.vram_png()
    }

    pub fn json(&self) -> String {
        format!(
            "{{\"cpu\":{},\"io\":{},\"zn_board\":{},\"native_sync\":{},\"platform\":{},\"rom_compatibility\":{},\"rom_bytes\":{},\"banked_rom_bytes\":{},\"ram_bytes\":{},\"scratchpad_bytes\":{},\"executed_steps\":{},\"last_step\":{},\"last_outcome\":\"{:?}\",\"playable\":false,\"development_stage\":\"mips_cpu_io_bootstrap\"}}",
            self.cpu.json(),
            self.bus.io_json(),
            self.bus.zn_board_json(),
            self.bus.native_sync_json(),
            native_platform_json(),
            self.rom_compatibility.summary_json(),
            self.bus.rom_len(),
            self.bus.banked_rom_len(),
            self.bus.ram_len(),
            self.bus.scratchpad_len(),
            self.executed_steps,
            optional_step_json(self.last_step),
            self.last_outcome
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct NativeTraceConfig {
    pub stop_pc: Option<u32>,
    pub stop_below_pc: Option<u32>,
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
}
