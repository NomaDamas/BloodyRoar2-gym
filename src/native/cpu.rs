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
const GTE_FRACTIONAL_BITS: u32 = 12;
const GTE_FLAG_ERROR: u32 = 1 << 31;
const GTE_FLAG_ERROR_BITS: u32 = 0x7f87_e000;
const GTE_FLAG_DIVIDE_OVERFLOW: u32 = 1 << 17;
const GTE_FLAG_SZ_OTZ_SATURATED: u32 = 1 << 18;
const GTE_FLAG_IR0_SATURATED: u32 = 1 << 12;
const GTE_FLAG_SX2_SATURATED: u32 = 1 << 14;
const GTE_FLAG_SY2_SATURATED: u32 = 1 << 13;

#[derive(Clone, Debug)]
pub struct Cpu {
    pub regs: [u32; 32],
    pub cp0: [u32; 32],
    pub cop2_data: [u32; 32],
    pub cop2_control: [u32; 32],
    pub gte_command_counts: [u64; 64],
    gte_projected_vertices: u64,
    gte_zero_depth_vertices: u64,
    gte_projection_saturated_vertices: u64,
    gte_screen_outlier_vertices: u64,
    gte_screen_min_x: i16,
    gte_screen_max_x: i16,
    gte_screen_min_y: i16,
    gte_screen_max_y: i16,
    gte_depth_min: u16,
    gte_depth_max: u16,
    gte_otz_min: u16,
    gte_otz_max: u16,
    gte_mvmva_mx_counts: [u64; 4],
    gte_mvmva_v_counts: [u64; 4],
    gte_mvmva_cv_counts: [u64; 4],
    gte_mvmva_cv2_special_cases: u64,
    gte_nclip_positive: u64,
    gte_nclip_negative: u64,
    gte_nclip_zero: u64,
    pub hi: u32,
    pub lo: u32,
    pub pc: u32,
    pub next_pc: u32,
    pub cycles: u64,
    pub halted: bool,
    pending_load: Option<(usize, u32)>,
    load_commit_register: Option<usize>,
    load_commit_value: Option<u32>,
    load_commit_cancelled: bool,
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
            gte_command_counts: [0; 64],
            gte_projected_vertices: 0,
            gte_zero_depth_vertices: 0,
            gte_projection_saturated_vertices: 0,
            gte_screen_outlier_vertices: 0,
            gte_screen_min_x: i16::MAX,
            gte_screen_max_x: i16::MIN,
            gte_screen_min_y: i16::MAX,
            gte_screen_max_y: i16::MIN,
            gte_depth_min: u16::MAX,
            gte_depth_max: 0,
            gte_otz_min: u16::MAX,
            gte_otz_max: 0,
            gte_mvmva_mx_counts: [0; 4],
            gte_mvmva_v_counts: [0; 4],
            gte_mvmva_cv_counts: [0; 4],
            gte_mvmva_cv2_special_cases: 0,
            gte_nclip_positive: 0,
            gte_nclip_negative: 0,
            gte_nclip_zero: 0,
            hi: 0,
            lo: 0,
            pc: 0x1fc0_0000,
            next_pc: 0x1fc0_0004,
            cycles: 0,
            halted: false,
            pending_load: None,
            load_commit_register: None,
            load_commit_value: None,
            load_commit_cancelled: false,
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

        let delayed_load = self.pending_load.take();
        self.load_commit_register = delayed_load.map(|(register, _)| register);
        self.load_commit_value = delayed_load.map(|(_, value)| value);
        self.load_commit_cancelled = false;

        let outcome = self.execute(instruction, current_pc, delay_slot_branch_pc, bus);
        self.commit_delayed_load(delayed_load);
        self.cycles += fixed_cycle_cost(Some(instruction), outcome).saturating_sub(1);
        self.regs[0] = 0;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;
        let report = self.step_report_from(start_pc, Some(instruction), cycles_before, outcome);
        bus.tick(report.cycles_elapsed);
        bus.clear_trace_context();
        report
    }

    pub fn json(&self) -> String {
        format!(
            "{{\"pc\":{},\"next_pc\":{},\"cycles\":{},\"halted\":{},\"status\":{},\"cause\":{},\"epc\":{},\"r2\":{},\"r3\":{},\"r4\":{},\"r5\":{},\"r6\":{},\"r8\":{},\"r9\":{},\"r10\":{},\"r11\":{},\"r16\":{},\"r29\":{},\"r31\":{},\"gte_command_counts\":[{}]}}",
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
            self.regs[31],
            self.gte_command_counts_json()
        )
    }

    pub fn gte_json(&self) -> String {
        format!(
            "{{\"projected_vertices\":{},\"zero_depth_vertices\":{},\"projection_saturated_vertices\":{},\"screen_outlier_vertices\":{},\"screen_min_x\":{},\"screen_max_x\":{},\"screen_min_y\":{},\"screen_max_y\":{},\"depth_min\":{},\"depth_max\":{},\"otz_min\":{},\"otz_max\":{},\"mvmva_mx_counts\":[{}],\"mvmva_v_counts\":[{}],\"mvmva_cv_counts\":[{}],\"mvmva_cv2_special_cases\":{},\"nclip_positive\":{},\"nclip_negative\":{},\"nclip_zero\":{},\"sxy0\":{},\"sxy1\":{},\"sxy2\":{},\"sz1\":{},\"sz2\":{},\"sz3\":{},\"otz\":{},\"ir0\":{},\"ir1\":{},\"ir2\":{},\"ir3\":{},\"mac0\":{},\"mac1\":{},\"mac2\":{},\"mac3\":{},\"flag\":{},\"lzcr\":{},\"ofx\":{},\"ofy\":{},\"h\":{},\"dqa\":{},\"dqb\":{},\"zsf3\":{},\"zsf4\":{}}}",
            self.gte_projected_vertices,
            self.gte_zero_depth_vertices,
            self.gte_projection_saturated_vertices,
            self.gte_screen_outlier_vertices,
            optional_i16_sample(self.gte_projected_vertices, self.gte_screen_min_x),
            optional_i16_sample(self.gte_projected_vertices, self.gte_screen_max_x),
            optional_i16_sample(self.gte_projected_vertices, self.gte_screen_min_y),
            optional_i16_sample(self.gte_projected_vertices, self.gte_screen_max_y),
            optional_u16_sample(self.gte_projected_vertices, self.gte_depth_min),
            optional_u16_sample(self.gte_projected_vertices, self.gte_depth_max),
            optional_u16_sample(
                self.gte_command_counts[0x2d] + self.gte_command_counts[0x2e],
                self.gte_otz_min
            ),
            optional_u16_sample(
                self.gte_command_counts[0x2d] + self.gte_command_counts[0x2e],
                self.gte_otz_max
            ),
            u64_array_json(&self.gte_mvmva_mx_counts),
            u64_array_json(&self.gte_mvmva_v_counts),
            u64_array_json(&self.gte_mvmva_cv_counts),
            self.gte_mvmva_cv2_special_cases,
            self.gte_nclip_positive,
            self.gte_nclip_negative,
            self.gte_nclip_zero,
            self.cop2_data[12],
            self.cop2_data[13],
            self.cop2_data[14],
            self.cop2_data[17],
            self.cop2_data[18],
            self.cop2_data[19],
            self.cop2_data[7],
            self.cop2_data[8],
            self.cop2_data[9],
            self.cop2_data[10],
            self.cop2_data[11],
            self.cop2_data[24],
            self.cop2_data[25],
            self.cop2_data[26],
            self.cop2_data[27],
            self.cop2_control[31],
            self.cop2_data[31],
            self.cop2_control[24],
            self.cop2_control[25],
            self.cop2_control[26],
            self.cop2_control[27],
            self.cop2_control[28],
            self.cop2_control[29],
            self.cop2_control[30]
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
                self.set_reg(31, self.next_pc);
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
                    Some(value) => self.set_reg(rt(instruction), value as u32),
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
                self.set_reg(
                    rt(instruction),
                    self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction)),
                );
                StepOutcome::Continue
            }
            0x0a => {
                self.set_reg(
                    rt(instruction),
                    ((self.regs[rs(instruction)] as i32) < (sign_extend_16(instruction) as i32))
                        as u32,
                );
                StepOutcome::Continue
            }
            0x0b => {
                self.set_reg(
                    rt(instruction),
                    (self.regs[rs(instruction)] < sign_extend_16(instruction)) as u32,
                );
                StepOutcome::Continue
            }
            0x0c => {
                self.set_reg(
                    rt(instruction),
                    self.regs[rs(instruction)] & (instruction & 0xffff),
                );
                StepOutcome::Continue
            }
            0x0d => {
                self.set_reg(
                    rt(instruction),
                    self.regs[rs(instruction)] | (instruction & 0xffff),
                );
                StepOutcome::Continue
            }
            0x0e => {
                self.set_reg(
                    rt(instruction),
                    self.regs[rs(instruction)] ^ (instruction & 0xffff),
                );
                StepOutcome::Continue
            }
            0x0f => {
                self.set_reg(rt(instruction), (instruction & 0xffff) << 16);
                StepOutcome::Continue
            }
            0x10 => self.execute_cop0(instruction, bus),
            0x12 => self.execute_cop2(instruction),
            0x20 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.schedule_load(rt(instruction), (bus.read_u8(address) as i8) as i32 as u32);
                StepOutcome::Continue
            }
            0x21 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.schedule_load(
                    rt(instruction),
                    (bus.read_u16(address) as i16) as i32 as u32,
                );
                StepOutcome::Continue
            }
            0x22 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.schedule_load(
                    rt(instruction),
                    load_word_left(bus, address, self.load_merge_value(rt(instruction))),
                );
                StepOutcome::Continue
            }
            0x23 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.schedule_load(rt(instruction), bus.read_u32(address));
                StepOutcome::Continue
            }
            0x24 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.schedule_load(rt(instruction), bus.read_u8(address) as u32);
                StepOutcome::Continue
            }
            0x25 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.schedule_load(rt(instruction), bus.read_u16(address) as u32);
                StepOutcome::Continue
            }
            0x26 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                self.schedule_load(
                    rt(instruction),
                    load_word_right(bus, address, self.load_merge_value(rt(instruction))),
                );
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
                self.gte_data_write(rt(instruction), bus.read_u32(address));
                StepOutcome::Continue
            }
            0x3a => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                bus.write_u32(address, self.gte_data_read(rt(instruction)));
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
                    self.set_reg(
                        rd(instruction),
                        self.regs[rt(instruction)] << shamt(instruction),
                    );
                }
                StepOutcome::Continue
            }
            0x04 => {
                self.set_reg(
                    rd(instruction),
                    self.regs[rt(instruction)] << (self.regs[rs(instruction)] & 0x1f),
                );
                StepOutcome::Continue
            }
            0x02 => {
                self.set_reg(
                    rd(instruction),
                    self.regs[rt(instruction)] >> shamt(instruction),
                );
                StepOutcome::Continue
            }
            0x03 => {
                self.set_reg(
                    rd(instruction),
                    ((self.regs[rt(instruction)] as i32) >> shamt(instruction)) as u32,
                );
                StepOutcome::Continue
            }
            0x06 => {
                self.set_reg(
                    rd(instruction),
                    self.regs[rt(instruction)] >> (self.regs[rs(instruction)] & 0x1f),
                );
                StepOutcome::Continue
            }
            0x07 => {
                self.set_reg(
                    rd(instruction),
                    ((self.regs[rt(instruction)] as i32) >> (self.regs[rs(instruction)] & 0x1f))
                        as u32,
                );
                StepOutcome::Continue
            }
            0x08 => {
                self.next_pc = self.regs[rs(instruction)];
                self.delay_slot_branch_pc = Some(current_pc);
                StepOutcome::Continue
            }
            0x09 => {
                self.set_reg(rd(instruction), self.next_pc);
                self.next_pc = self.regs[rs(instruction)];
                self.delay_slot_branch_pc = Some(current_pc);
                StepOutcome::Continue
            }
            0x10 => {
                self.set_reg(rd(instruction), self.hi);
                StepOutcome::Continue
            }
            0x11 => {
                self.hi = self.regs[rs(instruction)];
                StepOutcome::Continue
            }
            0x12 => {
                self.set_reg(rd(instruction), self.lo);
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
                    Some(value) => self.set_reg(rd(instruction), value as u32),
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
                self.set_reg(
                    rd(instruction),
                    self.regs[rs(instruction)].wrapping_add(self.regs[rt(instruction)]),
                );
                StepOutcome::Continue
            }
            0x22 => {
                match (self.regs[rs(instruction)] as i32)
                    .checked_sub(self.regs[rt(instruction)] as i32)
                {
                    Some(value) => self.set_reg(rd(instruction), value as u32),
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
                self.set_reg(
                    rd(instruction),
                    self.regs[rs(instruction)].wrapping_sub(self.regs[rt(instruction)]),
                );
                StepOutcome::Continue
            }
            0x24 => {
                self.set_reg(
                    rd(instruction),
                    self.regs[rs(instruction)] & self.regs[rt(instruction)],
                );
                StepOutcome::Continue
            }
            0x25 => {
                self.set_reg(
                    rd(instruction),
                    self.regs[rs(instruction)] | self.regs[rt(instruction)],
                );
                StepOutcome::Continue
            }
            0x26 => {
                self.set_reg(
                    rd(instruction),
                    self.regs[rs(instruction)] ^ self.regs[rt(instruction)],
                );
                StepOutcome::Continue
            }
            0x27 => {
                self.set_reg(
                    rd(instruction),
                    !(self.regs[rs(instruction)] | self.regs[rt(instruction)]),
                );
                StepOutcome::Continue
            }
            0x2a => {
                self.set_reg(
                    rd(instruction),
                    ((self.regs[rs(instruction)] as i32) < (self.regs[rt(instruction)] as i32))
                        as u32,
                );
                StepOutcome::Continue
            }
            0x2b => {
                self.set_reg(
                    rd(instruction),
                    (self.regs[rs(instruction)] < self.regs[rt(instruction)]) as u32,
                );
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
                self.set_reg(31, self.next_pc);
                if (self.regs[rs(instruction)] as i32) < 0 {
                    self.next_pc = branch_target(self.pc, instruction);
                }
                self.delay_slot_branch_pc = Some(current_pc);
                StepOutcome::Continue
            }
            0x11 => {
                self.set_reg(31, self.next_pc);
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
                self.set_reg(rt(instruction), self.cp0[rd(instruction)]);
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
                self.schedule_load(rt(instruction), self.gte_data_read(rd(instruction)));
                StepOutcome::Continue
            }
            0x02 => {
                self.schedule_load(rt(instruction), self.cop2_control[rd(instruction)]);
                StepOutcome::Continue
            }
            0x04 => {
                self.gte_data_write(rd(instruction), self.regs[rt(instruction)]);
                StepOutcome::Continue
            }
            0x06 => {
                self.gte_control_write(rd(instruction), self.regs[rt(instruction)]);
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
        self.gte_command_counts[command as usize] =
            self.gte_command_counts[command as usize].saturating_add(1);
        self.begin_gte_command();
        match command {
            0x01 => self.execute_gte_rtps(instruction),
            0x06 => self.execute_gte_nclip(),
            0x12 => self.execute_gte_mvmva(instruction),
            0x1b => self.execute_gte_nccs(instruction),
            0x1c => self.execute_gte_cc(instruction),
            0x28 => self.execute_gte_sqr(instruction),
            0x2d => self.execute_gte_avsz3(),
            0x2e => self.execute_gte_avsz4(),
            0x30 => self.execute_gte_rtpt(instruction),
            0x3d => self.execute_gte_gpf(instruction),
            0x3f => self.execute_gte_ncct(instruction),
            _ => {}
        }
        self.finish_gte_flag();
    }

    fn execute_gte_rtps(&mut self, instruction: u32) {
        self.transform_gte_vertex(0, gte_shift(instruction), gte_lm(instruction));
    }

    fn execute_gte_nclip(&mut self) {
        let (sx0, sy0) = gte_sxy(self.cop2_data[12]);
        let (sx1, sy1) = gte_sxy(self.cop2_data[13]);
        let (sx2, sy2) = gte_sxy(self.cop2_data[14]);
        let mut mac0 = sx0 as i64 * (sy1 as i64 - sy2 as i64)
            + sx1 as i64 * (sy2 as i64 - sy0 as i64)
            + sx2 as i64 * (sy0 as i64 - sy1 as i64);
        if invert_gte_nclip() {
            mac0 = -mac0;
        }
        self.cop2_data[24] = (mac0 as i32) as u32;
        match mac0.cmp(&0) {
            std::cmp::Ordering::Greater => {
                self.gte_nclip_positive = self.gte_nclip_positive.saturating_add(1);
            }
            std::cmp::Ordering::Less => {
                self.gte_nclip_negative = self.gte_nclip_negative.saturating_add(1);
            }
            std::cmp::Ordering::Equal => {
                self.gte_nclip_zero = self.gte_nclip_zero.saturating_add(1);
            }
        }
    }

    fn execute_gte_mvmva(&mut self, instruction: u32) {
        let mx = gte_matrix_select(instruction);
        let v = gte_vector_select(instruction);
        let cv = gte_translation_select(instruction);
        self.gte_mvmva_mx_counts[mx as usize] =
            self.gte_mvmva_mx_counts[mx as usize].saturating_add(1);
        self.gte_mvmva_v_counts[v as usize] = self.gte_mvmva_v_counts[v as usize].saturating_add(1);
        self.gte_mvmva_cv_counts[cv as usize] =
            self.gte_mvmva_cv_counts[cv as usize].saturating_add(1);
        let matrix = self.gte_matrix(mx);
        let vector = self.gte_vector(v);
        let translation = self.gte_translation(cv);
        let shift = gte_shift(instruction);
        let lm = gte_lm(instruction);

        if cv == 2 {
            self.gte_mvmva_cv2_special_cases = self.gte_mvmva_cv2_special_cases.saturating_add(1);
            self.execute_gte_mvmva_cv2_bug(matrix, vector, translation, shift, lm);
            return;
        }

        for index in 0..3 {
            let dot = matrix[index][0] as i64 * vector[0] as i64
                + matrix[index][1] as i64 * vector[1] as i64
                + matrix[index][2] as i64 * vector[2] as i64;
            let mac = ((translation[index] as i64) << 12).saturating_add(dot);
            self.set_gte_mac_ir(index + 1, mac, shift, lm);
        }
    }

    fn execute_gte_mvmva_cv2_bug(
        &mut self,
        matrix: [[i16; 3]; 3],
        vector: [i16; 3],
        translation: [i32; 3],
        shift: u32,
        lm: bool,
    ) {
        for index in 0..3 {
            let yz_mac = matrix[index][1] as i64 * vector[1] as i64
                + matrix[index][2] as i64 * vector[2] as i64;
            self.set_gte_mac_ir(index + 1, yz_mac, shift, lm);

            let x_mac = ((translation[index] as i64) << 12)
                .saturating_add(matrix[index][0] as i64 * vector[0] as i64);
            self.set_gte_mac_ir(index + 1, x_mac, shift, lm);
        }
    }

    fn execute_gte_sqr(&mut self, instruction: u32) {
        let shift = gte_shift(instruction);
        for index in 1..=3 {
            let value = self.cop2_data[index + 8] as i16 as i64;
            self.set_gte_mac_ir(
                index,
                value.saturating_mul(value),
                shift,
                gte_lm(instruction),
            );
        }
    }

    fn execute_gte_gpf(&mut self, instruction: u32) {
        let shift = gte_shift(instruction);
        let ir0 = self.cop2_data[8] as i16 as i64;
        for index in 1..=3 {
            let value = self.cop2_data[index + 8] as i16 as i64;
            self.set_gte_mac_ir(index, ir0.saturating_mul(value), shift, gte_lm(instruction));
        }
        self.update_gte_rgb_fifo_from_ir();
    }

    fn execute_gte_nccs(&mut self, instruction: u32) {
        let shift = gte_shift(instruction);
        self.gte_normal_color(0, shift, true);
        self.update_gte_rgb_fifo_from_ir();
    }

    fn execute_gte_cc(&mut self, instruction: u32) {
        self.gte_color_color(gte_shift(instruction), gte_lm(instruction));
        self.update_gte_rgb_fifo_from_ir();
    }

    fn execute_gte_ncct(&mut self, instruction: u32) {
        let shift = gte_shift(instruction);
        for vector_index in 0..3 {
            self.gte_normal_color(vector_index, shift, true);
            self.update_gte_rgb_fifo_from_ir();
        }
    }

    fn gte_normal_color(&mut self, vector_index: u32, shift: u32, lm: bool) {
        let normal = self.gte_vector(vector_index);
        let light = self.gte_matrix(1);
        let background = self.gte_translation(1);

        for index in 0..3 {
            let dot = light[index][0] as i64 * normal[0] as i64
                + light[index][1] as i64 * normal[1] as i64
                + light[index][2] as i64 * normal[2] as i64;
            let mac = ((background[index] as i64) << 12).saturating_add(dot);
            self.set_gte_mac_ir(index + 1, mac, shift, lm);
        }

        self.gte_color_color(shift, lm);
    }

    fn gte_color_color(&mut self, shift: u32, lm: bool) {
        let color = self.gte_matrix(2);
        let far_color = self.gte_translation(2);
        let vector = [
            self.cop2_data[9] as i16,
            self.cop2_data[10] as i16,
            self.cop2_data[11] as i16,
        ];
        for index in 0..3 {
            let dot = color[index][0] as i64 * vector[0] as i64
                + color[index][1] as i64 * vector[1] as i64
                + color[index][2] as i64 * vector[2] as i64;
            let mac = ((far_color[index] as i64) << 12).saturating_add(dot);
            self.set_gte_mac_ir(index + 1, mac, shift, lm);
        }
    }

    fn execute_gte_avsz3(&mut self) {
        let sum = self.cop2_data[17] as u16 as i64
            + self.cop2_data[18] as u16 as i64
            + self.cop2_data[19] as u16 as i64;
        self.set_gte_average_z(sum, self.cop2_control[29] as i16 as i64);
    }

    fn execute_gte_avsz4(&mut self) {
        let sum = self.cop2_data[16] as u16 as i64
            + self.cop2_data[17] as u16 as i64
            + self.cop2_data[18] as u16 as i64
            + self.cop2_data[19] as u16 as i64;
        self.set_gte_average_z(sum, self.cop2_control[30] as i16 as i64);
    }

    fn execute_gte_rtpt(&mut self, instruction: u32) {
        let shift = gte_shift(instruction);
        let lm = gte_lm(instruction);
        for vector_index in 0..3 {
            self.transform_gte_vertex(vector_index, shift, lm);
        }
    }

    fn begin_gte_command(&mut self) {
        self.cop2_control[31] = 0;
    }

    fn finish_gte_flag(&mut self) {
        if self.cop2_control[31] & GTE_FLAG_ERROR_BITS != 0 {
            self.cop2_control[31] |= GTE_FLAG_ERROR;
        } else {
            self.cop2_control[31] &= !GTE_FLAG_ERROR;
        }
    }

    fn set_gte_flag(&mut self, flag: u32) {
        self.cop2_control[31] |= flag;
    }

    fn gte_control_write(&mut self, register: usize, value: u32) {
        self.cop2_control[register] = value;
        if register == 31 {
            self.finish_gte_flag();
        }
    }

    fn gte_data_read(&self, register: usize) -> u32 {
        match register {
            1 | 3 | 5 | 8 | 9 | 10 | 11 => self.cop2_data[register] as i16 as i32 as u32,
            7 | 16 | 17 | 18 | 19 => self.cop2_data[register] & 0xffff,
            28 | 29 => gte_irgb(self.cop2_data[9], self.cop2_data[10], self.cop2_data[11]),
            _ => self.cop2_data[register],
        }
    }

    fn gte_data_write(&mut self, register: usize, value: u32) {
        match register {
            1 | 3 | 5 | 7 | 8 | 9 | 10 | 11 | 16 | 17 | 18 | 19 => {
                self.cop2_data[register] = value & 0xffff;
            }
            15 => {
                self.cop2_data[12] = self.cop2_data[13];
                self.cop2_data[13] = self.cop2_data[14];
                self.cop2_data[14] = value;
                self.cop2_data[15] = value;
            }
            28 => {
                self.cop2_data[9] = ((value & 0x1f) << 7) as i16 as u16 as u32;
                self.cop2_data[10] = (((value >> 5) & 0x1f) << 7) as i16 as u16 as u32;
                self.cop2_data[11] = (((value >> 10) & 0x1f) << 7) as i16 as u16 as u32;
                self.cop2_data[register] = value;
            }
            30 => {
                self.cop2_data[30] = value;
                self.cop2_data[31] = gte_leading_zero_count(value);
            }
            _ => self.cop2_data[register] = value,
        }
    }

    fn set_gte_mac_ir(&mut self, index: usize, mac: i64, shift: u32, lm: bool) {
        let shifted = mac >> shift;
        self.cop2_data[24 + index] = (shifted as i32) as u32;
        if gte_ir_saturated(shifted, lm) {
            self.set_gte_flag(gte_ir_saturation_flag(index));
        }
        self.cop2_data[8 + index] = clamp_gte_ir(shifted, lm) as i16 as u16 as u32;
    }

    fn set_gte_rt_mac_ir(&mut self, index: usize, mac: i64, shift: u32, lm: bool) {
        let shifted = mac >> shift;
        self.cop2_data[24 + index] = (shifted as i32) as u32;
        let flag_value = if index == 3 { mac >> 12 } else { shifted };
        if gte_ir_saturated(flag_value, lm) {
            self.set_gte_flag(gte_ir_saturation_flag(index));
        }
        self.cop2_data[8 + index] = clamp_gte_ir(shifted, lm) as i16 as u16 as u32;
    }

    fn gte_matrix(&self, select: u32) -> [[i16; 3]; 3] {
        match select {
            0 => packed_gte_matrix(&self.cop2_control, 0),
            1 => packed_gte_matrix(&self.cop2_control, 8),
            2 => packed_gte_matrix(&self.cop2_control, 16),
            _ => {
                let r = (self.cop2_data[6] & 0xff) as i16;
                let ir0 = self.cop2_data[8] as i16;
                let r13 = low_i16(self.cop2_control[1]);
                let r22 = low_i16(self.cop2_control[2]);
                [
                    [r.wrapping_neg().wrapping_shl(4), r.wrapping_shl(4), ir0],
                    [r13, r13, r13],
                    [r22, r22, r22],
                ]
            }
        }
    }

    fn gte_vector(&self, select: u32) -> [i16; 3] {
        match select {
            0 => packed_gte_vector(self.cop2_data[0], self.cop2_data[1]),
            1 => packed_gte_vector(self.cop2_data[2], self.cop2_data[3]),
            2 => packed_gte_vector(self.cop2_data[4], self.cop2_data[5]),
            _ => [
                self.cop2_data[9] as i16,
                self.cop2_data[10] as i16,
                self.cop2_data[11] as i16,
            ],
        }
    }

    fn gte_translation(&self, select: u32) -> [i32; 3] {
        let base = match select {
            0 => 5,
            1 => 13,
            2 => 21,
            _ => return [0, 0, 0],
        };
        [
            self.cop2_control[base] as i32,
            self.cop2_control[base + 1] as i32,
            self.cop2_control[base + 2] as i32,
        ]
    }

    fn update_gte_rgb_fifo_from_ir(&mut self) {
        self.cop2_data[20] = self.cop2_data[21];
        self.cop2_data[21] = self.cop2_data[22];
        self.cop2_data[22] = gte_rgb_from_ir(
            self.cop2_data[9],
            self.cop2_data[10],
            self.cop2_data[11],
            self.cop2_data[6],
        );
    }

    fn transform_gte_vertex(&mut self, vector_index: u32, shift: u32, lm: bool) {
        let matrix = self.gte_matrix(0);
        let vector = self.gte_vector(vector_index);
        let translation = self.gte_translation(0);
        let mut macs = [0_i64; 3];

        for index in 0..3 {
            let dot = matrix[index][0] as i64 * vector[0] as i64
                + matrix[index][1] as i64 * vector[1] as i64
                + matrix[index][2] as i64 * vector[2] as i64;
            let mac = ((translation[index] as i64) << 12).saturating_add(dot);
            macs[index] = mac;
            self.set_gte_rt_mac_ir(index + 1, mac, shift, lm);
        }

        self.push_gte_screen_fifo(macs[2]);
    }

    fn push_gte_screen_fifo(&mut self, mac3: i64) {
        let (depth, depth_saturated) = clamp_gte_depth(mac3 >> GTE_FRACTIONAL_BITS);
        if depth_saturated {
            self.set_gte_flag(GTE_FLAG_SZ_OTZ_SATURATED);
        }
        let (projection_factor, projection_saturated) =
            gte_projection_factor(gte_projection_plane(self.cop2_control[26]), depth);
        let (sx, sx_saturated) = project_gte_screen_component(
            self.cop2_control[24],
            self.cop2_data[9] as i16 as i64,
            projection_factor,
        );
        let (sy, sy_saturated) = project_gte_screen_component(
            self.cop2_control[25],
            self.cop2_data[10] as i16 as i64,
            projection_factor,
        );
        self.gte_projected_vertices = self.gte_projected_vertices.saturating_add(1);
        if depth == 0 {
            self.gte_zero_depth_vertices = self.gte_zero_depth_vertices.saturating_add(1);
        }
        self.gte_depth_min = self.gte_depth_min.min(depth);
        self.gte_depth_max = self.gte_depth_max.max(depth);
        if projection_saturated {
            self.gte_projection_saturated_vertices =
                self.gte_projection_saturated_vertices.saturating_add(1);
            self.set_gte_flag(GTE_FLAG_DIVIDE_OVERFLOW);
        }
        self.set_gte_screen_saturation_flags(sx_saturated, sy_saturated);
        if gte_screen_outlier(sx, sy) {
            self.gte_screen_outlier_vertices = self.gte_screen_outlier_vertices.saturating_add(1);
        }
        self.gte_screen_min_x = self.gte_screen_min_x.min(sx);
        self.gte_screen_max_x = self.gte_screen_max_x.max(sx);
        self.gte_screen_min_y = self.gte_screen_min_y.min(sy);
        self.gte_screen_max_y = self.gte_screen_max_y.max(sy);
        self.update_gte_depth_cue(projection_factor);

        self.cop2_data[16] = self.cop2_data[17];
        self.cop2_data[17] = self.cop2_data[18];
        self.cop2_data[18] = self.cop2_data[19];
        self.cop2_data[19] = depth as u32;

        self.cop2_data[12] = self.cop2_data[13];
        self.cop2_data[13] = self.cop2_data[14];
        self.cop2_data[14] = (sx as u16 as u32) | ((sy as u16 as u32) << 16);
        self.cop2_data[15] = self.cop2_data[14];
    }

    fn update_gte_depth_cue(&mut self, projection_factor: i64) {
        let dqa = self.cop2_control[27] as i16 as i64;
        let dqb = self.cop2_control[28] as i32 as i64;
        let mac0 = projection_factor.saturating_mul(dqa).saturating_add(dqb);
        self.cop2_data[24] = (mac0 as i32) as u32;
        let ir0 = mac0 >> 12;
        if !(0..=0x1000).contains(&ir0) {
            self.set_gte_flag(GTE_FLAG_IR0_SATURATED);
        }
        self.cop2_data[8] = ir0.clamp(0, 0x1000) as u32;
    }

    fn set_gte_average_z(&mut self, depth_sum: i64, scale: i64) {
        let mac0 = depth_sum.saturating_mul(scale);
        self.cop2_data[24] = (mac0 as i32) as u32;
        let otz = mac0 >> GTE_FRACTIONAL_BITS;
        if !(0..=u16::MAX as i64).contains(&otz) {
            self.set_gte_flag(GTE_FLAG_SZ_OTZ_SATURATED);
        }
        let otz = otz.clamp(0, u16::MAX as i64) as u16;
        self.gte_otz_min = self.gte_otz_min.min(otz);
        self.gte_otz_max = self.gte_otz_max.max(otz);
        self.cop2_data[7] = otz as u32;
    }

    fn set_gte_screen_saturation_flags(&mut self, sx_saturated: bool, sy_saturated: bool) {
        if sx_saturated {
            self.set_gte_flag(GTE_FLAG_SX2_SATURATED);
        }
        if sy_saturated {
            self.set_gte_flag(GTE_FLAG_SY2_SATURATED);
        }
    }

    fn gte_command_counts_json(&self) -> String {
        self.gte_command_counts
            .iter()
            .enumerate()
            .filter(|(_, count)| **count != 0)
            .map(|(command, count)| {
                format!(
                    "{{\"opcode\":{},\"opcode_hex\":\"0x{:02x}\",\"count\":{}}}",
                    command, command, count
                )
            })
            .collect::<Vec<_>>()
            .join(",")
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

    fn set_reg(&mut self, register: usize, value: u32) {
        if register == 0 {
            return;
        }
        if self.load_commit_register == Some(register) {
            self.load_commit_cancelled = true;
        }
        self.regs[register] = value;
    }

    fn schedule_load(&mut self, register: usize, value: u32) {
        if register != 0 {
            if self.load_commit_register == Some(register) {
                self.load_commit_cancelled = true;
            }
            self.pending_load = Some((register, value));
        }
    }

    fn load_merge_value(&self, register: usize) -> u32 {
        if self.load_commit_register == Some(register) {
            return self.load_commit_value.unwrap_or(self.regs[register]);
        }
        self.regs[register]
    }

    fn commit_delayed_load(&mut self, delayed_load: Option<(usize, u32)>) {
        let Some((register, value)) = delayed_load else {
            return;
        };
        if register != 0 && !self.load_commit_cancelled {
            self.regs[register] = value;
        }
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

fn gte_matrix_select(instruction: u32) -> u32 {
    (instruction >> 17) & 0x03
}

fn gte_vector_select(instruction: u32) -> u32 {
    (instruction >> 15) & 0x03
}

fn gte_translation_select(instruction: u32) -> u32 {
    (instruction >> 13) & 0x03
}

fn gte_shift(instruction: u32) -> u32 {
    if instruction & (1 << 19) != 0 { 12 } else { 0 }
}

fn gte_lm(instruction: u32) -> bool {
    instruction & (1 << 10) != 0
}

fn invert_gte_nclip() -> bool {
    std::env::var_os("BR2_NATIVE_INVERT_GTE_NCLIP").is_some()
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

fn packed_gte_matrix(registers: &[u32; 32], base: usize) -> [[i16; 3]; 3] {
    [
        [
            low_i16(registers[base]),
            high_i16(registers[base]),
            low_i16(registers[base + 1]),
        ],
        [
            high_i16(registers[base + 1]),
            low_i16(registers[base + 2]),
            high_i16(registers[base + 2]),
        ],
        [
            low_i16(registers[base + 3]),
            high_i16(registers[base + 3]),
            low_i16(registers[base + 4]),
        ],
    ]
}

fn packed_gte_vector(xy: u32, z: u32) -> [i16; 3] {
    [low_i16(xy), high_i16(xy), low_i16(z)]
}

fn low_i16(value: u32) -> i16 {
    value as u16 as i16
}

fn high_i16(value: u32) -> i16 {
    (value >> 16) as u16 as i16
}

fn optional_i16_sample(samples: u64, value: i16) -> String {
    if samples == 0 {
        "null".to_string()
    } else {
        value.to_string()
    }
}

fn optional_u16_sample(samples: u64, value: u16) -> String {
    if samples == 0 {
        "null".to_string()
    } else {
        value.to_string()
    }
}

fn u64_array_json(values: &[u64]) -> String {
    values
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn clamp_gte_ir(value: i64, lm: bool) -> i32 {
    let min = if lm { 0 } else { i16::MIN as i64 };
    value.clamp(min, i16::MAX as i64) as i32
}

fn gte_ir_saturated(value: i64, lm: bool) -> bool {
    let min = if lm { 0 } else { i16::MIN as i64 };
    !(min..=i16::MAX as i64).contains(&value)
}

fn gte_ir_saturation_flag(index: usize) -> u32 {
    match index {
        1 => 1 << 24,
        2 => 1 << 23,
        3 => 1 << 22,
        _ => 0,
    }
}

fn gte_irgb(ir1: u32, ir2: u32, ir3: u32) -> u32 {
    let r = ((ir1 as i16 as i32) >> 7).clamp(0, 0x1f) as u32;
    let g = ((ir2 as i16 as i32) >> 7).clamp(0, 0x1f) as u32;
    let b = ((ir3 as i16 as i32) >> 7).clamp(0, 0x1f) as u32;
    r | (g << 5) | (b << 10)
}

fn gte_rgb_from_ir(ir1: u32, ir2: u32, ir3: u32, rgb: u32) -> u32 {
    let r = ((ir1 as i16 as i32) >> 4).clamp(0, 0xff) as u32;
    let g = ((ir2 as i16 as i32) >> 4).clamp(0, 0xff) as u32;
    let b = ((ir3 as i16 as i32) >> 4).clamp(0, 0xff) as u32;
    let code = rgb & 0xff00_0000;
    code | (b << 16) | (g << 8) | r
}

fn gte_sxy(value: u32) -> (i16, i16) {
    (low_i16(value), high_i16(value))
}

fn gte_screen_offset(value: u32) -> i64 {
    value as i32 as i64
}

fn gte_projection_plane(value: u32) -> i64 {
    (value & 0xffff) as i64
}

fn clamp_gte_depth(value: i64) -> (u16, bool) {
    (
        value.clamp(0, u16::MAX as i64) as u16,
        !(0..=u16::MAX as i64).contains(&value),
    )
}

fn gte_projection_factor(h: i64, z: u16) -> (i64, bool) {
    let h = h.max(1);
    let z = i64::from(z).max(1);
    let raw = h.saturating_mul(1_i64 << 17).saturating_add(z / 2) / z;
    let saturated = raw > 0x1_ffff;
    (((raw.min(0x1_ffff) + 1) / 2), saturated)
}

fn project_gte_screen_component(offset: u32, value: i64, projection_factor: i64) -> (i16, bool) {
    let projected =
        gte_screen_offset(offset).saturating_add(value.saturating_mul(projection_factor));
    let screen = projected >> 16;
    let saturated = !(-1024..=1023).contains(&screen);
    (screen.clamp(-1024, 1023) as i16, saturated)
}

fn gte_screen_outlier(sx: i16, sy: i16) -> bool {
    !(-512..=1023).contains(&sx) || !(-512..=1023).contains(&sy)
}

fn gte_leading_zero_count(value: u32) -> u32 {
    if value & 0x8000_0000 != 0 {
        (!value).leading_zeros()
    } else {
        value.leading_zeros()
    }
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
    use super::{
        CAUSE_BD, CAUSE_IP2, CP0_CAUSE, CP0_EPC, CP0_STATUS, Cpu, GTE_FLAG_DIVIDE_OVERFLOW,
        GTE_FLAG_ERROR, GTE_FLAG_SX2_SATURATED, GTE_FLAG_SY2_SATURATED, GTE_FRACTIONAL_BITS,
        StepOutcome, gte_leading_zero_count, gte_sxy,
    };
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
            "{\"pc\":2147483776,\"next_pc\":2147483780,\"cycles\":4,\"halted\":true,\"status\":0,\"cause\":36,\"epc\":532676620,\"r2\":42,\"r3\":0,\"r4\":7,\"r5\":0,\"r6\":0,\"r8\":0,\"r9\":0,\"r10\":0,\"r11\":0,\"r16\":0,\"r29\":0,\"r31\":0,\"gte_command_counts\":[]}"
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
            (0x32 << 26) | (6 << 16),                            // lwc2 rgb, 0(zero)
            (0x12 << 26) | (9 << 16) | (6 << 11),                // mfc2 t1, rgb
            (0x12 << 26) | (0x10 << 21) | 0x01,                  // rtps placeholder
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        for _ in 0..7 {
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        }

        assert_eq!(bus.read_u32(0), 0x1234_5678);
        assert_eq!(cpu.cop2_data[6], 0x1234_5678);
        assert_eq!(cpu.regs[9], 0x1234_5678);
        assert_eq!(cpu.cop2_data[31], 0);
    }

    #[test]
    fn cop2_memory_transfers_use_gte_special_register_semantics() {
        let rom = program(&[
            i_type(0x0f, 0, 8, 0x0014),                           // lui t0, 0x0014
            i_type(0x0d, 8, 8, 0x000a),                           // ori t0, t0, 0x000a
            i_type(0x2b, 0, 8, 0),                                // sw t0, 0(zero)
            (0x32 << 26) | (15 << 16),                            // lwc2 sxy2, 0(zero)
            (0x12 << 26) | (0x04 << 21) | (8 << 16) | (28 << 11), // mtc2 t0, irgb
            (0x3a << 26) | (28 << 16) | 4,                        // swc2 irgb, 4(zero)
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();
        cpu.cop2_data[12] = 1 | (2 << 16);
        cpu.cop2_data[13] = 3 | (4 << 16);
        cpu.cop2_data[14] = 5 | (6 << 16);

        for _ in 0..6 {
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        }

        assert_eq!(cpu.cop2_data[12], 3 | (4 << 16));
        assert_eq!(cpu.cop2_data[13], 5 | (6 << 16));
        assert_eq!(cpu.cop2_data[14], 0x0014_000a);
        assert_eq!(bus.read_u32(4), 0x0000_000a);
    }

    #[test]
    fn cop2_data_reads_preserve_signed_halfword_register_semantics() {
        let rom = program(&[
            i_type(0x09, 0, 8, -2),                               // addiu t0, zero, -2
            (0x12 << 26) | (0x04 << 21) | (8 << 16) | (9 << 11),  // mtc2 t0, ir1
            (0x12 << 26) | (10 << 16) | (9 << 11),                // mfc2 t2, ir1
            (0x3a << 26) | (9 << 16),                             // swc2 ir1, 0(zero)
            (0x12 << 26) | (0x04 << 21) | (8 << 16) | (17 << 11), // mtc2 t0, sz1
            (0x12 << 26) | (11 << 16) | (17 << 11),               // mfc2 t3, sz1
            (0x3a << 26) | (17 << 16) | 4,                        // swc2 sz1, 4(zero)
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        for _ in 0..7 {
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        }

        assert_eq!(cpu.cop2_data[9], 0xfffe);
        assert_eq!(cpu.regs[10], 0xffff_fffe);
        assert_eq!(bus.read_u32(0), 0xffff_fffe);
        assert_eq!(cpu.cop2_data[17], 0xfffe);
        assert_eq!(cpu.regs[11], 0x0000_fffe);
        assert_eq!(bus.read_u32(4), 0x0000_fffe);
    }

    #[test]
    fn cop2_flag_control_register_is_separate_from_lzcr_data_register() {
        let rom = program(&[
            i_type(0x0f, 0, 8, 0x0002),                            // lui t0, 0x0002
            (0x12 << 26) | (0x06 << 21) | (8 << 16) | (31 << 11),  // ctc2 t0, flag
            (0x12 << 26) | (0x02 << 21) | (9 << 16) | (31 << 11),  // cfc2 t1, flag
            (0x12 << 26) | (0x04 << 21) | (8 << 16) | (30 << 11),  // mtc2 t0, lzcs
            (0x12 << 26) | (10 << 16) | (31 << 11),                // mfc2 t2, lzcr
            (0x12 << 26) | (0x02 << 21) | (11 << 16) | (31 << 11), // cfc2 t3, flag
            0,                                                     // cfc2 load delay slot
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        for _ in 0..7 {
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        }

        assert_eq!(cpu.regs[9], GTE_FLAG_ERROR | GTE_FLAG_DIVIDE_OVERFLOW);
        assert_eq!(cpu.regs[10], gte_leading_zero_count(0x0002_0000));
        assert_eq!(cpu.regs[11], GTE_FLAG_ERROR | GTE_FLAG_DIVIDE_OVERFLOW);
        assert_eq!(cpu.cop2_data[31], gte_leading_zero_count(0x0002_0000));
        assert_eq!(
            cpu.cop2_control[31],
            GTE_FLAG_ERROR | GTE_FLAG_DIVIDE_OVERFLOW
        );
    }

    #[test]
    fn mfc2_results_observe_r3000_load_delay() {
        let rom = program(&[
            i_type(0x0f, 0, 8, 0x1234),                          // lui t0, 0x1234
            i_type(0x0d, 8, 8, 0x5678),                          // ori t0, t0, 0x5678
            (0x12 << 26) | (0x04 << 21) | (8 << 16) | (6 << 11), // mtc2 t0, rgb
            (0x12 << 26) | (9 << 16) | (6 << 11),                // mfc2 t1, rgb
            i_type(0x09, 9, 10, 1),                              // addiu t2, t1, 1
            i_type(0x09, 9, 11, 1),                              // addiu t3, t1, 1
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        for _ in 0..6 {
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        }

        assert_eq!(cpu.regs[9], 0x1234_5678);
        assert_eq!(cpu.regs[10], 1);
        assert_eq!(cpu.regs[11], 0x1234_5679);
    }

    #[test]
    fn gte_mvmva_updates_mac_and_ir_registers() {
        let mut cpu = Cpu::default();
        cpu.cop2_control[0] = 0x0000_1000;
        cpu.cop2_control[1] = 0x0000_0000;
        cpu.cop2_control[2] = 0x0000_1000;
        cpu.cop2_control[3] = 0x0000_0000;
        cpu.cop2_control[4] = 0x0000_1000;
        cpu.cop2_data[0] = (2 << 16) | 1;
        cpu.cop2_data[1] = 3;

        cpu.execute_gte_command((1 << 19) | 0x12);

        assert_eq!(cpu.cop2_data[9] as i16, 1);
        assert_eq!(cpu.cop2_data[10] as i16, 2);
        assert_eq!(cpu.cop2_data[11] as i16, 3);
        assert_eq!(cpu.cop2_data[25] as i32, 1);
        assert_eq!(cpu.cop2_data[26] as i32, 2);
        assert_eq!(cpu.cop2_data[27] as i32, 3);
        assert_eq!(cpu.cop2_data[31], 0);
    }

    #[test]
    fn gte_mvmva_cv2_uses_psx_far_color_bug_path() {
        let mut cpu = Cpu::default();
        cpu.cop2_control[0] = (3 << 16) | 2;
        cpu.cop2_control[1] = (5 << 16) | 4;
        cpu.cop2_control[2] = (7 << 16) | 6;
        cpu.cop2_control[3] = (11 << 16) | 10;
        cpu.cop2_control[4] = 12;
        cpu.cop2_control[21] = 100;
        cpu.cop2_control[22] = 200;
        cpu.cop2_control[23] = 300;
        cpu.cop2_data[0] = (20 << 16) | 10;
        cpu.cop2_data[1] = 30;

        cpu.execute_gte_command((1 << 19) | (2 << 13) | 0x12);

        assert_eq!(cpu.cop2_data[25] as i32, 100);
        assert_eq!(cpu.cop2_data[26] as i32, 200);
        assert_eq!(cpu.cop2_data[27] as i32, 300);
        assert_eq!(cpu.cop2_data[9] as i16, 100);
        assert_eq!(cpu.cop2_data[10] as i16, 200);
        assert_eq!(cpu.cop2_data[11] as i16, 300);
        assert_eq!(cpu.gte_mvmva_cv2_special_cases, 1);
    }

    #[test]
    fn gte_rtpt_keeps_depth_fifo_fractional_scale_when_sf_is_set() {
        let mut cpu = Cpu::default();
        cpu.cop2_control[0] = 0x0000_1000;
        cpu.cop2_control[1] = 0x0000_0000;
        cpu.cop2_control[2] = 0x0000_1000;
        cpu.cop2_control[3] = 0x0000_0000;
        cpu.cop2_control[4] = 0x0000_1000;
        cpu.cop2_control[24] = 160 << 16;
        cpu.cop2_control[25] = 120 << 16;
        cpu.cop2_control[26] = 16;
        cpu.cop2_data[0] = (2 << 16) | 1;
        cpu.cop2_data[1] = 4;
        cpu.cop2_data[2] = (2 << 16) | 4;
        cpu.cop2_data[3] = 8;
        cpu.cop2_data[4] = (3 << 16) | 12;
        cpu.cop2_data[5] = 12;

        cpu.execute_gte_command((1 << 19) | 0x30);

        assert_eq!(cpu.cop2_data[12], (122 << 16) | 161);
        assert_eq!(cpu.cop2_data[13], (122 << 16) | 164);
        assert_eq!(cpu.cop2_data[14], (123 << 16) | 172);
        assert_eq!(cpu.cop2_data[15], cpu.cop2_data[14]);
        assert_eq!(cpu.cop2_data[17], 4);
        assert_eq!(cpu.cop2_data[18], 8);
        assert_eq!(cpu.cop2_data[19], 12);
        assert_eq!(cpu.cop2_data[9] as i16, 12);
        assert_eq!(cpu.cop2_data[10] as i16, 3);
        assert_eq!(cpu.cop2_data[11] as i16, 12);
    }

    #[test]
    fn gte_rtps_uses_unshifted_mac_depth_fifo_when_sf_is_clear() {
        let mut cpu = Cpu::default();
        cpu.cop2_control[0] = 0x0000_1000;
        cpu.cop2_control[1] = 0x0000_0000;
        cpu.cop2_control[2] = 0x0000_1000;
        cpu.cop2_control[3] = 0x0000_0000;
        cpu.cop2_control[4] = 0x0000_1000;
        cpu.cop2_control[24] = 160 << 16;
        cpu.cop2_control[25] = 120 << 16;
        cpu.cop2_control[26] = 16;
        cpu.cop2_data[0] = 0;
        cpu.cop2_data[1] = 4;

        cpu.execute_gte_command(0x01);

        assert_eq!(cpu.cop2_data[14], (120 << 16) | 160);
        assert_eq!(cpu.cop2_data[19], 4);
        assert_eq!(cpu.cop2_data[11] as i16, 16_384);
    }

    #[test]
    fn gte_projection_treats_h_as_unsigned_16_bit_distance() {
        let mut cpu = Cpu::default();
        cpu.cop2_control[0] = 0x0000_1000;
        cpu.cop2_control[1] = 0x0000_0000;
        cpu.cop2_control[2] = 0x0000_1000;
        cpu.cop2_control[3] = 0x0000_0000;
        cpu.cop2_control[4] = 0x0000_1000;
        cpu.cop2_control[24] = 160 << 16;
        cpu.cop2_control[25] = 120 << 16;
        cpu.cop2_control[26] = 0x8000;
        cpu.cop2_data[0] = 1;
        cpu.cop2_data[1] = 0x4000;

        cpu.execute_gte_command((1 << 19) | 0x01);

        assert_eq!(cpu.cop2_data[14], (120 << 16) | 161);
        assert_eq!(cpu.cop2_data[19], 0x4000);
    }

    #[test]
    fn gte_projection_saturation_sets_control_flag_without_overwriting_lzcr() {
        let mut cpu = Cpu::default();
        cpu.cop2_control[0] = 0x0000_1000;
        cpu.cop2_control[1] = 0x0000_0000;
        cpu.cop2_control[2] = 0x0000_1000;
        cpu.cop2_control[3] = 0x0000_0000;
        cpu.cop2_control[4] = 0x0000_1000;
        cpu.cop2_control[24] = 160 << 16;
        cpu.cop2_control[25] = 120 << 16;
        cpu.cop2_control[26] = 0xffff;
        cpu.cop2_data[0] = 1;
        cpu.cop2_data[1] = 1;
        cpu.cop2_data[31] = 17;

        cpu.execute_gte_command((1 << 19) | 0x01);

        assert_eq!(cpu.cop2_data[31], 17);
        assert_eq!(
            cpu.cop2_control[31] & GTE_FLAG_DIVIDE_OVERFLOW,
            GTE_FLAG_DIVIDE_OVERFLOW
        );
        assert_eq!(cpu.cop2_control[31] & GTE_FLAG_ERROR, GTE_FLAG_ERROR);
    }

    #[test]
    fn gte_screen_coordinates_saturate_to_psx_visible_guard_range() {
        let mut cpu = Cpu::default();
        cpu.cop2_control[0] = 0x0000_1000;
        cpu.cop2_control[1] = 0x0000_0000;
        cpu.cop2_control[2] = 0x0000_1000;
        cpu.cop2_control[3] = 0x0000_0000;
        cpu.cop2_control[4] = 0x0000_1000;
        cpu.cop2_control[24] = 160 << 16;
        cpu.cop2_control[25] = 120 << 16;
        cpu.cop2_control[26] = 0x100;
        cpu.cop2_data[0] = (0x7000 << 16) | 0x7000;
        cpu.cop2_data[1] = 1;

        cpu.execute_gte_command((1 << 19) | 0x01);

        let (sx, sy) = gte_sxy(cpu.cop2_data[14]);
        assert_eq!(sx, 1023);
        assert_eq!(sy, 1023);
        assert_eq!(
            cpu.cop2_control[31] & GTE_FLAG_SX2_SATURATED,
            GTE_FLAG_SX2_SATURATED
        );
        assert_eq!(
            cpu.cop2_control[31] & GTE_FLAG_SY2_SATURATED,
            GTE_FLAG_SY2_SATURATED
        );
    }

    #[test]
    fn gte_nclip_updates_mac0_from_screen_fifo() {
        let mut cpu = Cpu::default();
        cpu.cop2_data[12] = 10 | (10 << 16);
        cpu.cop2_data[13] = 20 | (10 << 16);
        cpu.cop2_data[14] = 10 | (20 << 16);

        cpu.execute_gte_command(0x06);

        assert_eq!(cpu.cop2_data[24] as i32, 100);
        assert_eq!(cpu.cop2_data[31], 0);
    }

    #[test]
    fn gte_sqr_and_gpf_update_ir_and_rgb_fifo() {
        let mut cpu = Cpu::default();
        cpu.cop2_data[8] = 2;
        cpu.cop2_data[9] = 3;
        cpu.cop2_data[10] = 4;
        cpu.cop2_data[11] = (-5i16) as u16 as u32;

        cpu.execute_gte_command(0x28);

        assert_eq!(cpu.cop2_data[9] as i16, 9);
        assert_eq!(cpu.cop2_data[10] as i16, 16);
        assert_eq!(cpu.cop2_data[11] as i16, 25);

        cpu.execute_gte_command(0x3d);

        assert_eq!(cpu.cop2_data[9] as i16, 18);
        assert_eq!(cpu.cop2_data[10] as i16, 32);
        assert_eq!(cpu.cop2_data[11] as i16, 50);
        assert_ne!(cpu.cop2_data[22], 0);
    }

    #[test]
    fn gte_avsz3_and_avsz4_update_otz_and_mac0() {
        let mut cpu = Cpu::default();
        cpu.cop2_data[16] = 400;
        cpu.cop2_data[17] = 100;
        cpu.cop2_data[18] = 200;
        cpu.cop2_data[19] = 300;
        cpu.cop2_control[29] = 0x1000;
        cpu.cop2_control[30] = 0x0800;

        cpu.execute_gte_command(0x2d);

        assert_eq!(cpu.cop2_data[7], 600);
        assert_eq!(cpu.cop2_data[24] as i32, 600 << GTE_FRACTIONAL_BITS);

        cpu.execute_gte_command(0x2e);

        assert_eq!(cpu.cop2_data[7], 500);
        assert_eq!(cpu.cop2_data[24] as i32, 500 << GTE_FRACTIONAL_BITS);
    }

    #[test]
    fn gte_nccs_updates_ir_and_rgb_fifo() {
        let mut cpu = Cpu::default();
        cpu.cop2_control[8] = 0x0000_1000;
        cpu.cop2_control[10] = 0x0000_1000;
        cpu.cop2_control[12] = 0x0000_1000;
        cpu.cop2_control[16] = 0x0000_1000;
        cpu.cop2_control[18] = 0x0000_1000;
        cpu.cop2_control[20] = 0x0000_1000;
        cpu.cop2_data[0] = (512 << 16) | 256;
        cpu.cop2_data[1] = 768;

        cpu.execute_gte_command((1 << 19) | 0x1b);

        assert_eq!(cpu.cop2_data[9] as i16, 256);
        assert_eq!(cpu.cop2_data[10] as i16, 512);
        assert_eq!(cpu.cop2_data[11] as i16, 768);
        assert_eq!(cpu.cop2_data[22], 0x0030_2010);
    }

    #[test]
    fn gte_cc_updates_color_matrix_result_and_rgb_fifo() {
        let mut cpu = Cpu::default();
        cpu.cop2_control[16] = 0x0000_1000;
        cpu.cop2_control[18] = 0x0000_1000;
        cpu.cop2_control[20] = 0x0000_1000;
        cpu.cop2_data[9] = 256;
        cpu.cop2_data[10] = 512;
        cpu.cop2_data[11] = 768;

        cpu.execute_gte_command((1 << 19) | 0x1c);

        assert_eq!(cpu.cop2_data[9] as i16, 256);
        assert_eq!(cpu.cop2_data[10] as i16, 512);
        assert_eq!(cpu.cop2_data[11] as i16, 768);
        assert_eq!(cpu.cop2_data[22], 0x0030_2010);
        assert_eq!(cpu.cop2_data[31], 0);
    }

    #[test]
    fn gte_ncct_processes_three_vectors_and_advances_rgb_fifo() {
        let mut cpu = Cpu::default();
        cpu.cop2_control[8] = 0x0000_1000;
        cpu.cop2_control[10] = 0x0000_1000;
        cpu.cop2_control[12] = 0x0000_1000;
        cpu.cop2_control[16] = 0x0000_1000;
        cpu.cop2_control[18] = 0x0000_1000;
        cpu.cop2_control[20] = 0x0000_1000;
        cpu.cop2_data[0] = (512 << 16) | 256;
        cpu.cop2_data[1] = 768;
        cpu.cop2_data[2] = (2048 << 16) | 1024;
        cpu.cop2_data[3] = 3072;
        cpu.cop2_data[4] = (200 << 16) | 100;
        cpu.cop2_data[5] = 300;

        cpu.execute_gte_command((1 << 19) | 0x3f);

        assert_eq!(cpu.cop2_data[20], 0x0030_2010);
        assert_eq!(cpu.cop2_data[21], 0x00c0_8040);
        assert_eq!(cpu.cop2_data[22], 0x0012_0c06);
        assert_eq!(cpu.cop2_data[9] as i16, 100);
        assert_eq!(cpu.cop2_data[10] as i16, 200);
        assert_eq!(cpu.cop2_data[11] as i16, 300);
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
            r_type(0, 0, 0, 0, 0x00),   // delay slot for final partial load
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

        for _ in 0..5 {
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        }

        assert_eq!(cpu.regs[9], 0x1122_3344);
    }

    #[test]
    fn load_results_are_delayed_one_instruction() {
        let rom = program(&[
            i_type(0x09, 0, 8, 7),    // addiu t0, zero, 7
            i_type(0x2b, 0, 8, 0),    // sw t0, 0(zero)
            i_type(0x23, 0, 9, 0),    // lw t1, 0(zero)
            i_type(0x09, 9, 10, 1),   // addiu t2, t1, 1; sees old t1
            i_type(0x09, 9, 11, 1),   // addiu t3, t1, 1; sees loaded t1
            r_type(0, 0, 0, 0, 0x0d), // break
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        for _ in 0..5 {
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        }

        assert_eq!(cpu.regs[9], 7);
        assert_eq!(cpu.regs[10], 1);
        assert_eq!(cpu.regs[11], 8);
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
