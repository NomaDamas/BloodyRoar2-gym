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
const BIOS_EXCEPTION_VECTOR_PHYSICAL: u32 = 0x0000_0080;
const BIOS_EXCEPTION_HANDLER_PHYSICAL: u32 = 0x0000_0c80;
const BIOS_EXCEPTION_VECTOR_TO_C80_STUB: [u32; 4] =
    [0x3c1a_0000, 0x275a_0c80, 0x0340_0008, 0x0000_0000];
const BIOS_EXCEPTION_C80_KERNEL_HANDLER_PREFIX: [u32; 12] = [
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0x241a_0100,
    0x8f5a_0008,
    0x0000_0000,
    0x8f5a_0000,
    0x0000_0000,
    0x235a_0008,
    0xaf41_0004,
    0xaf42_0008,
];
const BIOS_EXCEPTION_C80_IRQ_HANDLER_HLE_START: u32 = 0x0000_0c80;
const BIOS_EXCEPTION_C80_IRQ_HANDLER_HLE_END: u32 = 0x0000_0cac;
const BIOS_IRQ_DISPATCH_LOOP_HLE_START: u32 = 0x0000_1b7c;
const BIOS_IRQ_DISPATCH_LOOP_HLE_END: u32 = 0x0000_1bf0;
const BIOS_IRQ_DISPATCH_LOOP_SIGNATURE: [(u32, u32); 8] = [
    (0x0000_1b7c, 0x8e19_0004),
    (0x0000_1b80, 0x0000_0000),
    (0x0000_1b84, 0x1639_0017),
    (0x0000_1b88, 0x0000_0000),
    (0x0000_1be4, 0x2610_001c),
    (0x0000_1be8, 0x0214_082b),
    (0x0000_1bec, 0x1420_ffe3),
    (0x0000_1bf0, 0x0000_0000),
];
const BIOS_EXCEPTION_CONTEXT_POINTER_PHYSICAL: u32 = 0x0000_0108;
const BIOS_EXCEPTION_CONTEXT_POINTER_ADJUST: u32 = 8;
const BIOS_EXCEPTION_CONTEXT_GPR_OFFSETS: [(usize, u32); 29] = [
    (1, 0x04),
    (2, 0x08),
    (3, 0x0c),
    (4, 0x10),
    (5, 0x14),
    (6, 0x18),
    (7, 0x1c),
    (8, 0x20),
    (9, 0x24),
    (10, 0x28),
    (11, 0x2c),
    (12, 0x30),
    (13, 0x34),
    (14, 0x38),
    (15, 0x3c),
    (16, 0x40),
    (17, 0x44),
    (18, 0x48),
    (19, 0x4c),
    (20, 0x50),
    (21, 0x54),
    (22, 0x58),
    (23, 0x5c),
    (24, 0x60),
    (25, 0x64),
    (27, 0x6c),
    (28, 0x70),
    (29, 0x74),
    (30, 0x78),
];
const BIOS_EXCEPTION_CONTEXT_RA_OFFSET: u32 = 0x7c;
const BIOS_EXCEPTION_CONTEXT_LO_OFFSET: u32 = 0x84;
const BIOS_EXCEPTION_CONTEXT_HI_OFFSET: u32 = 0x88;
const GTE_FRACTIONAL_BITS: u32 = 12;
const GTE_FLAG_ERROR: u32 = 1 << 31;
const GTE_FLAG_ERROR_BITS: u32 = 0x7f87_e000;
const GTE_FLAG_DIVIDE_OVERFLOW: u32 = 1 << 17;
const GTE_FLAG_SZ_OTZ_SATURATED: u32 = 1 << 18;
const GTE_FLAG_IR0_SATURATED: u32 = 1 << 12;
const GTE_FLAG_SX2_SATURATED: u32 = 1 << 14;
const GTE_FLAG_SY2_SATURATED: u32 = 1 << 13;
const BIOS_DELAY_LOOP_START: u32 = 0x1fc0_a9b8;
const BIOS_DELAY_LOOP_EXIT: u32 = 0x1fc0_a9d0;
const BIOS_DELAY_PROLOGUE_LOOP_START: u32 = 0x1fc0_a9a0;
const BIOS_DELAY_LOOP_KSEG1_START: u32 = 0xbfc0_a9b8;
const BIOS_DELAY_LOOP_KSEG1_EXIT: u32 = 0xbfc0_a9d0;
const BIOS_DELAY_PROLOGUE_LOOP_KSEG1_START: u32 = 0xbfc0_a9a0;
const BIOS_SHORT_DELAY_LOOP_START: u32 = 0x1fc0_34a4;
const BIOS_SHORT_DELAY_LOOP_EXIT: u32 = 0x1fc0_34bc;
const BIOS_SHORT_DELAY_LOOP_KSEG1_START: u32 = 0xbfc0_34a4;
const BIOS_SHORT_DELAY_LOOP_KSEG1_EXIT: u32 = 0xbfc0_34bc;
const BIOS_DELAY_LOOP_MIN_SKIP_ITERATIONS: u32 = 1;
const BIOS_DELAY_PROLOGUE_LOOP_CYCLES_PER_ITERATION: u64 = 9;
const BIOS_DELAY_PROLOGUE_LOOP_INSTRUCTIONS: [u32; 6] = [
    0x8fa2_0000, // lw v0, 0(sp)
    0x8fae_0000, // lw t6, 0(sp)
    0x0000_0000, // nop
    0x25cf_ffff, // addiu t7, t6, -1
    0x1040_0007, // beq v0, zero, BIOS_DELAY_LOOP_EXIT
    0xafaf_0000, // sw t7, 0(sp)
];
const BIOS_DELAY_LOOP_INSTRUCTIONS: [u32; 6] = [
    0x8fa2_0000, // lw v0, 0(sp)
    0x8fb8_0000, // lw t8, 0(sp)
    0x0000_0000, // nop
    0x2719_ffff, // addiu t9, t8, -1
    0x1440_fffb, // bne v0, zero, BIOS_DELAY_LOOP_START
    0xafb9_0000, // sw t9, 0(sp)
];
const WORD_COPY_LOOP_INSTRUCTIONS: [u32; 9] = [
    0x8c87_0000, // lw a3, 0(a0)
    0x0000_0000, // nop
    0xaca7_0000, // sw a3, 0(a1)
    0x0000_0000, // nop
    0x2084_0004, // addiu a0, a0, 4
    0x20a5_0004, // addiu a1, a1, 4
    0x20c6_fffc, // addiu a2, a2, -4
    0x1cc0_fff8, // bgtz a2, loop start
    0x0000_0000, // nop
];
const BR2_BOOT_WORD_COPY_LOOP_START: u32 = 0x8001_011c;
const WORD_COPY_LOOP_CYCLES_PER_WORD: u64 = 11;
const ZERO_FILL_LOOP_INSTRUCTIONS: [u32; 5] = [
    0xac40_0000, // sw zero, 0(v0)
    0x2442_0004, // addiu v0, v0, 4
    0x0043_082b, // sltu at, v0, v1
    0x1420_fffc, // bne at, zero, loop start
    0x0000_0000, // nop
];
const BR2_BOOT_ZERO_FILL_LOOP_START: u32 = 0x802c_bab4;
const ZERO_FILL_LOOP_CYCLES_PER_WORD: u64 = 6;
const BIOS_INIT_ZERO_FILL_LOOP_START: u32 = 0x1fc0_0424;
const BIOS_INIT_ZERO_FILL_LOOP_EXIT: u32 = 0x1fc0_0434;
const BIOS_INIT_ZERO_FILL_LOOP_CYCLES_PER_WORD: u64 = 5;
const BIOS_INIT_ZERO_FILL_LOOP_INSTRUCTIONS: [u32; 4] = [
    0x2042_0004, // addi v0, v0, 4
    0x0043_082b, // sltu at, v0, v1
    0x1420_fffd, // bne at, zero, loop start
    0xac40_fffc, // sw zero, -4(v0)
];
const BIOS_BYTE_COPY_LOOP_START: u32 = 0x1fc0_4cd4;
const BIOS_BYTE_COPY_LOOP_INSTRUCTIONS: [u32; 21] = [
    0x922d_0000, // lbu t5, 0(s1)
    0x2631_0004, // addiu s1, s1, 4
    0xa20d_0000, // sb t5, 0(s0)
    0x922e_ffff, // lbu t6, -1(s1)
    0x0224_082b, // sltu at, s1, a0
    0x01c3_7823, // subu t7, t6, v1
    0xa20f_0001, // sb t7, 1(s0)
    0x9202_0001, // lbu v0, 1(s0)
    0x9238_fffc, // lbu t8, -4(s1)
    0xa202_0001, // sb v0, 1(s0)
    0x0058_c821, // addu t9, v0, t8
    0xa219_0003, // sb t9, 3(s0)
    0x9228_fffd, // lbu t0, -3(s1)
    0x2610_0004, // addiu s0, s0, 4
    0xa208_fffd, // sb t0, -3(s0)
    0x9229_fffe, // lbu t1, -2(s1)
    0x0000_0000, // nop
    0xa209_fffe, // sb t1, -2(s0)
    0x922a_ffff, // lbu t2, -1(s1)
    0x1420_ffec, // bne at, zero, loop start
    0xa20a_ffff, // sb t2, -1(s0)
];
const BIOS_BYTE_COPY_LOOP_CYCLES_PER_CHUNK: u64 = 35;
const BR2_DRAW_SYNC_WAIT_LOOP_START: u32 = 0x802d_080c;
const BR2_DRAW_SYNC_WAIT_LOOP_EXIT: u32 = 0x802d_081c;
const BR2_DRAW_SYNC_FLAG_VIRTUAL: u32 = 0x803a_2210;
const BR2_DRAW_SYNC_WAIT_LOOP_INSTRUCTIONS: [u32; 4] = [
    0x8c62_2210, // lw v0, 0x2210(v1)
    0x0000_0000, // nop
    0x1440_fffd, // bne v0, zero, loop start
    0x0000_0000, // nop
];
const BR2_FRAME_COUNTER_WAIT_LOOP_START: u32 = 0x8034_9fbc;
const BR2_FRAME_COUNTER_WAIT_LOOP_TARGET_CHECK: u32 = 0x8034_a004;
const BR2_FRAME_COUNTER_WAIT_LOOP_GLOBAL_COUNTER: u32 = 0x8036_c0b4;
const BR2_FRAME_COUNTER_WAIT_LOOP_STACK_OFFSET: u32 = 0x10;
const BR2_FRAME_COUNTER_WAIT_LOOP_MIN_COUNTER: u32 = 4;
const BR2_FRAME_COUNTER_WAIT_LOOP_CYCLES_PER_ITERATION: u64 = 18;
const BR2_FRAME_COUNTER_WAIT_LOOP_INSTRUCTIONS: [u32; 8] = [
    0x8fa2_0010, // lw v0, 0x10(sp)
    0x0000_0000, // nop
    0x2442_ffff, // addiu v0, v0, -1
    0xafa2_0010, // sw v0, 0x10(sp)
    0x8fa2_0010, // lw v0, 0x10(sp)
    0x0000_0000, // nop
    0x1443_000b, // bne v0, v1, target check
    0x0000_0000, // nop
];
const BR2_FRAME_COUNTER_WAIT_LOOP_TARGET_CHECK_INSTRUCTIONS: [u32; 6] = [
    0x3c02_8037, // lui v0, 0x8037
    0x8c42_c0b4, // lw v0, -0x3f4c(v0)
    0x0000_0000, // nop
    0x0044_102a, // slt v0, v0, a0
    0x1440_ffe9, // bne v0, zero, loop start
    0x0000_0000, // nop
];
const BR2_IRQ_POLL_TIMEOUT_INITIAL_DECREMENT: u32 = 0x8035_df68;
const BR2_IRQ_POLL_TIMEOUT_LOOP_START: u32 = 0x8035_df6c;
const BR2_IRQ_POLL_TIMEOUT_LOOP_EXIT: u32 = 0x8035_df8c;
const BR2_IRQ_POLL_STATUS_ADDRESS: u32 = 0x1f80_1070;
const BR2_IRQ_POLL_STATUS_MASK: u16 = 0x0080;
const BR2_IRQ_POLL_TIMEOUT_LOOP_CYCLES_PER_ITERATION: u64 = 8;
const BR2_IRQ_POLL_TIMEOUT_EXIT_CYCLES: u64 = 2;
const BR2_IRQ_POLL_TIMEOUT_INITIAL_INSTRUCTION: u32 = 0x2463_ffff; // addiu v1, v1, -1
const BR2_IRQ_POLL_TIMEOUT_LOOP_INSTRUCTIONS: [u32; 7] = [
    0x1065_0007, // beq v1, a1, BR2_IRQ_POLL_TIMEOUT_LOOP_EXIT
    0x0000_0000, // nop
    0x9482_0000, // lhu v0, 0(a0)
    0x0000_0000, // nop
    0x3042_0080, // andi v0, v0, 0x80
    0x1040_fffa, // beq v0, zero, BR2_IRQ_POLL_TIMEOUT_LOOP_START
    0x2463_ffff, // addiu v1, v1, -1
];
const BR2_BYTE_COPY_LOOP_START: u32 = 0x8030_6de0;
const BR2_BYTE_COPY_LOOP_EXIT: u32 = 0x8030_6df8;
const BR2_BYTE_COPY_LOOP_CYCLES_PER_BYTE: u64 = 8;
const BR2_BYTE_COPY_LOOP_INSTRUCTIONS: [u32; 6] = [
    0x90e2_0000, // lbu v0, 0(a3)
    0x24e7_0001, // addiu a3, a3, 1
    0x2463_ffff, // addiu v1, v1, -1
    0xa082_0000, // sb v0, 0(a0)
    0x1c60_fffb, // bgtz v1, loop start
    0x2484_0001, // addiu a0, a0, 1
];
const BR2_BANKED_HALFWORD_COPY_LOOP_START: u32 = 0x8033_34f4;
const BR2_BANKED_HALFWORD_COPY_LOOP_EXIT: u32 = 0x8033_352c;
const BR2_BANKED_HALFWORD_COPY_MASK: u32 = 0x007f_ffff;
const BR2_BANKED_HALFWORD_COPY_CYCLES_PER_HALFWORD: u64 = 13;
const BR2_BANKED_HALFWORD_COPY_LOOP_INSTRUCTIONS: [(u32, u32); 11] = [
    (0x00, 0x0233_1024), // and v0, s1, s3
    (0x04, 0x1440_0004), // bne v0, zero, copy halfword
    (0x08, 0x0000_0000), // nop
    (0x18, 0x9462_0000), // lhu v0, 0(v1)
    (0x1c, 0x2463_0002), // addiu v1, v1, 2
    (0x20, 0x2631_0002), // addiu s1, s1, 2
    (0x24, 0x2610_0002), // addiu s0, s0, 2
    (0x28, 0xa642_0000), // sh v0, 0(s2)
    (0x2c, 0x0214_102b), // sltu v0, s0, s4
    (0x30, 0x1440_fff3), // bne v0, zero, loop start
    (0x34, 0x2652_0002), // addiu s2, s2, 2
];
const BR2_POST_VS_TABLE_ACCUM_LOOP_START: u32 = 0x8035_6ef4;
const BR2_POST_VS_TABLE_ACCUM_LOOP_TAIL_INCREMENT: u32 = 0x8035_6f20;
const BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT: u32 = 0x8035_6f30;
const BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION: u64 = 20;
const BR2_POST_VS_TABLE_ACCUM_MIN_SKIP_ITERATIONS: u32 = 512;
const BR2_POST_VS_TABLE_ACCUM_MAX_SKIP_ITERATIONS: u32 = 8_000_000;
const BR2_POST_VS_TABLE_ACCUM_LOOP_INSTRUCTIONS: [u32; 15] = [
    0x8c42_0004, // lw v0, 4(v0)
    0x0005_1880, // sll v1, a1, 2
    0x0062_1821, // addu v1, v1, v0
    0x8c62_0000, // lw v0, 0(v1)
    0x0000_0000, // nop
    0x0044_1021, // addu v0, v0, a0
    0xac62_0000, // sw v0, 0(v1)
    0x8c83_007c, // lw v1, 0x7c(a0)
    0x0000_0000, // nop
    0x00c3_1021, // addu v0, a2, v1
    0x8c42_0000, // lw v0, 0(v0)
    0x24a5_0001, // addiu a1, a1, 1
    0x00a2_102a, // slt v0, a1, v0
    0x1440_fff2, // bne v0, zero, loop start
    0x00c3_1021, // addu v0, a2, v1
];
const BR2_REVERSE_POINTER_SCAN_LOOP_START: u32 = 0x8033_b1c0;
const BR2_REVERSE_POINTER_SCAN_LOOP_EXIT: u32 = 0x8033_b1d8;
const BR2_REVERSE_POINTER_SCAN_CYCLES_PER_ITERATION: u64 = 7;
const BR2_REVERSE_POINTER_SCAN_MIN_SKIP_ITERATIONS: u32 = 32;
const BR2_REVERSE_POINTER_SCAN_MAX_SKIP_ITERATIONS: u32 = 8192;
const BR2_REVERSE_POINTER_SCAN_LOOP_INSTRUCTIONS: [u32; 6] = [
    0x8d22_0000, // lw v0, 0(t1)
    0x24a5_ffff, // addiu a1, a1, -1
    0x18a0_0002, // blez a1, exit delay
    0x2463_fffc, // addiu v1, v1, -4
    0x1043_fffb, // beq v0, v1, loop start
    0x2529_fffc, // addiu t1, t1, -4
];
const BR2_REVERSE_MISMATCH_SCAN_LOOP_START: u32 = 0x8033_b1b0;
const BR2_REVERSE_MISMATCH_SCAN_CYCLES_PER_ITERATION: u64 = 9;
const BR2_REVERSE_MISMATCH_SCAN_MIN_SKIP_ITERATIONS: u32 = 32;
const BR2_REVERSE_MISMATCH_SCAN_MAX_SKIP_ITERATIONS: u32 = 131_072;
const BR2_REVERSE_MISMATCH_SCAN_LOOP_INSTRUCTIONS: [(u32, u32); 8] = [
    (0x00, 0x8c82_0000), // lw v0, 0(a0)
    (0x04, 0x2463_fffc), // addiu v1, v1, -4
    (0x08, 0x1443_0009), // bne v0, v1, mismatch path
    (0x0c, 0x2489_fffc), // addiu t1, a0, -4
    (0x30, 0x1048_0003), // beq v0, t0, exit
    (0x34, 0x24a5_ffff), // addiu a1, a1, -1
    (0x38, 0x1ca0_fff1), // bgtz a1, loop start
    (0x3c, 0x2484_fffc), // addiu a0, a0, -4
];
const BR2_SMALL_BYTE_COPY_LOOP_START: u32 = 0x8033_d83c;
const BR2_SMALL_BYTE_COPY_LOOP_EXIT: u32 = 0x8033_d854;
const BR2_SMALL_BYTE_COPY_CYCLES_PER_BYTE: u64 = 8;
const BR2_SMALL_BYTE_COPY_MIN_SKIP_BYTES: u32 = 1;
const BR2_SMALL_BYTE_COPY_MAX_SKIP_BYTES: u32 = 4096;
const BR2_SMALL_BYTE_COPY_LOOP_INSTRUCTIONS: [u32; 6] = [
    0x90a2_0000, // lbu v0, 0(a1)
    0x24c6_ffff, // addiu a2, a2, -1
    0x24a5_0001, // addiu a1, a1, 1
    0xa062_0000, // sb v0, 0(v1)
    0x1cc0_fffb, // bgtz a2, loop start
    0x2463_0001, // addiu v1, v1, 1
];

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
    pub end_sp: u32,
    pub end_ra: u32,
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
            end_sp: cpu.regs[29],
            end_ra: cpu.regs[31],
            cycles_before: cpu.cycles,
            cycles_after: cpu.cycles,
            cycles_elapsed: 0,
            outcome: StepOutcome::Halted,
        }
    }

    pub fn json(&self) -> String {
        format!(
            "{{\"start_pc\":{},\"start_pc_hex\":\"0x{:08x}\",\"end_pc\":{},\"end_pc_hex\":\"0x{:08x}\",\"next_pc\":{},\"next_pc_hex\":\"0x{:08x}\",\"instruction\":{},\"instruction_hex\":{},\"end_sp\":{},\"end_sp_hex\":\"0x{:08x}\",\"end_ra\":{},\"end_ra_hex\":\"0x{:08x}\",\"cycles_before\":{},\"cycles_after\":{},\"cycles_elapsed\":{},\"outcome\":\"{:?}\"}}",
            self.start_pc,
            self.start_pc,
            self.end_pc,
            self.end_pc,
            self.next_pc,
            self.next_pc,
            optional_u32_json(self.instruction),
            optional_u32_hex_json(self.instruction),
            self.end_sp,
            self.end_sp,
            self.end_ra,
            self.end_ra,
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
        if self.try_hle_br2_bios_irq_return(bus) {
            self.cycles += 1;
            self.regs[0] = 0;
            let report =
                self.step_report_from(start_pc, None, cycles_before, StepOutcome::Continue);
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }
        self.refresh_interrupts(bus);
        if self.delay_slot_branch_pc.is_none() && self.interrupt_pending() {
            if self.try_hle_blank_bios_irq_handler(bus) {
                self.cycles += 1;
                self.regs[0] = 0;
                let report =
                    self.step_report_from(start_pc, None, cycles_before, StepOutcome::Continue);
                bus.tick(report.cycles_elapsed);
                bus.clear_trace_context();
                return report;
            }
            self.cycles += 1;
            let outcome = self.raise_exception(self.pc, None, Exception::Interrupt);
            self.regs[0] = 0;
            let report = self.step_report_from(start_pc, None, cycles_before, outcome);
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) = self.try_fast_forward_bios_delay_loop(start_pc, cycles_before, bus) {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_bios_delay_prologue_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_bios_byte_copy_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_bios_init_zero_fill_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_br2_draw_sync_wait_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_br2_frame_counter_wait_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_br2_irq_poll_timeout_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) = self.try_fast_forward_br2_byte_copy_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_br2_banked_halfword_copy_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_br2_post_vs_table_accum_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_br2_reverse_mismatch_scan_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_br2_reverse_pointer_scan_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_br2_small_byte_copy_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) = self.try_fast_forward_word_copy_loop(start_pc, cycles_before, bus) {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) = self.try_fast_forward_zero_fill_loop(start_pc, cycles_before, bus) {
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

    pub fn gte_projected_vertices(&self) -> u64 {
        self.gte_projected_vertices
    }

    pub fn gte_command_counts_summary_json(&self) -> String {
        self.gte_command_counts_json()
    }

    pub fn native_3d_gameplay_signal(&self) -> bool {
        let projection_commands =
            self.gte_command_counts[0x01].saturating_add(self.gte_command_counts[0x30]);
        self.gte_projected_vertices >= 3 && projection_commands > 0
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
            end_sp: self.regs[29],
            end_ra: self.regs[31],
            cycles_before,
            cycles_after: self.cycles,
            cycles_elapsed: self.cycles.saturating_sub(cycles_before),
            outcome,
        }
    }

    fn try_fast_forward_bios_delay_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        let (loop_start, exit_pc) = bios_delay_loop_for_alias(self.pc)?;
        if self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        for (index, expected) in BIOS_DELAY_LOOP_INSTRUCTIONS.iter().copied().enumerate() {
            let address = loop_start + (index as u32) * 4;
            if bus.read_u32(address) != expected {
                return None;
            }
        }

        let stack_address = self.regs[29];
        if stack_address & 0x03 != 0 {
            return None;
        }
        let counter = bus.read_u32(stack_address);
        if counter < BIOS_DELAY_LOOP_MIN_SKIP_ITERATIONS {
            return None;
        }

        let iterations = u64::from(counter).saturating_add(1);
        let skipped_cycles = iterations.saturating_mul(BIOS_DELAY_LOOP_INSTRUCTIONS.len() as u64);
        self.regs[2] = 0;
        self.regs[24] = 0;
        self.regs[25] = u32::MAX;
        bus.write_u32(stack_address, u32::MAX);
        self.pc = exit_pc;
        self.next_pc = exit_pc.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(skipped_cycles);
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(BIOS_DELAY_LOOP_INSTRUCTIONS[0]),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_bios_delay_prologue_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        let exit_pc = bios_delay_prologue_loop_exit_for_alias(self.pc)?;
        if self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        let loop_start = bios_delay_prologue_loop_base_for_alias(self.pc)?;
        for (index, expected) in BIOS_DELAY_PROLOGUE_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            let address = loop_start + (index as u32) * 4;
            if bus.read_u32(address) != expected {
                return None;
            }
        }

        let stack_address = self.regs[29];
        if stack_address & 0x03 != 0 {
            return None;
        }
        let counter = bus.read_u32(stack_address);
        if counter < BIOS_DELAY_LOOP_MIN_SKIP_ITERATIONS {
            return None;
        }

        let skipped_cycles =
            u64::from(counter).saturating_mul(BIOS_DELAY_PROLOGUE_LOOP_CYCLES_PER_ITERATION);
        self.regs[2] = 0;
        self.regs[14] = 0;
        self.regs[15] = u32::MAX;
        bus.write_u32(stack_address, u32::MAX);
        self.pc = exit_pc;
        self.next_pc = exit_pc.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(skipped_cycles);
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(BIOS_DELAY_PROLOGUE_LOOP_INSTRUCTIONS[0]),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_bios_byte_copy_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        let loop_start = match self.pc {
            BIOS_BYTE_COPY_LOOP_START => BIOS_BYTE_COPY_LOOP_START,
            0xbfc0_4cd4 => 0xbfc0_4cd4,
            _ => return None,
        };
        if self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        for (index, expected) in BIOS_BYTE_COPY_LOOP_INSTRUCTIONS.iter().copied().enumerate() {
            let address = loop_start.wrapping_add((index as u32) * 4);
            if bus.read_u32(address) != expected {
                return None;
            }
        }

        let source = self.regs[17];
        let destination = self.regs[16];
        let limit = self.regs[4];
        if source >= limit {
            return None;
        }
        let remaining = limit.wrapping_sub(source);
        let chunks = remaining.checked_add(3)?.checked_div(4)?;
        let byte_count = chunks.checked_mul(4)?;
        let copied = bus.try_copy_bytes(source, destination, byte_count)?;
        let last = copied.get(copied.len().checked_sub(4)?..)?;
        let last_0 = last[0] as u32;
        let last_1 = last[1] as u32;
        let last_2 = last[2] as u32;
        let last_3 = last[3] as u32;
        let transformed_1 = last_3.wrapping_sub(self.regs[3]) & 0xff;

        self.regs[1] = 0;
        self.regs[2] = transformed_1;
        self.regs[8] = last_1;
        self.regs[9] = last_2;
        self.regs[10] = last_3;
        self.regs[13] = last_0;
        self.regs[14] = last_3;
        self.regs[15] = transformed_1;
        self.regs[16] = destination.wrapping_add(byte_count);
        self.regs[17] = source.wrapping_add(byte_count);
        self.regs[24] = last_0;
        self.regs[25] = transformed_1.wrapping_add(last_0) & 0xff;
        self.pc = loop_start.wrapping_add((BIOS_BYTE_COPY_LOOP_INSTRUCTIONS.len() as u32) * 4);
        self.next_pc = self.pc.wrapping_add(4);
        self.cycles = self
            .cycles
            .saturating_add(u64::from(chunks).saturating_mul(BIOS_BYTE_COPY_LOOP_CYCLES_PER_CHUNK));
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(BIOS_BYTE_COPY_LOOP_INSTRUCTIONS[0]),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_bios_init_zero_fill_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if self.pc != BIOS_INIT_ZERO_FILL_LOOP_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        for (index, expected) in BIOS_INIT_ZERO_FILL_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            let address = self.pc.wrapping_add((index as u32) * 4);
            if bus.read_u32(address) != expected {
                return None;
            }
        }

        let destination = self.regs[2];
        let end = self.regs[3];
        if destination >= end {
            return None;
        }
        let byte_count = end.wrapping_sub(destination);
        let words = bus.try_fill_aligned_words(destination, byte_count, 0)?;

        self.regs[1] = 0;
        self.regs[2] = end;
        self.pc = BIOS_INIT_ZERO_FILL_LOOP_EXIT;
        self.next_pc = BIOS_INIT_ZERO_FILL_LOOP_EXIT.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(
            u64::from(words).saturating_mul(BIOS_INIT_ZERO_FILL_LOOP_CYCLES_PER_WORD),
        );
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(BIOS_INIT_ZERO_FILL_LOOP_INSTRUCTIONS[0]),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_br2_draw_sync_wait_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if self.pc != BR2_DRAW_SYNC_WAIT_LOOP_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        for (index, expected) in BR2_DRAW_SYNC_WAIT_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            let address = self.pc.wrapping_add((index as u32) * 4);
            if bus.read_u32(address) != expected {
                return None;
            }
        }

        if self.regs[3].wrapping_add(0x2210) != BR2_DRAW_SYNC_FLAG_VIRTUAL {
            return None;
        }
        if bus.read_u32(BR2_DRAW_SYNC_FLAG_VIRTUAL) == 0 {
            return None;
        }

        let skipped_cycles = bus.cycles_until_next_vblank().max(1);
        self.regs[2] = 0;
        self.pc = BR2_DRAW_SYNC_WAIT_LOOP_EXIT;
        self.next_pc = BR2_DRAW_SYNC_WAIT_LOOP_EXIT.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(skipped_cycles);
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(BR2_DRAW_SYNC_WAIT_LOOP_INSTRUCTIONS[0]),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_br2_frame_counter_wait_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if self.pc != BR2_FRAME_COUNTER_WAIT_LOOP_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        for (index, expected) in BR2_FRAME_COUNTER_WAIT_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            let address =
                BR2_FRAME_COUNTER_WAIT_LOOP_START.wrapping_add((index as u32).wrapping_mul(4));
            if bus.read_u32(address) != expected {
                return None;
            }
        }
        for (index, expected) in BR2_FRAME_COUNTER_WAIT_LOOP_TARGET_CHECK_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            let address = BR2_FRAME_COUNTER_WAIT_LOOP_TARGET_CHECK.wrapping_add((index as u32) * 4);
            if bus.read_u32(address) != expected {
                return None;
            }
        }

        let frame_counter = bus.read_u32(BR2_FRAME_COUNTER_WAIT_LOOP_GLOBAL_COUNTER);
        let target_frame = self.regs[4];
        if frame_counter >= target_frame {
            return None;
        }

        let stack_counter_address =
            self.regs[29].wrapping_add(BR2_FRAME_COUNTER_WAIT_LOOP_STACK_OFFSET);
        if stack_counter_address & 0x03 != 0 {
            return None;
        }
        let stack_counter = bus.read_u32(stack_counter_address);
        if stack_counter < BR2_FRAME_COUNTER_WAIT_LOOP_MIN_COUNTER {
            return None;
        }

        let skipped_cycles = bus.cycles_until_next_vblank().max(1);
        let skipped_iterations = (skipped_cycles
            / BR2_FRAME_COUNTER_WAIT_LOOP_CYCLES_PER_ITERATION)
            .max(1)
            .min(u64::from(stack_counter.saturating_sub(1)));
        if skipped_iterations == 0 {
            return None;
        }
        bus.write_u32(
            stack_counter_address,
            stack_counter.saturating_sub(skipped_iterations as u32),
        );
        self.pc = BR2_FRAME_COUNTER_WAIT_LOOP_START;
        self.next_pc = BR2_FRAME_COUNTER_WAIT_LOOP_START.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(skipped_cycles);
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(BR2_FRAME_COUNTER_WAIT_LOOP_INSTRUCTIONS[0]),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_br2_irq_poll_timeout_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if (self.pc != BR2_IRQ_POLL_TIMEOUT_INITIAL_DECREMENT
            && self.pc != BR2_IRQ_POLL_TIMEOUT_LOOP_START)
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        if bus.read_u32(BR2_IRQ_POLL_TIMEOUT_INITIAL_DECREMENT)
            != BR2_IRQ_POLL_TIMEOUT_INITIAL_INSTRUCTION
        {
            return None;
        }
        for (index, expected) in BR2_IRQ_POLL_TIMEOUT_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            let address = BR2_IRQ_POLL_TIMEOUT_LOOP_START.wrapping_add((index as u32) * 4);
            if bus.read_u32(address) != expected {
                return None;
            }
        }

        if self.regs[4] != BR2_IRQ_POLL_STATUS_ADDRESS || self.regs[5] != u32::MAX {
            return None;
        }
        if bus.read_u16(BR2_IRQ_POLL_STATUS_ADDRESS) & BR2_IRQ_POLL_STATUS_MASK != 0 {
            return None;
        }
        if self.regs[3] == u32::MAX || (self.regs[3] as i32) < 0 {
            return None;
        }

        let mut skipped_cycles = 0u64;
        let mut counter = self.regs[3];
        let instruction = if self.pc == BR2_IRQ_POLL_TIMEOUT_INITIAL_DECREMENT {
            if counter == 0 {
                return None;
            }
            counter = counter.wrapping_sub(1);
            skipped_cycles = skipped_cycles.saturating_add(1);
            BR2_IRQ_POLL_TIMEOUT_INITIAL_INSTRUCTION
        } else {
            BR2_IRQ_POLL_TIMEOUT_LOOP_INSTRUCTIONS[0]
        };

        let iterations = u64::from(counter).saturating_add(1);
        skipped_cycles = skipped_cycles
            .saturating_add(
                iterations.saturating_mul(BR2_IRQ_POLL_TIMEOUT_LOOP_CYCLES_PER_ITERATION),
            )
            .saturating_add(BR2_IRQ_POLL_TIMEOUT_EXIT_CYCLES);

        self.regs[2] = 0;
        self.regs[3] = u32::MAX;
        self.pc = BR2_IRQ_POLL_TIMEOUT_LOOP_EXIT;
        self.next_pc = BR2_IRQ_POLL_TIMEOUT_LOOP_EXIT.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(skipped_cycles);
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(instruction),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_br2_byte_copy_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if self.pc != BR2_BYTE_COPY_LOOP_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        for (index, expected) in BR2_BYTE_COPY_LOOP_INSTRUCTIONS.iter().copied().enumerate() {
            let address = BR2_BYTE_COPY_LOOP_START.wrapping_add((index as u32) * 4);
            if bus.read_u32(address) != expected {
                return None;
            }
        }

        let byte_count = self.regs[3];
        if byte_count == 0 || byte_count as i32 <= 0 {
            return None;
        }

        let source = self.regs[7];
        let destination = self.regs[4];
        let copied = bus.try_copy_bytes(source, destination, byte_count)?;
        let last = copied.last().copied()? as u32;
        self.regs[2] = last;
        self.regs[3] = 0;
        self.regs[4] = destination.wrapping_add(byte_count);
        self.regs[7] = source.wrapping_add(byte_count);
        self.pc = BR2_BYTE_COPY_LOOP_EXIT;
        self.next_pc = BR2_BYTE_COPY_LOOP_EXIT.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(
            u64::from(byte_count).saturating_mul(BR2_BYTE_COPY_LOOP_CYCLES_PER_BYTE),
        );
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(BR2_BYTE_COPY_LOOP_INSTRUCTIONS[0]),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_br2_banked_halfword_copy_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if self.pc != BR2_BANKED_HALFWORD_COPY_LOOP_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        for (offset, expected) in BR2_BANKED_HALFWORD_COPY_LOOP_INSTRUCTIONS {
            if bus.read_u32(BR2_BANKED_HALFWORD_COPY_LOOP_START.wrapping_add(offset)) != expected {
                return None;
            }
        }

        let copied_halfbytes = self.regs[16];
        let copy_limit = self.regs[20];
        if copied_halfbytes >= copy_limit {
            return None;
        }
        if self.regs[19] != BR2_BANKED_HALFWORD_COPY_MASK {
            return None;
        }

        let remaining_halfbytes = copy_limit.wrapping_sub(copied_halfbytes);
        let halfwords = remaining_halfbytes.checked_add(1)?.checked_div(2)?;
        let byte_count = halfwords.checked_mul(2)?;
        let first_masked_source = self.regs[17] & self.regs[19];
        let last_masked_source = first_masked_source.checked_add(byte_count.checked_sub(2)?)?;
        if first_masked_source == 0 || last_masked_source > self.regs[19] {
            return None;
        }

        let source = self.regs[3];
        let destination = self.regs[18];
        let last_halfword = bus.try_copy_halfwords(source, destination, halfwords)?;

        self.regs[2] = 0;
        self.regs[3] = source.wrapping_add(byte_count);
        self.regs[16] = copied_halfbytes.wrapping_add(byte_count);
        self.regs[17] = self.regs[17].wrapping_add(byte_count);
        self.regs[18] = destination.wrapping_add(byte_count);
        self.pc = BR2_BANKED_HALFWORD_COPY_LOOP_EXIT;
        self.next_pc = BR2_BANKED_HALFWORD_COPY_LOOP_EXIT.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(
            u64::from(halfwords).saturating_mul(BR2_BANKED_HALFWORD_COPY_CYCLES_PER_HALFWORD),
        );
        self.regs[0] = 0;
        let _ = last_halfword;

        Some(self.step_report_from(
            start_pc,
            Some(BR2_BANKED_HALFWORD_COPY_LOOP_INSTRUCTIONS[0].1),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_br2_post_vs_table_accum_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if !matches!(
            self.pc,
            BR2_POST_VS_TABLE_ACCUM_LOOP_START | BR2_POST_VS_TABLE_ACCUM_LOOP_TAIL_INCREMENT
        ) || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
        {
            return None;
        }

        for (index, expected) in BR2_POST_VS_TABLE_ACCUM_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            let address = BR2_POST_VS_TABLE_ACCUM_LOOP_START.wrapping_add((index as u32) * 4);
            if bus.read_u32(address) != expected {
                return None;
            }
        }

        let (start_index, limit, table_meta_offset) =
            if self.pc == BR2_POST_VS_TABLE_ACCUM_LOOP_TAIL_INCREMENT {
                let table_meta_offset = self.regs[3];
                if bus.read_u32(self.regs[4].wrapping_add(0x7c)) != table_meta_offset {
                    return None;
                }
                let limit = match self.pending_load {
                    Some((2, value)) => value,
                    None => self.regs[2],
                    _ => return None,
                };
                (self.regs[5].wrapping_add(1), limit, table_meta_offset)
            } else {
                if self.pending_load.is_some() {
                    return None;
                }
                let table_meta_offset = bus.read_u32(self.regs[4].wrapping_add(0x7c));
                let count_address = self.regs[6].wrapping_add(table_meta_offset);
                if self.regs[2] != count_address {
                    return None;
                }
                (self.regs[5], bus.read_u32(count_address), table_meta_offset)
            };

        let count_address = self.regs[6].wrapping_add(table_meta_offset);
        let remaining = br2_signed_loop_remaining(start_index, limit)?;
        if remaining < BR2_POST_VS_TABLE_ACCUM_MIN_SKIP_ITERATIONS {
            return None;
        }

        let table_base = bus.read_u32(count_address.wrapping_add(4));
        let first_target = table_base.wrapping_add(start_index.wrapping_shl(2));

        let target_is_ram = br2_ram_word_range(first_target, remaining, bus.ram_len());
        let target_is_expansion_noop = br2_expansion_noop_address(first_target);
        let can_skip_noop_expansion_across_vblank = target_is_expansion_noop && !target_is_ram;
        let mut max_iterations = remaining.min(BR2_POST_VS_TABLE_ACCUM_MAX_SKIP_ITERATIONS);
        if can_skip_noop_expansion_across_vblank {
            max_iterations = remaining;
        } else if self.vblank_irq_can_preempt(bus) {
            let cycles_until_vblank = bus.cycles_until_next_vblank();
            if cycles_until_vblank <= BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION {
                return None;
            }
            let irq_limited_iterations =
                ((cycles_until_vblank - 1) / BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION) as u32;
            max_iterations = max_iterations.min(irq_limited_iterations);
        }
        if max_iterations < BR2_POST_VS_TABLE_ACCUM_MIN_SKIP_ITERATIONS {
            return None;
        }

        let skipped_iterations;
        let ram_iterations = max_iterations;
        if br2_ram_word_range(first_target, ram_iterations, bus.ram_len()) {
            skipped_iterations = ram_iterations;
            for index in 0..skipped_iterations {
                let target = first_target.wrapping_add(index.wrapping_shl(2));
                let value = bus.read_u32(target).wrapping_add(self.regs[4]);
                bus.write_u32(target, value);
            }
        } else {
            if !target_is_expansion_noop {
                return None;
            }
            skipped_iterations = max_iterations;
        }

        let final_index = start_index.wrapping_add(skipped_iterations);
        let completed_loop = skipped_iterations == remaining;
        self.regs[2] = count_address;
        self.regs[3] = table_meta_offset;
        self.regs[5] = final_index;
        self.pc = if completed_loop {
            BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT
        } else {
            BR2_POST_VS_TABLE_ACCUM_LOOP_START
        };
        self.next_pc = self.pc.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(
            u64::from(skipped_iterations)
                .saturating_mul(BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION),
        );
        self.regs[0] = 0;
        self.pending_load = None;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;

        Some(self.step_report_from(
            start_pc,
            Some(BR2_POST_VS_TABLE_ACCUM_LOOP_INSTRUCTIONS[0]),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_br2_reverse_mismatch_scan_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if self.pc != BR2_REVERSE_MISMATCH_SCAN_LOOP_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        for (offset, expected) in BR2_REVERSE_MISMATCH_SCAN_LOOP_INSTRUCTIONS.iter().copied() {
            if bus.read_u32(BR2_REVERSE_MISMATCH_SCAN_LOOP_START + offset) != expected {
                return None;
            }
        }

        let mut current_pointer = self.regs[4];
        let mut expected_pointer = self.regs[3];
        let mut count = self.regs[5];
        let sentinel = self.regs[8];
        if count <= 1 || !br2_ram_word_range(current_pointer, 1, bus.ram_len()) {
            return None;
        }

        let mut max_iterations = count
            .saturating_sub(1)
            .min(BR2_REVERSE_MISMATCH_SCAN_MAX_SKIP_ITERATIONS);
        if self.vblank_irq_can_preempt(bus) {
            let cycles_until_vblank = bus.cycles_until_next_vblank();
            if cycles_until_vblank <= BR2_REVERSE_MISMATCH_SCAN_CYCLES_PER_ITERATION {
                return None;
            }
            let irq_limited_iterations =
                ((cycles_until_vblank - 1) / BR2_REVERSE_MISMATCH_SCAN_CYCLES_PER_ITERATION) as u32;
            max_iterations = max_iterations.min(irq_limited_iterations);
        }
        if max_iterations < BR2_REVERSE_MISMATCH_SCAN_MIN_SKIP_ITERATIONS {
            return None;
        }

        let mut skipped_iterations = 0u32;
        let mut last_loaded = self.regs[2];
        for _ in 0..max_iterations {
            if !br2_ram_word_range(current_pointer, 1, bus.ram_len()) {
                return None;
            }

            let loaded = bus.read_u32(current_pointer);
            let next_expected = expected_pointer.wrapping_sub(4);
            if loaded == next_expected || loaded == sentinel {
                break;
            }

            last_loaded = loaded;
            expected_pointer = next_expected;
            current_pointer = current_pointer.wrapping_sub(4);
            count = count.wrapping_sub(1);
            skipped_iterations = skipped_iterations.saturating_add(1);
        }

        if skipped_iterations < BR2_REVERSE_MISMATCH_SCAN_MIN_SKIP_ITERATIONS {
            return None;
        }

        self.regs[2] = last_loaded;
        self.regs[3] = expected_pointer;
        self.regs[4] = current_pointer;
        self.regs[5] = count;
        self.regs[9] = current_pointer.wrapping_sub(4);
        self.pc = BR2_REVERSE_MISMATCH_SCAN_LOOP_START;
        self.next_pc = BR2_REVERSE_MISMATCH_SCAN_LOOP_START.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(
            u64::from(skipped_iterations)
                .saturating_mul(BR2_REVERSE_MISMATCH_SCAN_CYCLES_PER_ITERATION),
        );
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(BR2_REVERSE_MISMATCH_SCAN_LOOP_INSTRUCTIONS[0].1),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_br2_reverse_pointer_scan_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if self.pc != BR2_REVERSE_POINTER_SCAN_LOOP_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        for (index, expected) in BR2_REVERSE_POINTER_SCAN_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            let address = BR2_REVERSE_POINTER_SCAN_LOOP_START.wrapping_add((index as u32) * 4);
            if bus.read_u32(address) != expected {
                return None;
            }
        }

        let mut count = self.regs[5];
        let mut current_pointer = self.regs[9];
        let mut expected_pointer = self.regs[3];
        if count == 0 || !br2_ram_word_range(current_pointer, 1, bus.ram_len()) {
            return None;
        }

        let mut max_iterations = count.min(BR2_REVERSE_POINTER_SCAN_MAX_SKIP_ITERATIONS);
        if self.vblank_irq_can_preempt(bus) {
            let cycles_until_vblank = bus.cycles_until_next_vblank();
            if cycles_until_vblank <= BR2_REVERSE_POINTER_SCAN_CYCLES_PER_ITERATION {
                return None;
            }
            let irq_limited_iterations =
                ((cycles_until_vblank - 1) / BR2_REVERSE_POINTER_SCAN_CYCLES_PER_ITERATION) as u32;
            max_iterations = max_iterations.min(irq_limited_iterations);
        }
        if max_iterations < BR2_REVERSE_POINTER_SCAN_MIN_SKIP_ITERATIONS {
            return None;
        }

        let mut skipped_iterations = 0u32;
        let mut last_loaded = self.regs[2];
        let mut loop_continues = true;
        for _ in 0..max_iterations {
            if !br2_ram_word_range(current_pointer, 1, bus.ram_len()) {
                return None;
            }
            last_loaded = bus.read_u32(current_pointer);
            count = count.wrapping_sub(1);
            expected_pointer = expected_pointer.wrapping_sub(4);
            current_pointer = current_pointer.wrapping_sub(4);
            skipped_iterations = skipped_iterations.saturating_add(1);

            loop_continues = (count as i32) > 0 && last_loaded == expected_pointer;
            if !loop_continues {
                break;
            }
        }

        if skipped_iterations < BR2_REVERSE_POINTER_SCAN_MIN_SKIP_ITERATIONS {
            return None;
        }

        self.regs[2] = last_loaded;
        self.regs[3] = expected_pointer;
        self.regs[5] = count;
        self.regs[9] = current_pointer;
        self.pc = if loop_continues {
            BR2_REVERSE_POINTER_SCAN_LOOP_START
        } else {
            BR2_REVERSE_POINTER_SCAN_LOOP_EXIT
        };
        self.next_pc = self.pc.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(
            u64::from(skipped_iterations)
                .saturating_mul(BR2_REVERSE_POINTER_SCAN_CYCLES_PER_ITERATION),
        );
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(BR2_REVERSE_POINTER_SCAN_LOOP_INSTRUCTIONS[0]),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_br2_small_byte_copy_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if self.pc != BR2_SMALL_BYTE_COPY_LOOP_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        for (index, expected) in BR2_SMALL_BYTE_COPY_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            let address = BR2_SMALL_BYTE_COPY_LOOP_START + (index as u32) * 4;
            if bus.read_u32(address) != expected {
                return None;
            }
        }

        let source = self.regs[5];
        let destination = self.regs[3];
        let count = self.regs[6];
        if count < BR2_SMALL_BYTE_COPY_MIN_SKIP_BYTES {
            return None;
        }

        let mut byte_count = count.min(BR2_SMALL_BYTE_COPY_MAX_SKIP_BYTES);
        if self.vblank_irq_can_preempt(bus) {
            let cycles_until_vblank = bus.cycles_until_next_vblank();
            if cycles_until_vblank <= BR2_SMALL_BYTE_COPY_CYCLES_PER_BYTE {
                return None;
            }
            let irq_limited_bytes =
                ((cycles_until_vblank - 1) / BR2_SMALL_BYTE_COPY_CYCLES_PER_BYTE) as u32;
            byte_count = byte_count.min(irq_limited_bytes);
        }
        if byte_count < BR2_SMALL_BYTE_COPY_MIN_SKIP_BYTES {
            return None;
        }

        let copied = bus.try_copy_bytes(source, destination, byte_count)?;
        let last = copied.last().copied()? as u32;
        let remaining = count.wrapping_sub(byte_count);
        self.regs[2] = last;
        self.regs[3] = destination.wrapping_add(byte_count);
        self.regs[5] = source.wrapping_add(byte_count);
        self.regs[6] = remaining;
        self.pc = if remaining == 0 {
            BR2_SMALL_BYTE_COPY_LOOP_EXIT
        } else {
            BR2_SMALL_BYTE_COPY_LOOP_START
        };
        self.next_pc = self.pc.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(
            u64::from(byte_count).saturating_mul(BR2_SMALL_BYTE_COPY_CYCLES_PER_BYTE),
        );
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(BR2_SMALL_BYTE_COPY_LOOP_INSTRUCTIONS[0]),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_word_copy_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if self.pc != BR2_BOOT_WORD_COPY_LOOP_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        for (index, expected) in WORD_COPY_LOOP_INSTRUCTIONS.iter().copied().enumerate() {
            let address = self.pc.wrapping_add((index as u32) * 4);
            if bus.read_u32(address) != expected {
                return None;
            }
        }

        let byte_count = self.regs[6];
        if byte_count as i32 <= 0 {
            return None;
        }

        let source = self.regs[4];
        let destination = self.regs[5];
        let (words, last_word) = bus.try_copy_aligned_words(source, destination, byte_count)?;
        let copied_bytes = words.saturating_mul(4);
        self.regs[4] = source.wrapping_add(copied_bytes);
        self.regs[5] = destination.wrapping_add(copied_bytes);
        self.regs[6] = 0;
        self.regs[7] = last_word;
        self.pc = self
            .pc
            .wrapping_add((WORD_COPY_LOOP_INSTRUCTIONS.len() as u32) * 4);
        self.next_pc = self.pc.wrapping_add(4);
        self.cycles = self
            .cycles
            .saturating_add(u64::from(words).saturating_mul(WORD_COPY_LOOP_CYCLES_PER_WORD));
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(WORD_COPY_LOOP_INSTRUCTIONS[0]),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_zero_fill_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if self.pc != BR2_BOOT_ZERO_FILL_LOOP_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        for (index, expected) in ZERO_FILL_LOOP_INSTRUCTIONS.iter().copied().enumerate() {
            let address = self.pc.wrapping_add((index as u32) * 4);
            if bus.read_u32(address) != expected {
                return None;
            }
        }

        let destination = self.regs[2];
        let end = self.regs[3];
        if destination >= end {
            return None;
        }
        let byte_count = end.wrapping_sub(destination);
        let words = bus.try_fill_aligned_words(destination, byte_count, 0)?;
        self.regs[1] = 0;
        self.regs[2] = end;
        self.pc = self
            .pc
            .wrapping_add((ZERO_FILL_LOOP_INSTRUCTIONS.len() as u32) * 4);
        self.next_pc = self.pc.wrapping_add(4);
        self.cycles = self
            .cycles
            .saturating_add(u64::from(words).saturating_mul(ZERO_FILL_LOOP_CYCLES_PER_WORD));
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(ZERO_FILL_LOOP_INSTRUCTIONS[0]),
            cycles_before,
            StepOutcome::Continue,
        ))
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
                self.cp0[CP0_STATUS] = rfe_status(self.cp0[CP0_STATUS]);
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

    fn try_hle_blank_bios_irq_handler(&mut self, bus: &mut Bus) -> bool {
        let pending = bus.io.irq.status & bus.io.irq.mask;
        if pending == 0 {
            return false;
        }
        if !bios_exception_vector_points_to_blank_c80_handler(bus) {
            return false;
        }

        bus.acknowledge_hle_bios_irq_sources(pending);
        self.cp0[CP0_CAUSE] &= !CAUSE_IP2;
        true
    }

    fn try_hle_br2_bios_irq_return(&mut self, bus: &mut Bus) -> bool {
        if self.delay_slot_branch_pc.is_some() || self.cp0[CP0_CAUSE] & CAUSE_IP2 == 0 {
            return false;
        }
        let post_vs_c80_return = (BIOS_EXCEPTION_C80_IRQ_HANDLER_HLE_START
            ..=BIOS_EXCEPTION_C80_IRQ_HANDLER_HLE_END)
            .contains(&self.pc)
            && self.cp0[CP0_EPC] == BR2_POST_VS_TABLE_ACCUM_LOOP_START
            && bios_exception_c80_handler_has_kernel_prefix(bus);
        let draw_sync_dispatch_return =
            (BIOS_IRQ_DISPATCH_LOOP_HLE_START..=BIOS_IRQ_DISPATCH_LOOP_HLE_END).contains(&self.pc)
                && self.cp0[CP0_EPC] == BR2_DRAW_SYNC_WAIT_LOOP_EXIT
                && bios_irq_dispatch_loop_has_signature(bus);
        if !post_vs_c80_return && !draw_sync_dispatch_return {
            return false;
        }
        if draw_sync_dispatch_return && !self.restore_bios_exception_context(bus) {
            return false;
        }

        let pending = bus.io.irq.status & bus.io.irq.mask;
        if pending != 0 {
            bus.acknowledge_hle_bios_irq_sources(pending);
        }
        self.cp0[CP0_CAUSE] &= !CAUSE_IP2;
        self.cp0[CP0_STATUS] = rfe_status(self.cp0[CP0_STATUS]);
        self.pc = self.cp0[CP0_EPC];
        self.next_pc = self.pc.wrapping_add(4);
        self.pending_load = None;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;
        true
    }

    fn restore_bios_exception_context(&mut self, bus: &Bus) -> bool {
        let Some(context_base) = bios_exception_context_base_physical(bus) else {
            return false;
        };
        for (register, offset) in BIOS_EXCEPTION_CONTEXT_GPR_OFFSETS {
            let Some(value) = bus.read_ram_u32_physical(context_base.wrapping_add(offset)) else {
                return false;
            };
            self.regs[register] = value;
        }
        let Some(ra) =
            bus.read_ram_u32_physical(context_base.wrapping_add(BIOS_EXCEPTION_CONTEXT_RA_OFFSET))
        else {
            return false;
        };
        let Some(lo) =
            bus.read_ram_u32_physical(context_base.wrapping_add(BIOS_EXCEPTION_CONTEXT_LO_OFFSET))
        else {
            return false;
        };
        let Some(hi) =
            bus.read_ram_u32_physical(context_base.wrapping_add(BIOS_EXCEPTION_CONTEXT_HI_OFFSET))
        else {
            return false;
        };
        self.regs[31] = ra;
        self.hi = hi;
        self.lo = lo;
        true
    }

    fn vblank_irq_can_preempt(&self, bus: &Bus) -> bool {
        self.cp0[CP0_STATUS] & STATUS_IE != 0
            && self.cp0[CP0_STATUS] & CAUSE_IP2 != 0
            && bus.io.irq.mask & 1 != 0
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

fn bios_exception_vector_points_to_blank_c80_handler(bus: &Bus) -> bool {
    for (index, expected) in BIOS_EXCEPTION_VECTOR_TO_C80_STUB
        .iter()
        .copied()
        .enumerate()
    {
        let address = BIOS_EXCEPTION_VECTOR_PHYSICAL + (index as u32) * 4;
        if bus.read_ram_u32_physical(address) != Some(expected) {
            return false;
        }
    }

    (0..8).all(|index| {
        let address = BIOS_EXCEPTION_HANDLER_PHYSICAL + index * 4;
        bus.read_ram_u32_physical(address) == Some(0)
    })
}

fn bios_exception_c80_handler_has_kernel_prefix(bus: &Bus) -> bool {
    BIOS_EXCEPTION_C80_KERNEL_HANDLER_PREFIX
        .iter()
        .copied()
        .enumerate()
        .all(|(index, expected)| {
            let address = BIOS_EXCEPTION_HANDLER_PHYSICAL + (index as u32) * 4;
            bus.read_ram_u32_physical(address) == Some(expected)
        })
}

fn bios_irq_dispatch_loop_has_signature(bus: &Bus) -> bool {
    BIOS_IRQ_DISPATCH_LOOP_SIGNATURE
        .iter()
        .copied()
        .all(|(address, expected)| bus.read_ram_u32_physical(address) == Some(expected))
}

fn bios_exception_context_base_physical(bus: &Bus) -> Option<u32> {
    let context_pointer_address =
        bus.read_ram_u32_physical(BIOS_EXCEPTION_CONTEXT_POINTER_PHYSICAL)?;
    let context_pointer =
        bus.read_ram_u32_physical(psx_physical_address(context_pointer_address))?;
    Some(psx_physical_address(
        context_pointer.wrapping_add(BIOS_EXCEPTION_CONTEXT_POINTER_ADJUST),
    ))
}

fn psx_physical_address(address: u32) -> u32 {
    address & 0x1fff_ffff
}

fn rfe_status(status: u32) -> u32 {
    let mode_bits = status & 0x3f;
    (status & !0x0f) | ((mode_bits >> 2) & 0x0f)
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

fn br2_signed_loop_remaining(start_index: u32, limit: u32) -> Option<u32> {
    let start_index = start_index as i32;
    let limit = limit as i32;
    (start_index < limit).then_some((i64::from(limit) - i64::from(start_index)) as u32)
}

fn br2_expansion_noop_address(address: u32) -> bool {
    let start = address & 0x1fff_ffff;
    (0x0080_0000..0x1f80_0000).contains(&start)
}

fn br2_ram_word_range(address: u32, words: u32, ram_len: usize) -> bool {
    if words == 0 || ram_len == 0 {
        return false;
    }
    let Some(last_byte_offset) = words
        .checked_sub(1)
        .and_then(|last_word| last_word.checked_mul(4))
        .and_then(|last_word_offset| last_word_offset.checked_add(3))
    else {
        return false;
    };
    let start = address & 0x1fff_ffff;
    let end = address.wrapping_add(last_byte_offset) & 0x1fff_ffff;
    start <= end && (end as usize) < ram_len
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

fn bios_delay_loop_for_alias(pc: u32) -> Option<(u32, u32)> {
    match pc {
        BIOS_DELAY_LOOP_START => Some((BIOS_DELAY_LOOP_START, BIOS_DELAY_LOOP_EXIT)),
        BIOS_DELAY_LOOP_KSEG1_START => {
            Some((BIOS_DELAY_LOOP_KSEG1_START, BIOS_DELAY_LOOP_KSEG1_EXIT))
        }
        BIOS_SHORT_DELAY_LOOP_START => {
            Some((BIOS_SHORT_DELAY_LOOP_START, BIOS_SHORT_DELAY_LOOP_EXIT))
        }
        BIOS_SHORT_DELAY_LOOP_KSEG1_START => Some((
            BIOS_SHORT_DELAY_LOOP_KSEG1_START,
            BIOS_SHORT_DELAY_LOOP_KSEG1_EXIT,
        )),
        _ => None,
    }
}

fn bios_delay_prologue_loop_base_for_alias(pc: u32) -> Option<u32> {
    match pc {
        BIOS_DELAY_PROLOGUE_LOOP_START => Some(BIOS_DELAY_PROLOGUE_LOOP_START),
        BIOS_DELAY_PROLOGUE_LOOP_KSEG1_START => Some(BIOS_DELAY_PROLOGUE_LOOP_KSEG1_START),
        _ => None,
    }
}

fn bios_delay_prologue_loop_exit_for_alias(pc: u32) -> Option<u32> {
    match pc {
        BIOS_DELAY_PROLOGUE_LOOP_START => Some(BIOS_DELAY_LOOP_EXIT),
        BIOS_DELAY_PROLOGUE_LOOP_KSEG1_START => Some(BIOS_DELAY_LOOP_KSEG1_EXIT),
        _ => None,
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
    #![allow(clippy::field_reassign_with_default)]

    use super::{
        BIOS_BYTE_COPY_LOOP_INSTRUCTIONS, BIOS_BYTE_COPY_LOOP_START, BIOS_DELAY_LOOP_EXIT,
        BIOS_DELAY_LOOP_INSTRUCTIONS, BIOS_DELAY_PROLOGUE_LOOP_INSTRUCTIONS,
        BIOS_DELAY_PROLOGUE_LOOP_START, BIOS_EXCEPTION_C80_KERNEL_HANDLER_PREFIX,
        BIOS_EXCEPTION_CONTEXT_HI_OFFSET, BIOS_EXCEPTION_CONTEXT_LO_OFFSET,
        BIOS_EXCEPTION_CONTEXT_POINTER_ADJUST, BIOS_EXCEPTION_CONTEXT_POINTER_PHYSICAL,
        BIOS_EXCEPTION_CONTEXT_RA_OFFSET, BIOS_EXCEPTION_VECTOR_TO_C80_STUB,
        BIOS_INIT_ZERO_FILL_LOOP_EXIT, BIOS_INIT_ZERO_FILL_LOOP_INSTRUCTIONS,
        BIOS_INIT_ZERO_FILL_LOOP_START, BIOS_IRQ_DISPATCH_LOOP_SIGNATURE,
        BIOS_SHORT_DELAY_LOOP_EXIT, BIOS_SHORT_DELAY_LOOP_START,
        BR2_BANKED_HALFWORD_COPY_LOOP_EXIT, BR2_BANKED_HALFWORD_COPY_LOOP_INSTRUCTIONS,
        BR2_BANKED_HALFWORD_COPY_LOOP_START, BR2_BANKED_HALFWORD_COPY_MASK,
        BR2_BOOT_WORD_COPY_LOOP_START, BR2_BOOT_ZERO_FILL_LOOP_START, BR2_BYTE_COPY_LOOP_EXIT,
        BR2_BYTE_COPY_LOOP_INSTRUCTIONS, BR2_BYTE_COPY_LOOP_START, BR2_DRAW_SYNC_FLAG_VIRTUAL,
        BR2_DRAW_SYNC_WAIT_LOOP_EXIT, BR2_DRAW_SYNC_WAIT_LOOP_INSTRUCTIONS,
        BR2_DRAW_SYNC_WAIT_LOOP_START, BR2_FRAME_COUNTER_WAIT_LOOP_GLOBAL_COUNTER,
        BR2_FRAME_COUNTER_WAIT_LOOP_INSTRUCTIONS, BR2_FRAME_COUNTER_WAIT_LOOP_STACK_OFFSET,
        BR2_FRAME_COUNTER_WAIT_LOOP_START, BR2_FRAME_COUNTER_WAIT_LOOP_TARGET_CHECK,
        BR2_FRAME_COUNTER_WAIT_LOOP_TARGET_CHECK_INSTRUCTIONS, BR2_IRQ_POLL_STATUS_ADDRESS,
        BR2_IRQ_POLL_STATUS_MASK, BR2_IRQ_POLL_TIMEOUT_INITIAL_DECREMENT,
        BR2_IRQ_POLL_TIMEOUT_INITIAL_INSTRUCTION, BR2_IRQ_POLL_TIMEOUT_LOOP_EXIT,
        BR2_IRQ_POLL_TIMEOUT_LOOP_INSTRUCTIONS, BR2_IRQ_POLL_TIMEOUT_LOOP_START,
        BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT,
        BR2_POST_VS_TABLE_ACCUM_LOOP_INSTRUCTIONS, BR2_POST_VS_TABLE_ACCUM_LOOP_START,
        BR2_POST_VS_TABLE_ACCUM_LOOP_TAIL_INCREMENT, BR2_POST_VS_TABLE_ACCUM_MIN_SKIP_ITERATIONS,
        BR2_REVERSE_MISMATCH_SCAN_CYCLES_PER_ITERATION,
        BR2_REVERSE_MISMATCH_SCAN_LOOP_INSTRUCTIONS, BR2_REVERSE_MISMATCH_SCAN_LOOP_START,
        BR2_REVERSE_POINTER_SCAN_CYCLES_PER_ITERATION, BR2_REVERSE_POINTER_SCAN_LOOP_EXIT,
        BR2_REVERSE_POINTER_SCAN_LOOP_INSTRUCTIONS, BR2_REVERSE_POINTER_SCAN_LOOP_START,
        BR2_REVERSE_POINTER_SCAN_MAX_SKIP_ITERATIONS, BR2_SMALL_BYTE_COPY_CYCLES_PER_BYTE,
        BR2_SMALL_BYTE_COPY_LOOP_EXIT, BR2_SMALL_BYTE_COPY_LOOP_INSTRUCTIONS,
        BR2_SMALL_BYTE_COPY_LOOP_START, CAUSE_BD, CAUSE_IP2, CP0_CAUSE, CP0_EPC, CP0_STATUS, Cpu,
        EXCEPTION_VECTOR, GTE_FLAG_DIVIDE_OVERFLOW, GTE_FLAG_ERROR, GTE_FLAG_SX2_SATURATED,
        GTE_FLAG_SY2_SATURATED, GTE_FRACTIONAL_BITS, StepOutcome, gte_leading_zero_count, gte_sxy,
    };
    use crate::native::bus::Bus;
    use crate::native::io::{DMA_INTERRUPT, DMA_SPU_CHCR};

    fn install_br2_irq_poll_timeout_loop(bus: &mut Bus) {
        bus.write_u32(
            BR2_IRQ_POLL_TIMEOUT_INITIAL_DECREMENT,
            BR2_IRQ_POLL_TIMEOUT_INITIAL_INSTRUCTION,
        );
        for (index, instruction) in BR2_IRQ_POLL_TIMEOUT_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            bus.write_u32(
                BR2_IRQ_POLL_TIMEOUT_LOOP_START + (index as u32) * 4,
                instruction,
            );
        }
    }

    fn install_br2_post_vs_table_accum_loop(bus: &mut Bus) {
        for (index, instruction) in BR2_POST_VS_TABLE_ACCUM_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            bus.write_u32(
                BR2_POST_VS_TABLE_ACCUM_LOOP_START + (index as u32) * 4,
                instruction,
            );
        }
    }

    fn install_br2_reverse_pointer_scan_loop(bus: &mut Bus) {
        for (index, instruction) in BR2_REVERSE_POINTER_SCAN_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            bus.write_u32(
                BR2_REVERSE_POINTER_SCAN_LOOP_START + (index as u32) * 4,
                instruction,
            );
        }
    }

    fn install_br2_reverse_mismatch_scan_loop(bus: &mut Bus) {
        for (offset, instruction) in BR2_REVERSE_MISMATCH_SCAN_LOOP_INSTRUCTIONS.iter().copied() {
            bus.write_u32(BR2_REVERSE_MISMATCH_SCAN_LOOP_START + offset, instruction);
        }
    }

    fn install_br2_small_byte_copy_loop(bus: &mut Bus) {
        for (index, instruction) in BR2_SMALL_BYTE_COPY_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            bus.write_u32(
                BR2_SMALL_BYTE_COPY_LOOP_START + (index as u32) * 4,
                instruction,
            );
        }
    }

    fn install_bios_c80_kernel_handler_prefix(bus: &mut Bus) {
        for (index, instruction) in BIOS_EXCEPTION_C80_KERNEL_HANDLER_PREFIX
            .iter()
            .copied()
            .enumerate()
        {
            bus.write_u32(0x0000_0c80 + (index as u32) * 4, instruction);
        }
    }

    fn install_bios_irq_dispatch_loop_signature(bus: &mut Bus) {
        for (address, instruction) in BIOS_IRQ_DISPATCH_LOOP_SIGNATURE {
            bus.write_u32(address, instruction);
        }
    }

    fn install_bios_exception_context(bus: &mut Bus, sp: u32, ra: u32) {
        let context_pointer_slot = 0xa000_e1ec;
        let context_pointer = 0xa000_e1f4;
        let context_base = context_pointer + BIOS_EXCEPTION_CONTEXT_POINTER_ADJUST;
        bus.write_u32(
            BIOS_EXCEPTION_CONTEXT_POINTER_PHYSICAL,
            context_pointer_slot,
        );
        bus.write_u32(context_pointer_slot, context_pointer);
        bus.write_u32(context_base + 0x40, 0x1111_0000);
        bus.write_u32(context_base + 0x48, 0x2222_0000);
        bus.write_u32(context_base + 0x74, sp);
        bus.write_u32(context_base + BIOS_EXCEPTION_CONTEXT_RA_OFFSET, ra);
        bus.write_u32(context_base + BIOS_EXCEPTION_CONTEXT_LO_OFFSET, 0x3333_0000);
        bus.write_u32(context_base + BIOS_EXCEPTION_CONTEXT_HI_OFFSET, 0x4444_0000);
    }

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
    fn fast_forwards_bios_decrement_delay_loop() {
        let mut rom = vec![0; 0xa9d0 + 4];
        let loop_offset = 0xa9b8usize;
        for (index, instruction) in [
            i_type(0x23, 29, 2, 0),   // lw v0, 0(sp)
            i_type(0x23, 29, 24, 0),  // lw t8, 0(sp)
            0,                        // nop
            i_type(0x09, 24, 25, -1), // addiu t9, t8, -1
            i_type(0x05, 2, 0, -5),   // bne v0, zero, loop start
            i_type(0x2b, 29, 25, 0),  // sw t9, 0(sp)
        ]
        .iter()
        .enumerate()
        {
            let offset = loop_offset + index * 4;
            rom[offset..offset + 4].copy_from_slice(&instruction.to_le_bytes());
        }

        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();
        cpu.pc = 0x1fc0_a9b8;
        cpu.next_pc = 0x1fc0_a9bc;
        cpu.regs[29] = 0x8001_0000;
        bus.write_u32(cpu.regs[29], 100);

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x1fc0_a9b8);
        assert_eq!(report.cycles_elapsed, 606);
        assert_eq!(cpu.pc, 0x1fc0_a9d0);
        assert_eq!(cpu.next_pc, 0x1fc0_a9d4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[24], 0);
        assert_eq!(cpu.regs[25], u32::MAX);
        assert_eq!(bus.read_u32(cpu.regs[29]), u32::MAX);
    }

    #[test]
    fn fast_forwards_bios_decrement_delay_loop_from_kseg1_alias() {
        let mut rom = vec![0; 0xa9d0 + 4];
        let loop_offset = 0xa9b8usize;
        for (index, instruction) in [
            i_type(0x23, 29, 2, 0),   // lw v0, 0(sp)
            i_type(0x23, 29, 24, 0),  // lw t8, 0(sp)
            0,                        // nop
            i_type(0x09, 24, 25, -1), // addiu t9, t8, -1
            i_type(0x05, 2, 0, -5),   // bne v0, zero, loop start
            i_type(0x2b, 29, 25, 0),  // sw t9, 0(sp)
        ]
        .iter()
        .enumerate()
        {
            let offset = loop_offset + index * 4;
            rom[offset..offset + 4].copy_from_slice(&instruction.to_le_bytes());
        }

        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();
        cpu.pc = 0xbfc0_a9b8;
        cpu.next_pc = 0xbfc0_a9bc;
        cpu.regs[29] = 0x8001_0000;
        bus.write_u32(cpu.regs[29], 100);

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0xbfc0_a9b8);
        assert_eq!(report.cycles_elapsed, 606);
        assert_eq!(cpu.pc, 0xbfc0_a9d0);
        assert_eq!(cpu.next_pc, 0xbfc0_a9d4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[24], 0);
        assert_eq!(cpu.regs[25], u32::MAX);
        assert_eq!(bus.read_u32(cpu.regs[29]), u32::MAX);
    }

    #[test]
    fn fast_forwards_bios_short_decrement_delay_loop() {
        let loop_offset = (BIOS_SHORT_DELAY_LOOP_START - 0x1fc0_0000) as usize;
        let mut rom = vec![0; loop_offset + BIOS_DELAY_LOOP_INSTRUCTIONS.len() * 4];
        for (index, instruction) in BIOS_DELAY_LOOP_INSTRUCTIONS.iter().copied().enumerate() {
            let offset = loop_offset + index * 4;
            rom[offset..offset + 4].copy_from_slice(&instruction.to_le_bytes());
        }

        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();
        cpu.pc = BIOS_SHORT_DELAY_LOOP_START;
        cpu.next_pc = BIOS_SHORT_DELAY_LOOP_START + 4;
        cpu.regs[29] = 0x8001_0000;
        bus.write_u32(cpu.regs[29], 100);

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BIOS_SHORT_DELAY_LOOP_START);
        assert_eq!(report.cycles_elapsed, 606);
        assert_eq!(cpu.pc, BIOS_SHORT_DELAY_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BIOS_SHORT_DELAY_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[24], 0);
        assert_eq!(cpu.regs[25], u32::MAX);
        assert_eq!(bus.read_u32(cpu.regs[29]), u32::MAX);
    }

    #[test]
    fn fast_forwards_bios_delay_prologue_loop() {
        let mut rom = vec![0; 0xa9d0 + 4];
        let loop_offset = (BIOS_DELAY_PROLOGUE_LOOP_START - 0x1fc0_0000) as usize;
        for (index, instruction) in BIOS_DELAY_PROLOGUE_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            let offset = loop_offset + index * 4;
            rom[offset..offset + 4].copy_from_slice(&instruction.to_le_bytes());
        }

        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();
        cpu.pc = BIOS_DELAY_PROLOGUE_LOOP_START;
        cpu.next_pc = BIOS_DELAY_PROLOGUE_LOOP_START + 4;
        cpu.regs[29] = 0x8001_0000;
        bus.write_u32(cpu.regs[29], 100);

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BIOS_DELAY_PROLOGUE_LOOP_START);
        assert_eq!(report.cycles_elapsed, 900);
        assert_eq!(cpu.pc, BIOS_DELAY_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BIOS_DELAY_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[14], 0);
        assert_eq!(cpu.regs[15], u32::MAX);
        assert_eq!(bus.read_u32(cpu.regs[29]), u32::MAX);
    }

    #[test]
    fn fast_forwards_bios_byte_copy_loop() {
        let loop_offset = (BIOS_BYTE_COPY_LOOP_START - 0x1fc0_0000) as usize;
        let mut rom = vec![0; loop_offset + BIOS_BYTE_COPY_LOOP_INSTRUCTIONS.len() * 4];
        for (index, instruction) in BIOS_BYTE_COPY_LOOP_INSTRUCTIONS.iter().copied().enumerate() {
            let offset = loop_offset + index * 4;
            rom[offset..offset + 4].copy_from_slice(&instruction.to_le_bytes());
        }

        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();
        let source = 0x8001_0000;
        let destination = 0x8001_1000;
        for index in 0..12 {
            bus.write_u8(source + index, (index + 1) as u8);
        }
        cpu.pc = BIOS_BYTE_COPY_LOOP_START;
        cpu.next_pc = BIOS_BYTE_COPY_LOOP_START + 4;
        cpu.regs[3] = 1;
        cpu.regs[4] = source + 12;
        cpu.regs[16] = destination;
        cpu.regs[17] = source;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BIOS_BYTE_COPY_LOOP_START);
        assert_eq!(report.cycles_elapsed, 105);
        assert_eq!(
            cpu.pc,
            BIOS_BYTE_COPY_LOOP_START + (BIOS_BYTE_COPY_LOOP_INSTRUCTIONS.len() as u32) * 4
        );
        assert_eq!(cpu.next_pc, cpu.pc + 4);
        assert_eq!(cpu.regs[1], 0);
        assert_eq!(cpu.regs[2], 11);
        assert_eq!(cpu.regs[8], 10);
        assert_eq!(cpu.regs[9], 11);
        assert_eq!(cpu.regs[10], 12);
        assert_eq!(cpu.regs[16], destination + 12);
        assert_eq!(cpu.regs[17], source + 12);
        assert_eq!(cpu.regs[25], 20);
        for index in 0..12 {
            assert_eq!(bus.read_u8(destination + index), (index + 1) as u8);
        }
    }

    #[test]
    fn fast_forwards_bios_init_zero_fill_loop() {
        let loop_offset = (BIOS_INIT_ZERO_FILL_LOOP_START - 0x1fc0_0000) as usize;
        let mut rom = vec![0; loop_offset + BIOS_INIT_ZERO_FILL_LOOP_INSTRUCTIONS.len() * 4];
        for (index, instruction) in BIOS_INIT_ZERO_FILL_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            let offset = loop_offset + index * 4;
            rom[offset..offset + 4].copy_from_slice(&instruction.to_le_bytes());
        }

        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();
        cpu.pc = BIOS_INIT_ZERO_FILL_LOOP_START;
        cpu.next_pc = BIOS_INIT_ZERO_FILL_LOOP_START + 4;
        cpu.regs[2] = 0xa000_9000;
        cpu.regs[3] = 0xa000_9020;
        for index in 0..8 {
            bus.write_u32(0x8000_9000 + index * 4, 0xffff_ffff);
        }

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BIOS_INIT_ZERO_FILL_LOOP_START);
        assert_eq!(report.cycles_elapsed, 40);
        assert_eq!(cpu.pc, BIOS_INIT_ZERO_FILL_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BIOS_INIT_ZERO_FILL_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[1], 0);
        assert_eq!(cpu.regs[2], 0xa000_9020);
        for index in 0..8 {
            assert_eq!(bus.read_u32(0x8000_9000 + index * 4), 0);
        }
    }

    #[test]
    fn fast_forwards_br2_draw_sync_wait_loop_to_next_vblank() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        for (index, instruction) in BR2_DRAW_SYNC_WAIT_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            bus.write_u32(
                BR2_DRAW_SYNC_WAIT_LOOP_START + (index as u32) * 4,
                instruction,
            );
        }
        bus.write_u32(BR2_DRAW_SYNC_FLAG_VIRTUAL, 1);
        bus.tick(565_900);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_DRAW_SYNC_WAIT_LOOP_START;
        cpu.next_pc = BR2_DRAW_SYNC_WAIT_LOOP_START + 4;
        cpu.regs[3] = BR2_DRAW_SYNC_FLAG_VIRTUAL - 0x2210;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_DRAW_SYNC_WAIT_LOOP_START);
        assert_eq!(report.cycles_elapsed, 100);
        assert_eq!(cpu.pc, BR2_DRAW_SYNC_WAIT_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_DRAW_SYNC_WAIT_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(bus.vblank_count(), 1);
        assert_eq!(bus.read_u32(BR2_DRAW_SYNC_FLAG_VIRTUAL), 0);
    }

    #[test]
    fn fast_forwards_br2_frame_counter_wait_loop_to_next_vblank() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        for (index, instruction) in BR2_FRAME_COUNTER_WAIT_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            let address = BR2_FRAME_COUNTER_WAIT_LOOP_START + (index as u32) * 4;
            bus.write_u32(address, instruction);
        }
        for (index, instruction) in BR2_FRAME_COUNTER_WAIT_LOOP_TARGET_CHECK_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            let address = BR2_FRAME_COUNTER_WAIT_LOOP_TARGET_CHECK + (index as u32) * 4;
            bus.write_u32(address, instruction);
        }
        bus.write_u32(BR2_FRAME_COUNTER_WAIT_LOOP_GLOBAL_COUNTER, 31);
        bus.tick(565_820);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_FRAME_COUNTER_WAIT_LOOP_START;
        cpu.next_pc = BR2_FRAME_COUNTER_WAIT_LOOP_START + 4;
        cpu.regs[4] = 32;
        cpu.regs[29] = 0x8001_0000;
        bus.write_u32(cpu.regs[29] + BR2_FRAME_COUNTER_WAIT_LOOP_STACK_OFFSET, 100);

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_FRAME_COUNTER_WAIT_LOOP_START);
        assert_eq!(report.cycles_elapsed, 180);
        assert_eq!(cpu.pc, BR2_FRAME_COUNTER_WAIT_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_FRAME_COUNTER_WAIT_LOOP_START + 4);
        assert_eq!(bus.vblank_count(), 1);
        assert_eq!(
            bus.read_u32(cpu.regs[29] + BR2_FRAME_COUNTER_WAIT_LOOP_STACK_OFFSET),
            90
        );
    }

    #[test]
    fn fast_forwards_br2_irq_poll_timeout_loop_from_compare() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_irq_poll_timeout_loop(&mut bus);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_IRQ_POLL_TIMEOUT_LOOP_START;
        cpu.next_pc = BR2_IRQ_POLL_TIMEOUT_LOOP_START + 4;
        cpu.regs[3] = 3;
        cpu.regs[4] = BR2_IRQ_POLL_STATUS_ADDRESS;
        cpu.regs[5] = u32::MAX;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_IRQ_POLL_TIMEOUT_LOOP_START);
        assert_eq!(report.cycles_elapsed, 34);
        assert_eq!(cpu.pc, BR2_IRQ_POLL_TIMEOUT_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_IRQ_POLL_TIMEOUT_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[3], u32::MAX);
    }

    #[test]
    fn fast_forwards_br2_irq_poll_timeout_loop_from_initial_decrement() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_irq_poll_timeout_loop(&mut bus);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_IRQ_POLL_TIMEOUT_INITIAL_DECREMENT;
        cpu.next_pc = BR2_IRQ_POLL_TIMEOUT_INITIAL_DECREMENT + 4;
        cpu.regs[3] = 4;
        cpu.regs[4] = BR2_IRQ_POLL_STATUS_ADDRESS;
        cpu.regs[5] = u32::MAX;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_IRQ_POLL_TIMEOUT_INITIAL_DECREMENT);
        assert_eq!(report.cycles_elapsed, 35);
        assert_eq!(cpu.pc, BR2_IRQ_POLL_TIMEOUT_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_IRQ_POLL_TIMEOUT_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[3], u32::MAX);
    }

    #[test]
    fn br2_irq_poll_timeout_loop_does_not_fast_forward_when_irq_bit_is_set() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_irq_poll_timeout_loop(&mut bus);
        bus.io.irq.status = u32::from(BR2_IRQ_POLL_STATUS_MASK);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_IRQ_POLL_TIMEOUT_LOOP_START;
        cpu.next_pc = BR2_IRQ_POLL_TIMEOUT_LOOP_START + 4;
        cpu.regs[3] = 3;
        cpu.regs[4] = BR2_IRQ_POLL_STATUS_ADDRESS;
        cpu.regs[5] = u32::MAX;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_IRQ_POLL_TIMEOUT_LOOP_START);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, BR2_IRQ_POLL_TIMEOUT_LOOP_START + 4);
        assert_eq!(cpu.next_pc, BR2_IRQ_POLL_TIMEOUT_LOOP_START + 8);
        assert_eq!(cpu.regs[3], 3);
    }

    #[test]
    fn fast_forwards_br2_byte_copy_loop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        for (index, instruction) in BR2_BYTE_COPY_LOOP_INSTRUCTIONS.iter().copied().enumerate() {
            bus.write_u32(BR2_BYTE_COPY_LOOP_START + (index as u32) * 4, instruction);
        }
        let source = 0x8001_0000;
        let destination = 0x8001_1000;
        for index in 0..7 {
            bus.write_u8(source + index, (0xa0 + index) as u8);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_BYTE_COPY_LOOP_START;
        cpu.next_pc = BR2_BYTE_COPY_LOOP_START + 4;
        cpu.regs[3] = 7;
        cpu.regs[4] = destination;
        cpu.regs[7] = source;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_BYTE_COPY_LOOP_START);
        assert_eq!(report.cycles_elapsed, 56);
        assert_eq!(cpu.pc, BR2_BYTE_COPY_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_BYTE_COPY_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0xa6);
        assert_eq!(cpu.regs[3], 0);
        assert_eq!(cpu.regs[4], destination + 7);
        assert_eq!(cpu.regs[7], source + 7);
        for index in 0..7 {
            assert_eq!(bus.read_u8(destination + index), (0xa0 + index) as u8);
        }
    }

    #[test]
    fn fast_forwards_br2_banked_halfword_copy_loop() {
        let mut banked = vec![0; 0x0080_0000];
        for (index, value) in [0x1122u16, 0x3344, 0x5566, 0x7788]
            .iter()
            .copied()
            .enumerate()
        {
            let offset = 2 + index * 2;
            banked[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        let mut bus = Bus::with_banked_roms(Vec::new(), banked, 4 * 1024 * 1024);
        for (offset, instruction) in BR2_BANKED_HALFWORD_COPY_LOOP_INSTRUCTIONS {
            bus.write_u32(BR2_BANKED_HALFWORD_COPY_LOOP_START + offset, instruction);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_BANKED_HALFWORD_COPY_LOOP_START;
        cpu.next_pc = BR2_BANKED_HALFWORD_COPY_LOOP_START + 4;
        cpu.regs[3] = 0x1f00_0002;
        cpu.regs[16] = 0;
        cpu.regs[17] = 2;
        cpu.regs[18] = 0x8001_0001;
        cpu.regs[19] = BR2_BANKED_HALFWORD_COPY_MASK;
        cpu.regs[20] = 8;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_BANKED_HALFWORD_COPY_LOOP_START);
        assert_eq!(report.cycles_elapsed, 52);
        assert_eq!(cpu.pc, BR2_BANKED_HALFWORD_COPY_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_BANKED_HALFWORD_COPY_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[3], 0x1f00_000a);
        assert_eq!(cpu.regs[16], 8);
        assert_eq!(cpu.regs[17], 10);
        assert_eq!(cpu.regs[18], 0x8001_0009);
        assert_eq!(bus.read_u16(0x8001_0001), 0x1122);
        assert_eq!(bus.read_u16(0x8001_0003), 0x3344);
        assert_eq!(bus.read_u16(0x8001_0005), 0x5566);
        assert_eq!(bus.read_u16(0x8001_0007), 0x7788);
    }

    #[test]
    fn fast_forwards_br2_post_vs_unmapped_table_accum_loop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let start_index = 100u32;
        let limit = 5_000u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, 0x8300_0000);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 4;
        cpu.regs[2] = count_address;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_index;
        cpu.regs[6] = 0x10;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START);
        assert_eq!(report.cycles_elapsed, 98_000);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], count_address);
        assert_eq!(cpu.regs[3], table_meta_offset);
        assert_eq!(cpu.regs[5], limit);
    }

    #[test]
    fn fast_forwards_br2_post_vs_unmapped_table_accum_loop_across_noop_expansion() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let table_base = 0x8300_0000;
        let expansion_noop_words = (0x1f80_0000 - 0x0300_0000) / 4;
        let start_index = expansion_noop_words - 2_048;
        let limit = start_index + 5_000;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, table_base);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 4;
        cpu.regs[2] = count_address;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_index;
        cpu.regs[6] = 0x10;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            5_000 * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], limit);
    }

    #[test]
    fn fast_forwards_br2_reverse_mismatch_scan_loop_in_place() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_reverse_mismatch_scan_loop(&mut bus);
        let pointer = 0x803a_4000;
        let expected = 0x003a_4000;
        let sentinel = 0xfeed_face;
        let count = 96u32;
        for index in 0..count {
            bus.write_u32(pointer - index * 4, 0x1000_0000 + index);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_REVERSE_MISMATCH_SCAN_LOOP_START;
        cpu.next_pc = BR2_REVERSE_MISMATCH_SCAN_LOOP_START + 4;
        cpu.regs[3] = expected;
        cpu.regs[4] = pointer;
        cpu.regs[5] = count;
        cpu.regs[8] = sentinel;

        let report = cpu.step_report(&mut bus);

        let skipped = count - 1;
        assert_eq!(report.start_pc, BR2_REVERSE_MISMATCH_SCAN_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(skipped) * BR2_REVERSE_MISMATCH_SCAN_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_REVERSE_MISMATCH_SCAN_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_REVERSE_MISMATCH_SCAN_LOOP_START + 4);
        assert_eq!(cpu.regs[2], 0x1000_0000 + skipped - 1);
        assert_eq!(cpu.regs[3], expected - skipped * 4);
        assert_eq!(cpu.regs[4], pointer - skipped * 4);
        assert_eq!(cpu.regs[5], 1);
        assert_eq!(cpu.regs[9], pointer - skipped * 4 - 4);
    }

    #[test]
    fn fast_forwards_br2_reverse_mismatch_scan_loop_until_sentinel() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_reverse_mismatch_scan_loop(&mut bus);
        let pointer = 0x803a_6000;
        let expected = 0x003a_6000;
        let sentinel = 0xfeed_face;
        let sentinel_index = 64u32;
        for index in 0..128 {
            let value = if index == sentinel_index {
                sentinel
            } else {
                0x2000_0000 + index
            };
            bus.write_u32(pointer - index * 4, value);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_REVERSE_MISMATCH_SCAN_LOOP_START;
        cpu.next_pc = BR2_REVERSE_MISMATCH_SCAN_LOOP_START + 4;
        cpu.regs[3] = expected;
        cpu.regs[4] = pointer;
        cpu.regs[5] = 128;
        cpu.regs[8] = sentinel;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_REVERSE_MISMATCH_SCAN_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(sentinel_index) * BR2_REVERSE_MISMATCH_SCAN_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_REVERSE_MISMATCH_SCAN_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_REVERSE_MISMATCH_SCAN_LOOP_START + 4);
        assert_eq!(cpu.regs[2], 0x2000_0000 + sentinel_index - 1);
        assert_eq!(cpu.regs[3], expected - sentinel_index * 4);
        assert_eq!(cpu.regs[4], pointer - sentinel_index * 4);
        assert_eq!(cpu.regs[5], 128 - sentinel_index);
        assert_eq!(bus.read_u32(cpu.regs[4]), sentinel);
    }

    #[test]
    fn fast_forwards_br2_small_byte_copy_loop_to_exit() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_small_byte_copy_loop(&mut bus);
        let source = 0x8001_0000;
        let destination = 0x8001_1000;
        let count = 20u32;
        for index in 0..count {
            bus.write_u8(source + index, (0xa0 + index) as u8);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_SMALL_BYTE_COPY_LOOP_START;
        cpu.next_pc = BR2_SMALL_BYTE_COPY_LOOP_START + 4;
        cpu.regs[3] = destination;
        cpu.regs[5] = source;
        cpu.regs[6] = count;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_SMALL_BYTE_COPY_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(count) * BR2_SMALL_BYTE_COPY_CYCLES_PER_BYTE
        );
        assert_eq!(cpu.pc, BR2_SMALL_BYTE_COPY_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_SMALL_BYTE_COPY_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0xa0 + count - 1);
        assert_eq!(cpu.regs[3], destination + count);
        assert_eq!(cpu.regs[5], source + count);
        assert_eq!(cpu.regs[6], 0);
        for index in 0..count {
            assert_eq!(bus.read_u8(destination + index), (0xa0 + index) as u8);
        }
    }

    #[test]
    fn fast_forwards_br2_single_byte_copy_loop_to_exit() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_small_byte_copy_loop(&mut bus);
        let source = 0x8001_0100;
        let destination = 0x8001_1100;
        bus.write_u8(source, 0x7b);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_SMALL_BYTE_COPY_LOOP_START;
        cpu.next_pc = BR2_SMALL_BYTE_COPY_LOOP_START + 4;
        cpu.regs[3] = destination;
        cpu.regs[5] = source;
        cpu.regs[6] = 1;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_SMALL_BYTE_COPY_LOOP_START);
        assert_eq!(report.cycles_elapsed, BR2_SMALL_BYTE_COPY_CYCLES_PER_BYTE);
        assert_eq!(cpu.pc, BR2_SMALL_BYTE_COPY_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_SMALL_BYTE_COPY_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0x7b);
        assert_eq!(cpu.regs[3], destination + 1);
        assert_eq!(cpu.regs[5], source + 1);
        assert_eq!(cpu.regs[6], 0);
        assert_eq!(bus.read_u8(destination), 0x7b);
    }

    #[test]
    fn fast_forwards_br2_small_byte_copy_loop_in_capped_chunks() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_small_byte_copy_loop(&mut bus);
        let source = 0x8001_2000;
        let destination = 0x8001_4000;
        let count = 4_500u32;
        for index in 0..count {
            bus.write_u8(source + index, (index & 0xff) as u8);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_SMALL_BYTE_COPY_LOOP_START;
        cpu.next_pc = BR2_SMALL_BYTE_COPY_LOOP_START + 4;
        cpu.regs[3] = destination;
        cpu.regs[5] = source;
        cpu.regs[6] = count;

        let report = cpu.step_report(&mut bus);

        let copied = 4096u32;
        assert_eq!(report.start_pc, BR2_SMALL_BYTE_COPY_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(copied) * BR2_SMALL_BYTE_COPY_CYCLES_PER_BYTE
        );
        assert_eq!(cpu.pc, BR2_SMALL_BYTE_COPY_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_SMALL_BYTE_COPY_LOOP_START + 4);
        assert_eq!(cpu.regs[2], ((copied - 1) & 0xff));
        assert_eq!(cpu.regs[3], destination + copied);
        assert_eq!(cpu.regs[5], source + copied);
        assert_eq!(cpu.regs[6], count - copied);
        assert_eq!(
            bus.read_u8(destination + copied - 1),
            ((copied - 1) & 0xff) as u8
        );
    }

    #[test]
    fn fast_forwards_br2_reverse_pointer_scan_loop_until_mismatch() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_reverse_pointer_scan_loop(&mut bus);
        let pointer = 0x803a_2000;
        let expected = 0x003a_2000;
        let iterations = 96u32;
        for index in 0..iterations {
            bus.write_u32(pointer - index * 4, expected - (index + 1) * 4);
        }
        bus.write_u32(pointer - iterations * 4, 0x1234_5678);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_REVERSE_POINTER_SCAN_LOOP_START;
        cpu.next_pc = BR2_REVERSE_POINTER_SCAN_LOOP_START + 4;
        cpu.regs[3] = expected;
        cpu.regs[5] = iterations + 8;
        cpu.regs[9] = pointer;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_REVERSE_POINTER_SCAN_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(iterations + 1) * BR2_REVERSE_POINTER_SCAN_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_REVERSE_POINTER_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_REVERSE_POINTER_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0x1234_5678);
        assert_eq!(cpu.regs[3], expected - (iterations + 1) * 4);
        assert_eq!(cpu.regs[5], 7);
        assert_eq!(cpu.regs[9], pointer - (iterations + 1) * 4);
    }

    #[test]
    fn fast_forwards_br2_reverse_pointer_scan_loop_in_capped_chunks() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_reverse_pointer_scan_loop(&mut bus);
        let pointer = 0x803a_8000;
        let expected = 0x003a_8000;
        let count = 9_000u32;
        for index in 0..BR2_REVERSE_POINTER_SCAN_MAX_SKIP_ITERATIONS {
            bus.write_u32(pointer - index * 4, expected - (index + 1) * 4);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_REVERSE_POINTER_SCAN_LOOP_START;
        cpu.next_pc = BR2_REVERSE_POINTER_SCAN_LOOP_START + 4;
        cpu.regs[3] = expected;
        cpu.regs[5] = count;
        cpu.regs[9] = pointer;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_REVERSE_POINTER_SCAN_LOOP_START);
        assert_eq!(cpu.pc, BR2_REVERSE_POINTER_SCAN_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_REVERSE_POINTER_SCAN_LOOP_START + 4);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(BR2_REVERSE_POINTER_SCAN_MAX_SKIP_ITERATIONS)
                * BR2_REVERSE_POINTER_SCAN_CYCLES_PER_ITERATION
        );
        assert_eq!(
            cpu.regs[3],
            expected - BR2_REVERSE_POINTER_SCAN_MAX_SKIP_ITERATIONS * 4
        );
        assert_eq!(
            cpu.regs[5],
            count - BR2_REVERSE_POINTER_SCAN_MAX_SKIP_ITERATIONS
        );
        assert_eq!(
            cpu.regs[9],
            pointer - BR2_REVERSE_POINTER_SCAN_MAX_SKIP_ITERATIONS * 4
        );
    }

    #[test]
    fn fast_forwards_br2_post_vs_tail_with_pending_limit_load() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let current_index = 100u32;
        let limit = 5_000u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, 0x8300_0000);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_TAIL_INCREMENT;
        cpu.next_pc = BR2_POST_VS_TABLE_ACCUM_LOOP_TAIL_INCREMENT + 4;
        cpu.regs[2] = count_address;
        cpu.regs[3] = table_meta_offset;
        cpu.regs[4] = owner;
        cpu.regs[5] = current_index;
        cpu.regs[6] = 0x10;
        cpu.pending_load = Some((2, limit));

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_TAIL_INCREMENT);
        assert_eq!(report.cycles_elapsed, 97_980);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], count_address);
        assert_eq!(cpu.regs[3], table_meta_offset);
        assert_eq!(cpu.regs[5], limit);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_post_vs_mapped_table_accum_loop_and_preserves_writes() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_2000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let table_base = 0x8003_0000;
        let start_index = 4u32;
        let limit = 5_000u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, table_base);
        bus.write_u32(table_base + start_index * 4, 0x10);
        bus.write_u32(table_base + (start_index + 1) * 4, 0x20);
        bus.write_u32(table_base + (limit - 1) * 4, 0x30);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 4;
        cpu.regs[2] = count_address;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_index;
        cpu.regs[6] = 0x10;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], count_address);
        assert_eq!(cpu.regs[3], table_meta_offset);
        assert_eq!(cpu.regs[5], limit);
        assert_eq!(bus.read_u32(table_base + start_index * 4), owner + 0x10);
        assert_eq!(
            bus.read_u32(table_base + (start_index + 1) * 4),
            owner + 0x20
        );
        assert_eq!(bus.read_u32(table_base + (limit - 1) * 4), owner + 0x30);
    }

    #[test]
    fn takes_pending_interrupt_before_br2_post_vs_fast_forward() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let table_base = 0x8003_0000;
        let start_index = 100u32;
        let limit = 5_000u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, table_base);
        bus.io.irq.status = 1;
        bus.io.irq.mask = 1;

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 4;
        cpu.cp0[CP0_STATUS] = 1 | CAUSE_IP2;
        cpu.regs[2] = count_address;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_index;
        cpu.regs[6] = 0x10;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(cpu.pc, EXCEPTION_VECTOR);
        assert_eq!(cpu.next_pc, EXCEPTION_VECTOR + 4);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, CAUSE_IP2);
        assert_eq!(cpu.cp0[CP0_EPC], BR2_POST_VS_TABLE_ACCUM_LOOP_START);
    }

    #[test]
    fn hle_acknowledges_vblank_irq_when_bios_c80_handler_is_blank() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        for (index, instruction) in BIOS_EXCEPTION_VECTOR_TO_C80_STUB
            .iter()
            .copied()
            .enumerate()
        {
            bus.write_u32(EXCEPTION_VECTOR + (index as u32) * 4, instruction);
        }
        bus.io.irq.status = 9;
        bus.io.irq.mask = 9;

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 4;
        cpu.cp0[CP0_STATUS] = 1 | CAUSE_IP2;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START);
        assert_eq!(report.instruction, None);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 4);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(bus.io.irq.status, 0);
    }

    #[test]
    fn hle_acknowledges_dma_irq_when_bios_c80_handler_is_blank() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        for (index, instruction) in BIOS_EXCEPTION_VECTOR_TO_C80_STUB
            .iter()
            .copied()
            .enumerate()
        {
            bus.write_u32(EXCEPTION_VECTOR + (index as u32) * 4, instruction);
        }
        bus.write_u32(DMA_INTERRUPT, (1 << 23) | (1 << 20));
        bus.write_u32(DMA_SPU_CHCR, 1 << 24);
        bus.io.irq.mask = 1 << 3;

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 4;
        cpu.cp0[CP0_STATUS] = 1 | CAUSE_IP2;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START);
        assert_eq!(report.instruction, None);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 4);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(bus.io.irq.status & (1 << 3), 0);
        assert!(!bus.io.dma.irq_pending());
    }

    #[test]
    fn hle_returns_from_br2_post_vs_bios_irq_handler() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_bios_c80_kernel_handler_prefix(&mut bus);
        bus.io.irq.status = 1;
        bus.io.irq.mask = 9;
        bus.write_u32(DMA_INTERRUPT, (1 << 23) | (1 << 20));
        bus.write_u32(DMA_SPU_CHCR, 1 << 24);

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_0c94;
        cpu.next_pc = 0x0000_0c98;
        cpu.cp0[CP0_STATUS] = 0x4000_0404;
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2;
        cpu.cp0[CP0_EPC] = BR2_POST_VS_TABLE_ACCUM_LOOP_START;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_0c94);
        assert_eq!(report.instruction, None);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 4);
        assert_eq!(cpu.cp0[CP0_STATUS], 0x4000_0401);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, 0);
        assert_eq!(bus.io.irq.status & 9, 0);
        assert!(!bus.io.dma.irq_pending());
    }

    #[test]
    fn hle_returns_from_br2_draw_sync_bios_irq_dispatch_loop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_bios_irq_dispatch_loop_signature(&mut bus);
        install_bios_exception_context(&mut bus, 0x803f_ff70, 0x802d_07d0);
        bus.io.irq.status = 1;
        bus.io.irq.mask = 9;

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_1b84;
        cpu.next_pc = 0x0000_1b88;
        cpu.regs[16] = 0x0000_1234;
        cpu.regs[18] = 0x0000_5678;
        cpu.regs[29] = 0x0000_8b30;
        cpu.regs[31] = 0x0000_18d0;
        cpu.hi = 0xaaaa_aaaa;
        cpu.lo = 0xbbbb_bbbb;
        cpu.cp0[CP0_STATUS] = 0x4000_0404;
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2;
        cpu.cp0[CP0_EPC] = BR2_DRAW_SYNC_WAIT_LOOP_EXIT;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_1b84);
        assert_eq!(report.instruction, None);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, BR2_DRAW_SYNC_WAIT_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_DRAW_SYNC_WAIT_LOOP_EXIT + 4);
        assert_eq!(cpu.cp0[CP0_STATUS], 0x4000_0401);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, 0);
        assert_eq!(cpu.regs[16], 0x1111_0000);
        assert_eq!(cpu.regs[18], 0x2222_0000);
        assert_eq!(cpu.regs[29], 0x803f_ff70);
        assert_eq!(cpu.regs[31], 0x802d_07d0);
        assert_eq!(cpu.lo, 0x3333_0000);
        assert_eq!(cpu.hi, 0x4444_0000);
        assert_eq!(bus.io.irq.status & 9, 0);
    }

    #[test]
    fn caps_br2_post_vs_fast_forward_before_vblank_irq() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let table_base = 0x8003_0000;
        let start_index = 100u32;
        let limit = 5_000u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, table_base);
        bus.io.irq.mask = 1;
        bus.tick(550_000);

        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let expected_iterations =
            ((cycles_until_vblank - 1) / BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION) as u32;
        assert!(expected_iterations >= BR2_POST_VS_TABLE_ACCUM_MIN_SKIP_ITERATIONS);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 4;
        cpu.cp0[CP0_STATUS] = 1 | CAUSE_IP2;
        cpu.regs[2] = count_address;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_index;
        cpu.regs[6] = 0x10;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(expected_iterations) * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 4);
        assert_eq!(cpu.regs[5], start_index + expected_iterations);
        assert_eq!(bus.vblank_count(), 0);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn caps_br2_post_vs_noop_expansion_before_vblank_irq() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let table_base = 0x8300_0000;
        let start_index = 0u32;
        let limit = 0x0303_0303u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, table_base);
        bus.io.irq.mask = 1;
        bus.tick(550_000);

        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let expected_iterations =
            ((cycles_until_vblank - 1) / BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION) as u32;
        assert!(expected_iterations >= BR2_POST_VS_TABLE_ACCUM_MIN_SKIP_ITERATIONS);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 4;
        cpu.cp0[CP0_STATUS] = 1 | CAUSE_IP2;
        cpu.regs[2] = count_address;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_index;
        cpu.regs[6] = 0x10;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(limit) * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], limit);
        assert!(bus.vblank_count() > 0);
        assert_eq!(bus.io.irq.status & 1, 1);
    }

    #[test]
    fn br2_post_vs_table_accum_loop_skips_noop_expansion_without_touching_scratchpad() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, 5_000);
        bus.write_u32(count_address + 4, 0x9f7f_fdf0);
        bus.write_u32(0x1f80_0000, 0xfeed_beef);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 4;
        cpu.regs[2] = count_address;
        cpu.regs[4] = owner;
        cpu.regs[5] = 0;
        cpu.regs[6] = 0x10;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            5_000 * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], 5_000);
        assert_eq!(bus.read_u32(0x1f80_0000), 0xfeed_beef);
    }

    #[test]
    fn fast_forwards_br2_boot_word_copy_loop() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        for (index, instruction) in [
            i_type(0x23, 4, 7, 0),  // lw a3, 0(a0)
            0,                      // nop
            i_type(0x2b, 5, 7, 0),  // sw a3, 0(a1)
            0,                      // nop
            i_type(0x08, 4, 4, 4),  // addi a0, a0, 4
            i_type(0x08, 5, 5, 4),  // addi a1, a1, 4
            i_type(0x08, 6, 6, -4), // addi a2, a2, -4
            i_type(0x07, 6, 0, -8), // bgtz a2, loop start
            0,                      // nop
        ]
        .iter()
        .enumerate()
        {
            bus.write_u32(
                BR2_BOOT_WORD_COPY_LOOP_START + (index as u32) * 4,
                *instruction,
            );
        }
        for (index, value) in [0x1122_3344, 0x5566_7788, 0x99aa_bbcc, 0xddee_ff00]
            .iter()
            .copied()
            .enumerate()
        {
            bus.write_u32(0x8000_1000 + (index as u32) * 4, value);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_BOOT_WORD_COPY_LOOP_START;
        cpu.next_pc = BR2_BOOT_WORD_COPY_LOOP_START + 4;
        cpu.regs[4] = 0x8000_1000;
        cpu.regs[5] = 0x8000_2000;
        cpu.regs[6] = 16;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_BOOT_WORD_COPY_LOOP_START);
        assert_eq!(report.cycles_elapsed, 44);
        assert_eq!(cpu.pc, BR2_BOOT_WORD_COPY_LOOP_START + 36);
        assert_eq!(cpu.next_pc, BR2_BOOT_WORD_COPY_LOOP_START + 40);
        assert_eq!(cpu.regs[4], 0x8000_1010);
        assert_eq!(cpu.regs[5], 0x8000_2010);
        assert_eq!(cpu.regs[6], 0);
        assert_eq!(cpu.regs[7], 0xddee_ff00);
        assert_eq!(bus.read_u32(0x8000_2000), 0x1122_3344);
        assert_eq!(bus.read_u32(0x8000_2004), 0x5566_7788);
        assert_eq!(bus.read_u32(0x8000_2008), 0x99aa_bbcc);
        assert_eq!(bus.read_u32(0x8000_200c), 0xddee_ff00);
    }

    #[test]
    fn fast_forwards_br2_boot_zero_fill_loop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        for (index, instruction) in [
            i_type(0x2b, 2, 0, 0),    // sw zero, 0(v0)
            i_type(0x09, 2, 2, 4),    // addiu v0, v0, 4
            r_type(2, 3, 1, 0, 0x2b), // sltu at, v0, v1
            i_type(0x05, 1, 0, -4),   // bne at, zero, loop start
            0,                        // nop
        ]
        .iter()
        .enumerate()
        {
            bus.write_u32(
                BR2_BOOT_ZERO_FILL_LOOP_START + (index as u32) * 4,
                *instruction,
            );
        }
        for index in 0..4 {
            bus.write_u32(0x8001_0000 + index * 4, 0xffff_ffff);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_BOOT_ZERO_FILL_LOOP_START;
        cpu.next_pc = BR2_BOOT_ZERO_FILL_LOOP_START + 4;
        cpu.regs[1] = 1;
        cpu.regs[2] = 0x8001_0000;
        cpu.regs[3] = 0x8001_0010;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_BOOT_ZERO_FILL_LOOP_START);
        assert_eq!(report.cycles_elapsed, 24);
        assert_eq!(cpu.pc, BR2_BOOT_ZERO_FILL_LOOP_START + 20);
        assert_eq!(cpu.next_pc, BR2_BOOT_ZERO_FILL_LOOP_START + 24);
        assert_eq!(cpu.regs[1], 0);
        assert_eq!(cpu.regs[2], 0x8001_0010);
        for index in 0..4 {
            assert_eq!(bus.read_u32(0x8001_0000 + index * 4), 0);
        }
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
    fn native_3d_gameplay_signal_requires_real_projection_activity() {
        let mut cpu = Cpu::default();

        assert_eq!(cpu.gte_projected_vertices(), 0);
        assert!(!cpu.native_3d_gameplay_signal());

        cpu.gte_projected_vertices = 3;
        assert!(!cpu.native_3d_gameplay_signal());

        cpu.gte_command_counts[0x30] = 1;
        assert!(cpu.native_3d_gameplay_signal());
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
