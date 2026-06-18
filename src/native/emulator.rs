use std::path::PathBuf;

use crate::action::ActionButtons;
use crate::backend::BackendError;
use crate::native::bus::Bus;
use crate::native::cpu::{Cpu, StepOutcome, StepReport};
use crate::native::platform::native_platform_json;
use crate::native::romset::NativeRomSet;

#[derive(Clone, Debug)]
pub struct NativeEmulator {
    pub cpu: Cpu,
    bus: Bus,
    last_outcome: StepOutcome,
    executed_steps: u64,
    last_step: Option<StepReport>,
}

impl NativeEmulator {
    pub fn from_rom_zip(path: impl Into<PathBuf>) -> Result<Self, BackendError> {
        let romset = NativeRomSet::inspect(path.into())?;
        let boot_rom = romset.load_boot_rom()?;
        Ok(Self {
            cpu: Cpu::default(),
            bus: Bus::new(boot_rom, 2 * 1024 * 1024),
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

    pub fn set_input(&mut self, buttons: ActionButtons) {
        self.bus.set_input(buttons);
    }

    pub fn is_terminal(&self) -> bool {
        self.last_outcome != StepOutcome::Continue || self.cpu.halted
    }

    pub fn progress_signal(&self) -> f32 {
        ((self.cpu.cycles % 1_000_000) as f32) / 1_000_000.0
    }

    pub fn json(&self) -> String {
        format!(
            "{{\"cpu\":{},\"io\":{},\"platform\":{},\"rom_bytes\":{},\"ram_bytes\":{},\"executed_steps\":{},\"last_step\":{},\"last_outcome\":\"{:?}\",\"playable\":false,\"development_stage\":\"mips_cpu_io_bootstrap\"}}",
            self.cpu.json(),
            self.bus.io_json(),
            native_platform_json(),
            self.bus.rom_len(),
            self.bus.ram_len(),
            self.executed_steps,
            optional_step_json(self.last_step),
            self.last_outcome
        )
    }
}

fn optional_step_json(report: Option<StepReport>) -> String {
    report.map_or_else(|| "null".to_string(), |report| report.json())
}

#[cfg(test)]
mod tests {
    use super::{Bus, Cpu, NativeEmulator, StepOutcome};

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
