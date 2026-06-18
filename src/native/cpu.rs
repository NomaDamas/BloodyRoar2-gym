use crate::native::bus::Bus;

#[derive(Clone, Debug)]
pub struct Cpu {
    pub regs: [u32; 32],
    pub hi: u32,
    pub lo: u32,
    pub pc: u32,
    pub next_pc: u32,
    pub cycles: u64,
    pub halted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepOutcome {
    Continue,
    Halted,
    Unsupported(u32),
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            regs: [0; 32],
            hi: 0,
            lo: 0,
            pc: 0x1fc0_0000,
            next_pc: 0x1fc0_0004,
            cycles: 0,
            halted: false,
        }
    }
}

impl Cpu {
    pub fn step(&mut self, bus: &mut Bus) -> StepOutcome {
        if self.halted {
            return StepOutcome::Halted;
        }

        let instruction = bus.read_u32(self.pc);
        let current_pc = self.pc;
        self.pc = self.next_pc;
        self.next_pc = self.next_pc.wrapping_add(4);
        self.cycles += 1;

        let outcome = self.execute(instruction, current_pc, bus);
        self.regs[0] = 0;
        outcome
    }

    pub fn json(&self) -> String {
        format!(
            "{{\"pc\":{},\"next_pc\":{},\"cycles\":{},\"halted\":{},\"r2\":{},\"r4\":{},\"r29\":{},\"r31\":{}}}",
            self.pc,
            self.next_pc,
            self.cycles,
            self.halted,
            self.regs[2],
            self.regs[4],
            self.regs[29],
            self.regs[31]
        )
    }

    fn execute(&mut self, instruction: u32, current_pc: u32, bus: &mut Bus) -> StepOutcome {
        let opcode = instruction >> 26;
        match opcode {
            0x00 => self.execute_special(instruction),
            0x02 => {
                self.next_pc = jump_target(current_pc, instruction);
                StepOutcome::Continue
            }
            0x03 => {
                self.regs[31] = self.next_pc;
                self.next_pc = jump_target(current_pc, instruction);
                StepOutcome::Continue
            }
            0x04 => {
                if self.regs[rs(instruction)] == self.regs[rt(instruction)] {
                    self.next_pc = branch_target(self.pc, instruction);
                }
                StepOutcome::Continue
            }
            0x05 => {
                if self.regs[rs(instruction)] != self.regs[rt(instruction)] {
                    self.next_pc = branch_target(self.pc, instruction);
                }
                StepOutcome::Continue
            }
            0x08 | 0x09 => {
                self.regs[rt(instruction)] =
                    self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                StepOutcome::Continue
            }
            0x0c => {
                self.regs[rt(instruction)] = self.regs[rs(instruction)] & (instruction & 0xffff);
                StepOutcome::Continue
            }
            0x0d => {
                self.regs[rt(instruction)] = self.regs[rs(instruction)] | (instruction & 0xffff);
                StepOutcome::Continue
            }
            0x0f => {
                self.regs[rt(instruction)] = (instruction & 0xffff) << 16;
                StepOutcome::Continue
            }
            0x23 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.regs[rt(instruction)] = bus.read_u32(address);
                StepOutcome::Continue
            }
            0x2b => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                bus.write_u32(address, self.regs[rt(instruction)]);
                StepOutcome::Continue
            }
            _ => StepOutcome::Unsupported(instruction),
        }
    }

    fn execute_special(&mut self, instruction: u32) -> StepOutcome {
        match instruction & 0x3f {
            0x00 => {
                if instruction != 0 {
                    self.regs[rd(instruction)] = self.regs[rt(instruction)] << shamt(instruction);
                }
                StepOutcome::Continue
            }
            0x02 => {
                self.regs[rd(instruction)] = self.regs[rt(instruction)] >> shamt(instruction);
                StepOutcome::Continue
            }
            0x08 => {
                self.next_pc = self.regs[rs(instruction)];
                StepOutcome::Continue
            }
            0x20 | 0x21 => {
                self.regs[rd(instruction)] =
                    self.regs[rs(instruction)].wrapping_add(self.regs[rt(instruction)]);
                StepOutcome::Continue
            }
            0x22 | 0x23 => {
                self.regs[rd(instruction)] =
                    self.regs[rs(instruction)].wrapping_sub(self.regs[rt(instruction)]);
                StepOutcome::Continue
            }
            0x24 => {
                self.regs[rd(instruction)] =
                    self.regs[rs(instruction)] & self.regs[rt(instruction)];
                StepOutcome::Continue
            }
            0x25 => {
                self.regs[rd(instruction)] =
                    self.regs[rs(instruction)] | self.regs[rt(instruction)];
                StepOutcome::Continue
            }
            0x0d => {
                self.halted = true;
                StepOutcome::Halted
            }
            _ => StepOutcome::Unsupported(instruction),
        }
    }
}

fn rs(instruction: u32) -> usize {
    ((instruction >> 21) & 0x1f) as usize
}

fn rt(instruction: u32) -> usize {
    ((instruction >> 16) & 0x1f) as usize
}

fn rd(instruction: u32) -> usize {
    ((instruction >> 11) & 0x1f) as usize
}

fn shamt(instruction: u32) -> u32 {
    (instruction >> 6) & 0x1f
}

fn sign_extend_16(instruction: u32) -> u32 {
    (instruction as i16) as i32 as u32
}

fn jump_target(pc: u32, instruction: u32) -> u32 {
    (pc & 0xf000_0000) | ((instruction & 0x03ff_ffff) << 2)
}

fn branch_target(pc: u32, instruction: u32) -> u32 {
    pc.wrapping_add(sign_extend_16(instruction) << 2)
}

#[cfg(test)]
mod tests {
    use super::{Cpu, StepOutcome};
    use crate::native::bus::Bus;

    #[test]
    fn executes_addiu_and_break() {
        let rom = vec![
            0x2a, 0x00, 0x02, 0x24, // addiu v0, zero, 42
            0x0d, 0x00, 0x00, 0x00, // break
        ];
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.regs[2], 42);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Halted);
    }
}
