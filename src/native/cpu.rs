use crate::native::bus::Bus;

const CP0_STATUS: usize = 12;
const CP0_CAUSE: usize = 13;
const CP0_EPC: usize = 14;

const STATUS_IE: u32 = 1 << 0;
const STATUS_INTERRUPT_MASK: u32 = 0xff << 8;
const STATUS_ISOLATE_CACHE: u32 = 1 << 16;

const CAUSE_BD: u32 = 1 << 31;
const CAUSE_EXCODE_MASK: u32 = 0x1f << 2;
const CAUSE_IP_MASK: u32 = 0xff << 8;
const CAUSE_IP2: u32 = 1 << 10;
const EXCEPTION_VECTOR: u32 = 0x8000_0080;

#[derive(Clone, Debug)]
pub struct Cpu {
    pub regs: [u32; 32],
    pub cp0: [u32; 32],
    pub cop2_data: [u32; 32],
    pub cop2_control: [u32; 32],
    pub hi: u32,
    pub lo: u32,
    pub pc: u32,
    pub next_pc: u32,
    pub cycles: u64,
    pub halted: bool,
    delay_slot_branch_pc: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepOutcome {
    Continue,
    Halted,
    Unsupported(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepReport {
    pub start_pc: u32,
    pub end_pc: u32,
    pub next_pc: u32,
    pub instruction: Option<u32>,
    pub cycles_before: u64,
    pub cycles_after: u64,
    pub cycles_elapsed: u64,
    pub outcome: StepOutcome,
}

impl StepReport {
    fn halted(cpu: &Cpu) -> Self {
        Self {
            start_pc: cpu.pc,
            end_pc: cpu.pc,
            next_pc: cpu.next_pc,
            instruction: None,
            cycles_before: cpu.cycles,
            cycles_after: cpu.cycles,
            cycles_elapsed: 0,
            outcome: StepOutcome::Halted,
        }
    }

    pub fn json(&self) -> String {
        format!(
            "{{\"start_pc\":{},\"start_pc_hex\":\"0x{:08x}\",\"end_pc\":{},\"end_pc_hex\":\"0x{:08x}\",\"next_pc\":{},\"next_pc_hex\":\"0x{:08x}\",\"instruction\":{},\"instruction_hex\":{},\"cycles_before\":{},\"cycles_after\":{},\"cycles_elapsed\":{},\"outcome\":\"{:?}\"}}",
            self.start_pc,
            self.start_pc,
            self.end_pc,
            self.end_pc,
            self.next_pc,
            self.next_pc,
            optional_u32_json(self.instruction),
            optional_u32_hex_json(self.instruction),
            self.cycles_before,
            self.cycles_after,
            self.cycles_elapsed,
            self.outcome
        )
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            regs: [0; 32],
            cp0: [0; 32],
            cop2_data: [0; 32],
            cop2_control: [0; 32],
            hi: 0,
            lo: 0,
            pc: 0x1fc0_0000,
            next_pc: 0x1fc0_0004,
            cycles: 0,
            halted: false,
            delay_slot_branch_pc: None,
        }
    }
}

impl Cpu {
    pub fn step(&mut self, bus: &mut Bus) -> StepOutcome {
        self.step_report(bus).outcome
    }

    pub fn step_report(&mut self, bus: &mut Bus) -> StepReport {
        if self.halted {
            return StepReport::halted(self);
        }

        let start_pc = self.pc;
        let cycles_before = self.cycles;
        bus.set_trace_context(start_pc, cycles_before);
        self.refresh_interrupts(bus);
        if self.delay_slot_branch_pc.is_none() && self.interrupt_pending() {
            self.cycles += 1;
            let outcome = self.raise_exception(self.pc, None, Exception::Interrupt);
            self.regs[0] = 0;
            let report = self.step_report_from(start_pc, None, cycles_before, outcome);
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        let delay_slot_branch_pc = self.delay_slot_branch_pc.take();
        let instruction = bus.read_u32(self.pc);
        let current_pc = self.pc;
        self.pc = self.next_pc;
        self.next_pc = self.next_pc.wrapping_add(4);
        self.cycles += 1;
        bus.set_trace_context(current_pc, self.cycles);

        let outcome = self.execute(instruction, current_pc, delay_slot_branch_pc, bus);
        self.cycles += fixed_cycle_cost(Some(instruction), outcome).saturating_sub(1);
        self.regs[0] = 0;
        let report = self.step_report_from(start_pc, Some(instruction), cycles_before, outcome);
        bus.tick(report.cycles_elapsed);
        bus.clear_trace_context();
        report
    }

    pub fn json(&self) -> String {
        format!(
            "{{\"pc\":{},\"next_pc\":{},\"cycles\":{},\"halted\":{},\"status\":{},\"cause\":{},\"epc\":{},\"r2\":{},\"r3\":{},\"r4\":{},\"r5\":{},\"r6\":{},\"r8\":{},\"r9\":{},\"r10\":{},\"r11\":{},\"r16\":{},\"r29\":{},\"r31\":{}}}",
            self.pc,
            self.next_pc,
            self.cycles,
            self.halted,
            self.cp0[CP0_STATUS],
            self.cp0[CP0_CAUSE],
            self.cp0[CP0_EPC],
            self.regs[2],
            self.regs[3],
            self.regs[4],
            self.regs[5],
            self.regs[6],
            self.regs[8],
            self.regs[9],
            self.regs[10],
            self.regs[11],
            self.regs[16],
            self.regs[29],
            self.regs[31]
        )
    }

    fn step_report_from(
        &self,
        start_pc: u32,
        instruction: Option<u32>,
        cycles_before: u64,
        outcome: StepOutcome,
    ) -> StepReport {
        StepReport {
            start_pc,
            end_pc: self.pc,
            next_pc: self.next_pc,
            instruction,
            cycles_before,
            cycles_after: self.cycles,
            cycles_elapsed: self.cycles.saturating_sub(cycles_before),
            outcome,
        }
    }

    fn execute(
        &mut self,
        instruction: u32,
        current_pc: u32,
        delay_slot_branch_pc: Option<u32>,
        bus: &mut Bus,
    ) -> StepOutcome {
        let opcode = instruction >> 26;
        match opcode {
            0x00 => self.execute_special(instruction, current_pc, delay_slot_branch_pc),
            0x01 => self.execute_regimm(instruction, current_pc),
            0x02 => {
                self.next_pc = jump_target(current_pc, instruction);
                self.delay_slot_branch_pc = Some(current_pc);
                StepOutcome::Continue
            }
            0x03 => {
                self.regs[31] = self.next_pc;
                self.next_pc = jump_target(current_pc, instruction);
                self.delay_slot_branch_pc = Some(current_pc);
                StepOutcome::Continue
            }
            0x04 => {
                if self.regs[rs(instruction)] == self.regs[rt(instruction)] {
                    self.next_pc = branch_target(self.pc, instruction);
                }
                self.delay_slot_branch_pc = Some(current_pc);
                StepOutcome::Continue
            }
            0x05 => {
                if self.regs[rs(instruction)] != self.regs[rt(instruction)] {
                    self.next_pc = branch_target(self.pc, instruction);
                }
                self.delay_slot_branch_pc = Some(current_pc);
                StepOutcome::Continue
            }
            0x06 => {
                if (self.regs[rs(instruction)] as i32) <= 0 {
                    self.next_pc = branch_target(self.pc, instruction);
                }
                self.delay_slot_branch_pc = Some(current_pc);
                StepOutcome::Continue
            }
            0x07 => {
                if (self.regs[rs(instruction)] as i32) > 0 {
                    self.next_pc = branch_target(self.pc, instruction);
                }
                self.delay_slot_branch_pc = Some(current_pc);
                StepOutcome::Continue
            }
            0x08 => {
                match (self.regs[rs(instruction)] as i32)
                    .checked_add(sign_extend_16(instruction) as i32)
                {
                    Some(value) => self.regs[rt(instruction)] = value as u32,
                    None => {
                        return self.raise_exception(
                            current_pc,
                            delay_slot_branch_pc,
                            Exception::Overflow,
                        );
                    }
                }
                StepOutcome::Continue
            }
            0x09 => {
                self.regs[rt(instruction)] =
                    self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                StepOutcome::Continue
            }
            0x0a => {
                self.regs[rt(instruction)] = ((self.regs[rs(instruction)] as i32)
                    < (sign_extend_16(instruction) as i32))
                    as u32;
                StepOutcome::Continue
            }
            0x0b => {
                self.regs[rt(instruction)] =
                    (self.regs[rs(instruction)] < sign_extend_16(instruction)) as u32;
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
            0x0e => {
                self.regs[rt(instruction)] = self.regs[rs(instruction)] ^ (instruction & 0xffff);
                StepOutcome::Continue
            }
            0x0f => {
                self.regs[rt(instruction)] = (instruction & 0xffff) << 16;
                StepOutcome::Continue
            }
            0x10 => self.execute_cop0(instruction, bus),
            0x12 => self.execute_cop2(instruction),
            0x20 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.regs[rt(instruction)] = (bus.read_u8(address) as i8) as i32 as u32;
                StepOutcome::Continue
            }
            0x21 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.regs[rt(instruction)] = (bus.read_u16(address) as i16) as i32 as u32;
                StepOutcome::Continue
            }
            0x22 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.regs[rt(instruction)] =
                    load_word_left(bus, address, self.regs[rt(instruction)]);
                StepOutcome::Continue
            }
            0x23 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.regs[rt(instruction)] = bus.read_u32(address);
                StepOutcome::Continue
            }
            0x24 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.regs[rt(instruction)] = bus.read_u8(address) as u32;
                StepOutcome::Continue
            }
            0x25 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.regs[rt(instruction)] = bus.read_u16(address) as u32;
                StepOutcome::Continue
            }
            0x26 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.regs[rt(instruction)] =
                    load_word_right(bus, address, self.regs[rt(instruction)]);
                StepOutcome::Continue
            }
            0x28 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                bus.write_u8(address, self.regs[rt(instruction)] as u8);
                StepOutcome::Continue
            }
            0x29 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                bus.write_u16(address, self.regs[rt(instruction)] as u16);
                StepOutcome::Continue
            }
            0x2a => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                store_word_left(bus, address, self.regs[rt(instruction)]);
                StepOutcome::Continue
            }
            0x2b => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                bus.write_u32(address, self.regs[rt(instruction)]);
                StepOutcome::Continue
            }
            0x2e => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                store_word_right(bus, address, self.regs[rt(instruction)]);
                StepOutcome::Continue
            }
            0x32 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.cop2_data[rt(instruction)] = bus.read_u32(address);
                StepOutcome::Continue
            }
            0x3a => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                bus.write_u32(address, self.cop2_data[rt(instruction)]);
                StepOutcome::Continue
            }
            _ => StepOutcome::Unsupported(instruction),
        }
    }

    fn execute_special(
        &mut self,
        instruction: u32,
        current_pc: u32,
        delay_slot_branch_pc: Option<u32>,
    ) -> StepOutcome {
        match instruction & 0x3f {
            0x00 => {
                if instruction != 0 {
                    self.regs[rd(instruction)] = self.regs[rt(instruction)] << shamt(instruction);
                }
                StepOutcome::Continue
            }
            0x04 => {
                self.regs[rd(instruction)] =
                    self.regs[rt(instruction)] << (self.regs[rs(instruction)] & 0x1f);
                StepOutcome::Continue
            }
            0x02 => {
                self.regs[rd(instruction)] = self.regs[rt(instruction)] >> shamt(instruction);
                StepOutcome::Continue
            }
            0x03 => {
                self.regs[rd(instruction)] =
                    ((self.regs[rt(instruction)] as i32) >> shamt(instruction)) as u32;
                StepOutcome::Continue
            }
            0x06 => {
                self.regs[rd(instruction)] =
                    self.regs[rt(instruction)] >> (self.regs[rs(instruction)] & 0x1f);
                StepOutcome::Continue
            }
            0x07 => {
                self.regs[rd(instruction)] = ((self.regs[rt(instruction)] as i32)
                    >> (self.regs[rs(instruction)] & 0x1f))
                    as u32;
                StepOutcome::Continue
            }
            0x08 => {
                self.next_pc = self.regs[rs(instruction)];
                self.delay_slot_branch_pc = Some(current_pc);
                StepOutcome::Continue
            }
            0x09 => {
                self.regs[rd(instruction)] = self.next_pc;
                self.next_pc = self.regs[rs(instruction)];
                self.delay_slot_branch_pc = Some(current_pc);
                StepOutcome::Continue
            }
            0x10 => {
                self.regs[rd(instruction)] = self.hi;
                StepOutcome::Continue
            }
            0x11 => {
                self.hi = self.regs[rs(instruction)];
                StepOutcome::Continue
            }
            0x12 => {
                self.regs[rd(instruction)] = self.lo;
                StepOutcome::Continue
            }
            0x13 => {
                self.lo = self.regs[rs(instruction)];
                StepOutcome::Continue
            }
            0x18 => {
                let product = (self.regs[rs(instruction)] as i32 as i64)
                    * (self.regs[rt(instruction)] as i32 as i64);
                self.hi = (product >> 32) as u32;
                self.lo = product as u32;
                StepOutcome::Continue
            }
            0x19 => {
                let product =
                    (self.regs[rs(instruction)] as u64) * (self.regs[rt(instruction)] as u64);
                self.hi = (product >> 32) as u32;
                self.lo = product as u32;
                StepOutcome::Continue
            }
            0x1a => {
                let divisor = self.regs[rt(instruction)] as i32;
                if divisor != 0 {
                    self.lo = ((self.regs[rs(instruction)] as i32) / divisor) as u32;
                    self.hi = ((self.regs[rs(instruction)] as i32) % divisor) as u32;
                }
                StepOutcome::Continue
            }
            0x1b => {
                let divisor = self.regs[rt(instruction)];
                if let Some(quotient) = self.regs[rs(instruction)].checked_div(divisor) {
                    self.lo = quotient;
                    self.hi = self.regs[rs(instruction)] % divisor;
                }
                StepOutcome::Continue
            }
            0x0c => self.raise_exception(current_pc, delay_slot_branch_pc, Exception::Syscall),
            0x0d => {
                self.raise_exception(current_pc, delay_slot_branch_pc, Exception::Breakpoint);
                self.halted = true;
                StepOutcome::Halted
            }
            0x20 => {
                match (self.regs[rs(instruction)] as i32)
                    .checked_add(self.regs[rt(instruction)] as i32)
                {
                    Some(value) => self.regs[rd(instruction)] = value as u32,
                    None => {
                        return self.raise_exception(
                            current_pc,
                            delay_slot_branch_pc,
                            Exception::Overflow,
                        );
                    }
                }
                StepOutcome::Continue
            }
            0x21 => {
                self.regs[rd(instruction)] =
                    self.regs[rs(instruction)].wrapping_add(self.regs[rt(instruction)]);
                StepOutcome::Continue
            }
            0x22 => {
                match (self.regs[rs(instruction)] as i32)
                    .checked_sub(self.regs[rt(instruction)] as i32)
                {
                    Some(value) => self.regs[rd(instruction)] = value as u32,
                    None => {
                        return self.raise_exception(
                            current_pc,
                            delay_slot_branch_pc,
                            Exception::Overflow,
                        );
                    }
                }
                StepOutcome::Continue
            }
            0x23 => {
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
            0x26 => {
                self.regs[rd(instruction)] =
                    self.regs[rs(instruction)] ^ self.regs[rt(instruction)];
                StepOutcome::Continue
            }
            0x27 => {
                self.regs[rd(instruction)] =
                    !(self.regs[rs(instruction)] | self.regs[rt(instruction)]);
                StepOutcome::Continue
            }
            0x2a => {
                self.regs[rd(instruction)] = ((self.regs[rs(instruction)] as i32)
                    < (self.regs[rt(instruction)] as i32))
                    as u32;
                StepOutcome::Continue
            }
            0x2b => {
                self.regs[rd(instruction)] =
                    (self.regs[rs(instruction)] < self.regs[rt(instruction)]) as u32;
                StepOutcome::Continue
            }
            _ => StepOutcome::Unsupported(instruction),
        }
    }

    fn execute_regimm(&mut self, instruction: u32, current_pc: u32) -> StepOutcome {
        match rt(instruction) {
            0x00 => {
                if (self.regs[rs(instruction)] as i32) < 0 {
                    self.next_pc = branch_target(self.pc, instruction);
                }
                self.delay_slot_branch_pc = Some(current_pc);
                StepOutcome::Continue
            }
            0x01 => {
                if (self.regs[rs(instruction)] as i32) >= 0 {
                    self.next_pc = branch_target(self.pc, instruction);
                }
                self.delay_slot_branch_pc = Some(current_pc);
                StepOutcome::Continue
            }
            0x10 => {
                self.regs[31] = self.next_pc;
                if (self.regs[rs(instruction)] as i32) < 0 {
                    self.next_pc = branch_target(self.pc, instruction);
                }
                self.delay_slot_branch_pc = Some(current_pc);
                StepOutcome::Continue
            }
            0x11 => {
                self.regs[31] = self.next_pc;
                if (self.regs[rs(instruction)] as i32) >= 0 {
                    self.next_pc = branch_target(self.pc, instruction);
                }
                self.delay_slot_branch_pc = Some(current_pc);
                StepOutcome::Continue
            }
            _ => StepOutcome::Unsupported(instruction),
        }
    }

    fn execute_cop0(&mut self, instruction: u32, bus: &mut Bus) -> StepOutcome {
        match rs(instruction) {
            0x00 => {
                self.regs[rt(instruction)] = self.cp0[rd(instruction)];
                StepOutcome::Continue
            }
            0x04 => {
                self.cp0[rd(instruction)] = self.regs[rt(instruction)];
                if rd(instruction) == CP0_STATUS {
                    bus.set_cache_isolated(self.cp0[CP0_STATUS] & STATUS_ISOLATE_CACHE != 0);
                }
                StepOutcome::Continue
            }
            0x10 if (instruction & 0x3f) == 0x10 => {
                let mode_bits = self.cp0[CP0_STATUS] & 0x3f;
                self.cp0[CP0_STATUS] = (self.cp0[CP0_STATUS] & !0x0f) | ((mode_bits >> 2) & 0x0f);
                bus.set_cache_isolated(self.cp0[CP0_STATUS] & STATUS_ISOLATE_CACHE != 0);
                StepOutcome::Continue
            }
            _ => StepOutcome::Unsupported(instruction),
        }
    }

    fn execute_cop2(&mut self, instruction: u32) -> StepOutcome {
        match rs(instruction) {
            0x00 => {
                self.regs[rt(instruction)] = self.cop2_data[rd(instruction)];
                StepOutcome::Continue
            }
            0x02 => {
                self.regs[rt(instruction)] = self.cop2_control[rd(instruction)];
                StepOutcome::Continue
            }
            0x04 => {
                self.cop2_data[rd(instruction)] = self.regs[rt(instruction)];
                StepOutcome::Continue
            }
            0x06 => {
                self.cop2_control[rd(instruction)] = self.regs[rt(instruction)];
                StepOutcome::Continue
            }
            0x10..=0x1f => {
                self.execute_gte_command(instruction);
                StepOutcome::Continue
            }
            _ => StepOutcome::Unsupported(instruction),
        }
    }

    fn execute_gte_command(&mut self, instruction: u32) {
        let command = instruction & 0x3f;
        match command {
            // Minimal deterministic placeholders for BIOS/game progression. Accurate GTE
            // geometry and color math is implemented incrementally behind these registers.
            0x01 | 0x06 | 0x0c | 0x10 | 0x11 | 0x12 | 0x13 | 0x14 | 0x16 | 0x1b | 0x1c | 0x1e
            | 0x20 | 0x28 | 0x29 | 0x2a | 0x2d | 0x2e | 0x30 | 0x3d | 0x3e | 0x3f => {
                self.cop2_data[31] = 0;
            }
            _ => {
                self.cop2_data[31] = 0;
            }
        }
    }

    fn refresh_interrupts(&mut self, bus: &Bus) {
        if bus.io.irq.status & bus.io.irq.mask != 0 {
            self.cp0[CP0_CAUSE] |= CAUSE_IP2;
        } else {
            self.cp0[CP0_CAUSE] &= !CAUSE_IP2;
        }
    }

    fn interrupt_pending(&self) -> bool {
        let enabled = self.cp0[CP0_STATUS] & STATUS_IE != 0;
        let unmasked = self.cp0[CP0_STATUS] & self.cp0[CP0_CAUSE] & STATUS_INTERRUPT_MASK != 0;
        enabled && unmasked
    }

    fn raise_exception(
        &mut self,
        current_pc: u32,
        delay_slot_branch_pc: Option<u32>,
        exception: Exception,
    ) -> StepOutcome {
        let mut cause = self.cp0[CP0_CAUSE] & CAUSE_IP_MASK;
        cause |= (exception as u32) << 2;
        if let Some(branch_pc) = delay_slot_branch_pc {
            cause |= CAUSE_BD;
            self.cp0[CP0_EPC] = branch_pc;
        } else {
            self.cp0[CP0_EPC] = current_pc;
        }

        self.cp0[CP0_CAUSE] = cause & !CAUSE_EXCODE_MASK | ((exception as u32) << 2);
        self.cp0[CP0_STATUS] =
            (self.cp0[CP0_STATUS] & !0x3f) | ((self.cp0[CP0_STATUS] << 2) & 0x3f);
        self.delay_slot_branch_pc = None;
        self.pc = EXCEPTION_VECTOR;
        self.next_pc = EXCEPTION_VECTOR + 4;
        StepOutcome::Continue
    }
}

#[derive(Clone, Copy, Debug)]
enum Exception {
    Interrupt = 0,
    Syscall = 8,
    Breakpoint = 9,
    Overflow = 12,
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

fn fixed_cycle_cost(instruction: Option<u32>, outcome: StepOutcome) -> u64 {
    match (instruction, outcome) {
        (None, _) => 1,
        (_, StepOutcome::Halted) => 1,
        (Some(instruction), _) => instruction_cycle_cost(instruction),
    }
}

fn instruction_cycle_cost(instruction: u32) -> u64 {
    match instruction >> 26 {
        0x00 => match instruction & 0x3f {
            0x18 | 0x19 => 5,
            0x1a | 0x1b => 10,
            _ => 1,
        },
        0x20..=0x26 | 0x28..=0x2b | 0x2e => 2,
        _ => 1,
    }
}

fn optional_u32_json(value: Option<u32>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn optional_u32_hex_json(value: Option<u32>) -> String {
    value.map_or_else(|| "null".to_string(), |value| format!("\"0x{value:08x}\""))
}

fn load_word_left(bus: &Bus, address: u32, old_value: u32) -> u32 {
    let aligned = address & !3;
    let last = address & 3;
    let mut value = old_value;
    for byte in 0..=last {
        let shift = 24 - ((last - byte) * 8);
        value = (value & !(0xff << shift)) | ((bus.read_u8(aligned + byte) as u32) << shift);
    }
    value
}

fn load_word_right(bus: &Bus, address: u32, old_value: u32) -> u32 {
    let aligned = address & !3;
    let first = address & 3;
    let mut value = old_value;
    for byte in first..=3 {
        let shift = (byte - first) * 8;
        value = (value & !(0xff << shift)) | ((bus.read_u8(aligned + byte) as u32) << shift);
    }
    value
}

fn store_word_left(bus: &mut Bus, address: u32, value: u32) {
    let aligned = address & !3;
    let last = address & 3;
    for byte in 0..=last {
        let shift = 24 - ((last - byte) * 8);
        bus.write_u8(aligned + byte, (value >> shift) as u8);
    }
}

fn store_word_right(bus: &mut Bus, address: u32, value: u32) {
    let aligned = address & !3;
    let first = address & 3;
    for byte in first..=3 {
        let shift = (byte - first) * 8;
        bus.write_u8(aligned + byte, (value >> shift) as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::{CAUSE_BD, CAUSE_IP2, CP0_CAUSE, CP0_EPC, CP0_STATUS, Cpu, StepOutcome};
    use crate::native::bus::Bus;

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

    fn regimm(rs: u32, rt: u32, imm: i16) -> u32 {
        i_type(0x01, rs, rt, imm)
    }

    fn cop0_rfe() -> u32 {
        (0x10 << 26) | (0x10 << 21) | 0x10
    }

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
        assert_eq!(cpu.cp0[13], 9 << 2);
        assert_eq!(cpu.cp0[14], 0x1fc0_0004);
    }

    #[test]
    fn step_report_defines_single_instruction_boundary() {
        let rom = program(&[
            i_type(0x09, 0, 2, 42),   // addiu v0, zero, 42
            r_type(0, 0, 0, 0, 0x0d), // break
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x1fc0_0000);
        assert_eq!(report.end_pc, 0x1fc0_0004);
        assert_eq!(report.next_pc, 0x1fc0_0008);
        assert_eq!(report.instruction, Some(0x2402_002a));
        assert_eq!(report.cycles_before, 0);
        assert_eq!(report.cycles_after, 1);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[2], 42);
    }

    #[test]
    fn step_report_accounts_stable_instruction_cycle_costs() {
        let rom = program(&[
            i_type(0x23, 0, 9, 0),    // lw t1, 0(zero)
            r_type(8, 9, 0, 0, 0x18), // mult t0, t1
            r_type(8, 9, 0, 0, 0x1a), // div t0, t1
            r_type(0, 0, 0, 0, 0x0d), // break
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();
        cpu.regs[8] = 12;
        cpu.regs[9] = 3;

        let load = cpu.step_report(&mut bus);
        let multiply = cpu.step_report(&mut bus);
        let divide = cpu.step_report(&mut bus);

        assert_eq!(load.cycles_elapsed, 2);
        assert_eq!(multiply.cycles_elapsed, 5);
        assert_eq!(divide.cycles_elapsed, 10);
        assert_eq!(cpu.cycles, 17);
    }

    #[test]
    fn step_report_preserves_branch_delay_boundaries() {
        let rom = program(&[
            i_type(0x04, 0, 0, 2),   // beq zero, zero, +2
            i_type(0x09, 0, 9, 1),   // addiu t1, zero, 1 (delay slot)
            i_type(0x09, 0, 10, 99), // skipped when branch is taken
            i_type(0x09, 0, 11, 7),  // addiu t3, zero, 7
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        let branch = cpu.step_report(&mut bus);
        let delay = cpu.step_report(&mut bus);

        assert_eq!(branch.start_pc, 0x1fc0_0000);
        assert_eq!(branch.end_pc, 0x1fc0_0004);
        assert_eq!(branch.next_pc, 0x1fc0_000c);
        assert_eq!(delay.start_pc, 0x1fc0_0004);
        assert_eq!(delay.end_pc, 0x1fc0_000c);
        assert_eq!(delay.next_pc, 0x1fc0_0010);
        assert_eq!(cpu.regs[9], 1);
        assert_eq!(cpu.regs[10], 0);
    }

    #[test]
    fn repeated_instruction_stream_produces_identical_step_json() {
        let rom = program(&[
            i_type(0x09, 0, 2, 42),   // addiu v0, zero, 42
            i_type(0x04, 2, 2, 1),    // beq v0, v0, +1
            i_type(0x09, 0, 4, 7),    // addiu a0, zero, 7 (delay slot)
            r_type(0, 0, 0, 0, 0x0d), // break
        ]);

        fn run(rom: Vec<u8>) -> (Vec<String>, String) {
            let mut bus = Bus::new(rom, 2 * 1024 * 1024);
            let mut cpu = Cpu::default();
            let reports = (0..4)
                .map(|_| cpu.step_report(&mut bus).json())
                .collect::<Vec<_>>();
            (reports, cpu.json())
        }

        let first = run(rom.clone());
        let second = run(rom);

        assert_eq!(first, second);
        assert_eq!(
            first.1,
            "{\"pc\":2147483776,\"next_pc\":2147483780,\"cycles\":4,\"halted\":true,\"status\":0,\"cause\":36,\"epc\":532676620,\"r2\":42,\"r3\":0,\"r4\":7,\"r5\":0,\"r6\":0,\"r8\":0,\"r9\":0,\"r10\":0,\"r11\":0,\"r16\":0,\"r29\":0,\"r31\":0}"
        );
    }

    #[test]
    fn halted_step_report_is_idempotent_and_cycle_free() {
        let rom = program(&[r_type(0, 0, 0, 0, 0x0d)]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        let halt = cpu.step_report(&mut bus);
        let repeat = cpu.step_report(&mut bus);

        assert_eq!(halt.outcome, StepOutcome::Halted);
        assert_eq!(halt.cycles_elapsed, 1);
        assert_eq!(repeat.outcome, StepOutcome::Halted);
        assert_eq!(repeat.instruction, None);
        assert_eq!(repeat.cycles_before, halt.cycles_after);
        assert_eq!(repeat.cycles_after, halt.cycles_after);
        assert_eq!(repeat.cycles_elapsed, 0);
    }

    #[test]
    fn executes_store_and_load_widths() {
        let rom = vec![
            0xef, 0xbe, 0x08, 0x24, // addiu t0, zero, -16657
            0x00, 0x00, 0x08, 0xa0, // sb t0, 0(zero)
            0x00, 0x00, 0x09, 0x90, // lbu t1, 0(zero)
            0x0d, 0x00, 0x00, 0x00, // break
        ];
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        cpu.step(&mut bus);
        cpu.step(&mut bus);
        cpu.step(&mut bus);
        assert_eq!(cpu.regs[9], 0xef);
    }

    #[test]
    fn executes_cp0_round_trip() {
        let rom = vec![
            0x34, 0x12, 0x08, 0x24, // addiu t0, zero, 0x1234
            0x00, 0x60, 0x88, 0x40, // mtc0 t0, r12
            0x00, 0x60, 0x0c, 0x40, // mfc0 t4, r12
            0x0d, 0x00, 0x00, 0x00, // break
        ];
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        cpu.step(&mut bus);
        cpu.step(&mut bus);
        cpu.step(&mut bus);
        assert_eq!(cpu.regs[12], 0x1234);
    }

    #[test]
    fn executes_cop2_register_transfers_and_memory_accesses() {
        let rom = program(&[
            i_type(0x0f, 0, 8, 0x1234),                          // lui t0, 0x1234
            i_type(0x0d, 8, 8, 0x5678),                          // ori t0, t0, 0x5678
            (0x12 << 26) | (0x04 << 21) | (8 << 16) | (2 << 11), // mtc2 t0, r2
            (0x3a << 26) | (2 << 16),                            // swc2 r2, 0(zero)
            (0x32 << 26) | (3 << 16),                            // lwc2 r3, 0(zero)
            (0x12 << 26) | (9 << 16) | (3 << 11),                // mfc2 t1, r3
            (0x12 << 26) | (0x10 << 21) | 0x01,                  // rtps placeholder
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        for _ in 0..7 {
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        }

        assert_eq!(bus.read_u32(0), 0x1234_5678);
        assert_eq!(cpu.cop2_data[3], 0x1234_5678);
        assert_eq!(cpu.regs[9], 0x1234_5678);
        assert_eq!(cpu.cop2_data[31], 0);
    }

    #[test]
    fn executes_regimm_link_branch_with_delay_slot() {
        let rom = program(&[
            i_type(0x09, 0, 8, -1),   // addiu t0, zero, -1
            regimm(8, 0x10, 2),       // bltzal t0, +2
            i_type(0x09, 0, 9, 1),    // addiu t1, zero, 1 (delay slot)
            i_type(0x09, 0, 10, 99),  // skipped when branch is taken
            i_type(0x09, 0, 11, 7),   // addiu t3, zero, 7
            r_type(0, 0, 0, 0, 0x0d), // break
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.regs[31], 0x1fc0_000c);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);

        assert_eq!(cpu.regs[9], 1);
        assert_eq!(cpu.regs[10], 0);
        assert_eq!(cpu.regs[11], 7);
    }

    #[test]
    fn traps_signed_arithmetic_overflow_deterministically() {
        let rom = program(&[
            i_type(0x0f, 0, 8, 0x7fff), // lui t0, 0x7fff
            i_type(0x0d, 8, 8, -1),     // ori t0, t0, 0xffff
            i_type(0x08, 8, 9, 1),      // addi t1, t0, 1
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);

        assert_eq!(cpu.regs[9], 0);
        assert_eq!(cpu.cp0[13], 12 << 2);
        assert_eq!(cpu.cp0[14], 0x1fc0_0008);
        assert_eq!(cpu.pc, 0x8000_0080);
        assert_eq!(cpu.next_pc, 0x8000_0084);
    }

    #[test]
    fn executes_unaligned_word_load_store_pairs() {
        let rom = program(&[
            i_type(0x0f, 0, 8, 0x1122), // lui t0, 0x1122
            i_type(0x0d, 8, 8, 0x3344), // ori t0, t0, 0x3344
            i_type(0x2a, 0, 8, 1),      // swl t0, 1(zero)
            i_type(0x2e, 0, 8, 2),      // swr t0, 2(zero)
            i_type(0x0f, 0, 9, -21829), // lui t1, 0xaabb
            i_type(0x0d, 9, 9, -13091), // ori t1, t1, 0xccdd
            i_type(0x22, 0, 9, 1),      // lwl t1, 1(zero)
            i_type(0x26, 0, 9, 2),      // lwr t1, 2(zero)
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        for _ in 0..4 {
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        }

        assert_eq!(bus.read_u8(0), 0x22);
        assert_eq!(bus.read_u8(1), 0x11);
        assert_eq!(bus.read_u8(2), 0x44);
        assert_eq!(bus.read_u8(3), 0x33);

        for _ in 0..4 {
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        }

        assert_eq!(cpu.regs[9], 0x1122_3344);
    }

    #[test]
    fn syscall_records_exception_vector_state() {
        let rom = program(&[r_type(0, 0, 0, 0, 0x0c)]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);

        assert_eq!(cpu.cp0[13], 8 << 2);
        assert_eq!(cpu.cp0[14], 0x1fc0_0000);
        assert_eq!(cpu.pc, 0x8000_0080);
        assert_eq!(cpu.next_pc, 0x8000_0084);
    }

    #[test]
    fn ignores_masked_external_interrupts() {
        let rom = program(&[i_type(0x09, 0, 8, 7)]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();
        cpu.cp0[CP0_STATUS] = 1 | CAUSE_IP2;
        bus.io.irq.status = 1;
        bus.io.irq.mask = 0;

        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);

        assert_eq!(cpu.regs[8], 7);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, 0);
        assert_eq!(cpu.pc, 0x1fc0_0004);
    }

    #[test]
    fn takes_enabled_external_interrupt_and_preserves_pending_ip() {
        let rom = program(&[i_type(0x09, 0, 8, 7)]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();
        cpu.cp0[CP0_STATUS] = 1 | CAUSE_IP2;
        bus.io.irq.status = 1;
        bus.io.irq.mask = 1;

        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);

        assert_eq!(cpu.regs[8], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], CAUSE_IP2);
        assert_eq!(cpu.cp0[CP0_EPC], 0x1fc0_0000);
        assert_eq!(cpu.cp0[CP0_STATUS], CAUSE_IP2 | 0x04);
        assert_eq!(cpu.pc, 0x8000_0080);
        assert_eq!(cpu.next_pc, 0x8000_0084);
    }

    #[test]
    fn rfe_restores_status_interrupt_enable_stack() {
        let rom = program(&[i_type(0x09, 0, 8, 7)]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();
        cpu.cp0[CP0_STATUS] = 1 | CAUSE_IP2;
        bus.io.irq.status = 1;
        bus.io.irq.mask = 1;

        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        bus.io.irq.status = 0;
        bus.write_u32(0x8000_0080, cop0_rfe());

        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);

        assert_eq!(cpu.cp0[CP0_STATUS], 1 | CAUSE_IP2);
        assert_eq!(cpu.pc, 0x8000_0084);
    }

    #[test]
    fn delay_slot_exception_sets_bd_and_epc_to_branch_pc() {
        let rom = program(&[
            i_type(0x0f, 0, 8, 0x7fff), // lui t0, 0x7fff
            i_type(0x0d, 8, 8, -1),     // ori t0, t0, 0xffff
            i_type(0x04, 0, 0, 1),      // beq zero, zero, +1
            i_type(0x08, 8, 9, 1),      // addi t1, t0, 1 (delay slot)
            i_type(0x09, 0, 10, 1),     // addiu t2, zero, 1
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);

        assert_eq!(cpu.regs[9], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], CAUSE_BD | (12 << 2));
        assert_eq!(cpu.cp0[CP0_EPC], 0x1fc0_0008);
        assert_eq!(cpu.pc, 0x8000_0080);
        assert_eq!(cpu.next_pc, 0x8000_0084);
    }
}
