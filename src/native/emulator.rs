use std::path::PathBuf;

use crate::backend::BackendError;
use crate::native::bus::Bus;
use crate::native::cpu::{Cpu, StepOutcome};
use crate::native::romset::NativeRomSet;

#[derive(Clone, Debug)]
pub struct NativeEmulator {
    pub cpu: Cpu,
    bus: Bus,
    last_outcome: StepOutcome,
}

impl NativeEmulator {
    pub fn from_rom_zip(path: impl Into<PathBuf>) -> Result<Self, BackendError> {
        let romset = NativeRomSet::inspect(path.into())?;
        let boot_rom = romset.load_boot_rom()?;
        Ok(Self {
            cpu: Cpu::default(),
            bus: Bus::new(boot_rom, 2 * 1024 * 1024),
            last_outcome: StepOutcome::Continue,
        })
    }

    pub fn step_instructions(&mut self, count: u64) {
        for _ in 0..count {
            self.last_outcome = self.cpu.step(&mut self.bus);
            if self.last_outcome != StepOutcome::Continue {
                break;
            }
        }
    }

    pub fn json(&self) -> String {
        format!(
            "{{\"cpu\":{},\"io\":{},\"rom_bytes\":{},\"ram_bytes\":{},\"last_outcome\":\"{:?}\",\"playable\":false,\"development_stage\":\"mips_cpu_io_bootstrap\"}}",
            self.cpu.json(),
            self.bus.io_json(),
            self.bus.rom_len(),
            self.bus.ram_len(),
            self.last_outcome
        )
    }
}
