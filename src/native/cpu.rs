use crate::native::bus::{Br2NativeCreditHleCheck, Bus};
use std::sync::OnceLock;

const CP0_STATUS: usize = 12;
const CP0_CAUSE: usize = 13;
const CP0_EPC: usize = 14;
const CP0_BADVADDR: usize = 8;

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
const BR2_LOW_BIOS_IRQ_VECTOR_HLE_START: u32 = 0x0000_0080;
const BR2_LOW_BIOS_IRQ_VECTOR_HLE_END: u32 = 0x0000_00c0;
const BR2_LOW_BIOS_IRQ_HANDLER_HLE_START: u32 = 0x0000_0420;
const BR2_LOW_BIOS_IRQ_HANDLER_HLE_END: u32 = 0x0000_0600;
const BR2_BIOS_B0_WAIT_EVENT_HLE_START: u32 = 0x0000_1e08;
const BR2_BIOS_B0_WAIT_EVENT_HLE_END: u32 = 0x0000_1e78;
const BR2_BIOS_B0_WAIT_EVENT_ENABLED: u32 = 0x0000_2000;
const BR2_BIOS_B0_WAIT_EVENT_DELIVERED: u32 = 0x0000_4000;
const BR2_BIOS_B0_WAIT_EVENT_RETURN_PC: u32 = 0x8034_c974;
const BR2_BIOS_B0_VECTOR_PHYSICAL: u32 = 0x0000_00b0;
const BR2_BIOS_B0_WAIT_EVENT_FUNCTION: u32 = 0x0a;
const BR2_BIOS_B0_TEST_EVENT_FUNCTION: u32 = 0x0b;
const BR2_BIOS_B0_RESET_ENTRY_INT_FUNCTION: u32 = 0x18;
const BR2_BIOS_B0_RETURN_ONLY_FUNCTION: u32 = 0x35;
const BR2_BIOS_EVENT_TABLE_POINTER_PHYSICAL: u32 = 0x0000_0120;
const BR2_BIOS_EVENT_RECORD_BYTES: u32 = 28;
const BR2_BIOS_B0_TEST_EVENT_HLE_START: u32 = 0x0000_1e8c;
const BR2_BIOS_B0_TEST_EVENT_HLE_END: u32 = 0x0000_1ed0;
const BR2_BIOS_B0_TEST_EVENT_RETURN_PC: u32 = 0x8034_cf7c;
const BR2_BIOS_B0_TEST_EVENT_ID: u32 = 0;
const BR2_BIOS_KERNEL_SYSCALL_ENTER_CRITICAL_SECTION: u32 = 1;
const BR2_BIOS_KERNEL_SYSCALL_EXIT_CRITICAL_SECTION: u32 = 2;
const BR2_RUNTIME_RAM_PC_START: u32 = 0x8000_0000;
const BR2_GAME_RUNTIME_PC_START: u32 = 0x8001_0000;
const BR2_RUNTIME_RAM_PC_END: u32 = 0x8040_0000;
const BR2_BIOS_B0_WAIT_EVENT_SIGNATURE: [(u32, u32); 18] = [
    (0x0000_1e08, 0x3084_ffff),
    (0x0000_1e0c, 0x0004_78c0),
    (0x0000_1e10, 0x3c0e_a000),
    (0x0000_1e14, 0x8dce_0120),
    (0x0000_1e18, 0x01e4_7823),
    (0x0000_1e1c, 0x000f_7880),
    (0x0000_1e20, 0x01cf_1021),
    (0x0000_1e24, 0x8c58_0004),
    (0x0000_1e28, 0x2405_4000),
    (0x0000_1e2c, 0x14b8_0005),
    (0x0000_1e44, 0x8c59_0004),
    (0x0000_1e48, 0x2404_2000),
    (0x0000_1e4c, 0x1099_0003),
    (0x0000_1e5c, 0x8c48_0004),
    (0x0000_1e64, 0x10a8_0005),
    (0x0000_1e6c, 0x8c49_0004),
    (0x0000_1e70, 0x0000_0000),
    (0x0000_1e74, 0x14a9_fffd),
];
const BR2_BIOS_B0_TEST_EVENT_SIGNATURE: [(u32, u32); 14] = [
    (0x0000_1e8c, 0x3084_ffff),
    (0x0000_1e90, 0x0004_78c0),
    (0x0000_1e94, 0x3c0e_a000),
    (0x0000_1e98, 0x8dce_0120),
    (0x0000_1e9c, 0x01e4_7823),
    (0x0000_1ea0, 0x000f_7880),
    (0x0000_1ea4, 0x01cf_1021),
    (0x0000_1ea8, 0x8c58_0004),
    (0x0000_1eac, 0x2401_4000),
    (0x0000_1eb0, 0x1701_0005),
    (0x0000_1eb4, 0x0040_1821),
    (0x0000_1ec8, 0x0000_1021),
    (0x0000_1ecc, 0x03e0_0008),
    (0x0000_1ed0, 0x0000_0000),
];
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
const BR2_STATUS_HALFWORD_WAIT_LOOP_START: u32 = 0x802d_c198;
const BR2_STATUS_HALFWORD_WAIT_LOOP_TAIL_LOAD: u32 = 0x802d_c270;
const BR2_STATUS_HALFWORD_WAIT_LOOP_FC_STATUS: u16 = 0xfc00;
const BR2_STATUS_HALFWORD_WAIT_LOOP_FE_STATUS: u16 = 0xfe00;
const BR2_STATUS_HALFWORD_WAIT_LOOP_FF_STATUS: u16 = 0xff00;
const BR2_STATUS_HALFWORD_WAIT_LOOP_HIGH_MASK: u16 = 0xff00;
const BR2_STATUS_HALFWORD_WAIT_LOOP_LOW_MASK: u16 = 0x00ff;
const BR2_STATUS_POINTER_SCAN_START: u32 = 0x802d_c174;
const BR2_STATUS_POINTER_SCAN_FALLTHROUGH: u32 = BR2_STATUS_HALFWORD_WAIT_LOOP_START;
const BR2_STATUS_POINTER_SCAN_EXIT: u32 = 0x802d_c28c;
const BR2_STATUS_POINTER_SCAN_CYCLES: u64 = 12;
const BR2_STATUS_POINTER_SCAN_INSTRUCTIONS: [(u32, u32); 9] = [
    (0x802d_c174, 0x8cc3_0014), // lw v1, 0x14(a2)
    (0x802d_c178, 0x0000_0000), // nop
    (0x802d_c17c, 0x2462_0004), // addiu v0, v1, 4
    (0x802d_c180, 0xacc2_0014), // sw v0, 0x14(a2)
    (0x802d_c184, 0x8462_0004), // lh v0, 4(v1)
    (0x802d_c188, 0x0000_0000), // nop
    (0x802d_c18c, 0x3042_ff00), // andi v0, v0, 0xff00
    (0x802d_c190, 0x1040_003e), // beq v0, zero, exit
    (0x802d_c194, 0x0000_0000), // nop
];
const BR2_STATUS_HALFWORD_WAIT_LOOP_INSTRUCTIONS: [(u32, u32); 24] = [
    (0x802d_c198, 0x8cc5_0014), // lw a1, 0x14(a2)
    (0x802d_c19c, 0x0000_0000), // nop
    (0x802d_c1a0, 0x94a2_0000), // lhu v0, 0(a1)
    (0x802d_c1a4, 0x0000_0000), // nop
    (0x802d_c1a8, 0x3043_00ff), // andi v1, v0, 0xff
    (0x802d_c1ac, 0x3044_ff00), // andi a0, v0, 0xff00
    (0x802d_c1b0, 0x3402_fe00), // ori v0, zero, 0xfe00
    (0x802d_c1b4, 0x1082_000d), // beq a0, v0, alternate path
    (0x802d_c1b8, 0x0000_0000), // nop
    (0x802d_c1bc, 0x0044_102a), // slt v0, v0, a0
    (0x802d_c1c0, 0x1440_0006), // bne v0, zero, alternate path
    (0x802d_c1c4, 0x3402_ff00), // ori v0, zero, 0xff00
    (0x802d_c1c8, 0x3402_fc00), // ori v0, zero, 0xfc00
    (0x802d_c1cc, 0x1082_000b), // beq a0, v0, alternate path
    (0x802d_c1d0, 0x0000_0000), // nop
    (0x802d_c1d4, 0x080b_709c), // j tail load
    (0x802d_c1d8, 0x0000_0000), // nop
    (0x802d_c270, 0x8cc2_0014), // lw v0, 0x14(a2)
    (0x802d_c274, 0x0000_0000), // nop
    (0x802d_c278, 0x8442_0000), // lh v0, 0(v0)
    (0x802d_c27c, 0x0000_0000), // nop
    (0x802d_c280, 0x3042_ff00), // andi v0, v0, 0xff00
    (0x802d_c284, 0x1440_ffc4), // bne v0, zero, loop start
    (0x802d_c288, 0x0000_0000), // nop
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
const BR2_CREDIT_CHECK_ENTRY: u32 = 0x8030_8770;
const BR2_CREDIT_CHECK_HLE_CYCLES: u64 = 24;
const BR2_CREDIT_STATE_BASE: u32 = 0x803b_fa00;
const BR2_CREDIT_FREEPLAY_FLAG_OFFSET: u32 = 0x00;
const BR2_CREDIT_PLAYER_MODE_OFFSET: u32 = 0x01;
const BR2_CREDIT_REQUIRED_P1_OFFSET: u32 = 0x08;
const BR2_CREDIT_REQUIRED_P2_OFFSET: u32 = 0x09;
const BR2_CREDIT_SHARED_SLOT_OFFSET: u32 = 0x18;
const BR2_CREDIT_CHECK_ENTRY_SIGNATURE: [(u32, u32); 10] = [
    (0x8030_8770, 0x27bd_ffe8), // addiu sp, sp, -0x18
    (0x8030_8774, 0x3c05_803b), // lui a1, 0x803b
    (0x8030_8778, 0x24a5_fa00), // addiu a1, a1, -0x600
    (0x8030_877c, 0xafbf_0010), // sw ra, 0x10(sp)
    (0x8030_8780, 0x90a6_0008), // lbu a2, 8(a1)
    (0x8030_8784, 0x0c0c_21be), // jal 0x803086f8
    (0x8030_8788, 0x0000_0000), // nop
    (0x8030_878c, 0x8fbf_0010), // lw ra, 0x10(sp)
    (0x8030_8794, 0x03e0_0008), // jr ra
    (0x8030_8798, 0x27bd_0018), // addiu sp, sp, 0x18
];
const BR2_CREDIT_CHECK_CORE_SIGNATURE: [(u32, u32); 13] = [
    (0x8030_86f8, 0x90a2_0000), // lbu v0, 0(a1)
    (0x8030_8700, 0x1040_000a), // beq v0, zero, 0x8030872c
    (0x8030_8704, 0x2402_0001), // addiu v0, zero, 1
    (0x8030_872c, 0x90a3_0001), // lbu v1, 1(a1)
    (0x8030_8734, 0x1462_0004), // bne v1, v0, 0x80308748
    (0x8030_8738, 0x0044_1004), // sllv v0, a0, v0
    (0x8030_873c, 0x2442_0018), // addiu v0, v0, 0x18
    (0x8030_8740, 0x080c_21d3), // j 0x8030874c
    (0x8030_8748, 0x24a5_0018), // addiu a1, a1, 0x18
    (0x8030_874c, 0x90a2_0000), // lbu v0, 0(a1)
    (0x8030_8754, 0x0046_1023), // subu v0, v0, a2
    (0x8030_8758, 0x0440_0003), // bltz v0, 0x80308768
    (0x8030_8764, 0xa0a2_0000), // sb v0, 0(a1)
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
const BR2_POST_VS_TABLE_ACCUM_MAX_CHARGED_NOOP_ITERATIONS: u32 = 65_536;
const BR2_POST_VS_TABLE_GROUP_LOOP_START: u32 = 0x8035_6eb8;
const BR2_POST_VS_TABLE_GROUP_LOOP_EXIT: u32 = 0x8035_6f44;
const BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION: u64 = 24;
const BR2_POST_VS_TABLE_GROUP_NONPOSITIVE_MIN_SKIP_ITERATIONS: u32 = 1_000_000;
const BR2_POST_VS_TABLE_GROUP_MAX_CHARGED_NOOP_ITERATIONS: u32 = 65_536;
const BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START: u32 = 0x8035_6f5c;
const BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_EXIT: u32 = 0x8035_70a4;
const BR2_POST_VS_TABLE_SELECT_GROUP_CYCLES_PER_ITERATION: u64 = 18;
const BR2_POST_VS_TABLE_SELECT_GROUP_MAX_CHARGED_NOOP_ITERATIONS: u32 = 65_536;
const BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_DELAY: u32 = 0x8035_7030;
const BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_BRANCH: u32 = 0x8035_7034;
const BR2_POST_VS_TABLE_SELECT_GROUP_TAIL_LOAD: u32 = 0x8035_7090;
const BR2_POST_VS_TABLE_SELECT_GROUP_TAIL_INCREMENT: u32 = 0x8035_7094;
const BR2_POST_VS_NULL_LINK_SCAN_LOOP_START: u32 = 0x8031_566c;
const BR2_POST_VS_NULL_LINK_SCAN_TAIL_LOAD: u32 = 0x8031_56b4;
const BR2_POST_VS_NULL_LINK_SCAN_TAIL_DELAY: u32 = 0x8031_56b8;
const BR2_POST_VS_NULL_LINK_SCAN_TAIL_BRANCH: u32 = 0x8031_56bc;
const BR2_POST_VS_NULL_LINK_SCAN_LOOP_EXIT: u32 = 0x8031_56c4;
const BR2_POST_VS_NULL_LINK_SCAN_CYCLES: u64 = 9;
const BR2_POST_VS_STACK_LINK_SCAN_LOOP_START: u32 = 0x8031_4290;
const BR2_POST_VS_STACK_LINK_SCAN_RELOAD: u32 = 0x8031_54e8;
const BR2_POST_VS_STACK_LINK_SCAN_RELOAD_DELAY: u32 = 0x8031_54ec;
const BR2_POST_VS_STACK_LINK_SCAN_NEXT_LOAD: u32 = 0x8031_54f0;
const BR2_POST_VS_STACK_LINK_SCAN_COMPARE: u32 = 0x8031_54f4;
const BR2_POST_VS_STACK_LINK_SCAN_TAIL_BRANCH: u32 = 0x8031_54f8;
const BR2_POST_VS_STACK_LINK_SCAN_TAIL_STORE: u32 = 0x8031_54fc;
const BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT: u32 = 0x8031_5500;
const BR2_POST_VS_STACK_LINK_SCAN_STACK_SLOT_OFFSET: u32 = 0x158;
const BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_START_CYCLES: u64 = 22;
const BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_RELOAD_CYCLES: u64 = 9;
const BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_RELOAD_DELAY_CYCLES: u64 = 7;
const BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_NEXT_LOAD_CYCLES: u64 = 6;
const BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_COMPARE_CYCLES: u64 = 4;
const BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_TAIL_BRANCH_CYCLES: u64 = 3;
const BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_TAIL_STORE_CYCLES: u64 = 2;
const BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START: u32 = 0x8031_42bc;
const BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LENGTH_LOAD: u32 = 0x8031_54c4;
const BR2_POST_VS_STACK_PACKET_SCAN_TAIL_INDEX_LOAD: u32 = 0x8031_54c8;
const BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LIMIT_LOAD: u32 = 0x8031_54cc;
const BR2_POST_VS_STACK_PACKET_SCAN_TAIL_SHIFT: u32 = 0x8031_54d0;
const BR2_POST_VS_STACK_PACKET_SCAN_TAIL_CURSOR_ADD: u32 = 0x8031_54d4;
const BR2_POST_VS_STACK_PACKET_SCAN_TAIL_INDEX_ADD: u32 = 0x8031_54d8;
const BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPARE: u32 = 0x8031_54dc;
const BR2_POST_VS_STACK_PACKET_SCAN_TAIL_BRANCH: u32 = 0x8031_54e0;
const BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT: u32 = 0x8031_54e8;
const BR2_POST_VS_STACK_PACKET_SCAN_BODY_FAST_FORWARD_START: u32 = 0x8031_42f8;
const BR2_POST_VS_STACK_PACKET_SCAN_BODY_FAST_FORWARD_END: u32 = 0x8031_4340;
const BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET: u32 = 0x120;
const BR2_POST_VS_STACK_PACKET_SCAN_LENGTH_STACK_OFFSET: u32 = 0x124;
const BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET: u32 = 0x128;
const BR2_POST_VS_STACK_PACKET_SCAN_RA_STACK_OFFSET: u32 = 0x154;
const BR2_POST_VS_STACK_PACKET_SCAN_NOOP_TAG_LIMIT: u32 = 8;
const BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPLETION_CYCLES: u64 = 12;
const BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET: u64 = 50;
const BR2_POST_VS_STACK_PACKET_SCAN_MAX_SKIP_PACKETS: u32 = 16_777_216;
const BR2_POST_VS_STACK_PACKET_SCAN_MAX_CHARGED_NOOP_PACKETS: u32 = 65_536;
const BR2_POST_VS_STACK_PACKET_SCAN_MAX_VERIFIED_RAM_PACKETS: u32 = 32_768;
const BR2_POST_VS_STACK_PACKET_SCAN_LONG_MAX_VERIFIED_RAM_PACKETS: u32 =
    BR2_POST_VS_STACK_PACKET_SCAN_MAX_SKIP_PACKETS;
const BR2_POST_VS_STACK_PACKET_SCAN_UNIFORM_SAMPLE_PACKETS: u32 = 256;
const BR2_POST_VS_STACK_PACKET_SCAN_LONG_UNTHROTTLED_MIN_PACKETS: u32 = 1_000_000;
const BR2_PSX_SCRATCHPAD_END: u32 = 0x1f80_0400;
const BR2_PSX_HW_IO_START: u32 = 0x1f80_1000;
const BR2_PSX_HW_IO_END: u32 = 0x1f80_2000;
const BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START: u32 = 0x8031_32c0;
const BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_EXIT: u32 = 0x8031_32fc;
const BR2_POST_VS_STRIDED_POINTER_COPY_CYCLES_PER_ITERATION: u64 = 24;
const BR2_POST_VS_STRIDED_POINTER_COPY_MAX_REAL_ITERATIONS: u32 = 65_536;
const BR2_POST_VS_STRIDED_POINTER_COPY_HUGE_NOOP_MIN_ITERATIONS: u32 = 1_000_000;
const BR2_POST_VS_STRIDED_POINTER_COPY_MAX_CHARGED_NOOP_ITERATIONS: u32 = 65_536;
const BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_START: u32 = 0x8031_3324;
const BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_EXIT: u32 = 0x8031_3360;
const BR2_POST_VS_VERTEX_RECORD_LOOP_START: u32 = 0x8031_3548;
const BR2_POST_VS_VERTEX_RECORD_LOOP_EXIT: u32 = 0x8031_3628;
const BR2_POST_VS_VERTEX_RECORD_CYCLES_PER_ITERATION: u64 = 89;
const BR2_POST_VS_VERTEX_RECORD_MAX_SKIP_ITERATIONS: u32 = 65_536;
const BR2_POST_VS_RECORD_COPY_LOOP_START: u32 = 0x8031_552c;
const BR2_POST_VS_RECORD_COPY_LOOP_EXIT: u32 = 0x8031_5568;
const BR2_POST_VS_RECORD_COPY_COUNTER_STACK_OFFSET: u32 = 0x128;
const BR2_POST_VS_RECORD_COPY_CYCLES_PER_ITERATION: u64 = 23;
const BR2_POST_VS_RECORD_COPY_MAX_SKIP_ITERATIONS: u32 = 65_536;
const BR2_POST_VS_RECORD_COPY_HUGE_NOOP_MIN_ITERATIONS: u32 = 1_000_000;
const BR2_POST_VS_RECORD_COPY_MAX_CHARGED_NOOP_ITERATIONS: u32 = 65_536;
const BR2_POST_VS_PACKED_VERTEX_HELPER_START: u32 = 0x8035_9418;
const BR2_POST_VS_PACKED_VERTEX_HELPER_RETURN: u32 = 0x8035_6ce0;
const BR2_POST_VS_PACKED_VERTEX_HELPER_CYCLES: u64 = 16;
const BR2_RUNTIME_UNALIGNED_WORD_LOAD_PC: u32 = 0x802e_2ca4;
const BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC: u32 = 0x8034_47cc;
const BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_END_PC: u32 = 0x8034_4824;
const BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC: u32 = 0x8034_47d0;
const BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_END_PC: u32 = 0x8034_47d4;
const BR2_RUNTIME_UNALIGNED_VERTEX_SECOND_WORD_LOAD_PC: u32 = 0x8034_47f8;
const BR2_RUNTIME_UNALIGNED_VERTEX_SECOND_WORD_LOAD_END_PC: u32 = 0x8034_482c;
const BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC: u32 = 0x8033_c884;
const BR2_RUNTIME_UNALIGNED_GTE_LOAD_END_PC: u32 = 0x8033_c88c;
const BR2_RUNTIME_UNALIGNED_RENDER_SOURCE_WORD_LOAD_PC: u32 = 0x8033_c85c;
const BR2_RUNTIME_UNALIGNED_RENDER_SOURCE_WORD_LOAD_END_PC: u32 = 0x8033_c8b8;
const BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_PC: u32 = 0x8033_ca2c;
const BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_END_PC: u32 = 0x8033_ca80;
const BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_STORE_PC: u32 = 0x8033_ca44;
const BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_STORE_END_PC: u32 = 0x8033_ca7c;
const BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_PC: u32 = 0x8033_cae0;
const BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_END_PC: u32 = 0x8033_cb08;
const BR2_RUNTIME_UNALIGNED_WORD_STORE_PC: u32 = 0x8033_c8bc;
const BR2_RUNTIME_UNALIGNED_WORD_STORE_END_PC: u32 = 0x8033_c8c4;
const BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_PC: u32 = 0x8034_4864;
const BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_END_PC: u32 = 0x8034_489c;
const BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC: u32 = 0x8034_48a0;
const BR2_RUNTIME_NULL_CALLBACK_JALR_PC: u32 = 0x8035_0eb4;
const BR2_RUNTIME_RENDER_CALLBACK_JALR_PC: u32 = 0x8033_693c;
const BR2_RUNTIME_RENDER_CALLBACK_MIN_TARGET_PC: u32 = 0x802c_0000;
const BR2_RUNTIME_RENDER_CALLBACK_LOOP_MAX_REAL_ITERATIONS: u32 = 4096;
const BR2_POST_VS_RUNTIME_CODE_NOOP_START: u32 = 0x002c_0000;
const BR2_POST_VS_RUNTIME_CODE_NOOP_END: u32 = 0x0037_0000;
const BR2_POST_VS_LIVE_RENDER_RAM_NOOP_START: u32 = 0x0038_0000;
const BR2_POST_VS_LIVE_RENDER_RAM_NOOP_END: u32 = 0x003c_0000;
const BR2_POST_VS_STACK_GUARD_NOOP_START: u32 = 0x003f_0000;
const BR2_POST_VS_STACK_GUARD_NOOP_END: u32 = 0x0040_0000;
const BR2_POST_VS_CODE_PATCH_NOOP_START: u32 = 0x002c_c100;
const BR2_POST_VS_CODE_PATCH_NOOP_END: u32 = 0x002c_c220;
const BR2_POST_VS_PROTECTED_CODE_NOOP_RANGES: [(u32, u32); 4] = [
    (
        BR2_LOW_BIOS_IRQ_VECTOR_HLE_START,
        BR2_LOW_BIOS_IRQ_HANDLER_HLE_END + 0x20,
    ),
    (0x1fc0_0000, 0x1fc8_0000),
    (
        BR2_POST_VS_RUNTIME_CODE_NOOP_START,
        BR2_POST_VS_RUNTIME_CODE_NOOP_END,
    ),
    (
        BR2_POST_VS_CODE_PATCH_NOOP_START,
        BR2_POST_VS_CODE_PATCH_NOOP_END,
    ),
];
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
const BR2_POST_VS_TABLE_GROUP_PREFIX_INSTRUCTIONS: [u32; 15] = [
    0x8c83_007c, // lw v1, 0x7c(a0)
    0x0000_0000, // nop
    0x00c3_1821, // addu v1, a2, v1
    0x8c62_0004, // lw v0, 4(v1)
    0x0000_0000, // nop
    0x0044_1021, // addu v0, v0, a0
    0xac62_0004, // sw v0, 4(v1)
    0x8c83_007c, // lw v1, 0x7c(a0)
    0x0000_0000, // nop
    0x00c3_1021, // addu v0, a2, v1
    0x8c42_0000, // lw v0, 0(v0)
    0x0000_0000, // nop
    0x1840_0011, // blez v0, outer tail
    0x0000_2821, // addu a1, zero, zero
    0x00c3_1021, // addu v0, a2, v1
];
const BR2_POST_VS_TABLE_GROUP_TAIL_INSTRUCTIONS: [(u32, u32); 5] = [
    (0x8035_6f30, 0x8c82_0028), // lw v0, 0x28(a0)
    (0x8035_6f34, 0x24e7_0001), // addiu a3, a3, 1
    (0x8035_6f38, 0x00e2_102b), // sltu v0, a3, v0
    (0x8035_6f3c, 0x1440_ffde), // bne v0, zero, group start
    (0x8035_6f40, 0x24c6_0008), // addiu a2, a2, 8
];
const BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_INSTRUCTIONS: [(u32, u32); 15] = [
    (0x8035_6f5c, 0x8c82_000c), // lw v0, 0xc(a0)
    (0x8035_6f60, 0x0000_0000), // nop
    (0x8035_6f64, 0x2442_ffff), // addiu v0, v0, -1
    (0x8035_6f68, 0x2c42_0002), // sltiu v0, v0, 2
    (0x8035_6f6c, 0x1040_002f), // beq v0, zero, compare path
    (0x8035_6f70, 0x0000_0000), // nop
    (0x8035_702c, 0x8c82_0014), // lw v0, 0x14(a0)
    (0x8035_7030, 0x0000_0000), // nop
    (0x8035_7034, 0x1446_0016), // bne v0, a2, loop tail
    (0x8035_7038, 0x0000_0000), // nop
    (0x8035_7090, 0x8c82_0024), // lw v0, 0x24(a0)
    (0x8035_7094, 0x24e7_0001), // addiu a3, a3, 1
    (0x8035_7098, 0x00e2_102b), // sltu v0, a3, v0
    (0x8035_709c, 0x1440_ffaf), // bne v0, zero, loop start
    (0x8035_70a0, 0x24a5_0014), // addiu a1, a1, 0x14
];
const BR2_POST_VS_PACKED_VERTEX_CALLER_INSTRUCTIONS: [(u32, u32); 13] = [
    (0x8035_6c90, 0x8c44_0008), // lw a0, 8(v0)
    (0x8035_6c94, 0x8f83_0028), // lw v1, 0x28(gp)
    (0x8035_6c98, 0x8c45_000c), // lw a1, 0xc(v0)
    (0x8035_6c9c, 0x1060_0006), // beq v1, zero, no side path
    (0x8035_6ca0, 0x0000_0000), // nop
    (0x8035_6cb8, 0x8e06_0020), // lw a2, 0x20(s0)
    (0x8035_6cbc, 0x8602_0010), // lh v0, 0x10(s0)
    (0x8035_6cc4, 0xafa2_0010), // sw v0, 0x10(sp)
    (0x8035_6cc8, 0x8602_0040), // lh v0, 0x40(s0)
    (0x8035_6cd0, 0xafa2_0014), // sw v0, 0x14(sp)
    (0x8035_6cd4, 0x8e07_0014), // lw a3, 0x14(s0)
    (0x8035_6cd8, 0x0c0d_6506), // jal BR2_POST_VS_PACKED_VERTEX_HELPER_START
    (0x8035_6cdc, 0x0000_0000), // nop
];
const BR2_POST_VS_PACKED_VERTEX_HELPER_INSTRUCTIONS: [(u32, u32); 5] = [
    (0x8035_9418, 0x8fb8_0010), // lw t8, 0x10(sp)
    (0x8035_941c, 0x8faf_0014), // lw t7, 0x14(sp)
    (0x8035_9420, 0x1300_0026), // beq t8, zero, late return path
    (0x8035_9424, 0x488f_4000), // mtc2 t7, cop2 data 8
    (0x8035_9428, 0x8c89_0000), // lw t1, 0(a0)
];
const BR2_POST_VS_NULL_LINK_SCAN_LOOP_INSTRUCTIONS: [(u32, u32); 9] = [
    (0x8031_566c, 0x8ca2_0008), // lw v0, 8(a1)
    (0x8031_5670, 0x0000_0000), // nop
    (0x8031_5674, 0x004a_1024), // and v0, v0, t2
    (0x8031_5678, 0x1040_000e), // beq v0, zero, tail
    (0x8031_567c, 0x0000_0000), // nop
    (0x8031_56b4, 0x8ca5_0000), // lw a1, 0(a1)
    (0x8031_56b8, 0x0000_0000), // nop
    (0x8031_56bc, 0x14a9_ffeb), // bne a1, t1, loop start
    (0x8031_56c0, 0x0000_0000), // nop
];
const BR2_POST_VS_STACK_LINK_SCAN_LOOP_INSTRUCTIONS: [(u32, u32); 14] = [
    (0x8031_4290, 0x8fa9_0158), // lw t1, 0x158(sp)
    (0x8031_4294, 0xafa0_0128), // sw zero, 0x128(sp)
    (0x8031_4298, 0x8d2a_0008), // lw t2, 8(t1)
    (0x8031_429c, 0x0000_0000), // nop
    (0x8031_42a0, 0xafaa_0120), // sw t2, 0x120(sp)
    (0x8031_42a4, 0x8d32_0004), // lw s2, 4(t1)
    (0x8031_42a8, 0x1140_048f), // beq t2, zero, reload path
    (0x8031_42ac, 0x2534_000c), // addiu s4, t1, 0xc
    (0x8031_54e8, 0x8fa8_0158), // lw t0, 0x158(sp)
    (0x8031_54ec, 0x0000_0000), // nop
    (0x8031_54f0, 0x8d08_0000), // lw t0, 0(t0)
    (0x8031_54f4, 0x2402_ffff), // addiu v0, zero, -1
    (0x8031_54f8, 0x1502_fb65), // bne t0, v0, loop start
    (0x8031_54fc, 0xafa8_0158), // sw t0, 0x158(sp)
];
const BR2_POST_VS_STACK_PACKET_SCAN_LOOP_INSTRUCTIONS: [(u32, u32); 43] = [
    (0x8031_42bc, 0x8e84_0000), // lw a0, 0(s4)
    (0x8031_42c0, 0x2694_0004), // addiu s4, s4, 4
    (0x8031_42c4, 0x3c03_0f04), // lui v1, 0x0f04
    (0x8031_42c8, 0x3463_ffff), // ori v1, v1, 0xffff
    (0x8031_42cc, 0x3c05_0004), // lui a1, 4
    (0x8031_42d0, 0x34a5_0055), // ori a1, a1, 0x55
    (0x8031_42d4, 0x9682_0002), // lhu v0, 2(s4)
    (0x8031_42d8, 0x9688_0000), // lhu t0, 0(s4)
    (0x8031_42dc, 0x3047_7fff), // andi a3, v0, 0x7fff
    (0x8031_42e0, 0x0083_1824), // and v1, a0, v1
    (0x8031_42e4, 0x1065_02fe), // beq v1, a1, handled packet path
    (0x8031_42e8, 0xafa8_0124), // sw t0, 0x124(sp)
    (0x8031_42ec, 0x00a3_102b), // sltu v0, a1, v1
    (0x8031_42f0, 0x1440_0059), // bne v0, zero, handled packet path
    (0x8031_42f4, 0x3c02_0100), // lui v0, 0x0100
    (0x8031_42f8, 0x2402_0013), // addiu v0, zero, 0x13
    (0x8031_42fc, 0x1062_0471), // beq v1, v0, handled packet path
    (0x8031_4300, 0x2c62_0014), // sltiu v0, v1, 0x14
    (0x8031_4304, 0x1040_0026), // beq v0, zero, handled packet path
    (0x8031_4308, 0x2402_000d), // addiu v0, zero, 0x0d
    (0x8031_430c, 0x1062_00eb), // beq v1, v0, handled packet path
    (0x8031_4310, 0x2c62_000e), // sltiu v0, v1, 0x0e
    (0x8031_4314, 0x1040_0012), // beq v0, zero, handled packet path
    (0x8031_4318, 0x2402_000a), // addiu v0, zero, 0x0a
    (0x8031_431c, 0x1062_0160), // beq v1, v0, handled packet path
    (0x8031_4320, 0x2c62_000b), // sltiu v0, v1, 0x0b
    (0x8031_4324, 0x1040_0007), // beq v0, zero, handled packet path
    (0x8031_4328, 0x2402_0008), // addiu v0, zero, 8
    (0x8031_432c, 0x1062_009c), // beq v1, v0, handled packet path
    (0x8031_4330, 0x2402_0009), // addiu v0, zero, 9
    (0x8031_4334, 0x1062_00b1), // beq v1, v0, handled packet path
    (0x8031_4338, 0x0000_0000), // nop
    (0x8031_433c, 0x080c_5531), // j tail
    (0x8031_4340, 0x0000_0000), // nop
    (0x8031_54c4, 0x8fa9_0124), // lw t1, 0x124(sp)
    (0x8031_54c8, 0x8faa_0128), // lw t2, 0x128(sp)
    (0x8031_54cc, 0x8fab_0120), // lw t3, 0x120(sp)
    (0x8031_54d0, 0x0009_1080), // sll v0, t1, 2
    (0x8031_54d4, 0x0282_a021), // addu s4, s4, v0
    (0x8031_54d8, 0x254a_0001), // addiu t2, t2, 1
    (0x8031_54dc, 0x014b_102b), // sltu v0, t2, t3
    (0x8031_54e0, 0x1440_fb76), // bne v0, zero, loop start
    (0x8031_54e4, 0xafaa_0128), // sw t2, 0x128(sp)
];
const BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_INSTRUCTIONS: [u32; 15] = [
    0x2482_0008, // addiu v0, a0, 8
    0x88c8_0003, // lwl t0, 3(a2)
    0x98c8_0000, // lwr t0, 0(a2)
    0x88c9_0007, // lwl t1, 7(a2)
    0x98c9_0004, // lwr t1, 4(a2)
    0xa888_0003, // swl t0, 3(a0)
    0xb888_0000, // swr t0, 0(a0)
    0xa889_0007, // swl t1, 7(a0)
    0xb889_0004, // swr t1, 4(a0)
    0x2484_0010, // addiu a0, a0, 0x10
    0x2463_ffff, // addiu v1, v1, -1
    0xace2_0000, // sw v0, 0(a3)
    0x24e7_0008, // addiu a3, a3, 8
    0x1460_fff2, // bne v1, zero, loop start
    0x24c6_0008, // addiu a2, a2, 8
];
const BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_INSTRUCTIONS: [u32; 15] = [
    0x2482_0008, // addiu v0, a0, 8
    0x8868_0003, // lwl t0, 3(v1)
    0x9868_0000, // lwr t0, 0(v1)
    0x8869_0007, // lwl t1, 7(v1)
    0x9869_0004, // lwr t1, 4(v1)
    0xa888_0003, // swl t0, 3(a0)
    0xb888_0000, // swr t0, 0(a0)
    0xa889_0007, // swl t1, 7(a0)
    0xb889_0004, // swr t1, 4(a0)
    0x2484_0010, // addiu a0, a0, 0x10
    0x24c6_ffff, // addiu a2, a2, -1
    0xaca2_0000, // sw v0, 0(a1)
    0x24a5_0008, // addiu a1, a1, 8
    0x14c0_fff2, // bne a2, zero, loop start
    0x2463_0008, // addiu v1, v1, 8
];
const BR2_POST_VS_VERTEX_RECORD_LOOP_INSTRUCTIONS: [(u32, u32); 56] = [
    (0x8031_3548, 0xa10d_ffd5), // sb t5, -0x2b(t0)
    (0x8031_354c, 0xa10c_ffd9), // sb t4, -0x27(t0)
    (0x8031_3550, 0xa10d_fff5), // sb t5, -0x0b(t0)
    (0x8031_3554, 0xa10c_fff9), // sb t4, -0x07(t0)
    (0x8031_3558, 0x8d62_0000), // lw v0, 0(t3)
    (0x8031_355c, 0x0000_0000), // nop
    (0x8031_3560, 0xad02_fffe), // sw v0, -0x02(t0)
    (0x8031_3564, 0xad02_ffde), // sw v0, -0x22(t0)
    (0x8031_3568, 0x8ca2_fff2), // lw v0, -0x0e(a1)
    (0x8031_356c, 0x0000_0000), // nop
    (0x8031_3570, 0xad02_0006), // sw v0, 0x06(t0)
    (0x8031_3574, 0xad02_ffe6), // sw v0, -0x1a(t0)
    (0x8031_3578, 0x8ca2_fff6), // lw v0, -0x0a(a1)
    (0x8031_357c, 0x0000_0000), // nop
    (0x8031_3580, 0xad02_000e), // sw v0, 0x0e(t0)
    (0x8031_3584, 0xad02_ffee), // sw v0, -0x12(t0)
    (0x8031_3588, 0x84a2_fffa), // lh v0, -0x06(a1)
    (0x8031_358c, 0x0000_0000), // nop
    (0x8031_3590, 0x0002_10c0), // sll v0, v0, 3
    (0x8031_3594, 0x0047_1021), // addu v0, v0, a3
    (0x8031_3598, 0x8c42_0000), // lw v0, 0(v0)
    (0x8031_359c, 0x0000_0000), // nop
    (0x8031_35a0, 0xad02_ffce), // sw v0, -0x32(t0)
    (0x8031_35a4, 0x84a2_fffc), // lh v0, -0x04(a1)
    (0x8031_35a8, 0x256b_0014), // addiu t3, t3, 0x14
    (0x8031_35ac, 0x0002_10c0), // sll v0, v0, 3
    (0x8031_35b0, 0x0046_1021), // addu v0, v0, a2
    (0x8031_35b4, 0x8c42_0000), // lw v0, 0(v0)
    (0x8031_35b8, 0x2529_ffff), // addiu t1, t1, -1
    (0x8031_35bc, 0xad42_0000), // sw v0, 0(t2)
    (0x8031_35c0, 0x84a2_fffe), // lh v0, -0x02(a1)
    (0x8031_35c4, 0x9503_ffe0), // lhu v1, -0x20(t0)
    (0x8031_35c8, 0x0002_10c0), // sll v0, v0, 3
    (0x8031_35cc, 0x0046_1021), // addu v0, v0, a2
    (0x8031_35d0, 0x8c42_0000), // lw v0, 0(v0)
    (0x8031_35d4, 0x254a_0050), // addiu t2, t2, 0x50
    (0x8031_35d8, 0xad02_ffc6), // sw v0, -0x3a(t0)
    (0x8031_35dc, 0x84a2_0000), // lh v0, 0(a1)
    (0x8031_35e0, 0x006e_1821), // addu v1, v1, t6
    (0x8031_35e4, 0x0002_10c0), // sll v0, v0, 3
    (0x8031_35e8, 0x0046_1021), // addu v0, v0, a2
    (0x8031_35ec, 0x8c44_0000), // lw a0, 0(v0)
    (0x8031_35f0, 0x9502_ffe8), // lhu v0, -0x18(t0)
    (0x8031_35f4, 0x24a5_0014), // addiu a1, a1, 0x14
    (0x8031_35f8, 0xa503_ffe0), // sh v1, -0x20(t0)
    (0x8031_35fc, 0x9503_0000), // lhu v1, 0(t0)
    (0x8031_3600, 0x004f_1021), // addu v0, v0, t7
    (0x8031_3604, 0xa502_ffe8), // sh v0, -0x18(t0)
    (0x8031_3608, 0x9502_0008), // lhu v0, 0x08(t0)
    (0x8031_360c, 0x006e_1821), // addu v1, v1, t6
    (0x8031_3610, 0xa503_0000), // sh v1, 0(t0)
    (0x8031_3614, 0xad04_ffca), // sw a0, -0x36(t0)
    (0x8031_3618, 0x004f_1021), // addu v0, v0, t7
    (0x8031_361c, 0xa502_0008), // sh v0, 0x08(t0)
    (0x8031_3620, 0x1520_ffc9), // bne t1, zero, loop start
    (0x8031_3624, 0x2508_0050), // addiu t0, t0, 0x50
];
const BR2_POST_VS_RECORD_COPY_LOOP_INSTRUCTIONS: [(u32, u32); 15] = [
    (0x8031_552c, 0x8c6a_0000), // lw t2, 0(v1)
    (0x8031_5530, 0x8c6b_0004), // lw t3, 4(v1)
    (0x8031_5534, 0x8c68_0008), // lw t0, 8(v1)
    (0x8031_5538, 0x8c69_000c), // lw t1, 0xc(v1)
    (0x8031_553c, 0xae2a_0000), // sw t2, 0(s1)
    (0x8031_5540, 0xae2b_0004), // sw t3, 4(s1)
    (0x8031_5544, 0xae28_0008), // sw t0, 8(s1)
    (0x8031_5548, 0xae29_000c), // sw t1, 0xc(s1)
    (0x8031_554c, 0x2631_0010), // addiu s1, s1, 0x10
    (0x8031_5550, 0x8faa_0128), // lw t2, 0x128(sp)
    (0x8031_5554, 0x2463_0010), // addiu v1, v1, 0x10
    (0x8031_5558, 0x254a_0001), // addiu t2, t2, 1
    (0x8031_555c, 0x0153_102b), // sltu v0, t2, s3
    (0x8031_5560, 0x1440_fff2), // bne v0, zero, loop start
    (0x8031_5564, 0xafaa_0128), // sw t2, 0x128(sp)
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
const BR2_BITSTREAM_DECODE_LOOP_START: u32 = 0x8033_f57c;
const BR2_BITSTREAM_DECODE_DIRECT_START: u32 = 0x8033_f460;
const BR2_BITSTREAM_DECODE_EXIT_TABLE_SENTINEL: u32 = 0x8033_f660;
const BR2_BITSTREAM_DECODE_EXIT_STREAM_SENTINEL: u32 = 0x8033_f684;
const BR2_BITSTREAM_DECODE_EXIT_DEST_LIMIT: u32 = 0x8033_f6b4;
const BR2_BITSTREAM_DECODE_MAX_STEPS: u32 = 65_536;
const BR2_BITSTREAM_DECODE_MIN_CYCLES_PER_STEP: u64 = 18;
const BR2_BITSTREAM_DECODE_TABLE_CYCLES: u64 = 40;
const BR2_BITSTREAM_DECODE_DIRECT_CYCLES: u64 = 24;
const BR2_BITSTREAM_DECODE_LITERAL_PREFIX: u32 = 0xfe00;
const BR2_BITSTREAM_DECODE_TABLE_SENTINEL: u32 = 0x7c1f;
const BR2_BITSTREAM_DECODE_STREAM_SENTINEL: u32 = 0x01ff;
const BR2_BITSTREAM_DECODE_LOOP_INSTRUCTIONS: [(u32, u32); 54] = [
    (0x8033_f460, 0x11a0_0035), // beq t5, zero, direct refill path
    (0x8033_f464, 0x0002_4582), // srl t0, v0, 22
    (0x8033_f538, 0x3901_01ff), // xori at, t0, 0x1ff
    (0x8033_f53c, 0x1020_0051), // beq at, zero, stream sentinel exit
    (0x8033_f540, 0x20a5_0002), // addi a1, a1, 2
    (0x8033_f544, 0x0002_1280), // sll v0, v0, 10
    (0x8033_f548, 0x2063_000a), // addi v1, v1, 10
    (0x8033_f54c, 0x3061_0010), // andi at, v1, 0x10
    (0x8033_f550, 0x1020_0005), // beq at, zero, direct output
    (0x8033_f554, 0x3063_000f), // andi v1, v1, 0xf
    (0x8033_f558, 0x9489_0000), // lhu t1, 0(a0)
    (0x8033_f55c, 0x2084_0002), // addi a0, a0, 2
    (0x8033_f560, 0x0069_4804), // sllv t1, t1, v1
    (0x8033_f564, 0x0049_1025), // or v0, v0, t1
    (0x8033_f568, 0x0188_4025), // or t0, t4, t0
    (0x8033_f56c, 0xa4a8_0000), // sh t0, 0(a1)
    (0x8033_f570, 0x00ae_0823), // subu at, a1, t6
    (0x8033_f574, 0x0421_004f), // bgez at, dest limit exit
    (0x8033_f578, 0x20a5_0002), // addi a1, a1, 2
    (0x8033_f57c, 0x0002_44c2), // srl t0, v0, 19
    (0x8033_f580, 0x0008_40c0), // sll t0, t0, 3
    (0x8033_f584, 0x0106_4020), // add t0, t0, a2
    (0x8033_f588, 0x8d09_0000), // lw t1, 0(t0)
    (0x8033_f58c, 0x0000_0000), // nop
    (0x8033_f590, 0x1520_0011), // bne t1, zero, table hit path
    (0x8033_f594, 0x3121_00ff), // andi at, t1, 0xff
    (0x8033_f5d8, 0x8d0b_0004), // lw t3, 4(t0)
    (0x8033_f5dc, 0x0022_1004), // sllv v0, v0, at
    (0x8033_f5e0, 0x0061_1820), // add v1, v1, at
    (0x8033_f5e4, 0x3061_0010), // andi at, v1, 0x10
    (0x8033_f5e8, 0x1020_0005), // beq at, zero, emit table word
    (0x8033_f5ec, 0x3063_000f), // andi v1, v1, 0xf
    (0x8033_f5f0, 0x9488_0000), // lhu t0, 0(a0)
    (0x8033_f5f4, 0x2084_0002), // addi a0, a0, 2
    (0x8033_f5f8, 0x0068_4004), // sllv t0, t0, v1
    (0x8033_f5fc, 0x0048_1025), // or v0, v0, t0
    (0x8033_f600, 0x0009_4c02), // srl t1, t1, 16
    (0x8033_f604, 0x3921_7c1f), // xori at, t1, 0x7c1f
    (0x8033_f608, 0x1020_0015), // beq at, zero, table sentinel exit
    (0x8033_f60c, 0x3921_fe00), // xori at, t1, 0xfe00
    (0x8033_f610, 0x1020_ff93), // beq at, zero, direct refill path
    (0x8033_f614, 0xa4a9_0000), // sh t1, 0(a1)
    (0x8033_f618, 0x1160_ffd8), // beq t3, zero, next table code
    (0x8033_f61c, 0x20a5_0002), // addi a1, a1, 2
    (0x8033_f620, 0x316a_ffff), // andi t2, t3, 0xffff
    (0x8033_f624, 0x3941_7c1f), // xori at, t2, 0x7c1f
    (0x8033_f628, 0x1020_000d), // beq at, zero, table sentinel exit
    (0x8033_f62c, 0x3941_fe00), // xori at, t2, 0xfe00
    (0x8033_f630, 0x1020_ff8b), // beq at, zero, direct refill path
    (0x8033_f634, 0xa4aa_0000), // sh t2, 0(a1)
    (0x8033_f638, 0x000b_5402), // srl t2, t3, 16
    (0x8033_f63c, 0x1140_ffcf), // beq t2, zero, next table code
    (0x8033_f640, 0x20a5_0002), // addi a1, a1, 2
    (0x8033_f644, 0x3941_7c1f), // xori at, t2, 0x7c1f
];
const BR2_BITSTREAM_DECODE_LOOP_TAIL_INSTRUCTIONS: [(u32, u32); 6] = [
    (0x8033_f648, 0x1020_0005), // beq at, zero, table sentinel exit
    (0x8033_f64c, 0x3941_fe00), // xori at, t2, 0xfe00
    (0x8033_f650, 0x1020_ff83), // beq at, zero, direct refill path
    (0x8033_f654, 0xa4aa_0000), // sh t2, 0(a1)
    (0x8033_f658, 0x1000_ffc8), // beq zero, zero, next table code
    (0x8033_f65c, 0x20a5_0002), // addi a1, a1, 2
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
    pub end_r2: u32,
    pub end_r3: u32,
    pub end_r4: u32,
    pub end_r5: u32,
    pub end_r6: u32,
    pub end_r7: u32,
    pub end_r8: u32,
    pub end_r9: u32,
    pub end_r10: u32,
    pub end_r11: u32,
    pub end_r12: u32,
    pub end_r13: u32,
    pub end_r14: u32,
    pub end_r15: u32,
    pub end_r16: u32,
    pub end_r17: u32,
    pub end_r18: u32,
    pub end_r19: u32,
    pub end_r20: u32,
    pub end_r21: u32,
    pub end_r22: u32,
    pub end_r23: u32,
    pub end_r24: u32,
    pub end_r25: u32,
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
            end_r2: cpu.regs[2],
            end_r3: cpu.regs[3],
            end_r4: cpu.regs[4],
            end_r5: cpu.regs[5],
            end_r6: cpu.regs[6],
            end_r7: cpu.regs[7],
            end_r8: cpu.regs[8],
            end_r9: cpu.regs[9],
            end_r10: cpu.regs[10],
            end_r11: cpu.regs[11],
            end_r12: cpu.regs[12],
            end_r13: cpu.regs[13],
            end_r14: cpu.regs[14],
            end_r15: cpu.regs[15],
            end_r16: cpu.regs[16],
            end_r17: cpu.regs[17],
            end_r18: cpu.regs[18],
            end_r19: cpu.regs[19],
            end_r20: cpu.regs[20],
            end_r21: cpu.regs[21],
            end_r22: cpu.regs[22],
            end_r23: cpu.regs[23],
            end_r24: cpu.regs[24],
            end_r25: cpu.regs[25],
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
            "{{\"start_pc\":{},\"start_pc_hex\":\"0x{:08x}\",\"end_pc\":{},\"end_pc_hex\":\"0x{:08x}\",\"next_pc\":{},\"next_pc_hex\":\"0x{:08x}\",\"instruction\":{},\"instruction_hex\":{},\"end_r2\":{},\"end_r2_hex\":\"0x{:08x}\",\"end_r3\":{},\"end_r3_hex\":\"0x{:08x}\",\"end_r4\":{},\"end_r4_hex\":\"0x{:08x}\",\"end_r5\":{},\"end_r5_hex\":\"0x{:08x}\",\"end_r6\":{},\"end_r6_hex\":\"0x{:08x}\",\"end_r7\":{},\"end_r7_hex\":\"0x{:08x}\",\"end_r8\":{},\"end_r8_hex\":\"0x{:08x}\",\"end_r9\":{},\"end_r9_hex\":\"0x{:08x}\",\"end_r10\":{},\"end_r10_hex\":\"0x{:08x}\",\"end_r11\":{},\"end_r11_hex\":\"0x{:08x}\",\"end_r12\":{},\"end_r12_hex\":\"0x{:08x}\",\"end_r13\":{},\"end_r13_hex\":\"0x{:08x}\",\"end_r14\":{},\"end_r14_hex\":\"0x{:08x}\",\"end_r15\":{},\"end_r15_hex\":\"0x{:08x}\",\"end_r16\":{},\"end_r16_hex\":\"0x{:08x}\",\"end_r17\":{},\"end_r17_hex\":\"0x{:08x}\",\"end_r18\":{},\"end_r18_hex\":\"0x{:08x}\",\"end_r19\":{},\"end_r19_hex\":\"0x{:08x}\",\"end_r20\":{},\"end_r20_hex\":\"0x{:08x}\",\"end_r21\":{},\"end_r21_hex\":\"0x{:08x}\",\"end_r22\":{},\"end_r22_hex\":\"0x{:08x}\",\"end_r23\":{},\"end_r23_hex\":\"0x{:08x}\",\"end_r24\":{},\"end_r24_hex\":\"0x{:08x}\",\"end_r25\":{},\"end_r25_hex\":\"0x{:08x}\",\"end_sp\":{},\"end_sp_hex\":\"0x{:08x}\",\"end_ra\":{},\"end_ra_hex\":\"0x{:08x}\",\"cycles_before\":{},\"cycles_after\":{},\"cycles_elapsed\":{},\"outcome\":\"{:?}\"}}",
            self.start_pc,
            self.start_pc,
            self.end_pc,
            self.end_pc,
            self.next_pc,
            self.next_pc,
            optional_u32_json(self.instruction),
            optional_u32_hex_json(self.instruction),
            self.end_r2,
            self.end_r2,
            self.end_r3,
            self.end_r3,
            self.end_r4,
            self.end_r4,
            self.end_r5,
            self.end_r5,
            self.end_r6,
            self.end_r6,
            self.end_r7,
            self.end_r7,
            self.end_r8,
            self.end_r8,
            self.end_r9,
            self.end_r9,
            self.end_r10,
            self.end_r10,
            self.end_r11,
            self.end_r11,
            self.end_r12,
            self.end_r12,
            self.end_r13,
            self.end_r13,
            self.end_r14,
            self.end_r14,
            self.end_r15,
            self.end_r15,
            self.end_r16,
            self.end_r16,
            self.end_r17,
            self.end_r17,
            self.end_r18,
            self.end_r18,
            self.end_r19,
            self.end_r19,
            self.end_r20,
            self.end_r20,
            self.end_r21,
            self.end_r21,
            self.end_r22,
            self.end_r22,
            self.end_r23,
            self.end_r23,
            self.end_r24,
            self.end_r24,
            self.end_r25,
            self.end_r25,
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

fn br2_native_hle_disabled(feature: &str) -> bool {
    static DISABLED: OnceLock<Vec<String>> = OnceLock::new();

    let disabled = DISABLED.get_or_init(|| {
        std::env::var("BR2_NATIVE_DISABLE_HLE")
            .unwrap_or_default()
            .split([',', ';', ':', ' '])
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect()
    });

    disabled.iter().any(|value| {
        value == "all"
            || value == feature
            || (value == "post_vs" && feature.starts_with("post_vs_"))
    })
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
        if let Some(report) = self.try_hle_br2_bios_b0_dispatch(start_pc, cycles_before, bus) {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }
        if self.try_hle_br2_bios_irq_return(bus) {
            self.cycles += 1;
            self.regs[0] = 0;
            let report =
                self.step_report_from(start_pc, None, cycles_before, StepOutcome::Continue);
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }
        if let Some(report) = self.try_hle_br2_bios_b0_wait_event(start_pc, cycles_before, bus) {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }
        if let Some(report) = self.try_hle_br2_bios_b0_test_event(start_pc, cycles_before, bus) {
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

        if let Some(report) = self.try_hle_br2_credit_check(start_pc, cycles_before, bus) {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_hle_br2_post_vs_packed_vertex_helper(start_pc, cycles_before, bus)
        {
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
            self.try_fast_forward_br2_status_pointer_scan(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_br2_status_halfword_wait_loop(start_pc, cycles_before, bus)
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
            self.try_fast_forward_br2_post_vs_table_group_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_br2_post_vs_table_select_group_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_br2_post_vs_null_link_scan_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_br2_post_vs_stack_link_scan_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_br2_post_vs_stack_packet_scan_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_br2_post_vs_record_copy_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) =
            self.try_fast_forward_br2_post_vs_vertex_record_loop(start_pc, cycles_before, bus)
        {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) = self.try_fast_forward_br2_post_vs_strided_pointer_copy_loop(
            start_pc,
            cycles_before,
            bus,
        ) {
            bus.tick(report.cycles_elapsed);
            bus.clear_trace_context();
            return report;
        }

        if let Some(report) = self.try_fast_forward_br2_post_vs_alt_strided_pointer_copy_loop(
            start_pc,
            cycles_before,
            bus,
        ) {
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

        if let Some(report) =
            self.try_fast_forward_br2_bitstream_decode_loop(start_pc, cycles_before, bus)
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
            "{{\"pc\":{},\"next_pc\":{},\"cycles\":{},\"halted\":{},\"status\":{},\"cause\":{},\"epc\":{},\"badvaddr\":{},\"r2\":{},\"r3\":{},\"r4\":{},\"r5\":{},\"r6\":{},\"r8\":{},\"r9\":{},\"r10\":{},\"r11\":{},\"r16\":{},\"r17\":{},\"r18\":{},\"r19\":{},\"r20\":{},\"r21\":{},\"r22\":{},\"r23\":{},\"r29\":{},\"r31\":{},\"gte_command_counts\":[{}]}}",
            self.pc,
            self.next_pc,
            self.cycles,
            self.halted,
            self.cp0[CP0_STATUS],
            self.cp0[CP0_CAUSE],
            self.cp0[CP0_EPC],
            self.cp0[CP0_BADVADDR],
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
            self.regs[17],
            self.regs[18],
            self.regs[19],
            self.regs[20],
            self.regs[21],
            self.regs[22],
            self.regs[23],
            self.regs[29],
            self.regs[31],
            self.gte_command_counts_json()
        )
    }

    pub fn cause_excode(&self) -> u32 {
        (self.cp0[CP0_CAUSE] & CAUSE_EXCODE_MASK) >> 2
    }

    pub fn pending_load_json(&self) -> String {
        self.pending_load.map_or_else(
            || "null".to_string(),
            |(register, value)| {
                format!(
                    "{{\"register\":{},\"value\":{},\"value_hex\":\"0x{:08x}\"}}",
                    register, value, value
                )
            },
        )
    }

    pub fn delay_slot_branch_pc_json(&self) -> String {
        optional_u32_hex_json(self.delay_slot_branch_pc)
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
            end_r2: self.regs[2],
            end_r3: self.regs[3],
            end_r4: self.regs[4],
            end_r5: self.regs[5],
            end_r6: self.regs[6],
            end_r7: self.regs[7],
            end_r8: self.regs[8],
            end_r9: self.regs[9],
            end_r10: self.regs[10],
            end_r11: self.regs[11],
            end_r12: self.regs[12],
            end_r13: self.regs[13],
            end_r14: self.regs[14],
            end_r15: self.regs[15],
            end_r16: self.regs[16],
            end_r17: self.regs[17],
            end_r18: self.regs[18],
            end_r19: self.regs[19],
            end_r20: self.regs[20],
            end_r21: self.regs[21],
            end_r22: self.regs[22],
            end_r23: self.regs[23],
            end_r24: self.regs[24],
            end_r25: self.regs[25],
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
        if br2_native_hle_disabled("draw_sync_wait")
            || self.pc != BR2_DRAW_SYNC_WAIT_LOOP_START
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

    fn try_fast_forward_br2_status_pointer_scan(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if br2_native_hle_disabled("status_pointer_scan")
            || self.pc != BR2_STATUS_POINTER_SCAN_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
            || bus.cache_isolated()
            || !br2_status_pointer_scan_signature_matches(bus)
        {
            return None;
        }

        if self.vblank_irq_can_preempt(bus)
            && bus.cycles_until_next_vblank() <= BR2_STATUS_POINTER_SCAN_CYCLES
        {
            return None;
        }

        let pointer_slot = self.regs[6].wrapping_add(0x14);
        if pointer_slot & 0x03 != 0 || !br2_ram_byte_range(pointer_slot, 4, bus.ram_len()) {
            return None;
        }

        let pointer = bus.read_u32_fast_no_trace(pointer_slot);
        let next_pointer = pointer.checked_add(4)?;
        if pointer & 0x01 != 0 || !br2_ram_byte_range(next_pointer, 2, bus.ram_len()) {
            return None;
        }

        let high_status = bus.read_u16(next_pointer) & BR2_STATUS_HALFWORD_WAIT_LOOP_HIGH_MASK;
        if matches!(
            high_status,
            BR2_STATUS_HALFWORD_WAIT_LOOP_FC_STATUS
                | BR2_STATUS_HALFWORD_WAIT_LOOP_FE_STATUS
                | BR2_STATUS_HALFWORD_WAIT_LOOP_FF_STATUS
        ) {
            return None;
        }

        bus.write_u32(pointer_slot, next_pointer);
        self.regs[2] = u32::from(high_status);
        self.regs[3] = pointer;
        if high_status == 0 {
            self.pc = BR2_STATUS_POINTER_SCAN_EXIT;
            self.next_pc = BR2_STATUS_POINTER_SCAN_EXIT.wrapping_add(4);
        } else {
            self.pc = BR2_STATUS_POINTER_SCAN_FALLTHROUGH;
            self.next_pc = BR2_STATUS_POINTER_SCAN_FALLTHROUGH.wrapping_add(4);
        }
        self.cycles = self.cycles.saturating_add(BR2_STATUS_POINTER_SCAN_CYCLES);
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(BR2_STATUS_POINTER_SCAN_INSTRUCTIONS[0].1),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_br2_status_halfword_wait_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if br2_native_hle_disabled("status_halfword_wait")
            || (self.pc != BR2_STATUS_HALFWORD_WAIT_LOOP_START
                && self.pc != BR2_STATUS_HALFWORD_WAIT_LOOP_TAIL_LOAD)
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
            || bus.cache_isolated()
            || !br2_status_halfword_wait_loop_signature_matches(bus)
        {
            return None;
        }

        let pointer_slot = self.regs[6].wrapping_add(0x14);
        if pointer_slot & 0x03 != 0 || !br2_ram_byte_range(pointer_slot, 4, bus.ram_len()) {
            return None;
        }
        let watched_pointer = bus.read_u32_fast_no_trace(pointer_slot);
        if watched_pointer & 0x01 != 0 || !br2_ram_byte_range(watched_pointer, 2, bus.ram_len()) {
            return None;
        }
        let status = bus.read_u16(watched_pointer);
        let high_status = status & BR2_STATUS_HALFWORD_WAIT_LOOP_HIGH_MASK;
        if high_status == 0
            || matches!(
                high_status,
                BR2_STATUS_HALFWORD_WAIT_LOOP_FC_STATUS
                    | BR2_STATUS_HALFWORD_WAIT_LOOP_FE_STATUS
                    | BR2_STATUS_HALFWORD_WAIT_LOOP_FF_STATUS
            )
        {
            return None;
        }

        let skipped_cycles = bus.cycles_until_next_vblank().max(1);
        bus.write_u16(
            watched_pointer,
            status & BR2_STATUS_HALFWORD_WAIT_LOOP_LOW_MASK,
        );
        self.regs[5] = watched_pointer;
        self.pc = BR2_STATUS_HALFWORD_WAIT_LOOP_START;
        self.next_pc = BR2_STATUS_HALFWORD_WAIT_LOOP_START.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(skipped_cycles);
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(bus.read_u32_fast_no_trace(start_pc)),
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
        if br2_native_hle_disabled("frame_counter_wait")
            || self.pc != BR2_FRAME_COUNTER_WAIT_LOOP_START
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

        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let skipped_cycles = cycles_until_vblank.max(1);
        let skipped_iterations = (skipped_cycles
            / BR2_FRAME_COUNTER_WAIT_LOOP_CYCLES_PER_ITERATION)
            .max(1)
            .min(u64::from(stack_counter.saturating_sub(1)));
        if skipped_iterations == 0 {
            return None;
        }
        if skipped_cycles >= cycles_until_vblank {
            bus.write_u32(
                BR2_FRAME_COUNTER_WAIT_LOOP_GLOBAL_COUNTER,
                frame_counter.saturating_add(1),
            );
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
        if br2_native_hle_disabled("irq_poll_timeout")
            || (self.pc != BR2_IRQ_POLL_TIMEOUT_INITIAL_DECREMENT
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
        if self.vblank_irq_can_preempt(bus) && bus.cycles_until_next_vblank() <= skipped_cycles {
            return None;
        }

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
        let cycles_until_vblank = bus.cycles_until_next_vblank();
        if cycles_until_vblank <= BR2_BANKED_HALFWORD_COPY_CYCLES_PER_HALFWORD {
            return None;
        }
        let vblank_limited_halfwords =
            ((cycles_until_vblank - 1) / BR2_BANKED_HALFWORD_COPY_CYCLES_PER_HALFWORD) as u32;
        let capped_halfwords = halfwords.min(vblank_limited_halfwords);
        if capped_halfwords == 0 {
            return None;
        }
        let capped_byte_count = capped_halfwords.checked_mul(2)?;
        let last_halfword = bus.try_copy_halfwords(source, destination, capped_halfwords)?;

        self.regs[2] = 0;
        self.regs[3] = source.wrapping_add(capped_byte_count);
        self.regs[16] = copied_halfbytes.wrapping_add(capped_byte_count);
        self.regs[17] = self.regs[17].wrapping_add(capped_byte_count);
        self.regs[18] = destination.wrapping_add(capped_byte_count);
        let completed_loop = capped_halfwords == halfwords;
        self.pc = if completed_loop {
            BR2_BANKED_HALFWORD_COPY_LOOP_EXIT
        } else {
            BR2_BANKED_HALFWORD_COPY_LOOP_START
        };
        self.next_pc = self.pc.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(
            u64::from(capped_halfwords)
                .saturating_mul(BR2_BANKED_HALFWORD_COPY_CYCLES_PER_HALFWORD),
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

    fn try_fast_forward_br2_post_vs_table_group_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if br2_native_hle_disabled("post_vs_table_group") {
            return None;
        }

        if self.pc != BR2_POST_VS_TABLE_GROUP_LOOP_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        if !br2_post_vs_table_group_loop_signature_matches(bus) {
            return None;
        }

        let owner = self.regs[4];
        let table_meta_offset = bus.read_u32(owner.wrapping_add(0x7c));
        let outer_limit = bus.read_u32(owner.wrapping_add(0x28));
        let outer_index = self.regs[7];
        if outer_index >= outer_limit {
            return None;
        }

        let remaining = outer_limit.wrapping_sub(outer_index);
        let count_address = self.regs[6].wrapping_add(table_meta_offset);
        let noop_gap_iterations =
            br2_post_vs_table_group_noop_count_run(count_address, remaining, bus.ram_len());
        let nonpositive_count_iterations = if noop_gap_iterations == 0 {
            br2_post_vs_table_group_nonpositive_count_run(count_address, remaining, bus)
        } else {
            0
        };
        let (skipped_iterations, charged_iterations) = if noop_gap_iterations > 0 {
            (
                noop_gap_iterations,
                self.br2_post_vs_table_group_charged_noop_iterations(noop_gap_iterations, bus),
            )
        } else if nonpositive_count_iterations > 0 {
            (
                nonpositive_count_iterations,
                self.br2_post_vs_table_group_charged_noop_iterations(
                    nonpositive_count_iterations,
                    bus,
                ),
            )
        } else {
            let cycles_until_vblank = bus.cycles_until_next_vblank();
            if cycles_until_vblank <= BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION {
                return None;
            }
            let vblank_limited_iterations =
                ((cycles_until_vblank - 1) / BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION) as u32;
            let skipped_iterations = remaining.min(vblank_limited_iterations);
            (skipped_iterations, skipped_iterations)
        };
        if skipped_iterations == 0 {
            return None;
        }
        let completed_loop = skipped_iterations == remaining;

        self.regs[2] = 0;
        self.regs[3] = table_meta_offset;
        self.regs[5] = 0;
        self.regs[6] = self.regs[6].wrapping_add(skipped_iterations.wrapping_mul(8));
        self.regs[7] = outer_index.wrapping_add(skipped_iterations);
        self.pc = if completed_loop {
            BR2_POST_VS_TABLE_GROUP_LOOP_EXIT
        } else {
            BR2_POST_VS_TABLE_GROUP_LOOP_START
        };
        self.next_pc = self.pc.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(
            u64::from(charged_iterations)
                .saturating_mul(BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION),
        );
        self.regs[0] = 0;
        self.pending_load = None;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;

        Some(self.step_report_from(
            start_pc,
            Some(BR2_POST_VS_TABLE_GROUP_PREFIX_INSTRUCTIONS[0]),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn br2_post_vs_table_group_charged_iterations(&self, iterations: u32, bus: &Bus) -> u32 {
        if !self.vblank_irq_can_preempt(bus) {
            return iterations;
        }
        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let iterations_before_vblank = ((cycles_until_vblank.saturating_sub(1))
            / BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION)
            as u32;
        iterations.min(iterations_before_vblank)
    }

    fn br2_post_vs_table_group_charged_noop_iterations(&self, iterations: u32, bus: &Bus) -> u32 {
        self.br2_post_vs_table_group_charged_iterations(iterations, bus)
            .min(BR2_POST_VS_TABLE_GROUP_MAX_CHARGED_NOOP_ITERATIONS)
    }

    fn try_fast_forward_br2_post_vs_table_select_group_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if br2_native_hle_disabled("post_vs_table_select_group") {
            return None;
        }

        if !matches!(
            self.pc,
            BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START
                | BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_DELAY
                | BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_BRANCH
                | BR2_POST_VS_TABLE_SELECT_GROUP_TAIL_LOAD
                | BR2_POST_VS_TABLE_SELECT_GROUP_TAIL_INCREMENT
        ) || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
        {
            return None;
        }

        if !br2_post_vs_table_select_group_loop_signature_matches(bus) {
            return None;
        }

        let owner = self.regs[4];
        let instruction = match self.pc {
            BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START => {
                if !matches!(self.pending_load, None | Some((2, _))) {
                    return None;
                }
                let count = bus.read_u32_fast_no_trace(owner.wrapping_add(0x0c));
                if count.wrapping_sub(1) < 2 {
                    return None;
                }
                let compare_value = bus.read_u32_fast_no_trace(owner.wrapping_add(0x14));
                if compare_value == self.regs[6] {
                    return None;
                }
                BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_INSTRUCTIONS[0].1
            }
            BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_DELAY => {
                let compare_value = match self.pending_load {
                    Some((2, value)) => value,
                    None => bus.read_u32_fast_no_trace(owner.wrapping_add(0x14)),
                    _ => return None,
                };
                if compare_value == self.regs[6] {
                    return None;
                }
                bus.read_u32_fast_no_trace(self.pc)
            }
            BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_BRANCH => {
                if self.pending_load.is_some() || self.regs[2] == self.regs[6] {
                    return None;
                }
                bus.read_u32_fast_no_trace(self.pc)
            }
            BR2_POST_VS_TABLE_SELECT_GROUP_TAIL_LOAD => {
                if self.pending_load.is_some() {
                    return None;
                }
                bus.read_u32_fast_no_trace(self.pc)
            }
            BR2_POST_VS_TABLE_SELECT_GROUP_TAIL_INCREMENT => {
                if !matches!(self.pending_load, None | Some((2, _))) {
                    return None;
                }
                bus.read_u32_fast_no_trace(self.pc)
            }
            _ => return None,
        };

        let outer_limit = match self.pc {
            BR2_POST_VS_TABLE_SELECT_GROUP_TAIL_INCREMENT => match self.pending_load {
                Some((2, value)) => value,
                None => bus.read_u32_fast_no_trace(owner.wrapping_add(0x24)),
                _ => return None,
            },
            _ => bus.read_u32_fast_no_trace(owner.wrapping_add(0x24)),
        };
        let outer_index = self.regs[7];
        if outer_index >= outer_limit {
            return None;
        }

        let remaining = outer_limit.wrapping_sub(outer_index);
        let noop_gap_iterations =
            br2_post_vs_table_select_group_noop_record_run(self.regs[5], remaining, bus.ram_len());
        let (skipped_iterations, charged_iterations) = if noop_gap_iterations > 0 {
            (
                noop_gap_iterations,
                self.br2_post_vs_table_select_group_charged_noop_iterations(
                    noop_gap_iterations,
                    bus,
                ),
            )
        } else {
            let cycles_until_vblank = bus.cycles_until_next_vblank();
            if cycles_until_vblank <= BR2_POST_VS_TABLE_SELECT_GROUP_CYCLES_PER_ITERATION {
                return None;
            }
            let vblank_limited_iterations = ((cycles_until_vblank - 1)
                / BR2_POST_VS_TABLE_SELECT_GROUP_CYCLES_PER_ITERATION)
                as u32;
            let skipped_iterations = remaining.min(vblank_limited_iterations);
            (skipped_iterations, skipped_iterations)
        };
        if skipped_iterations == 0 {
            return None;
        }
        let completed_loop = skipped_iterations == remaining;

        self.regs[2] = 0;
        self.regs[5] = self.regs[5].wrapping_add(skipped_iterations.wrapping_mul(0x14));
        self.regs[7] = outer_index.wrapping_add(skipped_iterations);
        self.pc = if completed_loop {
            BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_EXIT
        } else {
            BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START
        };
        self.next_pc = self.pc.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(
            u64::from(charged_iterations)
                .saturating_mul(BR2_POST_VS_TABLE_SELECT_GROUP_CYCLES_PER_ITERATION),
        );
        self.regs[0] = 0;
        self.pending_load = None;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;

        Some(self.step_report_from(
            start_pc,
            Some(instruction),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn br2_post_vs_table_select_group_charged_iterations(&self, iterations: u32, bus: &Bus) -> u32 {
        if !self.vblank_irq_can_preempt(bus) {
            return iterations;
        }
        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let iterations_before_vblank = ((cycles_until_vblank.saturating_sub(1))
            / BR2_POST_VS_TABLE_SELECT_GROUP_CYCLES_PER_ITERATION)
            as u32;
        iterations.min(iterations_before_vblank)
    }

    fn br2_post_vs_table_select_group_charged_noop_iterations(
        &self,
        iterations: u32,
        bus: &Bus,
    ) -> u32 {
        self.br2_post_vs_table_select_group_charged_iterations(iterations, bus)
            .min(BR2_POST_VS_TABLE_SELECT_GROUP_MAX_CHARGED_NOOP_ITERATIONS)
    }

    fn try_fast_forward_br2_post_vs_null_link_scan_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if br2_native_hle_disabled("post_vs_null_link_scan") {
            return None;
        }

        if !matches!(
            self.pc,
            BR2_POST_VS_NULL_LINK_SCAN_LOOP_START
                | BR2_POST_VS_NULL_LINK_SCAN_TAIL_LOAD
                | BR2_POST_VS_NULL_LINK_SCAN_TAIL_DELAY
                | BR2_POST_VS_NULL_LINK_SCAN_TAIL_BRANCH
        ) || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
        {
            return None;
        }

        if !br2_post_vs_null_link_scan_loop_signature_matches(bus) {
            return None;
        }

        let sentinel = self.regs[9];
        if sentinel == 0 {
            return None;
        }

        let instruction = match self.pc {
            BR2_POST_VS_NULL_LINK_SCAN_LOOP_START | BR2_POST_VS_NULL_LINK_SCAN_TAIL_LOAD => {
                if self.pending_load.is_some()
                    || !br2_post_vs_null_link_scan_terminal_pointer(self.regs[5], bus.ram_len())
                {
                    return None;
                }
                bus.read_u32_fast_no_trace(self.pc)
            }
            BR2_POST_VS_NULL_LINK_SCAN_TAIL_DELAY => {
                let link = match self.pending_load {
                    Some((5, value)) => value,
                    None => self.regs[5],
                    _ => return None,
                };
                if !br2_post_vs_null_link_scan_terminal_pointer(link, bus.ram_len()) {
                    return None;
                }
                bus.read_u32_fast_no_trace(self.pc)
            }
            BR2_POST_VS_NULL_LINK_SCAN_TAIL_BRANCH => {
                if self.pending_load.is_some()
                    || !br2_post_vs_null_link_scan_terminal_pointer(self.regs[5], bus.ram_len())
                {
                    return None;
                }
                bus.read_u32_fast_no_trace(self.pc)
            }
            _ => return None,
        };

        self.regs[2] = 0;
        self.regs[5] = sentinel;
        self.pc = BR2_POST_VS_NULL_LINK_SCAN_LOOP_EXIT;
        self.next_pc = BR2_POST_VS_NULL_LINK_SCAN_LOOP_EXIT + 4;
        self.cycles = self
            .cycles
            .saturating_add(BR2_POST_VS_NULL_LINK_SCAN_CYCLES);
        self.regs[0] = 0;
        self.pending_load = None;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;

        Some(self.step_report_from(
            start_pc,
            Some(instruction),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_br2_post_vs_stack_link_scan_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if br2_native_hle_disabled("post_vs_stack_link_scan") {
            return None;
        }

        if !matches!(
            self.pc,
            BR2_POST_VS_STACK_LINK_SCAN_LOOP_START
                | BR2_POST_VS_STACK_LINK_SCAN_RELOAD
                | BR2_POST_VS_STACK_LINK_SCAN_RELOAD_DELAY
                | BR2_POST_VS_STACK_LINK_SCAN_NEXT_LOAD
                | BR2_POST_VS_STACK_LINK_SCAN_COMPARE
                | BR2_POST_VS_STACK_LINK_SCAN_TAIL_BRANCH
                | BR2_POST_VS_STACK_LINK_SCAN_TAIL_STORE
        ) || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
        {
            return None;
        }

        if !br2_post_vs_stack_link_scan_loop_signature_matches(bus)
            && !br2_post_vs_stack_link_scan_current_instruction_matches(self.pc, bus)
        {
            return None;
        }

        let stack_slot = self.regs[29].wrapping_add(BR2_POST_VS_STACK_LINK_SCAN_STACK_SLOT_OFFSET);
        if !br2_ram_word_range(stack_slot, 1, bus.ram_len()) {
            return None;
        }

        let stack_link = bus.read_u32_fast_no_trace(stack_slot);
        let (instruction, charged_cycles) = match self.pc {
            BR2_POST_VS_STACK_LINK_SCAN_LOOP_START => {
                if self.pending_load.is_some()
                    || (!br2_post_vs_stack_link_scan_terminal_pointer(stack_link, bus.ram_len())
                        && !br2_post_vs_stack_link_scan_empty_node_with_terminal_next(
                            stack_link, bus,
                        ))
                {
                    return None;
                }
                bus.write_u32(self.regs[29].wrapping_add(0x128), 0);
                bus.write_u32(self.regs[29].wrapping_add(0x120), 0);
                self.regs[9] = stack_link;
                self.regs[10] = 0;
                self.regs[18] = if br2_ram_word_range(stack_link.wrapping_add(4), 1, bus.ram_len())
                {
                    bus.read_u32_fast_no_trace(stack_link.wrapping_add(4))
                } else {
                    0
                };
                self.regs[20] = stack_link.wrapping_add(0x0c);
                (
                    BR2_POST_VS_STACK_LINK_SCAN_LOOP_INSTRUCTIONS[0].1,
                    BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_START_CYCLES,
                )
            }
            BR2_POST_VS_STACK_LINK_SCAN_RELOAD => {
                if self.pending_load.is_some()
                    || (!br2_post_vs_stack_link_scan_terminal_pointer(stack_link, bus.ram_len())
                        && !br2_post_vs_stack_link_scan_empty_node_with_terminal_next(
                            stack_link, bus,
                        ))
                {
                    return None;
                }
                (
                    bus.read_u32_fast_no_trace(self.pc),
                    BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_RELOAD_CYCLES,
                )
            }
            BR2_POST_VS_STACK_LINK_SCAN_RELOAD_DELAY => {
                let link = match self.pending_load {
                    Some((8, value)) => value,
                    None => self.regs[8],
                    _ => return None,
                };
                if !br2_post_vs_stack_link_scan_terminal_pointer(link, bus.ram_len())
                    && !br2_post_vs_stack_link_scan_empty_node_with_terminal_next(link, bus)
                {
                    return None;
                }
                (
                    bus.read_u32_fast_no_trace(self.pc),
                    BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_RELOAD_DELAY_CYCLES,
                )
            }
            BR2_POST_VS_STACK_LINK_SCAN_NEXT_LOAD => {
                if self.pending_load.is_some()
                    || (!br2_post_vs_stack_link_scan_terminal_pointer(self.regs[8], bus.ram_len())
                        && !br2_post_vs_stack_link_scan_empty_node_with_terminal_next(
                            self.regs[8],
                            bus,
                        ))
                {
                    return None;
                }
                (
                    bus.read_u32_fast_no_trace(self.pc),
                    BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_NEXT_LOAD_CYCLES,
                )
            }
            BR2_POST_VS_STACK_LINK_SCAN_COMPARE => {
                let link = match self.pending_load {
                    Some((8, value)) => value,
                    None => self.regs[8],
                    _ => return None,
                };
                if !br2_post_vs_stack_link_scan_terminal_pointer(link, bus.ram_len()) {
                    return None;
                }
                (
                    bus.read_u32_fast_no_trace(self.pc),
                    BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_COMPARE_CYCLES,
                )
            }
            BR2_POST_VS_STACK_LINK_SCAN_TAIL_BRANCH => {
                if self.pending_load.is_some()
                    || !br2_post_vs_stack_link_scan_terminal_pointer(self.regs[8], bus.ram_len())
                    || self.regs[2] != u32::MAX
                {
                    return None;
                }
                (
                    bus.read_u32_fast_no_trace(self.pc),
                    BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_TAIL_BRANCH_CYCLES,
                )
            }
            BR2_POST_VS_STACK_LINK_SCAN_TAIL_STORE => {
                if self.pending_load.is_some()
                    || !br2_post_vs_stack_link_scan_terminal_pointer(self.regs[8], bus.ram_len())
                {
                    return None;
                }
                (
                    bus.read_u32_fast_no_trace(self.pc),
                    BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_TAIL_STORE_CYCLES,
                )
            }
            _ => return None,
        };

        self.regs[2] = u32::MAX;
        self.regs[8] = u32::MAX;
        bus.write_u32(stack_slot, u32::MAX);
        self.pc = BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT;
        self.next_pc = BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT + 4;
        self.cycles = self.cycles.saturating_add(charged_cycles);
        self.regs[0] = 0;
        self.pending_load = None;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;

        Some(self.step_report_from(
            start_pc,
            Some(instruction),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_br2_post_vs_stack_packet_scan_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if br2_native_hle_disabled("post_vs_stack_packet_scan") {
            return None;
        }

        if !matches!(
            self.pc,
            BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START
                | BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LENGTH_LOAD
                | BR2_POST_VS_STACK_PACKET_SCAN_TAIL_INDEX_LOAD
                | BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LIMIT_LOAD
                | BR2_POST_VS_STACK_PACKET_SCAN_TAIL_SHIFT
                | BR2_POST_VS_STACK_PACKET_SCAN_TAIL_CURSOR_ADD
                | BR2_POST_VS_STACK_PACKET_SCAN_TAIL_INDEX_ADD
                | BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPARE
                | BR2_POST_VS_STACK_PACKET_SCAN_TAIL_BRANCH
        ) && !br2_post_vs_stack_packet_scan_body_fast_forward_pc(self.pc)
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
        {
            return None;
        }

        if !br2_post_vs_stack_packet_scan_loop_signature_matches(bus)
            && !br2_post_vs_stack_packet_scan_current_instruction_matches(self.pc, bus)
        {
            return None;
        }

        let sp = self.regs[29];
        let limit_slot = sp.wrapping_add(BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET);
        let length_slot = sp.wrapping_add(BR2_POST_VS_STACK_PACKET_SCAN_LENGTH_STACK_OFFSET);
        let index_slot = sp.wrapping_add(BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET);
        if !br2_ram_word_range(limit_slot, 1, bus.ram_len())
            || !br2_ram_word_range(length_slot, 1, bus.ram_len())
            || !br2_ram_word_range(index_slot, 1, bus.ram_len())
        {
            return None;
        }

        let instruction = bus.read_u32_fast_no_trace(self.pc);
        let limit_from_stack = bus.read_u32_fast_no_trace(limit_slot);
        let (mut cursor, mut index, limit, base_cycles, completed_tail) = match self.pc {
            BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START => {
                if self.pending_load.is_some() {
                    return None;
                }
                (
                    self.regs[20],
                    bus.read_u32_fast_no_trace(index_slot),
                    limit_from_stack,
                    0,
                    false,
                )
            }
            pc if br2_post_vs_stack_packet_scan_body_fast_forward_pc(pc) => {
                if self.pending_load.is_some() {
                    return None;
                }
                (
                    self.regs[20].wrapping_sub(4),
                    bus.read_u32_fast_no_trace(index_slot),
                    limit_from_stack,
                    0,
                    false,
                )
            }
            BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LENGTH_LOAD => {
                if self.pending_load.is_some() {
                    return None;
                }
                let length = bus.read_u32_fast_no_trace(length_slot);
                (
                    self.regs[20].wrapping_add(length.wrapping_shl(2)),
                    bus.read_u32_fast_no_trace(index_slot).wrapping_add(1),
                    limit_from_stack,
                    BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPLETION_CYCLES,
                    true,
                )
            }
            BR2_POST_VS_STACK_PACKET_SCAN_TAIL_INDEX_LOAD => {
                let length = match self.pending_load {
                    Some((9, value)) => value,
                    None => self.regs[9],
                    _ => return None,
                };
                (
                    self.regs[20].wrapping_add(length.wrapping_shl(2)),
                    bus.read_u32_fast_no_trace(index_slot).wrapping_add(1),
                    limit_from_stack,
                    BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPLETION_CYCLES,
                    true,
                )
            }
            BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LIMIT_LOAD => {
                let index_before_increment = match self.pending_load {
                    Some((10, value)) => value,
                    None => self.regs[10],
                    _ => return None,
                };
                (
                    self.regs[20].wrapping_add(self.regs[9].wrapping_shl(2)),
                    index_before_increment.wrapping_add(1),
                    limit_from_stack,
                    BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPLETION_CYCLES,
                    true,
                )
            }
            BR2_POST_VS_STACK_PACKET_SCAN_TAIL_SHIFT => {
                let limit = match self.pending_load {
                    Some((11, value)) => value,
                    None => self.regs[11],
                    _ => return None,
                };
                (
                    self.regs[20].wrapping_add(self.regs[9].wrapping_shl(2)),
                    self.regs[10].wrapping_add(1),
                    limit,
                    BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPLETION_CYCLES,
                    true,
                )
            }
            BR2_POST_VS_STACK_PACKET_SCAN_TAIL_CURSOR_ADD => {
                if self.pending_load.is_some() {
                    return None;
                }
                (
                    self.regs[20].wrapping_add(self.regs[9].wrapping_shl(2)),
                    self.regs[10].wrapping_add(1),
                    self.regs[11],
                    BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPLETION_CYCLES,
                    true,
                )
            }
            BR2_POST_VS_STACK_PACKET_SCAN_TAIL_INDEX_ADD => {
                if self.pending_load.is_some() {
                    return None;
                }
                (
                    self.regs[20],
                    self.regs[10].wrapping_add(1),
                    self.regs[11],
                    BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPLETION_CYCLES,
                    true,
                )
            }
            BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPARE => {
                if self.pending_load.is_some() {
                    return None;
                }
                (
                    self.regs[20],
                    self.regs[10],
                    self.regs[11],
                    BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPLETION_CYCLES,
                    true,
                )
            }
            BR2_POST_VS_STACK_PACKET_SCAN_TAIL_BRANCH => {
                if self.pending_load.is_some() {
                    return None;
                }
                let branch_value = (self.regs[10] < self.regs[11]) as u32;
                if self.regs[2] != branch_value {
                    return None;
                }
                (
                    self.regs[20],
                    self.regs[10],
                    self.regs[11],
                    BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPLETION_CYCLES,
                    true,
                )
            }
            _ => return None,
        };

        let long_unthrottled_scan =
            limit.wrapping_sub(index) >= BR2_POST_VS_STACK_PACKET_SCAN_LONG_UNTHROTTLED_MIN_PACKETS;
        let vblank_timing_observable = bus.io.irq.mask & 1 != 0;
        if completed_tail && vblank_timing_observable {
            let cycles_until_vblank = bus.cycles_until_next_vblank();
            if cycles_until_vblank <= base_cycles {
                return None;
            }
        }

        if completed_tail {
            bus.write_u32(index_slot, index);
            if index >= limit {
                self.restore_br2_post_vs_stack_packet_scan_return_address(bus, sp);
                self.regs[2] = 0;
                self.regs[10] = index;
                self.regs[11] = limit;
                self.regs[20] = cursor;
                self.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT;
                self.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT + 4;
                self.cycles = self.cycles.saturating_add(base_cycles);
                self.regs[0] = 0;
                self.pending_load = None;
                self.load_commit_register = None;
                self.load_commit_value = None;
                self.load_commit_cancelled = false;

                return Some(self.step_report_from(
                    start_pc,
                    Some(instruction),
                    cycles_before,
                    StepOutcome::Continue,
                ));
            }
        } else if index >= limit {
            return None;
        }

        let max_packets = limit
            .wrapping_sub(index)
            .min(BR2_POST_VS_STACK_PACKET_SCAN_MAX_SKIP_PACKETS);
        let mut scan_packet_limit = max_packets;
        let has_trusted_return_site = br2_game_runtime_pc(self.regs[31])
            && br2_ram_word_range(
                sp.wrapping_add(BR2_POST_VS_STACK_PACKET_SCAN_RA_STACK_OFFSET),
                1,
                bus.ram_len(),
            );
        let trusted_long_noop_gap_run = if long_unthrottled_scan
            && (has_trusted_return_site || bus.native_playable_candidate())
        {
            br2_post_vs_stack_packet_scan_noop_gap_run(cursor, max_packets, bus.ram_len())
        } else {
            0
        };
        let trusted_long_noop_gap_scan = trusted_long_noop_gap_run > 0;
        if vblank_timing_observable && !trusted_long_noop_gap_scan {
            let cycles_until_vblank = bus.cycles_until_next_vblank();
            if cycles_until_vblank <= base_cycles {
                return None;
            }
            let irq_limited_packets = ((cycles_until_vblank - base_cycles - 1)
                / BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET)
                as u32;
            scan_packet_limit = scan_packet_limit.min(irq_limited_packets);
        }
        if max_packets == 0 {
            return None;
        }
        if scan_packet_limit == 0 && !completed_tail {
            return None;
        }

        let mut skipped_packets = 0u32;
        let mut skipped_noop_gap_packets = trusted_long_noop_gap_scan;
        let mut verified_ram_packets = 0u32;
        let mut last_header = self.regs[4];
        let mut last_length = bus.read_u32_fast_no_trace(length_slot);
        let mut last_type = self.regs[7];
        let mut last_tag = self.regs[3];
        let max_verified_ram_packets = if long_unthrottled_scan {
            BR2_POST_VS_STACK_PACKET_SCAN_LONG_MAX_VERIFIED_RAM_PACKETS
        } else {
            BR2_POST_VS_STACK_PACKET_SCAN_MAX_VERIFIED_RAM_PACKETS
        };
        if trusted_long_noop_gap_scan {
            cursor = cursor.wrapping_add(scan_packet_limit.wrapping_shl(2));
            index = index.wrapping_add(scan_packet_limit);
            skipped_packets = scan_packet_limit;
            last_header = 0;
            last_length = 0;
            last_type = 0;
            last_tag = 0;
        }
        while !trusted_long_noop_gap_scan && skipped_packets < scan_packet_limit {
            let remaining_packets = scan_packet_limit - skipped_packets;

            let noop_gap_run = br2_post_vs_stack_packet_scan_noop_gap_run(
                cursor,
                remaining_packets,
                bus.ram_len(),
            );
            if noop_gap_run > 0 {
                cursor = cursor.wrapping_add(noop_gap_run.wrapping_shl(2));
                index = index.wrapping_add(noop_gap_run);
                skipped_packets = skipped_packets.saturating_add(noop_gap_run);
                skipped_noop_gap_packets = true;
                last_header = 0;
                last_length = 0;
                last_type = 0;
                last_tag = 0;
                if index >= limit {
                    break;
                }
                continue;
            }

            if verified_ram_packets >= max_verified_ram_packets {
                break;
            }
            let verified_remaining = remaining_packets
                .min(max_verified_ram_packets.saturating_sub(verified_ram_packets));
            let zero_run =
                br2_post_vs_stack_packet_scan_zero_ram_packet_run(cursor, verified_remaining, bus);
            if zero_run > 0 {
                cursor = cursor.wrapping_add(zero_run.wrapping_shl(2));
                index = index.wrapping_add(zero_run);
                skipped_packets = skipped_packets.saturating_add(zero_run);
                verified_ram_packets = verified_ram_packets.saturating_add(zero_run);
                last_header = 0;
                last_length = 0;
                last_type = 0;
                last_tag = 0;
                if index >= limit {
                    break;
                }
                continue;
            }

            if let Some(run) =
                br2_post_vs_stack_packet_scan_uniform_noop_run(cursor, verified_remaining, bus)
            {
                cursor = run.next_cursor;
                index = index.wrapping_add(run.packets);
                skipped_packets = skipped_packets.saturating_add(run.packets);
                verified_ram_packets = verified_ram_packets.saturating_add(run.packets);
                last_header = run.packet.header;
                last_length = run.packet.length;
                last_type = run.packet.packet_type;
                last_tag = run.packet.tag;
                if index >= limit {
                    break;
                }
                continue;
            }

            let Some(packet) = br2_post_vs_stack_packet_scan_noop_packet(cursor, bus) else {
                break;
            };

            last_header = packet.header;
            last_length = packet.length;
            last_type = packet.packet_type;
            last_tag = packet.tag;
            cursor = packet.next_cursor;
            index = index.wrapping_add(1);
            skipped_packets = skipped_packets.saturating_add(1);
            verified_ram_packets = verified_ram_packets.saturating_add(1);
            if index >= limit {
                break;
            }
        }

        if skipped_packets == 0 {
            if completed_tail {
                self.regs[2] = 1;
                self.regs[10] = index;
                self.regs[11] = limit;
                self.regs[20] = cursor;
                self.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
                self.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
                self.cycles = self.cycles.saturating_add(base_cycles);
                self.regs[0] = 0;
                self.pending_load = None;
                self.load_commit_register = None;
                self.load_commit_value = None;
                self.load_commit_cancelled = false;

                return Some(self.step_report_from(
                    start_pc,
                    Some(instruction),
                    cycles_before,
                    StepOutcome::Continue,
                ));
            }
            return None;
        }

        bus.write_u32(length_slot, last_length);
        bus.write_u32(index_slot, index);
        self.regs[2] = (index < limit) as u32;
        self.regs[3] = last_tag;
        self.regs[4] = last_header;
        self.regs[7] = last_type;
        self.regs[8] = last_length;
        self.regs[9] = last_length;
        self.regs[10] = index;
        self.regs[11] = limit;
        self.regs[20] = cursor;
        self.pc = if index >= limit {
            self.restore_br2_post_vs_stack_packet_scan_return_address(bus, sp);
            BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT
        } else {
            BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START
        };
        self.next_pc = self.pc.wrapping_add(4);
        let charged_packets = if skipped_noop_gap_packets {
            skipped_packets.min(BR2_POST_VS_STACK_PACKET_SCAN_MAX_CHARGED_NOOP_PACKETS)
        } else {
            skipped_packets
        };
        self.cycles = self.cycles.saturating_add(base_cycles).saturating_add(
            u64::from(charged_packets)
                .saturating_mul(BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET),
        );
        self.regs[0] = 0;
        self.pending_load = None;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;

        Some(self.step_report_from(
            start_pc,
            Some(instruction),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn restore_br2_post_vs_stack_packet_scan_return_address(&self, bus: &mut Bus, sp: u32) {
        let ra = self.regs[31];
        if !br2_game_runtime_pc(ra) {
            return;
        }
        let ra_slot = sp.wrapping_add(BR2_POST_VS_STACK_PACKET_SCAN_RA_STACK_OFFSET);
        if br2_ram_word_range(ra_slot, 1, bus.ram_len()) && bus.read_u32_fast_no_trace(ra_slot) == 0
        {
            bus.write_u32(ra_slot, ra);
        }
    }

    fn try_fast_forward_br2_post_vs_record_copy_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if br2_native_hle_disabled("post_vs_record_copy") {
            return None;
        }

        let instruction = br2_post_vs_record_copy_loop_instruction(self.pc)?;

        let loop_start_entry = self.pc == BR2_POST_VS_RECORD_COPY_LOOP_START;
        if self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || !br2_post_vs_record_copy_pending_load_matches(self.pc, self.pending_load)
        {
            return None;
        }

        if bus.cache_isolated() || !br2_post_vs_record_copy_loop_signature_matches(bus) {
            return None;
        }

        let counter_slot = self.regs[29].wrapping_add(BR2_POST_VS_RECORD_COPY_COUNTER_STACK_OFFSET);
        if !br2_ram_word_range(counter_slot, 1, bus.ram_len()) {
            return None;
        }

        let counter = match self.pc {
            0x8031_5554 => match self.pending_load {
                Some((10, value)) => value,
                None => bus.read_u32(counter_slot),
                _ => return None,
            },
            _ => bus.read_u32(counter_slot),
        };
        let limit = self.regs[19];
        let remaining = limit.wrapping_sub(counter);
        if counter >= limit || remaining == 0 {
            return None;
        }

        let source = if matches!(self.pc, 0x8031_5558 | 0x8031_555c | 0x8031_5560) {
            self.regs[3].wrapping_sub(16)
        } else {
            self.regs[3]
        };
        let destination = if matches!(
            self.pc,
            0x8031_5550 | 0x8031_5554 | 0x8031_5558 | 0x8031_555c | 0x8031_5560
        ) {
            self.regs[17].wrapping_sub(16)
        } else {
            self.regs[17]
        };
        let huge_expansion_noop = remaining >= BR2_POST_VS_RECORD_COPY_HUGE_NOOP_MIN_ITERATIONS
            && br2_high_expansion_noop_address(source)
            && br2_high_expansion_noop_address(destination);

        let mut iterations = if huge_expansion_noop {
            remaining
        } else {
            remaining.min(BR2_POST_VS_RECORD_COPY_MAX_SKIP_ITERATIONS)
        };
        if !huge_expansion_noop && self.vblank_irq_can_preempt(bus) {
            let cycles_until_vblank = bus.cycles_until_next_vblank();
            if cycles_until_vblank <= BR2_POST_VS_RECORD_COPY_CYCLES_PER_ITERATION {
                return None;
            }
            let irq_limited_iterations =
                ((cycles_until_vblank - 1) / BR2_POST_VS_RECORD_COPY_CYCLES_PER_ITERATION) as u32;
            iterations = iterations.min(irq_limited_iterations);
        }
        if iterations == 0 {
            return None;
        }

        let byte_count = u64::from(iterations).saturating_mul(16);
        let source_noop = huge_expansion_noop || br2_noop_read_byte_range(source, byte_count, bus);
        let destination_noop =
            huge_expansion_noop || br2_noop_write_byte_range(destination, byte_count, bus);
        if !huge_expansion_noop {
            if !(source_noop || br2_readable_byte_range(source, byte_count, bus))
                || !(destination_noop || br2_writable_byte_range(destination, byte_count, bus))
            {
                return None;
            }
            if !loop_start_entry
                && !source_noop
                && !destination_noop
                && br2_physical_byte_ranges_overlap(source, byte_count, destination, byte_count)
            {
                return None;
            }
        }

        let mut last_words = [0u32; 4];
        if !huge_expansion_noop && (!source_noop || !destination_noop) {
            for index in 0..iterations {
                let source_address = source.wrapping_add(index.wrapping_mul(16));
                let destination_address = destination.wrapping_add(index.wrapping_mul(16));
                if !source_noop {
                    last_words[0] = bus.read_u32(source_address);
                    last_words[1] = bus.read_u32(source_address.wrapping_add(4));
                    last_words[2] = bus.read_u32(source_address.wrapping_add(8));
                    last_words[3] = bus.read_u32(source_address.wrapping_add(12));
                }
                if !destination_noop {
                    bus.write_u32(destination_address, last_words[0]);
                    bus.write_u32(destination_address.wrapping_add(4), last_words[1]);
                    bus.write_u32(destination_address.wrapping_add(8), last_words[2]);
                    bus.write_u32(destination_address.wrapping_add(12), last_words[3]);
                }
            }
        }

        let final_counter = counter.wrapping_add(iterations);
        bus.write_u32(counter_slot, final_counter);
        self.regs[2] = u32::from(final_counter < limit);
        self.regs[3] = source.wrapping_add(iterations.wrapping_mul(16));
        self.regs[8] = last_words[2];
        self.regs[9] = last_words[3];
        self.regs[10] = final_counter;
        self.regs[11] = last_words[1];
        self.regs[17] = destination.wrapping_add(iterations.wrapping_mul(16));
        self.pc = if final_counter < limit {
            BR2_POST_VS_RECORD_COPY_LOOP_START
        } else {
            BR2_POST_VS_RECORD_COPY_LOOP_EXIT
        };
        self.next_pc = self.pc.wrapping_add(4);
        let charged_iterations = if huge_expansion_noop {
            iterations.min(BR2_POST_VS_RECORD_COPY_MAX_CHARGED_NOOP_ITERATIONS)
        } else {
            iterations
        };
        self.cycles = self.cycles.saturating_add(
            u64::from(charged_iterations)
                .saturating_mul(BR2_POST_VS_RECORD_COPY_CYCLES_PER_ITERATION),
        );
        self.regs[0] = 0;
        self.pending_load = None;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;

        Some(self.step_report_from(
            start_pc,
            Some(instruction),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_br2_post_vs_vertex_record_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if br2_native_hle_disabled("post_vs_vertex_record") {
            return None;
        }

        if self.pc != BR2_POST_VS_VERTEX_RECORD_LOOP_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        if bus.cache_isolated() || !br2_post_vs_vertex_record_loop_signature_matches(bus) {
            return None;
        }

        let count = self.regs[9];
        if count == 0 {
            return None;
        }

        let mut iterations = count.min(BR2_POST_VS_VERTEX_RECORD_MAX_SKIP_ITERATIONS);
        if self.vblank_irq_can_preempt(bus) {
            let cycles_until_vblank = bus.cycles_until_next_vblank();
            if cycles_until_vblank <= BR2_POST_VS_VERTEX_RECORD_CYCLES_PER_ITERATION {
                return None;
            }
            let irq_limited_iterations =
                ((cycles_until_vblank - 1) / BR2_POST_VS_VERTEX_RECORD_CYCLES_PER_ITERATION) as u32;
            iterations = iterations.min(irq_limited_iterations);
        }
        if iterations == 0 {
            return None;
        }

        let mut a1 = self.regs[5];
        let a2 = self.regs[6];
        let a3 = self.regs[7];
        let mut t0 = self.regs[8];
        let mut t1 = self.regs[9];
        let mut t2 = self.regs[10];
        let mut t3 = self.regs[11];
        let t4 = self.regs[12] as u8;
        let t5 = self.regs[13] as u8;
        let t6 = self.regs[14];
        let t7 = self.regs[15];
        let mut last_v0 = self.regs[2];
        let mut last_v1 = self.regs[3];
        let mut last_a0 = self.regs[4];

        for _ in 0..iterations {
            bus.write_u8(t0.wrapping_sub(0x2b), t5);
            bus.write_u8(t0.wrapping_sub(0x27), t4);
            bus.write_u8(t0.wrapping_sub(0x0b), t5);
            bus.write_u8(t0.wrapping_sub(0x07), t4);

            let mut v0 = bus.read_u32(t3);
            bus.write_u32(t0.wrapping_sub(0x02), v0);
            bus.write_u32(t0.wrapping_sub(0x22), v0);

            v0 = bus.read_u32(a1.wrapping_sub(0x0e));
            bus.write_u32(t0.wrapping_add(0x06), v0);
            bus.write_u32(t0.wrapping_sub(0x1a), v0);

            v0 = bus.read_u32(a1.wrapping_sub(0x0a));
            bus.write_u32(t0.wrapping_add(0x0e), v0);
            bus.write_u32(t0.wrapping_sub(0x12), v0);

            v0 = br2_signed_halfword_table_offset(bus.read_u16(a1.wrapping_sub(0x06)))
                .wrapping_add(a3);
            v0 = bus.read_u32(v0);
            bus.write_u32(t0.wrapping_sub(0x32), v0);

            v0 = br2_signed_halfword_table_offset(bus.read_u16(a1.wrapping_sub(0x04)))
                .wrapping_add(a2);
            v0 = bus.read_u32(v0);
            t3 = t3.wrapping_add(0x14);
            t1 = t1.wrapping_sub(1);
            bus.write_u32(t2, v0);

            v0 = br2_signed_halfword_table_offset(bus.read_u16(a1.wrapping_sub(0x02)))
                .wrapping_add(a2);
            let mut v1 = u32::from(bus.read_u16(t0.wrapping_sub(0x20)));
            v0 = bus.read_u32(v0);
            t2 = t2.wrapping_add(0x50);
            bus.write_u32(t0.wrapping_sub(0x3a), v0);

            v0 = br2_signed_halfword_table_offset(bus.read_u16(a1)).wrapping_add(a2);
            v1 = v1.wrapping_add(t6);
            v0 = bus.read_u32(v0);
            last_a0 = v0;
            v0 = u32::from(bus.read_u16(t0.wrapping_sub(0x18)));
            a1 = a1.wrapping_add(0x14);
            bus.write_u16(t0.wrapping_sub(0x20), v1 as u16);

            v1 = u32::from(bus.read_u16(t0));
            v0 = v0.wrapping_add(t7);
            bus.write_u16(t0.wrapping_sub(0x18), v0 as u16);

            let next_v0 = u32::from(bus.read_u16(t0.wrapping_add(0x08)));
            v1 = v1.wrapping_add(t6);
            bus.write_u16(t0, v1 as u16);
            bus.write_u32(t0.wrapping_sub(0x36), last_a0);
            last_v0 = next_v0.wrapping_add(t7);
            bus.write_u16(t0.wrapping_add(0x08), last_v0 as u16);
            last_v1 = v1;

            t0 = t0.wrapping_add(0x50);
        }

        self.regs[2] = last_v0;
        self.regs[3] = last_v1;
        self.regs[4] = last_a0;
        self.regs[5] = a1;
        self.regs[8] = t0;
        self.regs[9] = t1;
        self.regs[10] = t2;
        self.regs[11] = t3;
        self.pc = if t1 == 0 {
            BR2_POST_VS_VERTEX_RECORD_LOOP_EXIT
        } else {
            BR2_POST_VS_VERTEX_RECORD_LOOP_START
        };
        self.next_pc = self.pc.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(
            u64::from(iterations).saturating_mul(BR2_POST_VS_VERTEX_RECORD_CYCLES_PER_ITERATION),
        );
        self.regs[0] = 0;
        self.pending_load = None;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;

        Some(self.step_report_from(
            start_pc,
            Some(BR2_POST_VS_VERTEX_RECORD_LOOP_INSTRUCTIONS[0].1),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_br2_post_vs_strided_pointer_copy_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if br2_native_hle_disabled("post_vs_strided_pointer_copy") {
            return None;
        }

        if self.pc != BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        if bus.cache_isolated() {
            return None;
        }

        let count = self.regs[3];
        if count == 0 {
            return None;
        }

        let destination = self.regs[4];
        let source = self.regs[6];
        let pointer_table = self.regs[7];
        let huge_expansion_noop = count
            >= BR2_POST_VS_STRIDED_POINTER_COPY_HUGE_NOOP_MIN_ITERATIONS
            && br2_expansion_noop_address(source)
            && br2_expansion_noop_address(destination)
            && br2_expansion_noop_address(pointer_table);

        if !huge_expansion_noop && !br2_post_vs_strided_pointer_copy_loop_signature_matches(bus) {
            return None;
        }

        let mut iterations = if huge_expansion_noop {
            count
        } else {
            count.min(BR2_POST_VS_STRIDED_POINTER_COPY_MAX_REAL_ITERATIONS)
        };

        if !huge_expansion_noop {
            if self.vblank_irq_can_preempt(bus) {
                let cycles_until_vblank = bus.cycles_until_next_vblank();
                if cycles_until_vblank <= BR2_POST_VS_STRIDED_POINTER_COPY_CYCLES_PER_ITERATION {
                    return None;
                }
                let irq_limited_iterations = ((cycles_until_vblank - 1)
                    / BR2_POST_VS_STRIDED_POINTER_COPY_CYCLES_PER_ITERATION)
                    as u32;
                iterations = iterations.min(irq_limited_iterations);
            }
            if iterations == 0
                || !br2_strided_pointer_copy_ranges_fast_forwardable(
                    source,
                    destination,
                    pointer_table,
                    iterations,
                    bus,
                )
            {
                return None;
            }
        }

        let source_noop =
            huge_expansion_noop || br2_noop_read_byte_range(source, u64::from(iterations) * 8, bus);
        let destination_noop = huge_expansion_noop
            || br2_noop_strided_write_range(destination, iterations, 16, 8, bus);
        let pointer_table_noop = huge_expansion_noop
            || br2_noop_write_byte_range(pointer_table, u64::from(iterations) * 8, bus);

        let mut last_first_word = 0;
        let mut last_second_word = 0;
        if !huge_expansion_noop {
            for index in 0..iterations {
                let source_address = source.wrapping_add(index.wrapping_mul(8));
                let destination_address = destination.wrapping_add(index.wrapping_mul(16));
                let pointer_address = pointer_table.wrapping_add(index.wrapping_mul(8));
                let next_destination_pointer = destination_address.wrapping_add(8);

                let mut bytes = [0u8; 8];
                if !source_noop {
                    for (offset, byte) in bytes.iter_mut().enumerate() {
                        *byte = bus.read_u8(source_address.wrapping_add(offset as u32));
                    }
                }

                last_first_word = le_u32_from_4_bytes(&bytes[0..4]);
                last_second_word = le_u32_from_4_bytes(&bytes[4..8]);

                if !source_noop && !destination_noop {
                    for (offset, byte) in bytes.iter().copied().enumerate() {
                        bus.write_u8(destination_address.wrapping_add(offset as u32), byte);
                    }
                }
                if !source_noop && !pointer_table_noop {
                    bus.write_u32(pointer_address, next_destination_pointer);
                }
            }
        }

        let remaining = count.wrapping_sub(iterations);
        let last_destination_pointer_offset =
            iterations.wrapping_sub(1).wrapping_mul(16).wrapping_add(8);
        self.regs[2] = destination.wrapping_add(last_destination_pointer_offset);
        self.regs[3] = remaining;
        self.regs[4] = destination.wrapping_add(iterations.wrapping_mul(16));
        self.regs[6] = source.wrapping_add(iterations.wrapping_mul(8));
        self.regs[7] = pointer_table.wrapping_add(iterations.wrapping_mul(8));
        self.regs[8] = last_first_word;
        self.regs[9] = last_second_word;
        self.pc = if remaining == 0 {
            BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_EXIT
        } else {
            BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START
        };
        self.next_pc = self.pc.wrapping_add(4);
        let charged_iterations = if huge_expansion_noop {
            iterations.min(BR2_POST_VS_STRIDED_POINTER_COPY_MAX_CHARGED_NOOP_ITERATIONS)
        } else {
            iterations
        };
        self.cycles = self.cycles.saturating_add(
            u64::from(charged_iterations)
                .saturating_mul(BR2_POST_VS_STRIDED_POINTER_COPY_CYCLES_PER_ITERATION),
        );
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_INSTRUCTIONS[0]),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_fast_forward_br2_post_vs_alt_strided_pointer_copy_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if br2_native_hle_disabled("post_vs_alt_strided_pointer_copy") {
            return None;
        }

        if self.pc != BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        if bus.cache_isolated() {
            return None;
        }

        let count = self.regs[6];
        if count == 0 {
            return None;
        }

        let source = self.regs[3];
        let destination = self.regs[4];
        let pointer_table = self.regs[5];
        let huge_expansion_noop = count
            >= BR2_POST_VS_STRIDED_POINTER_COPY_HUGE_NOOP_MIN_ITERATIONS
            && br2_expansion_noop_address(source)
            && br2_expansion_noop_address(destination)
            && br2_expansion_noop_address(pointer_table);

        if !huge_expansion_noop && !br2_post_vs_alt_strided_pointer_copy_loop_signature_matches(bus)
        {
            return None;
        }

        let mut iterations = if huge_expansion_noop {
            count
        } else {
            count.min(BR2_POST_VS_STRIDED_POINTER_COPY_MAX_REAL_ITERATIONS)
        };

        if !huge_expansion_noop {
            if self.vblank_irq_can_preempt(bus) {
                let cycles_until_vblank = bus.cycles_until_next_vblank();
                if cycles_until_vblank <= BR2_POST_VS_STRIDED_POINTER_COPY_CYCLES_PER_ITERATION {
                    return None;
                }
                let irq_limited_iterations = ((cycles_until_vblank - 1)
                    / BR2_POST_VS_STRIDED_POINTER_COPY_CYCLES_PER_ITERATION)
                    as u32;
                iterations = iterations.min(irq_limited_iterations);
            }
            if iterations == 0
                || !br2_strided_pointer_copy_ranges_fast_forwardable(
                    source,
                    destination,
                    pointer_table,
                    iterations,
                    bus,
                )
            {
                return None;
            }
        }

        let source_noop =
            huge_expansion_noop || br2_noop_read_byte_range(source, u64::from(iterations) * 8, bus);
        let destination_noop = huge_expansion_noop
            || br2_noop_strided_write_range(destination, iterations, 16, 8, bus);
        let pointer_table_noop = huge_expansion_noop
            || br2_noop_write_byte_range(pointer_table, u64::from(iterations) * 8, bus);

        let mut last_first_word = 0;
        let mut last_second_word = 0;
        if !huge_expansion_noop {
            for index in 0..iterations {
                let source_address = source.wrapping_add(index.wrapping_mul(8));
                let destination_address = destination.wrapping_add(index.wrapping_mul(16));
                let pointer_address = pointer_table.wrapping_add(index.wrapping_mul(8));
                let next_destination_pointer = destination_address.wrapping_add(8);

                let mut bytes = [0u8; 8];
                if !source_noop {
                    for (offset, byte) in bytes.iter_mut().enumerate() {
                        *byte = bus.read_u8(source_address.wrapping_add(offset as u32));
                    }
                }

                last_first_word = le_u32_from_4_bytes(&bytes[0..4]);
                last_second_word = le_u32_from_4_bytes(&bytes[4..8]);

                if !source_noop && !destination_noop {
                    for (offset, byte) in bytes.iter().copied().enumerate() {
                        bus.write_u8(destination_address.wrapping_add(offset as u32), byte);
                    }
                }
                if !source_noop && !pointer_table_noop {
                    bus.write_u32(pointer_address, next_destination_pointer);
                }
            }
        }

        let remaining = count.wrapping_sub(iterations);
        let last_destination_pointer_offset =
            iterations.wrapping_sub(1).wrapping_mul(16).wrapping_add(8);
        self.regs[2] = destination.wrapping_add(last_destination_pointer_offset);
        self.regs[3] = source.wrapping_add(iterations.wrapping_mul(8));
        self.regs[4] = destination.wrapping_add(iterations.wrapping_mul(16));
        self.regs[5] = pointer_table.wrapping_add(iterations.wrapping_mul(8));
        self.regs[6] = remaining;
        self.regs[8] = last_first_word;
        self.regs[9] = last_second_word;
        self.pc = if remaining == 0 {
            BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_EXIT
        } else {
            BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_START
        };
        self.next_pc = self.pc.wrapping_add(4);
        let charged_iterations = if huge_expansion_noop {
            iterations.min(BR2_POST_VS_STRIDED_POINTER_COPY_MAX_CHARGED_NOOP_ITERATIONS)
        } else {
            iterations
        };
        self.cycles = self.cycles.saturating_add(
            u64::from(charged_iterations)
                .saturating_mul(BR2_POST_VS_STRIDED_POINTER_COPY_CYCLES_PER_ITERATION),
        );
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_INSTRUCTIONS[0]),
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
        if br2_native_hle_disabled("post_vs_table_accum") {
            return None;
        }

        if !matches!(
            self.pc,
            BR2_POST_VS_TABLE_ACCUM_LOOP_START | BR2_POST_VS_TABLE_ACCUM_LOOP_TAIL_INCREMENT
        ) || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
        {
            return None;
        }

        if !br2_post_vs_table_accum_loop_signature_matches(bus) {
            return None;
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
        if count_address & 0x03 != 0 {
            return None;
        }
        let Some(remaining) = br2_signed_loop_remaining(start_index, limit) else {
            if self.pc == BR2_POST_VS_TABLE_ACCUM_LOOP_TAIL_INCREMENT {
                self.regs[2] = count_address;
                self.regs[3] = table_meta_offset;
                self.regs[5] = start_index;
                self.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT;
                self.next_pc = BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4;
                self.cycles = self.cycles.saturating_add(4);
                self.regs[0] = 0;
                self.pending_load = None;
                self.load_commit_register = None;
                self.load_commit_value = None;
                self.load_commit_cancelled = false;
                return Some(self.step_report_from(
                    start_pc,
                    Some(BR2_POST_VS_TABLE_ACCUM_LOOP_INSTRUCTIONS[0]),
                    cycles_before,
                    StepOutcome::Continue,
                ));
            }
            return None;
        };
        let table_base = bus.read_u32(count_address.wrapping_add(4));
        let first_target = table_base.wrapping_add(start_index.wrapping_shl(2));

        let target_is_unaligned_ram_noop = first_target & 0x03 != 0
            && br2_ram_unaligned_word_range(first_target, 1, bus.ram_len());
        let target_is_protected_noop = br2_post_vs_code_patch_noop_range(first_target, remaining);
        let target_is_live_render_ram_noop =
            br2_post_vs_live_render_ram_noop_range(first_target, remaining);
        let target_is_stack_guard_noop =
            br2_post_vs_stack_guard_noop_range(first_target, remaining);
        let target_is_expansion_noop = br2_expansion_noop_address(first_target);
        let target_is_unmapped_gap_noop =
            br2_post_vs_unmapped_peripheral_gap_noop_address(first_target);
        let target_is_noop = target_is_expansion_noop
            || target_is_protected_noop
            || target_is_live_render_ram_noop
            || target_is_stack_guard_noop
            || target_is_unaligned_ram_noop
            || target_is_unmapped_gap_noop;
        if remaining < BR2_POST_VS_TABLE_ACCUM_MIN_SKIP_ITERATIONS && !target_is_noop {
            return None;
        }
        let mut max_iterations = remaining.min(BR2_POST_VS_TABLE_ACCUM_MAX_SKIP_ITERATIONS);
        if !target_is_noop && self.vblank_irq_can_preempt(bus) {
            let cycles_until_vblank = bus.cycles_until_next_vblank();
            if cycles_until_vblank <= BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION {
                return None;
            }
            let irq_limited_iterations =
                ((cycles_until_vblank - 1) / BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION) as u32;
            max_iterations = max_iterations.min(irq_limited_iterations);
        }
        if max_iterations < BR2_POST_VS_TABLE_ACCUM_MIN_SKIP_ITERATIONS && !target_is_noop {
            return None;
        }

        let skipped_iterations;
        let charged_iterations;
        let ram_iterations = max_iterations;
        if target_is_noop {
            skipped_iterations = remaining;
            charged_iterations =
                self.br2_post_vs_table_accum_charged_noop_iterations(skipped_iterations, bus);
        } else if br2_ram_word_range(first_target, ram_iterations, bus.ram_len()) {
            skipped_iterations = ram_iterations;
            charged_iterations = skipped_iterations;
            for index in 0..skipped_iterations {
                let target = first_target.wrapping_add(index.wrapping_shl(2));
                if br2_post_vs_table_accum_store_noop_address(target) {
                    continue;
                }
                let value = bus.read_u32(target).wrapping_add(self.regs[4]);
                bus.write_u32(target, value);
            }
        } else {
            return None;
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
            u64::from(charged_iterations)
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

    fn br2_post_vs_table_accum_charged_iterations(&self, iterations: u32, bus: &Bus) -> u32 {
        if !self.vblank_irq_can_preempt(bus) {
            return iterations;
        }
        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let iterations_before_vblank = ((cycles_until_vblank.saturating_sub(1))
            / BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION)
            as u32;
        iterations.min(iterations_before_vblank)
    }

    fn br2_post_vs_table_accum_charged_noop_iterations(&self, iterations: u32, bus: &Bus) -> u32 {
        self.br2_post_vs_table_accum_charged_iterations(iterations, bus)
            .min(BR2_POST_VS_TABLE_ACCUM_MAX_CHARGED_NOOP_ITERATIONS)
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

    fn try_fast_forward_br2_bitstream_decode_loop(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if !matches!(
            self.pc,
            BR2_BITSTREAM_DECODE_LOOP_START | BR2_BITSTREAM_DECODE_DIRECT_START
        ) || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
        {
            return None;
        }

        for (address, expected) in BR2_BITSTREAM_DECODE_LOOP_INSTRUCTIONS {
            if bus.read_u32(address) != expected {
                return None;
            }
        }
        for (address, expected) in BR2_BITSTREAM_DECODE_LOOP_TAIL_INSTRUCTIONS {
            if bus.read_u32(address) != expected {
                return None;
            }
        }

        let mut at = self.regs[1];
        let mut v0 = self.regs[2];
        let mut v1 = self.regs[3];
        let mut a0 = self.regs[4];
        let mut a1 = self.regs[5];
        let a2 = self.regs[6];
        let mut t0 = self.regs[8];
        let mut t1 = self.regs[9];
        let mut t2 = self.regs[10];
        let mut t3 = self.regs[11];
        let t4 = self.regs[12];
        let t5 = self.regs[13];
        let t6 = self.regs[14];
        let mut direct_mode = self.pc == BR2_BITSTREAM_DECODE_DIRECT_START;
        let mut cycles = 0u64;
        let mut steps = 0u32;
        let mut completed_pc = None;

        let mut cycle_budget =
            u64::from(BR2_BITSTREAM_DECODE_MAX_STEPS) * BR2_BITSTREAM_DECODE_TABLE_CYCLES;
        if self.vblank_irq_can_preempt(bus) {
            let cycles_until_vblank = bus.cycles_until_next_vblank();
            if cycles_until_vblank <= BR2_BITSTREAM_DECODE_MIN_CYCLES_PER_STEP {
                return None;
            }
            cycle_budget = cycle_budget.min(cycles_until_vblank - 1);
        }

        while steps < BR2_BITSTREAM_DECODE_MAX_STEPS {
            if direct_mode {
                if cycles.saturating_add(BR2_BITSTREAM_DECODE_DIRECT_CYCLES) > cycle_budget {
                    break;
                }
                if t5 != 0 {
                    break;
                }

                t0 = v0 >> 22;
                at = t0 ^ BR2_BITSTREAM_DECODE_STREAM_SENTINEL;
                a1 = a1.wrapping_add(2);
                if at == 0 {
                    completed_pc = Some(BR2_BITSTREAM_DECODE_EXIT_STREAM_SENTINEL);
                    cycles = cycles.saturating_add(BR2_BITSTREAM_DECODE_DIRECT_CYCLES);
                    steps = steps.saturating_add(1);
                    break;
                }

                v0 = v0.wrapping_shl(10);
                v1 = v1.wrapping_add(10);
                let needs_refill = v1 & 0x10 != 0;
                v1 &= 0x0f;
                if needs_refill {
                    t1 = u32::from(bus.read_u16(a0));
                    a0 = a0.wrapping_add(2);
                    t1 = t1.wrapping_shl(v1 & 0x1f);
                    v0 |= t1;
                }

                t0 |= t4;
                bus.write_u16(a1, t0 as u16);
                at = a1.wrapping_sub(t6);
                a1 = a1.wrapping_add(2);
                cycles = cycles.saturating_add(BR2_BITSTREAM_DECODE_DIRECT_CYCLES);
                steps = steps.saturating_add(1);
                if (at as i32) >= 0 {
                    completed_pc = Some(BR2_BITSTREAM_DECODE_EXIT_DEST_LIMIT);
                    break;
                }
                direct_mode = false;
                continue;
            }

            if cycles.saturating_add(BR2_BITSTREAM_DECODE_TABLE_CYCLES) > cycle_budget {
                break;
            }

            t0 = (v0 >> 19).wrapping_shl(3).wrapping_add(a2);
            t1 = bus.read_u32(t0);
            if t1 == 0 {
                break;
            }

            at = t1 & 0xff;
            t3 = bus.read_u32(t0.wrapping_add(4));
            v0 = v0.wrapping_shl(at & 0x1f);
            v1 = v1.wrapping_add(at);
            let needs_refill = v1 & 0x10 != 0;
            v1 &= 0x0f;
            if needs_refill {
                t0 = u32::from(bus.read_u16(a0));
                a0 = a0.wrapping_add(2);
                t0 = t0.wrapping_shl(v1 & 0x1f);
                v0 |= t0;
            }

            t1 >>= 16;
            at = t1 ^ BR2_BITSTREAM_DECODE_TABLE_SENTINEL;
            if at == 0 {
                completed_pc = Some(BR2_BITSTREAM_DECODE_EXIT_TABLE_SENTINEL);
                cycles = cycles.saturating_add(BR2_BITSTREAM_DECODE_TABLE_CYCLES);
                steps = steps.saturating_add(1);
                break;
            }
            at = t1 ^ BR2_BITSTREAM_DECODE_LITERAL_PREFIX;
            bus.write_u16(a1, t1 as u16);
            if at == 0 {
                direct_mode = true;
                cycles = cycles.saturating_add(BR2_BITSTREAM_DECODE_TABLE_CYCLES);
                steps = steps.saturating_add(1);
                continue;
            }

            a1 = a1.wrapping_add(2);
            if t3 == 0 {
                cycles = cycles.saturating_add(BR2_BITSTREAM_DECODE_TABLE_CYCLES);
                steps = steps.saturating_add(1);
                continue;
            }

            t2 = t3 & 0xffff;
            at = t2 ^ BR2_BITSTREAM_DECODE_TABLE_SENTINEL;
            if at == 0 {
                completed_pc = Some(BR2_BITSTREAM_DECODE_EXIT_TABLE_SENTINEL);
                cycles = cycles.saturating_add(BR2_BITSTREAM_DECODE_TABLE_CYCLES);
                steps = steps.saturating_add(1);
                break;
            }
            at = t2 ^ BR2_BITSTREAM_DECODE_LITERAL_PREFIX;
            bus.write_u16(a1, t2 as u16);
            if at == 0 {
                direct_mode = true;
                cycles = cycles.saturating_add(BR2_BITSTREAM_DECODE_TABLE_CYCLES);
                steps = steps.saturating_add(1);
                continue;
            }

            t2 = t3 >> 16;
            a1 = a1.wrapping_add(2);
            if t2 == 0 {
                cycles = cycles.saturating_add(BR2_BITSTREAM_DECODE_TABLE_CYCLES);
                steps = steps.saturating_add(1);
                continue;
            }

            at = t2 ^ BR2_BITSTREAM_DECODE_TABLE_SENTINEL;
            if at == 0 {
                completed_pc = Some(BR2_BITSTREAM_DECODE_EXIT_TABLE_SENTINEL);
                cycles = cycles.saturating_add(BR2_BITSTREAM_DECODE_TABLE_CYCLES);
                steps = steps.saturating_add(1);
                break;
            }
            at = t2 ^ BR2_BITSTREAM_DECODE_LITERAL_PREFIX;
            bus.write_u16(a1, t2 as u16);
            if at == 0 {
                direct_mode = true;
                cycles = cycles.saturating_add(BR2_BITSTREAM_DECODE_TABLE_CYCLES);
                steps = steps.saturating_add(1);
                continue;
            }

            a1 = a1.wrapping_add(2);
            cycles = cycles.saturating_add(BR2_BITSTREAM_DECODE_TABLE_CYCLES);
            steps = steps.saturating_add(1);
        }

        if steps == 0 || cycles == 0 {
            return None;
        }

        self.regs[1] = at;
        self.regs[2] = v0;
        self.regs[3] = v1;
        self.regs[4] = a0;
        self.regs[5] = a1;
        self.regs[8] = t0;
        self.regs[9] = t1;
        self.regs[10] = t2;
        self.regs[11] = t3;
        self.pc = completed_pc.unwrap_or(if direct_mode {
            BR2_BITSTREAM_DECODE_DIRECT_START
        } else {
            BR2_BITSTREAM_DECODE_LOOP_START
        });
        self.next_pc = self.pc.wrapping_add(4);
        self.cycles = self.cycles.saturating_add(cycles);
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(if start_pc == BR2_BITSTREAM_DECODE_DIRECT_START {
                BR2_BITSTREAM_DECODE_LOOP_INSTRUCTIONS[0].1
            } else {
                BR2_BITSTREAM_DECODE_LOOP_INSTRUCTIONS[19].1
            }),
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
        if byte_count as i32 <= 0 || byte_count & 0x03 != 0 {
            return None;
        }

        let source = self.regs[4];
        let destination = self.regs[5];
        let total_words = byte_count / 4;
        let mut capped_words = total_words;
        let cycles_until_vblank = bus.cycles_until_next_vblank();
        if cycles_until_vblank <= WORD_COPY_LOOP_CYCLES_PER_WORD {
            return None;
        }
        let vblank_limited_words =
            ((cycles_until_vblank - 1) / WORD_COPY_LOOP_CYCLES_PER_WORD) as u32;
        capped_words = capped_words.min(vblank_limited_words);
        if capped_words == 0 {
            return None;
        }

        let capped_byte_count = capped_words.saturating_mul(4);
        let (words, last_word) =
            bus.try_copy_aligned_words(source, destination, capped_byte_count)?;
        let copied_bytes = words.saturating_mul(4);
        let remaining_bytes = byte_count.wrapping_sub(copied_bytes);
        self.regs[4] = source.wrapping_add(copied_bytes);
        self.regs[5] = destination.wrapping_add(copied_bytes);
        self.regs[6] = remaining_bytes;
        self.regs[7] = last_word;
        self.pc = if remaining_bytes == 0 {
            self.pc
                .wrapping_add((WORD_COPY_LOOP_INSTRUCTIONS.len() as u32) * 4)
        } else {
            BR2_BOOT_WORD_COPY_LOOP_START
        };
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
            0x00 => self.execute_special(instruction, current_pc, delay_slot_branch_pc, bus),
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
                if address & 0x01 != 0 {
                    if self.try_hle_br2_runtime_unaligned_halfword_load(
                        current_pc,
                        delay_slot_branch_pc,
                        instruction,
                        address,
                        true,
                        bus,
                    ) {
                        return StepOutcome::Continue;
                    }
                    return self.raise_address_exception(
                        current_pc,
                        delay_slot_branch_pc,
                        Exception::AddressLoad,
                        address,
                    );
                }
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
                if address & 0x03 != 0 {
                    if self.try_hle_br2_post_vs_unaligned_group_prefix_load(
                        current_pc,
                        delay_slot_branch_pc,
                        address,
                        bus,
                    ) {
                        return StepOutcome::Continue;
                    }
                    if self.try_hle_br2_post_vs_unaligned_inner_load(
                        current_pc,
                        delay_slot_branch_pc,
                        address,
                        bus,
                    ) {
                        return StepOutcome::Continue;
                    }
                    if self.try_hle_br2_runtime_unaligned_word_load(
                        current_pc,
                        delay_slot_branch_pc,
                        instruction,
                        address,
                        bus,
                    ) {
                        return StepOutcome::Continue;
                    }
                    return self.raise_address_exception(
                        current_pc,
                        delay_slot_branch_pc,
                        Exception::AddressLoad,
                        address,
                    );
                }
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
                if address & 0x01 != 0 {
                    if self.try_hle_br2_runtime_unaligned_halfword_load(
                        current_pc,
                        delay_slot_branch_pc,
                        instruction,
                        address,
                        false,
                        bus,
                    ) {
                        return StepOutcome::Continue;
                    }
                    return self.raise_address_exception(
                        current_pc,
                        delay_slot_branch_pc,
                        Exception::AddressLoad,
                        address,
                    );
                }
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
                if address & 0x01 != 0 {
                    return self.raise_address_exception(
                        current_pc,
                        delay_slot_branch_pc,
                        Exception::AddressStore,
                        address,
                    );
                }
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
                if address & 0x03 != 0 {
                    if self.try_hle_br2_runtime_unaligned_word_store(
                        current_pc,
                        delay_slot_branch_pc,
                        instruction,
                        address,
                        bus,
                    ) {
                        return StepOutcome::Continue;
                    }
                    return self.raise_address_exception(
                        current_pc,
                        delay_slot_branch_pc,
                        Exception::AddressStore,
                        address,
                    );
                }
                if self.try_ignore_br2_post_vs_protected_table_accum_store(
                    current_pc,
                    delay_slot_branch_pc,
                    address,
                ) {
                    return StepOutcome::Continue;
                }
                bus.write_u32(address, self.regs[rt(instruction)]);
                StepOutcome::Continue
            }
            0x2e => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                store_word_right(bus, address, self.regs[rt(instruction)]);
                StepOutcome::Continue
            }
            0x2f => {
                // PS1 software may emit MIPS cache-management opcodes on later code paths.
                // The R3000A-compatible native harness does not model CPU cache effects, so
                // treating CACHE as a no-op preserves execution without mutating memory.
                StepOutcome::Continue
            }
            0x31 | 0x35 | 0x39 | 0x3d => {
                // Bloody Roar 2 can reach stray COP1 memory opcodes while running through
                // permissive native compatibility paths. The PS1 has no usable COP1/FPU and
                // the runtime does not emulate a FPU exception handler, so keep these side
                // effect-free instead of terminating the native play loop.
                StepOutcome::Continue
            }
            0x32 => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                if address & 0x03 != 0 {
                    if self.try_hle_br2_runtime_unaligned_gte_load(
                        current_pc,
                        delay_slot_branch_pc,
                        instruction,
                        address,
                        bus,
                    ) {
                        return StepOutcome::Continue;
                    }
                    return self.raise_address_exception(
                        current_pc,
                        delay_slot_branch_pc,
                        Exception::AddressLoad,
                        address,
                    );
                }
                self.gte_data_write(rt(instruction), bus.read_u32(address));
                StepOutcome::Continue
            }
            0x3a => {
                let address = self.regs[rs(instruction)].wrapping_add(sign_extend_16(instruction));
                if address & 0x03 != 0 {
                    if self.try_hle_br2_runtime_unaligned_gte_store(
                        current_pc,
                        delay_slot_branch_pc,
                        instruction,
                        address,
                        bus,
                    ) {
                        return StepOutcome::Continue;
                    }
                    return self.raise_address_exception(
                        current_pc,
                        delay_slot_branch_pc,
                        Exception::AddressStore,
                        address,
                    );
                }
                bus.write_u32(address, self.gte_data_read(rt(instruction)));
                StepOutcome::Continue
            }
            _ => StepOutcome::Unsupported(instruction),
        }
    }

    fn try_hle_br2_credit_check(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if br2_native_hle_disabled("credit_check")
            || self.pc != BR2_CREDIT_CHECK_ENTRY
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
            || bus.cache_isolated()
            || !br2_credit_check_signature_matches(bus)
        {
            return None;
        }

        let player = self.regs[4].min(1);
        let freeplay = bus.read_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_FREEPLAY_FLAG_OFFSET) != 0;
        let required_offset = if player == 0 {
            BR2_CREDIT_REQUIRED_P1_OFFSET
        } else {
            BR2_CREDIT_REQUIRED_P2_OFFSET
        };
        let required = bus.read_u8(BR2_CREDIT_STATE_BASE + required_offset);
        let credit_slot = br2_credit_slot_address(player, bus);
        let credit_before = bus.read_u8(credit_slot);
        let mut pending_coin_edges = 0;

        if !freeplay {
            pending_coin_edges = bus.consume_br2_native_credit_hle_coin_edges();
            if pending_coin_edges > 0 {
                let coin_value = u64::from(required.max(1));
                let inserted_value = pending_coin_edges.saturating_mul(coin_value).min(0xff) as u8;
                let current = bus.read_u8(credit_slot);
                bus.write_u8(credit_slot, current.saturating_add(inserted_value));
            }
        }

        let result = if freeplay {
            0
        } else {
            let current = bus.read_u8(credit_slot);
            if current >= required {
                let remaining = current.saturating_sub(required);
                bus.write_u8(credit_slot, remaining);
                u32::from(remaining)
            } else {
                u32::MAX
            }
        };
        let credit_after = bus.read_u8(credit_slot);

        bus.record_br2_native_credit_hle_check(Br2NativeCreditHleCheck {
            player,
            freeplay,
            required,
            credit_slot,
            credit_before,
            credit_after,
            pending_coin_edges,
            result,
        });

        self.regs[2] = result;
        self.pc = self.regs[31];
        self.next_pc = self.regs[31].wrapping_add(4);
        self.cycles = self.cycles.saturating_add(BR2_CREDIT_CHECK_HLE_CYCLES);
        self.regs[0] = 0;

        Some(self.step_report_from(
            start_pc,
            Some(BR2_CREDIT_CHECK_ENTRY_SIGNATURE[0].1),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_hle_br2_post_vs_unaligned_group_prefix_load(
        &mut self,
        current_pc: u32,
        delay_slot_branch_pc: Option<u32>,
        address: u32,
        bus: &mut Bus,
    ) -> bool {
        if current_pc != BR2_POST_VS_TABLE_GROUP_LOOP_START + 0x0c
            || delay_slot_branch_pc.is_some()
            || address != self.regs[3].wrapping_add(4)
            || !br2_post_vs_unaligned_inner_load_noop_address(address, bus.ram_len())
        {
            return false;
        }

        if !br2_post_vs_table_group_loop_signature_matches(bus) {
            return false;
        }

        let owner = self.regs[4];
        let table_meta_offset = bus.read_u32(owner.wrapping_add(0x7c));
        let count_address = self.regs[6].wrapping_add(table_meta_offset);
        if self.regs[3] != count_address || count_address.wrapping_add(4) != address {
            return false;
        }

        let outer_limit = bus.read_u32(owner.wrapping_add(0x28));
        let outer_index = self.regs[7];
        if outer_index >= outer_limit {
            return false;
        }

        let skipped_iterations = outer_limit.wrapping_sub(outer_index);
        let charged_iterations =
            self.br2_post_vs_table_group_charged_noop_iterations(skipped_iterations, bus);

        self.regs[2] = 0;
        self.regs[3] = table_meta_offset;
        self.regs[5] = 0;
        self.regs[6] = self.regs[6].wrapping_add(skipped_iterations.wrapping_mul(8));
        self.regs[7] = outer_limit;
        self.pc = BR2_POST_VS_TABLE_GROUP_LOOP_EXIT;
        self.next_pc = BR2_POST_VS_TABLE_GROUP_LOOP_EXIT + 4;
        self.cycles = self.cycles.saturating_add(
            u64::from(charged_iterations)
                .saturating_mul(BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION)
                .saturating_sub(2),
        );
        self.pending_load = None;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;
        true
    }

    fn try_hle_br2_post_vs_packed_vertex_helper(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &Bus,
    ) -> Option<StepReport> {
        if br2_native_hle_disabled("post_vs_packed_vertex_helper") {
            return None;
        }

        if self.pc != BR2_POST_VS_PACKED_VERTEX_HELPER_START
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.pending_load.is_some()
            || self.regs[31] != BR2_POST_VS_PACKED_VERTEX_HELPER_RETURN
        {
            return None;
        }

        if !br2_post_vs_packed_vertex_helper_signature_matches(bus) {
            return None;
        }

        let stack_count = bus.read_u32_fast_no_trace(self.regs[29].wrapping_add(0x10));
        let stack_shade = bus.read_u32_fast_no_trace(self.regs[29].wrapping_add(0x14));
        if !br2_post_vs_packed_vertex_helper_arguments_match(
            self.regs[4],
            self.regs[5],
            self.regs[6],
            self.regs[7],
            stack_count,
            stack_shade,
        ) {
            return None;
        }

        self.pc = self.regs[31];
        self.next_pc = self.pc.wrapping_add(4);
        self.cycles = self
            .cycles
            .saturating_add(BR2_POST_VS_PACKED_VERTEX_HELPER_CYCLES);
        self.regs[0] = 0;
        self.pending_load = None;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;

        Some(self.step_report_from(
            start_pc,
            Some(BR2_POST_VS_PACKED_VERTEX_HELPER_INSTRUCTIONS[0].1),
            cycles_before,
            StepOutcome::Continue,
        ))
    }

    fn try_hle_br2_post_vs_unaligned_inner_load(
        &mut self,
        current_pc: u32,
        delay_slot_branch_pc: Option<u32>,
        address: u32,
        bus: &mut Bus,
    ) -> bool {
        if current_pc != BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x0c
            || delay_slot_branch_pc.is_some()
            || address != self.regs[3]
            || !br2_post_vs_unaligned_inner_load_noop_address(address, bus.ram_len())
        {
            return false;
        }

        if !br2_post_vs_table_accum_loop_signature_matches(bus) {
            return false;
        }

        let table_meta_offset = bus.read_u32(self.regs[4].wrapping_add(0x7c));
        let count_address = self.regs[6].wrapping_add(table_meta_offset);
        if count_address & 0x03 != 0 {
            return false;
        }
        let limit = bus.read_u32(count_address);
        let Some(remaining) = br2_signed_loop_remaining(self.regs[5], limit) else {
            return false;
        };
        let skipped_iterations = remaining;
        let charged_iterations =
            self.br2_post_vs_table_accum_charged_noop_iterations(skipped_iterations, bus);
        let final_index = self.regs[5].wrapping_add(skipped_iterations);
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
            u64::from(charged_iterations)
                .saturating_mul(BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION)
                .saturating_sub(2),
        );
        self.pending_load = None;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;
        true
    }

    fn try_hle_br2_runtime_unaligned_word_load(
        &mut self,
        current_pc: u32,
        delay_slot_branch_pc: Option<u32>,
        instruction: u32,
        address: u32,
        bus: &Bus,
    ) -> bool {
        if !(current_pc == BR2_RUNTIME_UNALIGNED_WORD_LOAD_PC
            || (BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_PC
                ..=BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_END_PC)
                .contains(&current_pc)
            || (BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_PC
                ..=BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_END_PC)
                .contains(&current_pc)
            || (BR2_RUNTIME_UNALIGNED_RENDER_SOURCE_WORD_LOAD_PC
                ..=BR2_RUNTIME_UNALIGNED_RENDER_SOURCE_WORD_LOAD_END_PC)
                .contains(&current_pc)
            || (BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC
                ..=BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_END_PC)
                .contains(&current_pc)
            || (BR2_RUNTIME_UNALIGNED_VERTEX_SECOND_WORD_LOAD_PC
                ..=BR2_RUNTIME_UNALIGNED_VERTEX_SECOND_WORD_LOAD_END_PC)
                .contains(&current_pc))
            || delay_slot_branch_pc.is_some()
            || address & 0x03 == 0
        {
            return false;
        }

        let value = if br2_readable_byte_range(address, 4, bus) {
            u32::from(bus.read_u8(address))
                | (u32::from(bus.read_u8(address.wrapping_add(1))) << 8)
                | (u32::from(bus.read_u8(address.wrapping_add(2))) << 16)
                | (u32::from(bus.read_u8(address.wrapping_add(3))) << 24)
        } else if br2_noop_read_byte_range(address, 4, bus) {
            0
        } else {
            return false;
        };
        self.schedule_load(rt(instruction), value);
        true
    }

    fn try_hle_br2_runtime_unaligned_halfword_load(
        &mut self,
        current_pc: u32,
        delay_slot_branch_pc: Option<u32>,
        instruction: u32,
        address: u32,
        signed: bool,
        bus: &Bus,
    ) -> bool {
        if !(BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC
            ..=BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_END_PC)
            .contains(&current_pc)
            || delay_slot_branch_pc.is_some()
            || address & 0x01 == 0
        {
            return false;
        }

        let value = if br2_readable_byte_range(address, 2, bus) {
            u16::from(bus.read_u8(address)) | (u16::from(bus.read_u8(address.wrapping_add(1))) << 8)
        } else if br2_noop_read_byte_range(address, 2, bus) {
            0
        } else {
            return false;
        };
        let loaded = if signed {
            (value as i16) as i32 as u32
        } else {
            u32::from(value)
        };
        self.schedule_load(rt(instruction), loaded);
        true
    }

    fn try_hle_br2_runtime_unaligned_gte_load(
        &mut self,
        current_pc: u32,
        delay_slot_branch_pc: Option<u32>,
        instruction: u32,
        address: u32,
        bus: &Bus,
    ) -> bool {
        if !(BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC..=BR2_RUNTIME_UNALIGNED_GTE_LOAD_END_PC)
            .contains(&current_pc)
            || delay_slot_branch_pc.is_some()
            || address & 0x03 == 0
        {
            return false;
        }

        let value = if br2_readable_byte_range(address, 4, bus) {
            u32::from(bus.read_u8(address))
                | (u32::from(bus.read_u8(address.wrapping_add(1))) << 8)
                | (u32::from(bus.read_u8(address.wrapping_add(2))) << 16)
                | (u32::from(bus.read_u8(address.wrapping_add(3))) << 24)
        } else if br2_noop_read_byte_range(address, 4, bus) {
            0
        } else {
            return false;
        };
        self.gte_data_write(rt(instruction), value);
        true
    }

    fn try_hle_br2_runtime_unaligned_gte_store(
        &mut self,
        current_pc: u32,
        delay_slot_branch_pc: Option<u32>,
        instruction: u32,
        address: u32,
        bus: &mut Bus,
    ) -> bool {
        if current_pc != BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC
            || delay_slot_branch_pc.is_some()
            || address & 0x03 == 0
        {
            return false;
        }

        if !br2_writable_byte_range(address, 4, bus) {
            return br2_noop_write_byte_range(address, 4, bus);
        }

        let value = self.gte_data_read(rt(instruction));
        bus.write_u8(address, value as u8);
        bus.write_u8(address.wrapping_add(1), (value >> 8) as u8);
        bus.write_u8(address.wrapping_add(2), (value >> 16) as u8);
        bus.write_u8(address.wrapping_add(3), (value >> 24) as u8);
        true
    }

    fn try_hle_br2_runtime_unaligned_word_store(
        &mut self,
        current_pc: u32,
        delay_slot_branch_pc: Option<u32>,
        instruction: u32,
        address: u32,
        bus: &mut Bus,
    ) -> bool {
        if !((BR2_RUNTIME_UNALIGNED_WORD_STORE_PC..=BR2_RUNTIME_UNALIGNED_WORD_STORE_END_PC)
            .contains(&current_pc)
            || (BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_STORE_PC
                ..=BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_STORE_END_PC)
                .contains(&current_pc)
            || (BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_PC
                ..=BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_END_PC)
                .contains(&current_pc))
            || delay_slot_branch_pc.is_some()
            || address & 0x03 == 0
        {
            return false;
        }

        if !br2_writable_byte_range(address, 4, bus) {
            return br2_noop_write_byte_range(address, 4, bus);
        }

        let value = self.regs[rt(instruction)];
        bus.write_u8(address, value as u8);
        bus.write_u8(address.wrapping_add(1), (value >> 8) as u8);
        bus.write_u8(address.wrapping_add(2), (value >> 16) as u8);
        bus.write_u8(address.wrapping_add(3), (value >> 24) as u8);
        true
    }

    fn try_hle_br2_runtime_null_callback_jalr(
        &mut self,
        current_pc: u32,
        delay_slot_branch_pc: Option<u32>,
        instruction: u32,
        _bus: &Bus,
    ) -> bool {
        let target_register = rs(instruction);
        let target = self.regs[target_register];
        if !br2_runtime_callback_jalr_site(current_pc, target_register)
            || delay_slot_branch_pc.is_some()
            || br2_runtime_callback_target_valid(current_pc, target)
        {
            return false;
        }

        if current_pc == BR2_RUNTIME_RENDER_CALLBACK_JALR_PC
            && self.regs[22] > BR2_RUNTIME_RENDER_CALLBACK_LOOP_MAX_REAL_ITERATIONS
        {
            self.regs[22] = 1;
        }

        self.set_reg(rd(instruction), self.next_pc);
        self.next_pc = current_pc.wrapping_add(8);
        self.delay_slot_branch_pc = Some(current_pc);
        true
    }

    fn try_ignore_br2_post_vs_protected_table_accum_store(
        &self,
        current_pc: u32,
        delay_slot_branch_pc: Option<u32>,
        address: u32,
    ) -> bool {
        current_pc == BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x18
            && delay_slot_branch_pc.is_none()
            && br2_post_vs_table_accum_store_noop_address(address)
    }

    fn try_hle_br2_bios_kernel_syscall(
        &mut self,
        current_pc: u32,
        delay_slot_branch_pc: Option<u32>,
    ) -> bool {
        if !br2_game_runtime_pc(current_pc) || delay_slot_branch_pc.is_some() {
            return false;
        }

        match self.regs[4] {
            BR2_BIOS_KERNEL_SYSCALL_ENTER_CRITICAL_SECTION => {
                self.regs[2] = u32::from(self.cp0[CP0_STATUS] & STATUS_IE != 0);
                self.cp0[CP0_STATUS] &= !STATUS_IE;
                true
            }
            BR2_BIOS_KERNEL_SYSCALL_EXIT_CRITICAL_SECTION => {
                self.regs[2] = 1;
                self.cp0[CP0_STATUS] |= STATUS_IE;
                true
            }
            _ => false,
        }
    }

    fn execute_special(
        &mut self,
        instruction: u32,
        current_pc: u32,
        delay_slot_branch_pc: Option<u32>,
        bus: &Bus,
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
                if self.try_hle_br2_runtime_null_callback_jalr(
                    current_pc,
                    delay_slot_branch_pc,
                    instruction,
                    bus,
                ) {
                    return StepOutcome::Continue;
                }
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
            0x0c => {
                if self.try_hle_br2_bios_kernel_syscall(current_pc, delay_slot_branch_pc) {
                    StepOutcome::Continue
                } else {
                    self.raise_exception(current_pc, delay_slot_branch_pc, Exception::Syscall)
                }
            }
            0x0d => {
                if br2_nonfatal_runtime_breakpoint(current_pc, delay_slot_branch_pc, instruction) {
                    return StepOutcome::Continue;
                }
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
        if br2_native_hle_disabled("blank_bios_irq_handler") {
            return false;
        }
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
        if br2_native_hle_disabled("bios_irq_return") {
            return false;
        }
        if self.delay_slot_branch_pc.is_some() || self.cp0[CP0_CAUSE] & CAUSE_IP2 == 0 {
            return false;
        }
        if self.br2_bios_b0_dispatch_call() {
            return false;
        }
        let low_bios_irq_return =
            br2_low_bios_irq_handler_pc(self.pc) && br2_game_runtime_pc(self.cp0[CP0_EPC]);
        let post_vs_c80_return = (BIOS_EXCEPTION_C80_IRQ_HANDLER_HLE_START
            ..=BIOS_EXCEPTION_C80_IRQ_HANDLER_HLE_END)
            .contains(&self.pc)
            && br2_post_vs_table_accum_loop_pc(self.cp0[CP0_EPC])
            && bios_exception_c80_handler_has_kernel_prefix(bus);
        let draw_sync_c80_return = (BIOS_EXCEPTION_C80_IRQ_HANDLER_HLE_START
            ..=BIOS_EXCEPTION_C80_IRQ_HANDLER_HLE_END)
            .contains(&self.pc)
            && self.cp0[CP0_EPC] == BR2_DRAW_SYNC_WAIT_LOOP_EXIT
            && bios_exception_c80_handler_has_kernel_prefix(bus);
        let blank_c80_return = (BIOS_EXCEPTION_C80_IRQ_HANDLER_HLE_START
            ..=BIOS_EXCEPTION_C80_IRQ_HANDLER_HLE_END)
            .contains(&self.pc)
            && br2_runtime_ram_pc(self.cp0[CP0_EPC])
            && bios_exception_vector_points_to_blank_c80_handler(bus);
        let irq_dispatch_return =
            (BIOS_IRQ_DISPATCH_LOOP_HLE_START..=BIOS_IRQ_DISPATCH_LOOP_HLE_END).contains(&self.pc)
                && br2_game_runtime_pc(self.cp0[CP0_EPC])
                && bios_irq_dispatch_loop_has_signature(bus);
        if !post_vs_c80_return
            && !draw_sync_c80_return
            && !blank_c80_return
            && !irq_dispatch_return
            && !low_bios_irq_return
        {
            return false;
        }
        if irq_dispatch_return && !self.restore_bios_exception_context(bus) {
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
        // Preserve delayed loads across the HLE IRQ round trip. BR2 can take a
        // vblank IRQ immediately after an epilogue `lw ra,...`; dropping the
        // pending load leaves the old nested-call RA live and unwinds to the
        // wrong frame.
        true
    }

    fn br2_bios_b0_dispatch_call(&self) -> bool {
        psx_physical_address(self.pc) == BR2_BIOS_B0_VECTOR_PHYSICAL
            && self.next_pc == self.pc.wrapping_add(4)
            && br2_game_runtime_pc(self.regs[31])
            && matches!(
                self.regs[9],
                BR2_BIOS_B0_WAIT_EVENT_FUNCTION
                    | BR2_BIOS_B0_TEST_EVENT_FUNCTION
                    | BR2_BIOS_B0_RESET_ENTRY_INT_FUNCTION
                    | BR2_BIOS_B0_RETURN_ONLY_FUNCTION
            )
    }

    fn try_hle_br2_bios_b0_dispatch(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if psx_physical_address(self.pc) != BR2_BIOS_B0_VECTOR_PHYSICAL
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || !br2_game_runtime_pc(self.regs[31])
        {
            return None;
        }

        match self.regs[9] {
            BR2_BIOS_B0_RETURN_ONLY_FUNCTION | BR2_BIOS_B0_RESET_ENTRY_INT_FUNCTION => {
                self.regs[2] = 0;
            }
            BR2_BIOS_B0_WAIT_EVENT_FUNCTION
                if self.regs[31] == BR2_BIOS_B0_WAIT_EVENT_RETURN_PC =>
            {
                self.try_deliver_br2_bios_b0_event(bus, self.regs[4] & 0xffff)?;
                self.regs[2] = 1;
            }
            BR2_BIOS_B0_TEST_EVENT_FUNCTION
                if self.regs[31] == BR2_BIOS_B0_TEST_EVENT_RETURN_PC
                    && (self.regs[4] & 0xffff) == BR2_BIOS_B0_TEST_EVENT_ID =>
            {
                self.try_deliver_br2_bios_b0_event(bus, self.regs[4] & 0xffff)?;
                self.regs[2] = 1;
            }
            _ => return None,
        }

        self.pc = self.regs[31];
        self.next_pc = self.pc.wrapping_add(4);
        self.pending_load = None;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;
        self.cycles = self.cycles.saturating_add(1);
        self.regs[0] = 0;

        Some(self.step_report_from(start_pc, None, cycles_before, StepOutcome::Continue))
    }

    fn try_deliver_br2_bios_b0_event(&mut self, bus: &mut Bus, event_id: u32) -> Option<()> {
        let event_table = bus.read_ram_u32_physical(BR2_BIOS_EVENT_TABLE_POINTER_PHYSICAL)?;
        let event_entry =
            event_table.wrapping_add(event_id.wrapping_mul(BR2_BIOS_EVENT_RECORD_BYTES));
        let event_status_address = event_entry.wrapping_add(4);
        if event_status_address & 0x03 != 0 {
            return None;
        }

        let event_status = bus.read_u32(event_status_address);
        match event_status {
            BR2_BIOS_B0_WAIT_EVENT_ENABLED => {
                bus.write_u32(event_status_address, BR2_BIOS_B0_WAIT_EVENT_DELIVERED);
                Some(())
            }
            BR2_BIOS_B0_WAIT_EVENT_DELIVERED => Some(()),
            _ => None,
        }
    }

    fn try_hle_br2_bios_b0_wait_event(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if !br2_bios_b0_wait_event_pc(self.pc)
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.regs[31] != BR2_BIOS_B0_WAIT_EVENT_RETURN_PC
            || !br2_bios_b0_wait_event_has_signature(bus)
        {
            return None;
        }

        let event_status_address = self.regs[2].wrapping_add(4);
        if event_status_address & 0x03 != 0 {
            return None;
        }
        let event_status = bus.read_u32(event_status_address);
        match event_status {
            BR2_BIOS_B0_WAIT_EVENT_ENABLED => {
                bus.write_u32(event_status_address, BR2_BIOS_B0_WAIT_EVENT_DELIVERED);
            }
            BR2_BIOS_B0_WAIT_EVENT_DELIVERED => {}
            _ => return None,
        }

        self.regs[2] = 1;
        self.pc = self.regs[31];
        self.next_pc = self.pc.wrapping_add(4);
        self.pending_load = None;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;
        self.cycles = self.cycles.saturating_add(1);
        self.regs[0] = 0;

        Some(self.step_report_from(start_pc, None, cycles_before, StepOutcome::Continue))
    }

    fn try_hle_br2_bios_b0_test_event(
        &mut self,
        start_pc: u32,
        cycles_before: u64,
        bus: &mut Bus,
    ) -> Option<StepReport> {
        if !br2_bios_b0_test_event_pc(self.pc)
            || self.next_pc != self.pc.wrapping_add(4)
            || self.delay_slot_branch_pc.is_some()
            || self.regs[31] != BR2_BIOS_B0_TEST_EVENT_RETURN_PC
            || (self.regs[4] & 0xffff) != BR2_BIOS_B0_TEST_EVENT_ID
            || !br2_bios_b0_test_event_has_signature(bus)
        {
            return None;
        }

        let event_table = bus.read_ram_u32_physical(BR2_BIOS_EVENT_TABLE_POINTER_PHYSICAL)?;
        let event_entry = event_table
            .wrapping_add((self.regs[4] & 0xffff).wrapping_mul(BR2_BIOS_EVENT_RECORD_BYTES));
        let event_status_address = event_entry.wrapping_add(4);
        if event_status_address & 0x03 != 0 {
            return None;
        }

        let event_status = bus.read_u32(event_status_address);
        match event_status {
            BR2_BIOS_B0_WAIT_EVENT_ENABLED => {
                bus.write_u32(event_status_address, BR2_BIOS_B0_WAIT_EVENT_DELIVERED);
            }
            BR2_BIOS_B0_WAIT_EVENT_DELIVERED => {}
            _ => return None,
        }

        self.regs[2] = 1;
        self.pc = self.regs[31];
        self.next_pc = self.pc.wrapping_add(4);
        self.pending_load = None;
        self.load_commit_register = None;
        self.load_commit_value = None;
        self.load_commit_cancelled = false;
        self.cycles = self.cycles.saturating_add(1);
        self.regs[0] = 0;

        Some(self.step_report_from(start_pc, None, cycles_before, StepOutcome::Continue))
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

    fn raise_address_exception(
        &mut self,
        current_pc: u32,
        delay_slot_branch_pc: Option<u32>,
        exception: Exception,
        bad_vaddr: u32,
    ) -> StepOutcome {
        self.cp0[CP0_BADVADDR] = bad_vaddr;
        self.raise_exception(current_pc, delay_slot_branch_pc, exception)
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
    AddressLoad = 4,
    AddressStore = 5,
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

fn br2_bios_b0_wait_event_has_signature(bus: &Bus) -> bool {
    BR2_BIOS_B0_WAIT_EVENT_SIGNATURE
        .iter()
        .copied()
        .all(|(address, expected)| bus.read_ram_u32_physical(address) == Some(expected))
}

fn br2_bios_b0_test_event_has_signature(bus: &Bus) -> bool {
    BR2_BIOS_B0_TEST_EVENT_SIGNATURE
        .iter()
        .copied()
        .all(|(address, expected)| bus.read_ram_u32_physical(address) == Some(expected))
}

fn br2_bios_b0_wait_event_pc(pc: u32) -> bool {
    let physical = psx_physical_address(pc);
    (BR2_BIOS_B0_WAIT_EVENT_HLE_START..=BR2_BIOS_B0_WAIT_EVENT_HLE_END).contains(&physical)
        && matches!(physical, 0x0000_1e6c | 0x0000_1e74)
}

fn br2_bios_b0_test_event_pc(pc: u32) -> bool {
    let physical = psx_physical_address(pc);
    (BR2_BIOS_B0_TEST_EVENT_HLE_START..=BR2_BIOS_B0_TEST_EVENT_HLE_END).contains(&physical)
        && physical == BR2_BIOS_B0_TEST_EVENT_HLE_START
}

fn br2_low_bios_irq_handler_pc(pc: u32) -> bool {
    let physical = psx_physical_address(pc);
    (BR2_LOW_BIOS_IRQ_VECTOR_HLE_START..=BR2_LOW_BIOS_IRQ_VECTOR_HLE_END).contains(&physical)
        || (BR2_LOW_BIOS_IRQ_HANDLER_HLE_START..=BR2_LOW_BIOS_IRQ_HANDLER_HLE_END)
            .contains(&physical)
}

fn br2_runtime_ram_pc(pc: u32) -> bool {
    (BR2_RUNTIME_RAM_PC_START..BR2_RUNTIME_RAM_PC_END).contains(&pc)
}

fn br2_game_runtime_pc(pc: u32) -> bool {
    (BR2_GAME_RUNTIME_PC_START..BR2_RUNTIME_RAM_PC_END).contains(&pc)
}

fn br2_runtime_callback_jalr_site(pc: u32, target_register: usize) -> bool {
    matches!(
        (pc, target_register),
        (BR2_RUNTIME_NULL_CALLBACK_JALR_PC, 2) | (BR2_RUNTIME_RENDER_CALLBACK_JALR_PC, 8)
    )
}

fn br2_runtime_callback_target_valid(site_pc: u32, target: u32) -> bool {
    if target == 0 || target & 0x03 != 0 || !br2_game_runtime_pc(target) {
        return false;
    }

    if site_pc == BR2_RUNTIME_RENDER_CALLBACK_JALR_PC {
        return target >= BR2_RUNTIME_RENDER_CALLBACK_MIN_TARGET_PC;
    }

    true
}

fn br2_nonfatal_runtime_breakpoint(
    pc: u32,
    delay_slot_branch_pc: Option<u32>,
    instruction: u32,
) -> bool {
    br2_game_runtime_pc(pc) && delay_slot_branch_pc.is_none() && instruction == 0x0007_000d
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
    (status & !0x3f) | ((status & 0x3c) >> 2)
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

fn br2_credit_check_signature_matches(bus: &Bus) -> bool {
    BR2_CREDIT_CHECK_ENTRY_SIGNATURE
        .iter()
        .chain(BR2_CREDIT_CHECK_CORE_SIGNATURE.iter())
        .copied()
        .all(|(address, expected)| bus.read_u32_executable_no_trace(address) == expected)
}

fn br2_credit_slot_address(player: u32, bus: &Bus) -> u32 {
    let player_mode = bus.read_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_PLAYER_MODE_OFFSET);
    if player_mode == 1 {
        BR2_CREDIT_STATE_BASE + BR2_CREDIT_SHARED_SLOT_OFFSET + player.min(1).saturating_mul(2)
    } else {
        BR2_CREDIT_STATE_BASE + BR2_CREDIT_SHARED_SLOT_OFFSET
    }
}

fn br2_status_halfword_wait_loop_signature_matches(bus: &Bus) -> bool {
    BR2_STATUS_HALFWORD_WAIT_LOOP_INSTRUCTIONS
        .iter()
        .copied()
        .all(|(address, expected)| bus.read_u32_executable_no_trace(address) == expected)
}

fn br2_status_pointer_scan_signature_matches(bus: &Bus) -> bool {
    BR2_STATUS_POINTER_SCAN_INSTRUCTIONS
        .iter()
        .copied()
        .all(|(address, expected)| bus.read_u32_executable_no_trace(address) == expected)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Br2PostVsStackPacketScanPacket {
    header: u32,
    length: u32,
    packet_type: u32,
    tag: u32,
    next_cursor: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Br2PostVsStackPacketScanRun {
    packet: Br2PostVsStackPacketScanPacket,
    packets: u32,
    next_cursor: u32,
}

fn br2_post_vs_table_accum_loop_pc(pc: u32) -> bool {
    let loop_end = BR2_POST_VS_TABLE_ACCUM_LOOP_START
        + (BR2_POST_VS_TABLE_ACCUM_LOOP_INSTRUCTIONS.len() as u32) * 4;
    (BR2_POST_VS_TABLE_ACCUM_LOOP_START..loop_end).contains(&pc)
}

fn br2_post_vs_table_accum_loop_signature_matches(bus: &Bus) -> bool {
    BR2_POST_VS_TABLE_ACCUM_LOOP_INSTRUCTIONS
        .iter()
        .copied()
        .enumerate()
        .all(|(index, expected)| {
            let address = BR2_POST_VS_TABLE_ACCUM_LOOP_START.wrapping_add((index as u32) * 4);
            bus.read_u32_fast_no_trace(address) == expected
        })
}

fn br2_post_vs_table_group_loop_signature_matches(bus: &Bus) -> bool {
    BR2_POST_VS_TABLE_GROUP_PREFIX_INSTRUCTIONS
        .iter()
        .copied()
        .enumerate()
        .all(|(index, expected)| {
            let address = BR2_POST_VS_TABLE_GROUP_LOOP_START.wrapping_add((index as u32) * 4);
            bus.read_u32_fast_no_trace(address) == expected
        })
        && br2_post_vs_table_accum_loop_signature_matches(bus)
        && BR2_POST_VS_TABLE_GROUP_TAIL_INSTRUCTIONS
            .iter()
            .copied()
            .all(|(address, expected)| bus.read_u32_fast_no_trace(address) == expected)
}

fn br2_post_vs_table_select_group_loop_signature_matches(bus: &Bus) -> bool {
    BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_INSTRUCTIONS
        .iter()
        .copied()
        .all(|(address, expected)| bus.read_u32_fast_no_trace(address) == expected)
}

fn br2_post_vs_packed_vertex_helper_signature_matches(bus: &Bus) -> bool {
    BR2_POST_VS_PACKED_VERTEX_CALLER_INSTRUCTIONS
        .iter()
        .copied()
        .chain(
            BR2_POST_VS_PACKED_VERTEX_HELPER_INSTRUCTIONS
                .iter()
                .copied(),
        )
        .all(|(address, expected)| bus.read_u32_executable_no_trace(address) == expected)
}

fn br2_post_vs_packed_vertex_helper_arguments_match(
    a0: u32,
    a1: u32,
    a2: u32,
    a3: u32,
    stack_count: u32,
    stack_shade: u32,
) -> bool {
    br2_post_vs_packed_vertex_word(a0)
        && br2_post_vs_packed_vertex_word(a1)
        && br2_post_vs_packed_vertex_word(a3)
        && br2_post_vs_live_render_ram_noop_range(a2, 1)
        && stack_count != 0
        && stack_count <= 0x1000
        && stack_shade <= 0xffff
}

fn br2_post_vs_packed_vertex_word(value: u32) -> bool {
    let x = value & 0xffff;
    let y = value >> 16;
    x <= 0x03ff && y <= 0x03ff
}

fn br2_post_vs_null_link_scan_loop_signature_matches(bus: &Bus) -> bool {
    BR2_POST_VS_NULL_LINK_SCAN_LOOP_INSTRUCTIONS
        .iter()
        .copied()
        .all(|(address, expected)| bus.read_u32_fast_no_trace(address) == expected)
}

fn br2_post_vs_stack_link_scan_loop_signature_matches(bus: &Bus) -> bool {
    BR2_POST_VS_STACK_LINK_SCAN_LOOP_INSTRUCTIONS
        .iter()
        .copied()
        .all(|(address, expected)| bus.read_u32_fast_no_trace(address) == expected)
}

fn br2_post_vs_stack_link_scan_current_instruction_matches(pc: u32, bus: &Bus) -> bool {
    BR2_POST_VS_STACK_LINK_SCAN_LOOP_INSTRUCTIONS
        .iter()
        .copied()
        .any(|(address, expected)| address == pc && bus.read_u32_fast_no_trace(address) == expected)
}

fn br2_post_vs_stack_packet_scan_loop_signature_matches(bus: &Bus) -> bool {
    BR2_POST_VS_STACK_PACKET_SCAN_LOOP_INSTRUCTIONS
        .iter()
        .copied()
        .all(|(address, expected)| bus.read_u32_fast_no_trace(address) == expected)
}

fn br2_post_vs_strided_pointer_copy_loop_signature_matches(bus: &Bus) -> bool {
    BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_INSTRUCTIONS
        .iter()
        .copied()
        .enumerate()
        .all(|(index, expected)| {
            bus.read_u32_executable_no_trace(
                BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START + (index as u32) * 4,
            ) == expected
        })
}

fn br2_post_vs_alt_strided_pointer_copy_loop_signature_matches(bus: &Bus) -> bool {
    BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_INSTRUCTIONS
        .iter()
        .copied()
        .enumerate()
        .all(|(index, expected)| {
            bus.read_u32_executable_no_trace(
                BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_START + (index as u32) * 4,
            ) == expected
        })
}

fn br2_post_vs_record_copy_loop_signature_matches(bus: &Bus) -> bool {
    BR2_POST_VS_RECORD_COPY_LOOP_INSTRUCTIONS
        .iter()
        .copied()
        .all(|(address, expected)| bus.read_u32_executable_no_trace(address) == expected)
}

fn br2_post_vs_record_copy_loop_instruction(pc: u32) -> Option<u32> {
    BR2_POST_VS_RECORD_COPY_LOOP_INSTRUCTIONS
        .iter()
        .copied()
        .find_map(|(address, instruction)| (address == pc).then_some(instruction))
        .filter(|_| pc != 0x8031_5564)
}

fn br2_post_vs_record_copy_pending_load_matches(
    pc: u32,
    pending_load: Option<(usize, u32)>,
) -> bool {
    match pc {
        0x8031_5530 => matches!(pending_load, None | Some((10, _))),
        0x8031_5534 => matches!(pending_load, None | Some((11, _))),
        0x8031_5538 => matches!(pending_load, None | Some((8, _))),
        0x8031_553c => matches!(pending_load, None | Some((9, _))),
        0x8031_5554 => matches!(pending_load, None | Some((10, _))),
        _ => pending_load.is_none(),
    }
}

fn br2_post_vs_vertex_record_loop_signature_matches(bus: &Bus) -> bool {
    BR2_POST_VS_VERTEX_RECORD_LOOP_INSTRUCTIONS
        .iter()
        .copied()
        .all(|(address, expected)| bus.read_u32_executable_no_trace(address) == expected)
}

fn br2_signed_halfword_table_offset(value: u16) -> u32 {
    ((value as i16) as i32 as u32) << 3
}

fn br2_post_vs_stack_packet_scan_current_instruction_matches(pc: u32, bus: &Bus) -> bool {
    BR2_POST_VS_STACK_PACKET_SCAN_LOOP_INSTRUCTIONS
        .iter()
        .copied()
        .any(|(address, expected)| address == pc && bus.read_u32_fast_no_trace(address) == expected)
}

fn br2_post_vs_stack_packet_scan_body_fast_forward_pc(pc: u32) -> bool {
    (BR2_POST_VS_STACK_PACKET_SCAN_BODY_FAST_FORWARD_START
        ..=BR2_POST_VS_STACK_PACKET_SCAN_BODY_FAST_FORWARD_END)
        .contains(&pc)
        && (pc - BR2_POST_VS_STACK_PACKET_SCAN_BODY_FAST_FORWARD_START).is_multiple_of(4)
}

fn br2_post_vs_stack_packet_scan_noop_packet(
    cursor: u32,
    bus: &Bus,
) -> Option<Br2PostVsStackPacketScanPacket> {
    if cursor & 0x03 != 0 {
        return None;
    }
    if !br2_post_vs_stack_packet_scan_readable_noop_address(cursor, bus.ram_len()) {
        return None;
    }

    let body = cursor.wrapping_add(4);
    let (header, body_word) = if br2_ram_byte_range(cursor, 8, bus.ram_len()) {
        let physical = cursor & 0x1fff_ffff;
        (
            bus.read_ram_u32_physical(physical)?,
            bus.read_ram_u32_physical(physical.wrapping_add(4))?,
        )
    } else {
        (
            bus.read_u32_fast_no_trace(cursor),
            bus.read_u32_fast_no_trace(body),
        )
    };
    let length = body_word & 0xffff;
    let packet_type = (body_word >> 16) & 0x7fff;
    let tag = header & 0x0f04_ffff;
    if tag >= BR2_POST_VS_STACK_PACKET_SCAN_NOOP_TAG_LIMIT {
        return None;
    }

    let next_cursor = body.wrapping_add(length.wrapping_shl(2));
    if next_cursor == cursor {
        return None;
    }

    Some(Br2PostVsStackPacketScanPacket {
        header,
        length,
        packet_type,
        tag,
        next_cursor,
    })
}

fn br2_post_vs_stack_packet_scan_uniform_noop_run(
    cursor: u32,
    max_packets: u32,
    bus: &Bus,
) -> Option<Br2PostVsStackPacketScanRun> {
    if max_packets < BR2_POST_VS_STACK_PACKET_SCAN_UNIFORM_SAMPLE_PACKETS {
        return None;
    }

    let first = br2_post_vs_stack_packet_scan_noop_packet(cursor, bus)?;
    let stride = first.next_cursor.wrapping_sub(cursor);
    if stride == 0 || stride & 0x03 != 0 {
        return None;
    }

    let mut sample_cursor = first.next_cursor;
    for _ in 1..BR2_POST_VS_STACK_PACKET_SCAN_UNIFORM_SAMPLE_PACKETS {
        let packet = br2_post_vs_stack_packet_scan_noop_packet(sample_cursor, bus)?;
        if packet.header != first.header
            || packet.length != first.length
            || packet.packet_type != first.packet_type
            || packet.tag != first.tag
            || packet.next_cursor.wrapping_sub(sample_cursor) != stride
        {
            return None;
        }
        sample_cursor = packet.next_cursor;
    }

    let total_stride = stride.checked_mul(max_packets)?;
    Some(Br2PostVsStackPacketScanRun {
        packet: first,
        packets: max_packets,
        next_cursor: cursor.wrapping_add(total_stride),
    })
}

fn br2_post_vs_stack_packet_scan_noop_gap_run(
    cursor: u32,
    max_packets: u32,
    ram_len: usize,
) -> u32 {
    if max_packets == 0 || cursor & 0x03 != 0 {
        return 0;
    }

    let physical = cursor & 0x1fff_ffff;
    let ram_len = (ram_len as u32).min(0x0080_0000);
    let range_end: u32 = if (ram_len..0x0080_0000).contains(&physical) {
        0x0080_0000
    } else if (0x0080_0000..0x1f80_0000).contains(&physical) {
        0x1f80_0000
    } else if (BR2_PSX_SCRATCHPAD_END..BR2_PSX_HW_IO_START).contains(&physical) {
        BR2_PSX_HW_IO_START
    } else if (0x1f80_2000..0x1fc0_0000).contains(&physical) {
        0x1fc0_0000
    } else if (0x1fc8_0000..0x2000_0000).contains(&physical) {
        0x2000_0000
    } else {
        return 0;
    };

    let available_words = range_end.saturating_sub(physical) / 4;
    available_words.saturating_sub(1).min(max_packets)
}

fn br2_post_vs_stack_packet_scan_zero_ram_packet_run(
    cursor: u32,
    max_packets: u32,
    bus: &Bus,
) -> u32 {
    if max_packets == 0 || cursor & 0x03 != 0 {
        return 0;
    }

    if !br2_ram_word_range(cursor, 1, bus.ram_len()) {
        return 0;
    }

    let physical = cursor & 0x1fff_ffff;
    let ram_len = bus.ram_len() as u32;
    let Some(word_count) = max_packets.checked_add(1) else {
        return 0;
    };
    let max_ram_words = ((ram_len - physical) / 4).min(word_count);
    let mut zero_words = 0u32;
    while zero_words < max_ram_words {
        let address = physical + zero_words * 4;
        if bus.read_ram_u32_physical(address) != Some(0) {
            break;
        }
        zero_words += 1;
    }
    zero_words.saturating_sub(1).min(max_packets)
}

fn br2_post_vs_table_group_noop_count_run(
    count_address: u32,
    max_groups: u32,
    ram_len: usize,
) -> u32 {
    if max_groups == 0 || count_address & 0x03 != 0 {
        return 0;
    }

    let physical = count_address & 0x1fff_ffff;
    let ram_len = (ram_len as u32).min(0x0080_0000);
    let range_end: u32 = if (ram_len..0x0080_0000).contains(&physical) {
        0x0080_0000
    } else if (0x0080_0000..0x1f80_0000).contains(&physical) {
        0x1f80_0000
    } else if (BR2_PSX_SCRATCHPAD_END..BR2_PSX_HW_IO_START).contains(&physical) {
        BR2_PSX_HW_IO_START
    } else if (0x1f80_2000..0x1fc0_0000).contains(&physical) {
        0x1fc0_0000
    } else if (0x1fc8_0000..0x2000_0000).contains(&physical) {
        0x2000_0000
    } else {
        return 0;
    };

    if range_end.saturating_sub(physical) < 4 {
        return 0;
    }

    let available_groups = (range_end - physical - 4) / 8 + 1;
    available_groups.min(max_groups)
}

fn br2_post_vs_table_group_nonpositive_count_run(
    count_address: u32,
    max_groups: u32,
    bus: &Bus,
) -> u32 {
    if max_groups < BR2_POST_VS_TABLE_GROUP_NONPOSITIVE_MIN_SKIP_ITERATIONS
        || count_address & 0x03 != 0
    {
        return 0;
    }

    let ram_len = bus.ram_len();
    if ram_len < 8 {
        return 0;
    }

    let physical = count_address & 0x1fff_ffff;
    if physical >= 0x0080_0000 {
        return 0;
    }

    let ram_physical = physical % (ram_len as u32);
    let Some(count) = bus.read_ram_u32_physical(ram_physical) else {
        return 0;
    };
    if (count as i32) > 0 {
        return 0;
    }

    max_groups
}

fn br2_post_vs_table_select_group_noop_record_run(
    record_offset: u32,
    max_groups: u32,
    ram_len: usize,
) -> u32 {
    if max_groups == 0 {
        return 0;
    }

    let physical = record_offset & 0x1fff_ffff;
    let ram_len = (ram_len as u32).min(0x0080_0000);
    let in_noop_gap = (ram_len..0x1f80_0000).contains(&physical)
        || (BR2_PSX_SCRATCHPAD_END..BR2_PSX_HW_IO_START).contains(&physical)
        || (0x1f80_2000..0x1fc0_0000).contains(&physical)
        || (0x1fc8_0000..0x2000_0000).contains(&physical);
    if !in_noop_gap {
        return 0;
    }

    max_groups
}

fn br2_post_vs_stack_packet_scan_readable_noop_address(address: u32, ram_len: usize) -> bool {
    let physical = address & 0x1fff_ffff;
    let body = address.wrapping_add(4);
    let body_physical = body & 0x1fff_ffff;
    br2_ram_byte_range(address, 8, ram_len)
        || (br2_expansion_noop_address(address) && br2_expansion_noop_address(body))
        || (((ram_len as u32)..0x0080_0000).contains(&physical)
            && ((ram_len as u32)..0x0080_0000).contains(&body_physical))
        || (br2_post_vs_unmapped_peripheral_gap_noop_address(address)
            && br2_post_vs_unmapped_peripheral_gap_noop_address(body))
        || (br2_post_vs_stack_packet_scan_io_metadata_address(address)
            && br2_post_vs_stack_packet_scan_io_metadata_address(body))
}

fn br2_expansion_noop_address(address: u32) -> bool {
    let start = address & 0x1fff_ffff;
    (0x0080_0000..0x1f80_0000).contains(&start)
}

fn br2_high_expansion_noop_address(address: u32) -> bool {
    let start = address & 0x1fff_ffff;
    (0x0800_0000..0x1f80_0000).contains(&start)
}

fn br2_ram_byte_range(address: u32, bytes: u32, ram_len: usize) -> bool {
    if bytes == 0 || ram_len == 0 {
        return false;
    }
    let start = u64::from(address & 0x1fff_ffff);
    let end = start.saturating_add(u64::from(bytes - 1));
    end < ram_len as u64
}

fn br2_ram_byte_range_u64(address: u32, bytes: u64, ram_len: usize) -> bool {
    if bytes == 0 || ram_len == 0 {
        return false;
    }
    br2_physical_byte_range_within(address, bytes, 0, (ram_len as u32).min(0x0080_0000))
}

fn br2_ram_word_range(address: u32, words: u32, ram_len: usize) -> bool {
    if address & 0x03 != 0 {
        return false;
    }
    br2_ram_unaligned_word_range(address, words, ram_len)
}

fn br2_ram_unaligned_word_range(address: u32, words: u32, ram_len: usize) -> bool {
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

fn br2_post_vs_code_patch_noop_range(address: u32, words: u32) -> bool {
    br2_physical_word_range_overlaps(
        address,
        words,
        BR2_POST_VS_PROTECTED_CODE_NOOP_RANGES.as_slice(),
    )
}

fn br2_post_vs_live_render_ram_noop_range(address: u32, words: u32) -> bool {
    br2_physical_word_range_overlaps(
        address,
        words,
        &[(
            BR2_POST_VS_LIVE_RENDER_RAM_NOOP_START,
            BR2_POST_VS_LIVE_RENDER_RAM_NOOP_END,
        )],
    )
}

fn br2_post_vs_stack_guard_noop_range(address: u32, words: u32) -> bool {
    br2_physical_word_range_overlaps(
        address,
        words,
        &[(
            BR2_POST_VS_STACK_GUARD_NOOP_START,
            BR2_POST_VS_STACK_GUARD_NOOP_END,
        )],
    )
}

fn br2_post_vs_table_accum_store_noop_address(address: u32) -> bool {
    br2_post_vs_code_patch_noop_range(address, 1)
        || br2_post_vs_live_render_ram_noop_range(address, 1)
        || br2_post_vs_stack_guard_noop_range(address, 1)
}

fn br2_physical_word_range_overlaps(address: u32, words: u32, ranges: &[(u32, u32)]) -> bool {
    if words == 0 {
        return false;
    }
    let last_byte_offset = u64::from(words - 1).saturating_mul(4).saturating_add(3);
    let start = u64::from(address & 0x1fff_ffff);
    let end = start + last_byte_offset;
    ranges.iter().any(|&(range_start, range_end)| {
        start < u64::from(range_end) && end >= u64::from(range_start)
    })
}

fn br2_post_vs_unaligned_inner_load_noop_address(address: u32, ram_len: usize) -> bool {
    if address & 0x03 == 0 {
        return false;
    }
    let physical = address & 0x1fff_ffff;
    br2_ram_unaligned_word_range(address, 1, ram_len)
        || br2_post_vs_code_patch_noop_range(address, 1)
        || br2_expansion_noop_address(address)
        || ((ram_len as u32)..0x0080_0000).contains(&physical)
        || br2_post_vs_unmapped_peripheral_gap_noop_address(address)
}

fn br2_post_vs_null_link_scan_terminal_pointer(address: u32, ram_len: usize) -> bool {
    address == 0
        || !br2_ram_word_range(address, 1, ram_len)
        || !br2_ram_word_range(address.wrapping_add(8), 1, ram_len)
}

fn br2_post_vs_stack_link_scan_terminal_pointer(address: u32, ram_len: usize) -> bool {
    address == 0
        || address == u32::MAX
        || !br2_ram_word_range(address, 1, ram_len)
        || !br2_ram_word_range(address.wrapping_add(8), 1, ram_len)
}

fn br2_post_vs_stack_link_scan_empty_node_with_terminal_next(address: u32, bus: &Bus) -> bool {
    if !br2_ram_word_range(address, 3, bus.ram_len()) {
        return false;
    }
    if bus.read_u32_fast_no_trace(address.wrapping_add(8)) != 0 {
        return false;
    }
    let next = bus.read_u32_fast_no_trace(address);
    next != 0 && br2_post_vs_stack_link_scan_terminal_pointer(next, bus.ram_len())
}

fn br2_post_vs_unmapped_peripheral_gap_noop_address(address: u32) -> bool {
    let physical = address & 0x1fff_ffff;
    (BR2_PSX_SCRATCHPAD_END..BR2_PSX_HW_IO_START).contains(&physical)
        || (0x1f80_2000..0x1fc0_0000).contains(&physical)
        || (0x1fc8_0000..0x2000_0000).contains(&physical)
}

fn br2_strided_pointer_copy_ranges_fast_forwardable(
    source: u32,
    destination: u32,
    pointer_table: u32,
    iterations: u32,
    bus: &Bus,
) -> bool {
    let source_bytes = u64::from(iterations) * 8;
    let pointer_table_bytes = u64::from(iterations) * 8;
    br2_readable_or_noop_byte_range(source, source_bytes, bus)
        && br2_writable_or_noop_strided_range(destination, iterations, 16, 8, bus)
        && br2_writable_or_noop_byte_range(pointer_table, pointer_table_bytes, bus)
}

fn br2_readable_or_noop_byte_range(address: u32, bytes: u64, bus: &Bus) -> bool {
    br2_readable_byte_range(address, bytes, bus) || br2_noop_read_byte_range(address, bytes, bus)
}

fn br2_writable_or_noop_byte_range(address: u32, bytes: u64, bus: &Bus) -> bool {
    br2_writable_byte_range(address, bytes, bus) || br2_noop_write_byte_range(address, bytes, bus)
}

fn br2_writable_or_noop_strided_range(
    address: u32,
    iterations: u32,
    stride: u32,
    width: u32,
    bus: &Bus,
) -> bool {
    br2_writable_strided_byte_range(address, iterations, stride, width, bus)
        || br2_noop_strided_write_range(address, iterations, stride, width, bus)
}

fn br2_readable_byte_range(address: u32, bytes: u64, bus: &Bus) -> bool {
    br2_ram_byte_range_u64(address, bytes, bus.ram_len())
        || br2_scratchpad_byte_range(address, bytes, bus.scratchpad_len())
        || br2_rom_byte_range(address, bytes, bus.rom_len())
}

fn br2_writable_byte_range(address: u32, bytes: u64, bus: &Bus) -> bool {
    br2_ram_byte_range_u64(address, bytes, bus.ram_len())
        || br2_scratchpad_byte_range(address, bytes, bus.scratchpad_len())
}

fn br2_writable_strided_byte_range(
    address: u32,
    iterations: u32,
    stride: u32,
    width: u32,
    bus: &Bus,
) -> bool {
    let Some(span) = br2_strided_span_bytes(iterations, stride, width) else {
        return false;
    };
    br2_writable_byte_range(address, span, bus)
}

fn br2_noop_read_byte_range(address: u32, bytes: u64, bus: &Bus) -> bool {
    br2_noop_physical_byte_range(address, bytes, bus.ram_len())
}

fn br2_noop_write_byte_range(address: u32, bytes: u64, bus: &Bus) -> bool {
    br2_noop_physical_byte_range(address, bytes, bus.ram_len())
        || br2_rom_byte_range(address, bytes, bus.rom_len())
}

fn br2_noop_strided_write_range(
    address: u32,
    iterations: u32,
    stride: u32,
    width: u32,
    bus: &Bus,
) -> bool {
    let Some(span) = br2_strided_span_bytes(iterations, stride, width) else {
        return false;
    };
    br2_noop_write_byte_range(address, span, bus)
}

fn br2_noop_physical_byte_range(address: u32, bytes: u64, ram_len: usize) -> bool {
    let ram_len = (ram_len as u32).min(0x0080_0000);
    br2_physical_byte_range_within(address, bytes, ram_len, 0x0080_0000)
        || br2_physical_byte_range_within(address, bytes, 0x0080_0000, 0x1f80_0000)
        || br2_physical_byte_range_within(
            address,
            bytes,
            BR2_PSX_SCRATCHPAD_END,
            BR2_PSX_HW_IO_START,
        )
        || br2_physical_byte_range_within(address, bytes, 0x1f80_2000, 0x1fc0_0000)
        || br2_physical_byte_range_within(address, bytes, 0x1fc8_0000, 0x2000_0000)
}

fn br2_scratchpad_byte_range(address: u32, bytes: u64, scratchpad_len: usize) -> bool {
    let Some(end) = (0x1f80_0000u64).checked_add(scratchpad_len as u64) else {
        return false;
    };
    br2_physical_byte_range_within(address, bytes, 0x1f80_0000, end as u32)
}

fn br2_rom_byte_range(address: u32, bytes: u64, rom_len: usize) -> bool {
    let Some(end) = (0x1fc0_0000u64).checked_add(rom_len as u64) else {
        return false;
    };
    br2_physical_byte_range_within(address, bytes, 0x1fc0_0000, end.min(0x2000_0000) as u32)
}

fn br2_physical_byte_range_within(
    address: u32,
    bytes: u64,
    range_start: u32,
    range_end: u32,
) -> bool {
    if bytes == 0 || range_start >= range_end {
        return false;
    }
    let start = u64::from(address & 0x1fff_ffff);
    let Some(end) = start.checked_add(bytes - 1) else {
        return false;
    };
    u64::from(range_start) <= start && end < u64::from(range_end)
}

fn br2_physical_byte_ranges_overlap(
    first_address: u32,
    first_bytes: u64,
    second_address: u32,
    second_bytes: u64,
) -> bool {
    if first_bytes == 0 || second_bytes == 0 {
        return false;
    }
    let first_start = u64::from(first_address & 0x1fff_ffff);
    let second_start = u64::from(second_address & 0x1fff_ffff);
    let Some(first_end) = first_start.checked_add(first_bytes - 1) else {
        return true;
    };
    let Some(second_end) = second_start.checked_add(second_bytes - 1) else {
        return true;
    };
    first_start <= second_end && second_start <= first_end
}

fn br2_strided_span_bytes(iterations: u32, stride: u32, width: u32) -> Option<u64> {
    if iterations == 0 || width == 0 || width > stride {
        return None;
    }
    let last_offset = u64::from(iterations.checked_sub(1)?).checked_mul(u64::from(stride))?;
    last_offset.checked_add(u64::from(width))
}

fn le_u32_from_4_bytes(bytes: &[u8]) -> u32 {
    debug_assert!(bytes.len() >= 4);
    u32::from(bytes[0])
        | (u32::from(bytes[1]) << 8)
        | (u32::from(bytes[2]) << 16)
        | (u32::from(bytes[3]) << 24)
}

fn br2_post_vs_stack_packet_scan_io_metadata_address(address: u32) -> bool {
    let physical = address & 0x1fff_ffff;
    (BR2_PSX_HW_IO_START..BR2_PSX_HW_IO_END).contains(&physical)
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
        BR2_BANKED_HALFWORD_COPY_CYCLES_PER_HALFWORD, BR2_BANKED_HALFWORD_COPY_LOOP_EXIT,
        BR2_BANKED_HALFWORD_COPY_LOOP_INSTRUCTIONS, BR2_BANKED_HALFWORD_COPY_LOOP_START,
        BR2_BANKED_HALFWORD_COPY_MASK, BR2_BIOS_B0_RESET_ENTRY_INT_FUNCTION,
        BR2_BIOS_B0_TEST_EVENT_RETURN_PC, BR2_BIOS_B0_TEST_EVENT_SIGNATURE,
        BR2_BIOS_B0_WAIT_EVENT_DELIVERED, BR2_BIOS_B0_WAIT_EVENT_ENABLED,
        BR2_BIOS_B0_WAIT_EVENT_RETURN_PC, BR2_BIOS_B0_WAIT_EVENT_SIGNATURE,
        BR2_BIOS_EVENT_TABLE_POINTER_PHYSICAL, BR2_BIOS_KERNEL_SYSCALL_ENTER_CRITICAL_SECTION,
        BR2_BIOS_KERNEL_SYSCALL_EXIT_CRITICAL_SECTION, BR2_BITSTREAM_DECODE_EXIT_TABLE_SENTINEL,
        BR2_BITSTREAM_DECODE_LOOP_INSTRUCTIONS, BR2_BITSTREAM_DECODE_LOOP_START,
        BR2_BITSTREAM_DECODE_LOOP_TAIL_INSTRUCTIONS, BR2_BITSTREAM_DECODE_TABLE_CYCLES,
        BR2_BITSTREAM_DECODE_TABLE_SENTINEL, BR2_BOOT_WORD_COPY_LOOP_START,
        BR2_BOOT_ZERO_FILL_LOOP_START, BR2_BYTE_COPY_LOOP_EXIT, BR2_BYTE_COPY_LOOP_INSTRUCTIONS,
        BR2_BYTE_COPY_LOOP_START, BR2_CREDIT_CHECK_CORE_SIGNATURE, BR2_CREDIT_CHECK_ENTRY,
        BR2_CREDIT_CHECK_ENTRY_SIGNATURE, BR2_CREDIT_CHECK_HLE_CYCLES,
        BR2_CREDIT_REQUIRED_P1_OFFSET, BR2_CREDIT_SHARED_SLOT_OFFSET, BR2_CREDIT_STATE_BASE,
        BR2_DRAW_SYNC_FLAG_VIRTUAL, BR2_DRAW_SYNC_WAIT_LOOP_EXIT,
        BR2_DRAW_SYNC_WAIT_LOOP_INSTRUCTIONS, BR2_DRAW_SYNC_WAIT_LOOP_START,
        BR2_FRAME_COUNTER_WAIT_LOOP_GLOBAL_COUNTER, BR2_FRAME_COUNTER_WAIT_LOOP_INSTRUCTIONS,
        BR2_FRAME_COUNTER_WAIT_LOOP_STACK_OFFSET, BR2_FRAME_COUNTER_WAIT_LOOP_START,
        BR2_FRAME_COUNTER_WAIT_LOOP_TARGET_CHECK,
        BR2_FRAME_COUNTER_WAIT_LOOP_TARGET_CHECK_INSTRUCTIONS, BR2_IRQ_POLL_STATUS_ADDRESS,
        BR2_IRQ_POLL_STATUS_MASK, BR2_IRQ_POLL_TIMEOUT_INITIAL_DECREMENT,
        BR2_IRQ_POLL_TIMEOUT_INITIAL_INSTRUCTION, BR2_IRQ_POLL_TIMEOUT_LOOP_EXIT,
        BR2_IRQ_POLL_TIMEOUT_LOOP_INSTRUCTIONS, BR2_IRQ_POLL_TIMEOUT_LOOP_START,
        BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_EXIT,
        BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_INSTRUCTIONS,
        BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_START, BR2_POST_VS_LIVE_RENDER_RAM_NOOP_START,
        BR2_POST_VS_NULL_LINK_SCAN_CYCLES, BR2_POST_VS_NULL_LINK_SCAN_LOOP_EXIT,
        BR2_POST_VS_NULL_LINK_SCAN_LOOP_INSTRUCTIONS, BR2_POST_VS_NULL_LINK_SCAN_LOOP_START,
        BR2_POST_VS_NULL_LINK_SCAN_TAIL_BRANCH, BR2_POST_VS_NULL_LINK_SCAN_TAIL_DELAY,
        BR2_POST_VS_NULL_LINK_SCAN_TAIL_LOAD, BR2_POST_VS_PACKED_VERTEX_CALLER_INSTRUCTIONS,
        BR2_POST_VS_PACKED_VERTEX_HELPER_CYCLES, BR2_POST_VS_PACKED_VERTEX_HELPER_INSTRUCTIONS,
        BR2_POST_VS_PACKED_VERTEX_HELPER_RETURN, BR2_POST_VS_PACKED_VERTEX_HELPER_START,
        BR2_POST_VS_RECORD_COPY_COUNTER_STACK_OFFSET, BR2_POST_VS_RECORD_COPY_CYCLES_PER_ITERATION,
        BR2_POST_VS_RECORD_COPY_HUGE_NOOP_MIN_ITERATIONS, BR2_POST_VS_RECORD_COPY_LOOP_EXIT,
        BR2_POST_VS_RECORD_COPY_LOOP_INSTRUCTIONS, BR2_POST_VS_RECORD_COPY_LOOP_START,
        BR2_POST_VS_RECORD_COPY_MAX_CHARGED_NOOP_ITERATIONS, BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT,
        BR2_POST_VS_STACK_LINK_SCAN_LOOP_INSTRUCTIONS, BR2_POST_VS_STACK_LINK_SCAN_LOOP_START,
        BR2_POST_VS_STACK_LINK_SCAN_NEXT_LOAD, BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_RELOAD_CYCLES,
        BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_RELOAD_DELAY_CYCLES,
        BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_START_CYCLES,
        BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_TAIL_BRANCH_CYCLES,
        BR2_POST_VS_STACK_LINK_SCAN_RELOAD, BR2_POST_VS_STACK_LINK_SCAN_RELOAD_DELAY,
        BR2_POST_VS_STACK_LINK_SCAN_STACK_SLOT_OFFSET, BR2_POST_VS_STACK_LINK_SCAN_TAIL_BRANCH,
        BR2_POST_VS_STACK_PACKET_SCAN_BODY_FAST_FORWARD_START,
        BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET,
        BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET,
        BR2_POST_VS_STACK_PACKET_SCAN_LENGTH_STACK_OFFSET,
        BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET,
        BR2_POST_VS_STACK_PACKET_SCAN_LONG_MAX_VERIFIED_RAM_PACKETS,
        BR2_POST_VS_STACK_PACKET_SCAN_LONG_UNTHROTTLED_MIN_PACKETS,
        BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_INSTRUCTIONS,
        BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START,
        BR2_POST_VS_STACK_PACKET_SCAN_MAX_CHARGED_NOOP_PACKETS,
        BR2_POST_VS_STACK_PACKET_SCAN_MAX_SKIP_PACKETS,
        BR2_POST_VS_STACK_PACKET_SCAN_MAX_VERIFIED_RAM_PACKETS,
        BR2_POST_VS_STACK_PACKET_SCAN_RA_STACK_OFFSET,
        BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPLETION_CYCLES,
        BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LENGTH_LOAD,
        BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LIMIT_LOAD,
        BR2_POST_VS_STRIDED_POINTER_COPY_CYCLES_PER_ITERATION,
        BR2_POST_VS_STRIDED_POINTER_COPY_HUGE_NOOP_MIN_ITERATIONS,
        BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_EXIT,
        BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_INSTRUCTIONS,
        BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START,
        BR2_POST_VS_STRIDED_POINTER_COPY_MAX_CHARGED_NOOP_ITERATIONS,
        BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT,
        BR2_POST_VS_TABLE_ACCUM_LOOP_INSTRUCTIONS, BR2_POST_VS_TABLE_ACCUM_LOOP_START,
        BR2_POST_VS_TABLE_ACCUM_LOOP_TAIL_INCREMENT,
        BR2_POST_VS_TABLE_ACCUM_MAX_CHARGED_NOOP_ITERATIONS,
        BR2_POST_VS_TABLE_ACCUM_MAX_SKIP_ITERATIONS, BR2_POST_VS_TABLE_ACCUM_MIN_SKIP_ITERATIONS,
        BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION, BR2_POST_VS_TABLE_GROUP_LOOP_EXIT,
        BR2_POST_VS_TABLE_GROUP_LOOP_START, BR2_POST_VS_TABLE_GROUP_MAX_CHARGED_NOOP_ITERATIONS,
        BR2_POST_VS_TABLE_GROUP_NONPOSITIVE_MIN_SKIP_ITERATIONS,
        BR2_POST_VS_TABLE_GROUP_PREFIX_INSTRUCTIONS, BR2_POST_VS_TABLE_GROUP_TAIL_INSTRUCTIONS,
        BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_BRANCH,
        BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_DELAY,
        BR2_POST_VS_TABLE_SELECT_GROUP_CYCLES_PER_ITERATION,
        BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_EXIT, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_INSTRUCTIONS,
        BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START,
        BR2_POST_VS_TABLE_SELECT_GROUP_MAX_CHARGED_NOOP_ITERATIONS,
        BR2_POST_VS_TABLE_SELECT_GROUP_TAIL_INCREMENT, BR2_POST_VS_TABLE_SELECT_GROUP_TAIL_LOAD,
        BR2_POST_VS_VERTEX_RECORD_CYCLES_PER_ITERATION, BR2_POST_VS_VERTEX_RECORD_LOOP_EXIT,
        BR2_POST_VS_VERTEX_RECORD_LOOP_INSTRUCTIONS, BR2_POST_VS_VERTEX_RECORD_LOOP_START,
        BR2_PSX_HW_IO_START, BR2_REVERSE_MISMATCH_SCAN_CYCLES_PER_ITERATION,
        BR2_REVERSE_MISMATCH_SCAN_LOOP_INSTRUCTIONS, BR2_REVERSE_MISMATCH_SCAN_LOOP_START,
        BR2_REVERSE_POINTER_SCAN_CYCLES_PER_ITERATION, BR2_REVERSE_POINTER_SCAN_LOOP_EXIT,
        BR2_REVERSE_POINTER_SCAN_LOOP_INSTRUCTIONS, BR2_REVERSE_POINTER_SCAN_LOOP_START,
        BR2_REVERSE_POINTER_SCAN_MAX_SKIP_ITERATIONS, BR2_RUNTIME_NULL_CALLBACK_JALR_PC,
        BR2_RUNTIME_RENDER_CALLBACK_JALR_PC, BR2_RUNTIME_RENDER_CALLBACK_LOOP_MAX_REAL_ITERATIONS,
        BR2_RUNTIME_RENDER_CALLBACK_MIN_TARGET_PC, BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC,
        BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_END_PC,
        BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_PC,
        BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_STORE_PC,
        BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_END_PC,
        BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_PC,
        BR2_RUNTIME_UNALIGNED_RENDER_SOURCE_WORD_LOAD_PC,
        BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC,
        BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_END_PC,
        BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC,
        BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_END_PC,
        BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_PC,
        BR2_RUNTIME_UNALIGNED_VERTEX_SECOND_WORD_LOAD_PC,
        BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC, BR2_RUNTIME_UNALIGNED_WORD_LOAD_PC,
        BR2_RUNTIME_UNALIGNED_WORD_STORE_PC, BR2_SMALL_BYTE_COPY_CYCLES_PER_BYTE,
        BR2_SMALL_BYTE_COPY_LOOP_EXIT, BR2_SMALL_BYTE_COPY_LOOP_INSTRUCTIONS,
        BR2_SMALL_BYTE_COPY_LOOP_START, BR2_STATUS_HALFWORD_WAIT_LOOP_FC_STATUS,
        BR2_STATUS_HALFWORD_WAIT_LOOP_FE_STATUS, BR2_STATUS_HALFWORD_WAIT_LOOP_FF_STATUS,
        BR2_STATUS_HALFWORD_WAIT_LOOP_INSTRUCTIONS, BR2_STATUS_HALFWORD_WAIT_LOOP_START,
        BR2_STATUS_HALFWORD_WAIT_LOOP_TAIL_LOAD, BR2_STATUS_POINTER_SCAN_CYCLES,
        BR2_STATUS_POINTER_SCAN_EXIT, BR2_STATUS_POINTER_SCAN_INSTRUCTIONS,
        BR2_STATUS_POINTER_SCAN_START, CAUSE_BD, CAUSE_EXCODE_MASK, CAUSE_IP2, CP0_BADVADDR,
        CP0_CAUSE, CP0_EPC, CP0_STATUS, Cpu, EXCEPTION_VECTOR, GTE_FLAG_DIVIDE_OVERFLOW,
        GTE_FLAG_ERROR, GTE_FLAG_SX2_SATURATED, GTE_FLAG_SY2_SATURATED, GTE_FRACTIONAL_BITS,
        STATUS_IE, StepOutcome, WORD_COPY_LOOP_CYCLES_PER_WORD,
        br2_post_vs_stack_packet_scan_noop_gap_run, gte_leading_zero_count, gte_sxy, rfe_status,
    };
    use crate::action::ActionButtons;
    use crate::native::bus::Bus;
    use crate::native::io::{DMA_INTERRUPT, DMA_SPU_CHCR};

    fn install_br2_credit_check(bus: &mut Bus) {
        for (address, instruction) in BR2_CREDIT_CHECK_ENTRY_SIGNATURE
            .iter()
            .chain(BR2_CREDIT_CHECK_CORE_SIGNATURE.iter())
            .copied()
        {
            bus.write_u32(address, instruction);
        }
    }

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

    fn install_br2_status_halfword_wait_loop(bus: &mut Bus) {
        for (address, instruction) in BR2_STATUS_HALFWORD_WAIT_LOOP_INSTRUCTIONS {
            bus.write_u32(address, instruction);
        }
    }

    fn install_br2_status_pointer_scan(bus: &mut Bus) {
        for (address, instruction) in BR2_STATUS_POINTER_SCAN_INSTRUCTIONS {
            bus.write_u32(address, instruction);
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

    fn install_br2_post_vs_record_copy_loop(bus: &mut Bus) {
        for (address, instruction) in BR2_POST_VS_RECORD_COPY_LOOP_INSTRUCTIONS {
            bus.write_u32(address, instruction);
        }
    }

    fn install_br2_post_vs_table_group_loop(bus: &mut Bus) {
        for (index, instruction) in BR2_POST_VS_TABLE_GROUP_PREFIX_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            bus.write_u32(
                BR2_POST_VS_TABLE_GROUP_LOOP_START + (index as u32) * 4,
                instruction,
            );
        }
        install_br2_post_vs_table_accum_loop(bus);
        for (address, instruction) in BR2_POST_VS_TABLE_GROUP_TAIL_INSTRUCTIONS {
            bus.write_u32(address, instruction);
        }
    }

    fn install_br2_post_vs_table_select_group_loop(bus: &mut Bus) {
        for (address, instruction) in BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_INSTRUCTIONS {
            bus.write_u32(address, instruction);
        }
    }

    fn install_br2_post_vs_packed_vertex_helper(bus: &mut Bus) {
        for (address, instruction) in BR2_POST_VS_PACKED_VERTEX_CALLER_INSTRUCTIONS {
            bus.write_u32(address, instruction);
        }
        for (address, instruction) in BR2_POST_VS_PACKED_VERTEX_HELPER_INSTRUCTIONS {
            bus.write_u32(address, instruction);
        }
    }

    fn install_br2_post_vs_null_link_scan_loop(bus: &mut Bus) {
        for (address, instruction) in BR2_POST_VS_NULL_LINK_SCAN_LOOP_INSTRUCTIONS {
            bus.write_u32(address, instruction);
        }
    }

    fn install_br2_post_vs_stack_link_scan_loop(bus: &mut Bus) {
        for (address, instruction) in BR2_POST_VS_STACK_LINK_SCAN_LOOP_INSTRUCTIONS {
            bus.write_u32(address, instruction);
        }
    }

    fn install_br2_post_vs_stack_packet_scan_loop(bus: &mut Bus) {
        for (address, instruction) in BR2_POST_VS_STACK_PACKET_SCAN_LOOP_INSTRUCTIONS {
            bus.write_u32(address, instruction);
        }
    }

    fn install_br2_post_vs_strided_pointer_copy_loop(bus: &mut Bus) {
        for (index, instruction) in BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            bus.write_u32(
                BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START + (index as u32) * 4,
                instruction,
            );
        }
    }

    fn install_br2_post_vs_alt_strided_pointer_copy_loop(bus: &mut Bus) {
        for (index, instruction) in BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_INSTRUCTIONS
            .iter()
            .copied()
            .enumerate()
        {
            bus.write_u32(
                BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_START + (index as u32) * 4,
                instruction,
            );
        }
    }

    #[test]
    fn hle_br2_credit_check_consumes_pending_coin_edge() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_credit_check(&mut bus);
        bus.write_u8(BR2_CREDIT_STATE_BASE + 1, 1);
        bus.write_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_REQUIRED_P1_OFFSET, 1);
        bus.write_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_SHARED_SLOT_OFFSET, 0);
        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });
        bus.set_input(ActionButtons::default());

        let return_pc = 0x802f_7770;
        let mut cpu = Cpu::default();
        cpu.pc = BR2_CREDIT_CHECK_ENTRY;
        cpu.next_pc = BR2_CREDIT_CHECK_ENTRY + 4;
        cpu.regs[4] = 0;
        cpu.regs[31] = return_pc;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_CREDIT_CHECK_ENTRY);
        assert_eq!(report.cycles_elapsed, BR2_CREDIT_CHECK_HLE_CYCLES);
        assert_eq!(cpu.pc, return_pc);
        assert_eq!(cpu.next_pc, return_pc + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(
            bus.read_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_SHARED_SLOT_OFFSET),
            0
        );
    }

    #[test]
    fn hle_br2_credit_check_preserves_no_credit_rejection() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_credit_check(&mut bus);
        bus.write_u8(BR2_CREDIT_STATE_BASE + 1, 1);
        bus.write_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_REQUIRED_P1_OFFSET, 1);

        let return_pc = 0x802f_7770;
        let mut cpu = Cpu::default();
        cpu.pc = BR2_CREDIT_CHECK_ENTRY;
        cpu.next_pc = BR2_CREDIT_CHECK_ENTRY + 4;
        cpu.regs[4] = 0;
        cpu.regs[31] = return_pc;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_CREDIT_CHECK_ENTRY);
        assert_eq!(cpu.pc, return_pc);
        assert_eq!(cpu.regs[2], u32::MAX);
    }

    fn install_br2_post_vs_vertex_record_loop(bus: &mut Bus) {
        for (address, instruction) in BR2_POST_VS_VERTEX_RECORD_LOOP_INSTRUCTIONS {
            bus.write_u32(address, instruction);
        }
    }

    fn install_br2_bitstream_decode_loop(bus: &mut Bus) {
        for (address, instruction) in BR2_BITSTREAM_DECODE_LOOP_INSTRUCTIONS {
            bus.write_u32(address, instruction);
        }
        for (address, instruction) in BR2_BITSTREAM_DECODE_LOOP_TAIL_INSTRUCTIONS {
            bus.write_u32(address, instruction);
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

    fn install_br2_bios_b0_wait_event_signature(bus: &mut Bus) {
        for (address, instruction) in BR2_BIOS_B0_WAIT_EVENT_SIGNATURE {
            bus.write_u32(address, instruction);
        }
    }

    fn install_br2_bios_b0_test_event_signature(bus: &mut Bus) {
        for (address, instruction) in BR2_BIOS_B0_TEST_EVENT_SIGNATURE {
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
    fn br2_runtime_debug_breakpoint_is_nonfatal() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let break_pc = 0x8035_6b50;
        bus.write_u32(break_pc, r_type(0, 7, 0, 0, 0x0d));

        let mut cpu = Cpu::default();
        cpu.pc = break_pc;
        cpu.next_pc = break_pc + 4;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, break_pc);
        assert_eq!(report.instruction, Some(0x0007_000d));
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert!(!cpu.halted);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_EXCODE_MASK, 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.pc, break_pc + 4);
        assert_eq!(cpu.next_pc, break_pc + 8);
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
    fn draw_sync_wait_loop_fast_forward_preserves_vblank_irq() {
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
        bus.io.irq.mask = 1;
        bus.tick(565_900);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_DRAW_SYNC_WAIT_LOOP_START;
        cpu.next_pc = BR2_DRAW_SYNC_WAIT_LOOP_START + 4;
        cpu.cp0[CP0_STATUS] = 1 | CAUSE_IP2;
        cpu.regs[3] = BR2_DRAW_SYNC_FLAG_VIRTUAL - 0x2210;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_DRAW_SYNC_WAIT_LOOP_START);
        assert_eq!(report.cycles_elapsed, 100);
        assert_eq!(cpu.pc, BR2_DRAW_SYNC_WAIT_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_DRAW_SYNC_WAIT_LOOP_EXIT + 4);
        assert_eq!(bus.vblank_count(), 1);
        assert_eq!(bus.read_u32(BR2_DRAW_SYNC_FLAG_VIRTUAL), 0);
        assert_eq!(bus.io.irq.status & 1, 1);
    }

    #[test]
    fn fast_forwards_br2_status_pointer_scan_to_status_wait() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_status_pointer_scan(&mut bus);
        let pointer_slot = 0x8038_cab4;
        let pointer = 0x8035_eeb4;
        bus.write_u32(pointer_slot, pointer);
        bus.write_u16(pointer + 4, 0x3700);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_STATUS_POINTER_SCAN_START;
        cpu.next_pc = BR2_STATUS_POINTER_SCAN_START + 4;
        cpu.regs[6] = pointer_slot - 0x14;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_STATUS_POINTER_SCAN_START);
        assert_eq!(report.cycles_elapsed, BR2_STATUS_POINTER_SCAN_CYCLES);
        assert_eq!(cpu.pc, BR2_STATUS_HALFWORD_WAIT_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_STATUS_HALFWORD_WAIT_LOOP_START + 4);
        assert_eq!(cpu.regs[2], 0x3700);
        assert_eq!(cpu.regs[3], pointer);
        assert_eq!(bus.read_u32(pointer_slot), pointer + 4);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_status_pointer_scan_to_clear_exit() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_status_pointer_scan(&mut bus);
        let pointer_slot = 0x8038_cab4;
        let pointer = 0x8035_eeb4;
        bus.write_u32(pointer_slot, pointer);
        bus.write_u16(pointer + 4, 0x00e7);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_STATUS_POINTER_SCAN_START;
        cpu.next_pc = BR2_STATUS_POINTER_SCAN_START + 4;
        cpu.regs[6] = pointer_slot - 0x14;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_STATUS_POINTER_SCAN_START);
        assert_eq!(report.cycles_elapsed, BR2_STATUS_POINTER_SCAN_CYCLES);
        assert_eq!(cpu.pc, BR2_STATUS_POINTER_SCAN_EXIT);
        assert_eq!(cpu.next_pc, BR2_STATUS_POINTER_SCAN_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[3], pointer);
        assert_eq!(bus.read_u32(pointer_slot), pointer + 4);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn status_pointer_scan_does_not_fast_forward_special_status_handlers() {
        for status in [
            BR2_STATUS_HALFWORD_WAIT_LOOP_FC_STATUS,
            BR2_STATUS_HALFWORD_WAIT_LOOP_FE_STATUS,
            BR2_STATUS_HALFWORD_WAIT_LOOP_FF_STATUS,
        ] {
            let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
            install_br2_status_pointer_scan(&mut bus);
            let pointer_slot = 0x8038_cab4;
            let pointer = 0x8035_eeb4;
            bus.write_u32(pointer_slot, pointer);
            bus.write_u16(pointer + 4, status | 0x00e7);

            let mut cpu = Cpu::default();
            cpu.pc = BR2_STATUS_POINTER_SCAN_START;
            cpu.next_pc = BR2_STATUS_POINTER_SCAN_START + 4;
            cpu.regs[6] = pointer_slot - 0x14;

            let report = cpu.step_report(&mut bus);

            assert_eq!(report.start_pc, BR2_STATUS_POINTER_SCAN_START);
            assert_eq!(report.instruction, Some(0x8cc3_0014));
            assert!(report.cycles_elapsed < BR2_STATUS_POINTER_SCAN_CYCLES);
            assert_eq!(cpu.pc, BR2_STATUS_POINTER_SCAN_START + 4);
            assert_eq!(cpu.next_pc, BR2_STATUS_POINTER_SCAN_START + 8);
            assert_eq!(bus.read_u32(pointer_slot), pointer);
            assert_eq!(cpu.pending_load, Some((3, pointer)));
        }
    }

    #[test]
    fn status_pointer_scan_does_not_fast_forward_when_signature_mismatch() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_status_pointer_scan(&mut bus);
        bus.write_u32(BR2_STATUS_POINTER_SCAN_START + 0x18, 0x0000_0001);
        let pointer_slot = 0x8038_cab4;
        let pointer = 0x8035_eeb4;
        bus.write_u32(pointer_slot, pointer);
        bus.write_u16(pointer + 4, 0x3700);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_STATUS_POINTER_SCAN_START;
        cpu.next_pc = BR2_STATUS_POINTER_SCAN_START + 4;
        cpu.regs[6] = pointer_slot - 0x14;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_STATUS_POINTER_SCAN_START);
        assert_eq!(report.instruction, Some(0x8cc3_0014));
        assert!(report.cycles_elapsed < BR2_STATUS_POINTER_SCAN_CYCLES);
        assert_eq!(cpu.pc, BR2_STATUS_POINTER_SCAN_START + 4);
        assert_eq!(cpu.next_pc, BR2_STATUS_POINTER_SCAN_START + 8);
        assert_eq!(bus.read_u32(pointer_slot), pointer);
        assert_eq!(cpu.pending_load, Some((3, pointer)));
    }

    #[test]
    fn status_pointer_scan_does_not_fast_forward_invalid_pointer() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_status_pointer_scan(&mut bus);
        let pointer_slot = 0x8038_cab4;
        let pointer = 0x803f_ffff;
        bus.write_u32(pointer_slot, pointer);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_STATUS_POINTER_SCAN_START;
        cpu.next_pc = BR2_STATUS_POINTER_SCAN_START + 4;
        cpu.regs[6] = pointer_slot - 0x14;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_STATUS_POINTER_SCAN_START);
        assert_eq!(report.instruction, Some(0x8cc3_0014));
        assert!(report.cycles_elapsed < BR2_STATUS_POINTER_SCAN_CYCLES);
        assert_eq!(cpu.pc, BR2_STATUS_POINTER_SCAN_START + 4);
        assert_eq!(cpu.next_pc, BR2_STATUS_POINTER_SCAN_START + 8);
        assert_eq!(bus.read_u32(pointer_slot), pointer);
        assert_eq!(cpu.pending_load, Some((3, pointer)));
    }

    #[test]
    fn fast_forwards_br2_status_halfword_wait_loop_to_next_vblank() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_status_halfword_wait_loop(&mut bus);
        let pointer_slot = 0x8038_cab4;
        let watched_pointer = 0x8035_ee34;
        bus.write_u32(pointer_slot, watched_pointer);
        bus.write_u16(watched_pointer, 0x37e7);
        bus.io.irq.mask = 1;
        bus.tick(565_900);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_STATUS_HALFWORD_WAIT_LOOP_START;
        cpu.next_pc = BR2_STATUS_HALFWORD_WAIT_LOOP_START + 4;
        cpu.regs[6] = pointer_slot - 0x14;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_STATUS_HALFWORD_WAIT_LOOP_START);
        assert_eq!(report.cycles_elapsed, 100);
        assert_eq!(cpu.pc, BR2_STATUS_HALFWORD_WAIT_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_STATUS_HALFWORD_WAIT_LOOP_START + 4);
        assert_eq!(cpu.regs[5], watched_pointer);
        assert_eq!(bus.read_u16(watched_pointer), 0x00e7);
        assert_eq!(bus.vblank_count(), 1);
        assert_eq!(bus.io.irq.status & 1, 1);
    }

    #[test]
    fn fast_forwards_br2_status_halfword_wait_loop_from_tail_to_next_vblank() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_status_halfword_wait_loop(&mut bus);
        let pointer_slot = 0x8038_cab4;
        let watched_pointer = 0x8035_ee34;
        bus.write_u32(pointer_slot, watched_pointer);
        bus.write_u16(watched_pointer, 0x37e7);
        bus.tick(565_820);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_STATUS_HALFWORD_WAIT_LOOP_TAIL_LOAD;
        cpu.next_pc = BR2_STATUS_HALFWORD_WAIT_LOOP_TAIL_LOAD + 4;
        cpu.regs[6] = pointer_slot - 0x14;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_STATUS_HALFWORD_WAIT_LOOP_TAIL_LOAD);
        assert_eq!(report.cycles_elapsed, 180);
        assert_eq!(cpu.pc, BR2_STATUS_HALFWORD_WAIT_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_STATUS_HALFWORD_WAIT_LOOP_START + 4);
        assert_eq!(cpu.regs[5], watched_pointer);
        assert_eq!(bus.read_u16(watched_pointer), 0x00e7);
        assert_eq!(bus.vblank_count(), 1);
    }

    #[test]
    fn status_halfword_wait_loop_does_not_fast_forward_when_high_byte_is_clear() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_status_halfword_wait_loop(&mut bus);
        let pointer_slot = 0x8038_cab4;
        let watched_pointer = 0x8035_ee34;
        bus.write_u32(pointer_slot, watched_pointer);
        bus.write_u16(watched_pointer, 0x00e7);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_STATUS_HALFWORD_WAIT_LOOP_START;
        cpu.next_pc = BR2_STATUS_HALFWORD_WAIT_LOOP_START + 4;
        cpu.regs[6] = pointer_slot - 0x14;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_STATUS_HALFWORD_WAIT_LOOP_START);
        assert_eq!(report.instruction, Some(0x8cc5_0014));
        assert!(report.cycles_elapsed < 100);
        assert_eq!(cpu.pc, BR2_STATUS_HALFWORD_WAIT_LOOP_START + 4);
        assert_eq!(cpu.next_pc, BR2_STATUS_HALFWORD_WAIT_LOOP_START + 8);
        assert_eq!(cpu.pending_load, Some((5, watched_pointer)));
        assert_eq!(bus.vblank_count(), 0);
    }

    #[test]
    fn status_halfword_wait_loop_does_not_fast_forward_special_status_handlers() {
        for status in [
            BR2_STATUS_HALFWORD_WAIT_LOOP_FC_STATUS | 0x00e7,
            BR2_STATUS_HALFWORD_WAIT_LOOP_FE_STATUS | 0x00e7,
            BR2_STATUS_HALFWORD_WAIT_LOOP_FF_STATUS | 0x00e7,
        ] {
            let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
            install_br2_status_halfword_wait_loop(&mut bus);
            let pointer_slot = 0x8038_cab4;
            let watched_pointer = 0x8035_ee34;
            bus.write_u32(pointer_slot, watched_pointer);
            bus.write_u16(watched_pointer, status);
            bus.tick(565_900);

            let mut cpu = Cpu::default();
            cpu.pc = BR2_STATUS_HALFWORD_WAIT_LOOP_START;
            cpu.next_pc = BR2_STATUS_HALFWORD_WAIT_LOOP_START + 4;
            cpu.regs[6] = pointer_slot - 0x14;

            let report = cpu.step_report(&mut bus);

            assert_eq!(report.start_pc, BR2_STATUS_HALFWORD_WAIT_LOOP_START);
            assert_eq!(report.instruction, Some(0x8cc5_0014));
            assert!(report.cycles_elapsed < 100);
            assert_eq!(cpu.pc, BR2_STATUS_HALFWORD_WAIT_LOOP_START + 4);
            assert_eq!(cpu.next_pc, BR2_STATUS_HALFWORD_WAIT_LOOP_START + 8);
            assert_eq!(cpu.pending_load, Some((5, watched_pointer)));
            assert_eq!(bus.vblank_count(), 0);
        }
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
        assert_eq!(bus.read_u32(BR2_FRAME_COUNTER_WAIT_LOOP_GLOBAL_COUNTER), 32);
        assert_eq!(
            bus.read_u32(cpu.regs[29] + BR2_FRAME_COUNTER_WAIT_LOOP_STACK_OFFSET),
            90
        );
    }

    #[test]
    fn frame_counter_wait_loop_fast_forward_preserves_vblank_irq() {
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
        bus.io.irq.mask = 1;
        bus.tick(565_820);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_FRAME_COUNTER_WAIT_LOOP_START;
        cpu.next_pc = BR2_FRAME_COUNTER_WAIT_LOOP_START + 4;
        cpu.cp0[CP0_STATUS] = 1 | CAUSE_IP2;
        cpu.regs[4] = 32;
        cpu.regs[29] = 0x8001_0000;
        bus.write_u32(cpu.regs[29] + BR2_FRAME_COUNTER_WAIT_LOOP_STACK_OFFSET, 100);

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_FRAME_COUNTER_WAIT_LOOP_START);
        assert_eq!(report.cycles_elapsed, 180);
        assert_eq!(cpu.pc, BR2_FRAME_COUNTER_WAIT_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_FRAME_COUNTER_WAIT_LOOP_START + 4);
        assert_eq!(bus.vblank_count(), 1);
        assert_eq!(bus.read_u32(BR2_FRAME_COUNTER_WAIT_LOOP_GLOBAL_COUNTER), 32);
        assert_eq!(
            bus.read_u32(cpu.regs[29] + BR2_FRAME_COUNTER_WAIT_LOOP_STACK_OFFSET),
            90
        );
        assert_eq!(bus.io.irq.status & 1, 1);
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
    fn br2_irq_poll_timeout_loop_does_not_fast_forward_across_vblank_irq() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_irq_poll_timeout_loop(&mut bus);
        bus.io.irq.mask = 1;
        bus.tick(565_999);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_IRQ_POLL_TIMEOUT_LOOP_START;
        cpu.next_pc = BR2_IRQ_POLL_TIMEOUT_LOOP_START + 4;
        cpu.regs[3] = 0x1000;
        cpu.regs[4] = BR2_IRQ_POLL_STATUS_ADDRESS;
        cpu.regs[5] = u32::MAX;
        cpu.cp0[CP0_STATUS] = STATUS_IE | CAUSE_IP2;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_IRQ_POLL_TIMEOUT_LOOP_START);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(bus.vblank_count(), 1);
        assert_eq!(cpu.pc, BR2_IRQ_POLL_TIMEOUT_LOOP_START + 4);
        assert_eq!(cpu.next_pc, BR2_IRQ_POLL_TIMEOUT_LOOP_START + 8);
        assert_eq!(cpu.regs[3], 0x1000);
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
    fn caps_br2_banked_halfword_copy_loop_before_vblank() {
        let mut banked = vec![0; 0x0080_0000];
        for index in 0..256u32 {
            let value = 0x7000u16 | index as u16;
            let offset = (2 + index * 2) as usize;
            banked[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        let mut bus = Bus::with_banked_roms(Vec::new(), banked, 4 * 1024 * 1024);
        for (offset, instruction) in BR2_BANKED_HALFWORD_COPY_LOOP_INSTRUCTIONS {
            bus.write_u32(BR2_BANKED_HALFWORD_COPY_LOOP_START + offset, instruction);
        }
        bus.tick(565_000);

        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let expected_halfwords =
            ((cycles_until_vblank - 1) / BR2_BANKED_HALFWORD_COPY_CYCLES_PER_HALFWORD) as u32;
        assert!(expected_halfwords > 0);
        assert!(expected_halfwords < 256);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_BANKED_HALFWORD_COPY_LOOP_START;
        cpu.next_pc = BR2_BANKED_HALFWORD_COPY_LOOP_START + 4;
        cpu.regs[3] = 0x1f00_0002;
        cpu.regs[16] = 0;
        cpu.regs[17] = 2;
        cpu.regs[18] = 0x8001_0001;
        cpu.regs[19] = BR2_BANKED_HALFWORD_COPY_MASK;
        cpu.regs[20] = 512;

        let report = cpu.step_report(&mut bus);
        let expected_bytes = expected_halfwords * 2;

        assert_eq!(report.start_pc, BR2_BANKED_HALFWORD_COPY_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(expected_halfwords) * BR2_BANKED_HALFWORD_COPY_CYCLES_PER_HALFWORD
        );
        assert_eq!(cpu.pc, BR2_BANKED_HALFWORD_COPY_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_BANKED_HALFWORD_COPY_LOOP_START + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[3], 0x1f00_0002 + expected_bytes);
        assert_eq!(cpu.regs[16], expected_bytes);
        assert_eq!(cpu.regs[17], 2 + expected_bytes);
        assert_eq!(cpu.regs[18], 0x8001_0001 + expected_bytes);
        assert_eq!(
            bus.read_u16(0x8001_0001 + (expected_halfwords - 1) * 2),
            0x7000 | (expected_halfwords - 1) as u16
        );
        assert_eq!(bus.read_u16(0x8001_0001 + expected_halfwords * 2), 0);
        assert_eq!(bus.vblank_count(), 0);
    }

    #[test]
    fn fast_forwards_br2_bitstream_decode_loop_to_table_sentinel() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_bitstream_decode_loop(&mut bus);
        let table_base = 0x8002_0000;
        let destination = 0x8003_0000;
        bus.write_u32(table_base, 0x1234_0001);
        bus.write_u32(table_base + 4, BR2_BITSTREAM_DECODE_TABLE_SENTINEL);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_BITSTREAM_DECODE_LOOP_START;
        cpu.next_pc = BR2_BITSTREAM_DECODE_LOOP_START + 4;
        cpu.regs[2] = 0;
        cpu.regs[3] = 0;
        cpu.regs[5] = destination;
        cpu.regs[6] = table_base;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_BITSTREAM_DECODE_LOOP_START);
        assert_eq!(report.cycles_elapsed, BR2_BITSTREAM_DECODE_TABLE_CYCLES);
        assert_eq!(cpu.pc, BR2_BITSTREAM_DECODE_EXIT_TABLE_SENTINEL);
        assert_eq!(cpu.next_pc, BR2_BITSTREAM_DECODE_EXIT_TABLE_SENTINEL + 4);
        assert_eq!(cpu.regs[3], 1);
        assert_eq!(cpu.regs[5], destination + 2);
        assert_eq!(bus.read_u16(destination), 0x1234);
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
    fn fast_forwards_br2_post_vs_unaligned_noop_expansion_without_writes() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let table_base = 0x99f6_d44e;
        let start_index = 0u32;
        let limit = 5_000u32;
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
            u64::from(limit) * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
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
    fn fast_forwards_br2_post_vs_table_group_loop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let outer_index = 4u32;
        let outer_limit = 12_000u32;
        let start_group_offset = 0x20u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(owner + 0x28, outer_limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_GROUP_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_GROUP_LOOP_START + 4;
        cpu.regs[4] = owner;
        cpu.regs[6] = start_group_offset;
        cpu.regs[7] = outer_index;

        let report = cpu.step_report(&mut bus);

        let remaining = outer_limit - outer_index;
        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_GROUP_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(remaining) * BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_GROUP_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_GROUP_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[3], table_meta_offset);
        assert_eq!(cpu.regs[5], 0);
        assert_eq!(cpu.regs[6], start_group_offset + remaining * 8);
        assert_eq!(cpu.regs[7], outer_limit);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn caps_br2_post_vs_table_group_huge_noop_gap_host_cycles() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0;
        let outer_index = 0u32;
        let outer_limit = BR2_POST_VS_TABLE_GROUP_MAX_CHARGED_NOOP_ITERATIONS * 4;
        let start_group_offset = 0x0040_0000u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(owner + 0x28, outer_limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_GROUP_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_GROUP_LOOP_START + 4;
        cpu.regs[4] = owner;
        cpu.regs[6] = start_group_offset;
        cpu.regs[7] = outer_index;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_GROUP_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(BR2_POST_VS_TABLE_GROUP_MAX_CHARGED_NOOP_ITERATIONS)
                * BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_GROUP_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_GROUP_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[6], start_group_offset + outer_limit * 8);
        assert_eq!(cpu.regs[7], outer_limit);
    }

    #[test]
    fn fast_forwards_br2_post_vs_table_group_unaligned_prefix_load_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_group_loop(&mut bus);
        let owner = 0x8001_4994;
        let table_meta_offset = 0x8001_4dd3;
        let start_group_offset = 0x0002_cd88;
        let count_address = 0x8004_1b5b;
        let outer_index = 22_961u32;
        let outer_limit = outer_index + 17;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(owner + 0x28, outer_limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_GROUP_LOOP_START + 0x0c;
        cpu.next_pc = cpu.pc + 4;
        cpu.regs[3] = count_address;
        cpu.regs[4] = owner;
        cpu.regs[6] = start_group_offset;
        cpu.regs[7] = outer_index;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_GROUP_LOOP_START + 0x0c);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(
            report.cycles_elapsed,
            17 * BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_GROUP_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_GROUP_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[3], table_meta_offset);
        assert_eq!(cpu.regs[5], 0);
        assert_eq!(cpu.regs[6], start_group_offset + 17 * 8);
        assert_eq!(cpu.regs[7], outer_limit);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn caps_br2_post_vs_table_group_loop_before_vblank_irq() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let outer_index = 0u32;
        let outer_limit = 100_000u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(owner + 0x28, outer_limit);
        bus.io.irq.mask = 1;
        bus.tick(550_000);
        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let expected_iterations =
            ((cycles_until_vblank - 1) / BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION) as u32;
        assert!(expected_iterations > 0);
        assert!(expected_iterations < outer_limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_GROUP_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_GROUP_LOOP_START + 4;
        cpu.cp0[CP0_STATUS] = STATUS_IE | CAUSE_IP2;
        cpu.regs[4] = owner;
        cpu.regs[6] = 0x10;
        cpu.regs[7] = outer_index;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_GROUP_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(expected_iterations) * BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_GROUP_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_GROUP_LOOP_START + 4);
        assert_eq!(cpu.regs[3], table_meta_offset);
        assert_eq!(cpu.regs[6], 0x10 + expected_iterations * 8);
        assert_eq!(cpu.regs[7], expected_iterations);
        assert_eq!(bus.vblank_count(), 0);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn completes_br2_post_vs_table_group_noop_gap_across_vblank_budget() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x8100_0000;
        let outer_index = 0x0172_d136u32;
        let outer_limit = outer_index + 100_000;
        let start_group_offset = 0x0100_0000u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(owner + 0x28, outer_limit);
        bus.io.irq.mask = 1;
        bus.tick(550_000);
        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let expected_charged_iterations =
            ((cycles_until_vblank - 1) / BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION) as u32;
        assert!(expected_charged_iterations > 0);
        assert!(expected_charged_iterations < outer_limit - outer_index);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_GROUP_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_GROUP_LOOP_START + 4;
        cpu.cp0[CP0_STATUS] = STATUS_IE | CAUSE_IP2;
        cpu.regs[4] = owner;
        cpu.regs[6] = start_group_offset;
        cpu.regs[7] = outer_index;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_GROUP_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(expected_charged_iterations) * BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_GROUP_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_GROUP_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[3], table_meta_offset);
        assert_eq!(
            cpu.regs[6],
            start_group_offset + (outer_limit - outer_index) * 8
        );
        assert_eq!(cpu.regs[7], outer_limit);
        assert_eq!(bus.vblank_count(), 0);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn completes_br2_post_vs_table_group_nonpositive_ram_scan_across_vblank_budget() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let outer_index = 0x0172_d136u32;
        let outer_limit =
            outer_index + BR2_POST_VS_TABLE_GROUP_NONPOSITIVE_MIN_SKIP_ITERATIONS + 32;
        let start_group_offset = 0x0006_9fb8u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(owner + 0x28, outer_limit);
        bus.io.irq.mask = 1;
        bus.tick(550_000);
        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let expected_charged_iterations =
            ((cycles_until_vblank - 1) / BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION) as u32;
        assert!(expected_charged_iterations > 0);
        assert!(expected_charged_iterations < outer_limit - outer_index);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_GROUP_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_GROUP_LOOP_START + 4;
        cpu.cp0[CP0_STATUS] = STATUS_IE | CAUSE_IP2;
        cpu.regs[4] = owner;
        cpu.regs[6] = start_group_offset;
        cpu.regs[7] = outer_index;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_GROUP_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(expected_charged_iterations) * BR2_POST_VS_TABLE_GROUP_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_GROUP_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_GROUP_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[3], table_meta_offset);
        assert_eq!(
            cpu.regs[6],
            start_group_offset + (outer_limit - outer_index) * 8
        );
        assert_eq!(cpu.regs[7], outer_limit);
        assert_eq!(bus.vblank_count(), 0);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn fast_forwards_br2_post_vs_table_group_loop_with_tiny_vblank_budget() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let outer_limit = 100_000u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(owner + 0x28, outer_limit);
        bus.io.irq.mask = 1;
        bus.tick(565_990);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_GROUP_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_GROUP_LOOP_START + 4;
        cpu.cp0[CP0_STATUS] = STATUS_IE | CAUSE_IP2;
        cpu.regs[4] = owner;
        cpu.regs[6] = 0x10;
        cpu.regs[7] = 0;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_GROUP_LOOP_START);
        assert_eq!(
            report.instruction,
            Some(BR2_POST_VS_TABLE_GROUP_PREFIX_INSTRUCTIONS[0])
        );
        assert_eq!(report.cycles_elapsed, 2);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_GROUP_LOOP_START + 4);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_GROUP_LOOP_START + 8);
        assert_eq!(cpu.regs[7], 0);
        assert_eq!(cpu.pending_load, Some((3, table_meta_offset)));
        assert_eq!(bus.vblank_count(), 0);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn fast_forwards_br2_post_vs_table_select_group_loop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_select_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let outer_index = 3u32;
        let outer_limit = 10_000u32;
        let start_record_offset = 0x40u32;
        bus.write_u32(owner + 0x0c, 4);
        bus.write_u32(owner + 0x14, 0x1234_5678);
        bus.write_u32(owner + 0x24, outer_limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START + 4;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_record_offset;
        cpu.regs[6] = 1;
        cpu.regs[7] = outer_index;

        let report = cpu.step_report(&mut bus);

        let remaining = outer_limit - outer_index;
        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(remaining) * BR2_POST_VS_TABLE_SELECT_GROUP_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[5], start_record_offset + remaining * 0x14);
        assert_eq!(cpu.regs[6], 1);
        assert_eq!(cpu.regs[7], outer_limit);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn hle_returns_from_br2_post_vs_packed_vertex_helper_for_packed_args() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_packed_vertex_helper(&mut bus);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_PACKED_VERTEX_HELPER_START;
        cpu.next_pc = BR2_POST_VS_PACKED_VERTEX_HELPER_START + 4;
        cpu.regs[4] = 0x0015_0013;
        cpu.regs[5] = 0x0016_0014;
        cpu.regs[6] = 0x8038_b128;
        cpu.regs[7] = 0x0017_0015;
        cpu.regs[29] = 0x803f_feb0;
        cpu.regs[31] = BR2_POST_VS_PACKED_VERTEX_HELPER_RETURN;
        bus.write_u32(cpu.regs[29] + 0x10, 0x11);
        bus.write_u32(cpu.regs[29] + 0x14, 0xe3);

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_PACKED_VERTEX_HELPER_START);
        assert_eq!(
            report.instruction,
            Some(BR2_POST_VS_PACKED_VERTEX_HELPER_INSTRUCTIONS[0].1)
        );
        assert_eq!(
            report.cycles_elapsed,
            BR2_POST_VS_PACKED_VERTEX_HELPER_CYCLES
        );
        assert_eq!(cpu.pc, BR2_POST_VS_PACKED_VERTEX_HELPER_RETURN);
        assert_eq!(cpu.next_pc, BR2_POST_VS_PACKED_VERTEX_HELPER_RETURN + 4);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn does_not_hle_br2_post_vs_packed_vertex_helper_for_aligned_pointer_args() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_packed_vertex_helper(&mut bus);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_PACKED_VERTEX_HELPER_START;
        cpu.next_pc = BR2_POST_VS_PACKED_VERTEX_HELPER_START + 4;
        cpu.regs[4] = 0x8001_0000;
        cpu.regs[5] = 0x8001_0010;
        cpu.regs[6] = 0x8038_b128;
        cpu.regs[7] = 0x8001_0020;
        cpu.regs[29] = 0x803f_feb0;
        cpu.regs[31] = BR2_POST_VS_PACKED_VERTEX_HELPER_RETURN;
        bus.write_u32(cpu.regs[29] + 0x10, 0x11);
        bus.write_u32(cpu.regs[29] + 0x14, 0xe3);

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_PACKED_VERTEX_HELPER_START);
        assert_eq!(
            report.instruction,
            Some(BR2_POST_VS_PACKED_VERTEX_HELPER_INSTRUCTIONS[0].1)
        );
        assert_eq!(cpu.pc, BR2_POST_VS_PACKED_VERTEX_HELPER_START + 4);
        assert_eq!(cpu.next_pc, BR2_POST_VS_PACKED_VERTEX_HELPER_START + 8);
        assert_eq!(cpu.pending_load, Some((24, 0x11)));
    }

    #[test]
    fn caps_br2_post_vs_table_select_group_huge_noop_gap_host_cycles() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_select_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let outer_index = 0u32;
        let outer_limit = BR2_POST_VS_TABLE_SELECT_GROUP_MAX_CHARGED_NOOP_ITERATIONS * 4;
        let start_record_offset = 0x0040_0000u32;
        bus.write_u32(owner + 0x0c, 4);
        bus.write_u32(owner + 0x14, 0x1234_5678);
        bus.write_u32(owner + 0x24, outer_limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START + 4;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_record_offset;
        cpu.regs[6] = 1;
        cpu.regs[7] = outer_index;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(BR2_POST_VS_TABLE_SELECT_GROUP_MAX_CHARGED_NOOP_ITERATIONS)
                * BR2_POST_VS_TABLE_SELECT_GROUP_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], start_record_offset + outer_limit * 0x14);
        assert_eq!(cpu.regs[7], outer_limit);
    }

    #[test]
    fn fast_forwards_br2_post_vs_table_select_group_loop_with_pending_v0_load() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_select_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let outer_index = 3u32;
        let outer_limit = 10_000u32;
        bus.write_u32(owner + 0x0c, 4);
        bus.write_u32(owner + 0x14, 0x1234_5678);
        bus.write_u32(owner + 0x24, outer_limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START + 4;
        cpu.regs[4] = owner;
        cpu.regs[5] = 0x40;
        cpu.regs[6] = 1;
        cpu.regs[7] = outer_index;
        cpu.pending_load = Some((2, outer_limit));

        let report = cpu.step_report(&mut bus);

        let remaining = outer_limit - outer_index;
        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_EXIT);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[5], 0x40 + remaining * 0x14);
        assert_eq!(cpu.regs[7], outer_limit);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_post_vs_table_select_group_from_compare_delay() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_select_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let outer_index = 3u32;
        let outer_limit = 10_000u32;
        let start_record_offset = 0x80u32;
        bus.write_u32(owner + 0x14, 0x1234_5678);
        bus.write_u32(owner + 0x24, outer_limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_DELAY;
        cpu.next_pc = BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_DELAY + 4;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_record_offset;
        cpu.regs[6] = 1;
        cpu.regs[7] = outer_index;
        cpu.pending_load = Some((2, 0x1234_5678));

        let report = cpu.step_report(&mut bus);

        let remaining = outer_limit - outer_index;
        assert_eq!(
            report.start_pc,
            BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_DELAY
        );
        assert_eq!(report.instruction, Some(0));
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[5], start_record_offset + remaining * 0x14);
        assert_eq!(cpu.regs[7], outer_limit);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_post_vs_table_select_group_from_compare_branch() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_select_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let outer_index = 3u32;
        let outer_limit = 10_000u32;
        let start_record_offset = 0x80u32;
        bus.write_u32(owner + 0x24, outer_limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_BRANCH;
        cpu.next_pc = BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_BRANCH + 4;
        cpu.regs[2] = 0x1234_5678;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_record_offset;
        cpu.regs[6] = 1;
        cpu.regs[7] = outer_index;

        let report = cpu.step_report(&mut bus);

        let remaining = outer_limit - outer_index;
        assert_eq!(
            report.start_pc,
            BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_BRANCH
        );
        assert_eq!(report.instruction, Some(0x1446_0016));
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_EXIT);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[5], start_record_offset + remaining * 0x14);
        assert_eq!(cpu.regs[7], outer_limit);
    }

    #[test]
    fn fast_forwards_br2_post_vs_table_select_group_from_tail_load() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_select_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let outer_index = 3u32;
        let outer_limit = 10_000u32;
        let start_record_offset = 0x80u32;
        bus.write_u32(owner + 0x24, outer_limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_SELECT_GROUP_TAIL_LOAD;
        cpu.next_pc = BR2_POST_VS_TABLE_SELECT_GROUP_TAIL_LOAD + 4;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_record_offset;
        cpu.regs[6] = 1;
        cpu.regs[7] = outer_index;

        let report = cpu.step_report(&mut bus);

        let remaining = outer_limit - outer_index;
        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_SELECT_GROUP_TAIL_LOAD);
        assert_eq!(report.instruction, Some(0x8c82_0024));
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_EXIT);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[5], start_record_offset + remaining * 0x14);
        assert_eq!(cpu.regs[7], outer_limit);
    }

    #[test]
    fn fast_forwards_br2_post_vs_table_select_group_from_tail_increment() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_select_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let outer_index = 3u32;
        let outer_limit = 10_000u32;
        let start_record_offset = 0x80u32;

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_SELECT_GROUP_TAIL_INCREMENT;
        cpu.next_pc = BR2_POST_VS_TABLE_SELECT_GROUP_TAIL_INCREMENT + 4;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_record_offset;
        cpu.regs[6] = 1;
        cpu.regs[7] = outer_index;
        cpu.pending_load = Some((2, outer_limit));

        let report = cpu.step_report(&mut bus);

        let remaining = outer_limit - outer_index;
        assert_eq!(
            report.start_pc,
            BR2_POST_VS_TABLE_SELECT_GROUP_TAIL_INCREMENT
        );
        assert_eq!(report.instruction, Some(0x24e7_0001));
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_EXIT);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[5], start_record_offset + remaining * 0x14);
        assert_eq!(cpu.regs[7], outer_limit);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn does_not_fast_forward_br2_post_vs_table_select_group_compare_equal_mid_path() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_select_group_loop(&mut bus);
        let owner = 0x8001_0000;
        bus.write_u32(owner + 0x24, 100);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_BRANCH;
        cpu.next_pc = BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_BRANCH + 4;
        cpu.regs[2] = 1;
        cpu.regs[4] = owner;
        cpu.regs[6] = 1;
        cpu.regs[7] = 0;

        let report = cpu.step_report(&mut bus);

        assert_eq!(
            report.start_pc,
            BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_BRANCH
        );
        assert_eq!(report.instruction, Some(0x1446_0016));
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_BRANCH + 4);
        assert_eq!(
            cpu.next_pc,
            BR2_POST_VS_TABLE_SELECT_GROUP_COMPARE_BRANCH + 8
        );
        assert_eq!(cpu.regs[7], 0);
    }

    #[test]
    fn fast_forwards_br2_post_vs_null_link_scan_from_loop_start() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_null_link_scan_loop(&mut bus);
        let sentinel = 0x8012_e568;

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_NULL_LINK_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_NULL_LINK_SCAN_LOOP_START + 4;
        cpu.regs[5] = 0;
        cpu.regs[9] = sentinel;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_NULL_LINK_SCAN_LOOP_START);
        assert_eq!(report.instruction, Some(0x8ca2_0008));
        assert_eq!(report.cycles_elapsed, BR2_POST_VS_NULL_LINK_SCAN_CYCLES);
        assert_eq!(cpu.pc, BR2_POST_VS_NULL_LINK_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_NULL_LINK_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[5], sentinel);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_post_vs_null_link_scan_from_tail_delay() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_null_link_scan_loop(&mut bus);
        let sentinel = 0x8012_e568;

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_NULL_LINK_SCAN_TAIL_DELAY;
        cpu.next_pc = BR2_POST_VS_NULL_LINK_SCAN_TAIL_DELAY + 4;
        cpu.regs[5] = 0x1111_1111;
        cpu.regs[9] = sentinel;
        cpu.pending_load = Some((5, 0));

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_NULL_LINK_SCAN_TAIL_DELAY);
        assert_eq!(report.instruction, Some(0));
        assert_eq!(cpu.pc, BR2_POST_VS_NULL_LINK_SCAN_LOOP_EXIT);
        assert_eq!(cpu.regs[5], sentinel);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_post_vs_null_link_scan_from_tail_branch() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_null_link_scan_loop(&mut bus);
        let sentinel = 0x8012_e568;

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_NULL_LINK_SCAN_TAIL_BRANCH;
        cpu.next_pc = BR2_POST_VS_NULL_LINK_SCAN_TAIL_BRANCH + 4;
        cpu.regs[5] = 0;
        cpu.regs[9] = sentinel;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_NULL_LINK_SCAN_TAIL_BRANCH);
        assert_eq!(report.instruction, Some(0x14a9_ffeb));
        assert_eq!(cpu.pc, BR2_POST_VS_NULL_LINK_SCAN_LOOP_EXIT);
        assert_eq!(cpu.regs[5], sentinel);
    }

    #[test]
    fn does_not_fast_forward_br2_post_vs_null_link_scan_for_non_null_node() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_null_link_scan_loop(&mut bus);
        let node = 0x8001_0000;
        bus.write_u32(node + 8, 0);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_NULL_LINK_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_NULL_LINK_SCAN_LOOP_START + 4;
        cpu.regs[5] = node;
        cpu.regs[9] = 0x8012_e568;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_NULL_LINK_SCAN_LOOP_START);
        assert_eq!(report.instruction, Some(0x8ca2_0008));
        assert_eq!(report.cycles_elapsed, 2);
        assert_eq!(cpu.pc, BR2_POST_VS_NULL_LINK_SCAN_LOOP_START + 4);
        assert_eq!(cpu.next_pc, BR2_POST_VS_NULL_LINK_SCAN_LOOP_START + 8);
        assert_eq!(cpu.regs[5], node);
    }

    #[test]
    fn fast_forwards_br2_post_vs_null_link_scan_corrupt_pointer_to_sentinel() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_null_link_scan_loop(&mut bus);
        let sentinel = u32::MAX;

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_NULL_LINK_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_NULL_LINK_SCAN_LOOP_START + 4;
        cpu.regs[5] = 0x1080_0129;
        cpu.regs[9] = sentinel;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_NULL_LINK_SCAN_LOOP_START);
        assert_eq!(report.instruction, Some(0x8ca2_0008));
        assert_eq!(cpu.pc, BR2_POST_VS_NULL_LINK_SCAN_LOOP_EXIT);
        assert_eq!(cpu.regs[5], sentinel);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_EXCODE_MASK, 0);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
    }

    #[test]
    fn does_not_fast_forward_br2_post_vs_null_link_scan_for_zero_sentinel() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_null_link_scan_loop(&mut bus);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_NULL_LINK_SCAN_TAIL_LOAD;
        cpu.next_pc = BR2_POST_VS_NULL_LINK_SCAN_TAIL_LOAD + 4;
        cpu.regs[5] = 0;
        cpu.regs[9] = 0;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_NULL_LINK_SCAN_TAIL_LOAD);
        assert_eq!(report.instruction, Some(0x8ca5_0000));
        assert_eq!(report.cycles_elapsed, 2);
        assert_eq!(cpu.pc, BR2_POST_VS_NULL_LINK_SCAN_TAIL_LOAD + 4);
        assert_eq!(cpu.next_pc, BR2_POST_VS_NULL_LINK_SCAN_TAIL_LOAD + 8);
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_link_scan_from_loop_start_null_current() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_link_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let stack_slot = sp + BR2_POST_VS_STACK_LINK_SCAN_STACK_SLOT_OFFSET;
        bus.write_u32(stack_slot, 0);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_LINK_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_LINK_SCAN_LOOP_START + 4;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_START);
        assert_eq!(report.instruction, Some(0x8fa9_0158));
        assert_eq!(
            report.cycles_elapsed,
            BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_START_CYCLES
        );
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], u32::MAX);
        assert_eq!(cpu.regs[8], u32::MAX);
        assert_eq!(cpu.regs[9], 0);
        assert_eq!(cpu.regs[10], 0);
        assert_eq!(cpu.regs[20], 0x0c);
        assert_eq!(bus.read_u32(stack_slot), u32::MAX);
        assert_eq!(bus.read_u32(sp + 0x120), 0);
        assert_eq!(bus.read_u32(sp + 0x128), 0);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_link_scan_from_reload_delay_null_current() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_link_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let stack_slot = sp + BR2_POST_VS_STACK_LINK_SCAN_STACK_SLOT_OFFSET;
        bus.write_u32(stack_slot, 0);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_LINK_SCAN_RELOAD_DELAY;
        cpu.next_pc = BR2_POST_VS_STACK_LINK_SCAN_RELOAD_DELAY + 4;
        cpu.regs[29] = sp;
        cpu.pending_load = Some((8, 0));

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_LINK_SCAN_RELOAD_DELAY);
        assert_eq!(report.instruction, Some(0));
        assert_eq!(
            report.cycles_elapsed,
            BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_RELOAD_DELAY_CYCLES
        );
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], u32::MAX);
        assert_eq!(cpu.regs[8], u32::MAX);
        assert_eq!(bus.read_u32(stack_slot), u32::MAX);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_link_scan_from_tail_branch_null_current() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_link_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let stack_slot = sp + BR2_POST_VS_STACK_LINK_SCAN_STACK_SLOT_OFFSET;
        bus.write_u32(stack_slot, 0);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_LINK_SCAN_TAIL_BRANCH;
        cpu.next_pc = BR2_POST_VS_STACK_LINK_SCAN_TAIL_BRANCH + 4;
        cpu.regs[2] = u32::MAX;
        cpu.regs[8] = 0;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_LINK_SCAN_TAIL_BRANCH);
        assert_eq!(report.instruction, Some(0x1502_fb65));
        assert_eq!(
            report.cycles_elapsed,
            BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_TAIL_BRANCH_CYCLES
        );
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT);
        assert_eq!(cpu.regs[2], u32::MAX);
        assert_eq!(cpu.regs[8], u32::MAX);
        assert_eq!(bus.read_u32(stack_slot), u32::MAX);
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_link_scan_when_unused_signature_word_differs() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_link_scan_loop(&mut bus);
        bus.write_u32(BR2_POST_VS_STACK_LINK_SCAN_NEXT_LOAD, 0);
        let sp = 0x803f_fd30;
        let stack_slot = sp + BR2_POST_VS_STACK_LINK_SCAN_STACK_SLOT_OFFSET;
        bus.write_u32(stack_slot, 0);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_LINK_SCAN_RELOAD;
        cpu.next_pc = BR2_POST_VS_STACK_LINK_SCAN_RELOAD + 4;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_LINK_SCAN_RELOAD);
        assert_eq!(report.instruction, Some(0x8fa8_0158));
        assert_eq!(
            report.cycles_elapsed,
            BR2_POST_VS_STACK_LINK_SCAN_NULL_FROM_RELOAD_CYCLES
        );
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT);
        assert_eq!(bus.read_u32(stack_slot), u32::MAX);
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_link_scan_corrupt_stack_pointer_to_exit() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_link_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let stack_slot = sp + BR2_POST_VS_STACK_LINK_SCAN_STACK_SLOT_OFFSET;
        bus.write_u32(stack_slot, 0x013e_9aa1);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_LINK_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_LINK_SCAN_LOOP_START + 4;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_START);
        assert_eq!(report.instruction, Some(0x8fa9_0158));
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[8], u32::MAX);
        assert_eq!(bus.read_u32(stack_slot), u32::MAX);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_EXCODE_MASK, 0);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_link_scan_terminal_next_to_exit() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_link_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let stack_slot = sp + BR2_POST_VS_STACK_LINK_SCAN_STACK_SLOT_OFFSET;
        let current = 0x8014_7d2c;
        bus.write_u32(stack_slot, current);
        bus.write_u32(current, 0x013e_9aa1);
        bus.write_u32(current + 8, 0);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_LINK_SCAN_NEXT_LOAD;
        cpu.next_pc = BR2_POST_VS_STACK_LINK_SCAN_NEXT_LOAD + 4;
        cpu.regs[8] = current;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_LINK_SCAN_NEXT_LOAD);
        assert_eq!(report.instruction, Some(0x8d08_0000));
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[8], u32::MAX);
        assert_eq!(bus.read_u32(stack_slot), u32::MAX);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_EXCODE_MASK, 0);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_link_scan_tail_corrupt_next_to_exit() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_link_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let stack_slot = sp + BR2_POST_VS_STACK_LINK_SCAN_STACK_SLOT_OFFSET;
        bus.write_u32(stack_slot, 0x8014_7d2c);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_LINK_SCAN_TAIL_BRANCH;
        cpu.next_pc = BR2_POST_VS_STACK_LINK_SCAN_TAIL_BRANCH + 4;
        cpu.regs[2] = u32::MAX;
        cpu.regs[8] = 0x013e_9aa1;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_LINK_SCAN_TAIL_BRANCH);
        assert_eq!(report.instruction, Some(0x1502_fb65));
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[8], u32::MAX);
        assert_eq!(bus.read_u32(stack_slot), u32::MAX);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_EXCODE_MASK, 0);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
    }

    #[test]
    fn does_not_fast_forward_br2_post_vs_stack_link_scan_for_non_null_current() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_link_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let stack_slot = sp + BR2_POST_VS_STACK_LINK_SCAN_STACK_SLOT_OFFSET;
        let node = 0x8001_0000;
        bus.write_u32(stack_slot, node);
        bus.write_u32(node + 8, 0);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_LINK_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_LINK_SCAN_LOOP_START + 4;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_START);
        assert_eq!(report.instruction, Some(0x8fa9_0158));
        assert_eq!(report.cycles_elapsed, 2);
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_START + 4);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_LINK_SCAN_LOOP_START + 8);
        assert_eq!(cpu.pending_load, Some((9, node)));
        assert_eq!(bus.read_u32(stack_slot), node);
    }

    #[test]
    fn does_not_fast_forward_br2_post_vs_stack_link_scan_when_signature_damaged() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_link_scan_loop(&mut bus);
        bus.write_u32(BR2_POST_VS_STACK_LINK_SCAN_RELOAD, 0);
        let sp = 0x803f_fd30;
        bus.write_u32(sp + BR2_POST_VS_STACK_LINK_SCAN_STACK_SLOT_OFFSET, 0);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_LINK_SCAN_RELOAD;
        cpu.next_pc = BR2_POST_VS_STACK_LINK_SCAN_RELOAD + 4;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_LINK_SCAN_RELOAD);
        assert_eq!(report.instruction, Some(0));
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_LINK_SCAN_RELOAD + 4);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_LINK_SCAN_RELOAD + 8);
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_packet_scan_noop_run_to_exit() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x8008_0000;
        let start_index = 2u32;
        let limit = 40u32;
        let remaining = limit - start_index;
        bus.write_u32(
            sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET,
            start_index,
        );
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);

        for packet_index in 0..remaining {
            let packet = cursor + packet_index * 8;
            bus.write_u32(packet, 0);
            bus.write_u32(packet + 4, 1);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(report.instruction, Some(0x8e84_0000));
        assert_eq!(
            report.cycles_elapsed,
            u64::from(remaining) * BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET
        );
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[10], limit);
        assert_eq!(cpu.regs[11], limit);
        assert_eq!(cpu.regs[20], cursor + remaining * 8);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            limit
        );
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LENGTH_STACK_OFFSET),
            1
        );
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn caps_br2_post_vs_stack_packet_scan_before_vblank_irq() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x8008_0000;
        let limit = 1_000u32;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);
        bus.io.irq.mask = 1;
        bus.tick(565_000);

        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let expected_packets =
            ((cycles_until_vblank - 1) / BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET) as u32;
        assert!(expected_packets > 0);
        assert!(expected_packets < limit);

        for packet_index in 0..limit {
            let packet = cursor + packet_index * 8;
            bus.write_u32(packet, 0);
            bus.write_u32(packet + 4, 1);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.cp0[CP0_STATUS] = STATUS_IE | CAUSE_IP2;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(expected_packets) * BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET
        );
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4);
        assert_eq!(cpu.regs[10], expected_packets);
        assert_eq!(cpu.regs[11], limit);
        assert_eq!(cpu.regs[20], cursor + expected_packets * 8);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            expected_packets
        );
        assert_eq!(bus.vblank_count(), 0);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn caps_br2_post_vs_stack_packet_scan_before_host_visible_vblank() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x8008_0000;
        let limit = 1_000u32;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);
        bus.io.irq.mask = 1;
        bus.tick(565_000);

        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let expected_packets =
            ((cycles_until_vblank - 1) / BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET) as u32;

        for packet_index in 0..limit {
            let packet = cursor + packet_index * 8;
            bus.write_u32(packet, 0);
            bus.write_u32(packet + 4, 1);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(expected_packets) * BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET
        );
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.regs[10], expected_packets);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            expected_packets
        );
        assert_eq!(bus.vblank_count(), 0);
    }

    #[test]
    fn completes_br2_post_vs_stack_packet_scan_long_gap_across_vblank_with_valid_return_address() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x8040_0000;
        let limit = BR2_POST_VS_STACK_PACKET_SCAN_LONG_UNTHROTTLED_MIN_PACKETS + 128;
        let return_address = 0x8031_4950;
        let ra_slot = sp + BR2_POST_VS_STACK_PACKET_SCAN_RA_STACK_OFFSET;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);
        bus.write_u32(ra_slot, 0);
        bus.io.irq.mask = 1;
        bus.tick(565_000);

        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let charged_cycles = u64::from(BR2_POST_VS_STACK_PACKET_SCAN_MAX_CHARGED_NOOP_PACKETS)
            * BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET;
        assert!(cycles_until_vblank < charged_cycles);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;
        cpu.regs[31] = return_address;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(BR2_POST_VS_STACK_PACKET_SCAN_MAX_CHARGED_NOOP_PACKETS)
                * BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET
        );
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[10], limit);
        assert_eq!(cpu.regs[11], limit);
        assert_eq!(cpu.regs[20], cursor + limit * 4);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            limit
        );
        assert_eq!(bus.read_u32(ra_slot), return_address);
        assert!(bus.vblank_count() > 0);
        assert_eq!(bus.io.irq.status & 1, 1);
    }

    #[test]
    fn advances_br2_post_vs_stack_packet_scan_partial_long_gap_across_vblank_with_valid_return_address()
     {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x9e0f_b0dc;
        let index = 125_006_470u32;
        let limit = 708_419_683u32;
        let return_address = 0x8031_5780;
        let ra_slot = sp + BR2_POST_VS_STACK_PACKET_SCAN_RA_STACK_OFFSET;
        let noop_gap_run = br2_post_vs_stack_packet_scan_noop_gap_run(
            cursor,
            BR2_POST_VS_STACK_PACKET_SCAN_MAX_SKIP_PACKETS,
            bus.ram_len(),
        );
        assert!(noop_gap_run > 1_000_000);
        assert!(noop_gap_run < BR2_POST_VS_STACK_PACKET_SCAN_MAX_SKIP_PACKETS);
        let expected_packets = BR2_POST_VS_STACK_PACKET_SCAN_MAX_SKIP_PACKETS;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, index);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);
        bus.write_u32(ra_slot, 0);
        bus.io.irq.mask = 1;
        bus.tick(565_000);

        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let charged_cycles = u64::from(BR2_POST_VS_STACK_PACKET_SCAN_MAX_CHARGED_NOOP_PACKETS)
            * BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET;
        assert!(cycles_until_vblank < charged_cycles);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LENGTH_LOAD;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LENGTH_LOAD + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;
        cpu.regs[31] = return_address;

        let report = cpu.step_report(&mut bus);

        assert_eq!(
            report.start_pc,
            BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LENGTH_LOAD
        );
        assert_eq!(
            report.cycles_elapsed,
            BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPLETION_CYCLES
                + u64::from(BR2_POST_VS_STACK_PACKET_SCAN_MAX_CHARGED_NOOP_PACKETS)
                    * BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET
        );
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4);
        assert_eq!(cpu.regs[10], index + 1 + expected_packets);
        assert_eq!(cpu.regs[11], limit);
        assert_eq!(cpu.regs[20], cursor + expected_packets * 4);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            index + 1 + expected_packets
        );
        assert_eq!(bus.read_u32(ra_slot), 0);
        assert!(bus.vblank_count() > 0);
    }

    #[test]
    fn caps_br2_post_vs_stack_packet_scan_long_gap_before_vblank_without_valid_return_address() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x8040_0000;
        let limit = BR2_POST_VS_STACK_PACKET_SCAN_LONG_UNTHROTTLED_MIN_PACKETS + 128;
        let ra_slot = sp + BR2_POST_VS_STACK_PACKET_SCAN_RA_STACK_OFFSET;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);
        bus.write_u32(ra_slot, 0x1111_2222);
        bus.io.irq.mask = 1;
        bus.tick(565_000);

        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let expected_packets =
            ((cycles_until_vblank - 1) / BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET) as u32;
        assert!(expected_packets > 0);
        assert!(expected_packets < limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;
        cpu.regs[31] = 0;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(expected_packets) * BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET
        );
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4);
        assert_eq!(cpu.regs[10], expected_packets);
        assert_eq!(cpu.regs[11], limit);
        assert_eq!(cpu.regs[20], cursor + expected_packets * 4);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            expected_packets
        );
        assert_eq!(bus.read_u32(ra_slot), 0x1111_2222);
        assert_eq!(bus.vblank_count(), 0);
    }

    #[test]
    fn preserves_br2_post_vs_stack_packet_scan_existing_return_address() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x8040_0000;
        let limit = BR2_POST_VS_STACK_PACKET_SCAN_LONG_UNTHROTTLED_MIN_PACKETS + 128;
        let existing_return_address = 0x802c_cde8;
        let current_return_address = 0x8031_4774;
        let ra_slot = sp + BR2_POST_VS_STACK_PACKET_SCAN_RA_STACK_OFFSET;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);
        bus.write_u32(ra_slot, existing_return_address);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;
        cpu.regs[31] = current_return_address;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT + 4);
        assert_eq!(bus.read_u32(ra_slot), existing_return_address);
    }

    #[test]
    fn verifies_br2_post_vs_stack_packet_scan_long_ram_zero_run_beyond_short_budget() {
        let mut bus = Bus::new(Vec::new(), 8 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x8040_0000;
        let limit = BR2_POST_VS_STACK_PACKET_SCAN_LONG_UNTHROTTLED_MIN_PACKETS + 128;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);
        bus.io.irq.mask = 1;
        bus.tick(565_000);
        let expected_packets = ((bus.cycles_until_next_vblank() - 1)
            / BR2_POST_VS_STACK_PACKET_SCAN_CYCLES_PER_PACKET)
            as u32;

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert!(expected_packets > 0);
        assert!(expected_packets < limit);
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4);
        assert_eq!(cpu.regs[10], expected_packets);
        assert_eq!(cpu.regs[11], limit);
        assert!(limit > BR2_POST_VS_STACK_PACKET_SCAN_MAX_VERIFIED_RAM_PACKETS);
        assert!(limit <= BR2_POST_VS_STACK_PACKET_SCAN_LONG_MAX_VERIFIED_RAM_PACKETS);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            expected_packets
        );
    }

    #[test]
    fn verifies_br2_post_vs_stack_packet_scan_long_ram_zero_run_to_ram_boundary() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x8032_0000;
        let expected_packets = ((0x0040_0000 - 0x0032_0000) / 4) - 1;
        let limit = BR2_POST_VS_STACK_PACKET_SCAN_LONG_UNTHROTTLED_MIN_PACKETS + 128;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[10], limit);
        assert_eq!(cpu.regs[11], limit);
        assert!(cpu.regs[20] >= cursor + limit * 4);
        assert!(expected_packets > BR2_POST_VS_STACK_PACKET_SCAN_MAX_VERIFIED_RAM_PACKETS);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            limit
        );
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_packet_scan_from_noop_body_mid_path() {
        let mut bus = Bus::new(Vec::new(), 8 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x8040_0000;
        let limit = BR2_POST_VS_STACK_PACKET_SCAN_LONG_UNTHROTTLED_MIN_PACKETS + 128;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_BODY_FAST_FORWARD_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_BODY_FAST_FORWARD_START + 4;
        cpu.regs[20] = cursor + 4;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(
            report.start_pc,
            BR2_POST_VS_STACK_PACKET_SCAN_BODY_FAST_FORWARD_START
        );
        assert_eq!(report.instruction, Some(0x2402_0013));
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[10], limit);
        assert_eq!(cpu.regs[11], limit);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            limit
        );
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_packet_scan_when_unused_signature_word_differs() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        bus.write_u32(0x8031_4300, 0x0000_0000);
        let sp = 0x803f_fd30;
        let cursor = 0x01c7_765c;
        let start_index = 0x202c_64ae;
        let limit = start_index + BR2_POST_VS_STACK_PACKET_SCAN_LONG_UNTHROTTLED_MIN_PACKETS + 64;
        let expected_packets =
            BR2_POST_VS_STACK_PACKET_SCAN_MAX_SKIP_PACKETS.min(limit - start_index);
        bus.write_u32(
            sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET,
            start_index,
        );
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert!(report.cycles_elapsed > 2);
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[10], start_index + expected_packets);
        assert_eq!(cpu.regs[11], limit);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            start_index + expected_packets
        );
    }

    #[test]
    fn does_not_fast_forward_br2_post_vs_stack_packet_scan_when_current_instruction_differs() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        bus.write_u32(BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START, 0x0000_0000);
        let sp = 0x803f_fd30;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(
            sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET,
            1_000_000,
        );

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = 0x01c7_765c;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(report.instruction, Some(0x0000_0000));
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4);
    }

    #[test]
    fn does_not_mutate_br2_post_vs_stack_packet_scan_tail_when_vblank_preempts() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor_after_header = 0x8008_0004;
        let index_before_increment = 5u32;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LENGTH_STACK_OFFSET, 1);
        bus.write_u32(
            sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET,
            index_before_increment,
        );
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, 10);
        bus.io.irq.mask = 1;
        bus.tick(565_995);
        assert!(
            bus.cycles_until_next_vblank() <= BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPLETION_CYCLES
        );

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LIMIT_LOAD;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LIMIT_LOAD + 4;
        cpu.cp0[CP0_STATUS] = STATUS_IE | CAUSE_IP2;
        cpu.pending_load = Some((10, index_before_increment));
        cpu.regs[9] = 1;
        cpu.regs[20] = cursor_after_header;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(
            report.start_pc,
            BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LIMIT_LOAD
        );
        assert_eq!(report.cycles_elapsed, 2);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            index_before_increment
        );
    }

    #[test]
    fn fast_forwards_br2_post_vs_vertex_record_loop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_vertex_record_loop(&mut bus);

        let a1 = 0x8000_1012;
        let t0 = 0x8000_203e;
        let t2 = 0x8000_2000;
        let t3 = 0x8000_3000;
        let a2 = 0x8000_4000;
        let a3 = 0x8000_5000;
        bus.write_u32(t3, 0x1111_1111);
        bus.write_u32(t3 + 0x14, 0x2222_2222);
        bus.write_u32(a1 - 0x0e, 0x3333_3333);
        bus.write_u32(a1 - 0x0a, 0x4444_4444);
        bus.write_u32(a1 + 0x14 - 0x0e, 0x5555_5555);
        bus.write_u32(a1 + 0x14 - 0x0a, 0x6666_6666);
        for offset in [-0x06i32, -0x04, -0x02, 0] {
            bus.write_u16(a1.wrapping_add(offset as u32), 0);
            bus.write_u16(a1.wrapping_add(0x14).wrapping_add(offset as u32), 0);
        }
        bus.write_u32(a2, 0xaaaa_0001);
        bus.write_u32(a3, 0xbbbb_0002);
        bus.write_u16(t0 - 0x20, 10);
        bus.write_u16(t0 - 0x18, 20);
        bus.write_u16(t0, 30);
        bus.write_u16(t0 + 0x08, 40);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_VERTEX_RECORD_LOOP_START;
        cpu.next_pc = BR2_POST_VS_VERTEX_RECORD_LOOP_START + 4;
        cpu.regs[5] = a1;
        cpu.regs[6] = a2;
        cpu.regs[7] = a3;
        cpu.regs[8] = t0;
        cpu.regs[9] = 2;
        cpu.regs[10] = t2;
        cpu.regs[11] = t3;
        cpu.regs[12] = 0x24;
        cpu.regs[13] = 7;
        cpu.regs[14] = 2;
        cpu.regs[15] = 3;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_VERTEX_RECORD_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            2 * BR2_POST_VS_VERTEX_RECORD_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_VERTEX_RECORD_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_VERTEX_RECORD_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], a1 + 0x28);
        assert_eq!(cpu.regs[8], t0 + 0xa0);
        assert_eq!(cpu.regs[9], 0);
        assert_eq!(cpu.regs[10], t2 + 0xa0);
        assert_eq!(cpu.regs[11], t3 + 0x28);

        assert_eq!(bus.read_u8(t0 - 0x2b), 7);
        assert_eq!(bus.read_u8(t0 - 0x27), 0x24);
        assert_eq!(bus.read_u32(t0 + 0x0e), 0x4444_4444);
        assert_eq!(bus.read_u32(t0 - 0x32), 0xbbbb_0002);
        assert_eq!(bus.read_u32(t2), 0xaaaa_0001);
        assert_eq!(bus.read_u32(t0 - 0x3a), 0xaaaa_0001);
        assert_eq!(bus.read_u16(t0 - 0x20), 0x1113);
        assert_eq!(bus.read_u16(t0 - 0x18), 0x3336);
        assert_eq!(bus.read_u16(t0), 0x1113);
        assert_eq!(bus.read_u16(t0 + 0x08), 0x3336);
    }

    #[test]
    fn fast_forwards_br2_post_vs_record_copy_loop_with_ram_records() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_record_copy_loop(&mut bus);
        let source = 0x8001_0000;
        let destination = 0x8002_0000;
        let sp = 0x8003_0000;
        let counter_slot = sp + BR2_POST_VS_RECORD_COPY_COUNTER_STACK_OFFSET;
        for index in 0..8u32 {
            bus.write_u32(source + index * 4, 0x1000_0000 | index);
        }
        bus.write_u32(counter_slot, 3);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_RECORD_COPY_LOOP_START;
        cpu.next_pc = BR2_POST_VS_RECORD_COPY_LOOP_START + 4;
        cpu.regs[3] = source;
        cpu.regs[17] = destination;
        cpu.regs[19] = 5;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_RECORD_COPY_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            2 * BR2_POST_VS_RECORD_COPY_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_RECORD_COPY_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_RECORD_COPY_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[3], source + 0x20);
        assert_eq!(cpu.regs[10], 5);
        assert_eq!(cpu.regs[17], destination + 0x20);
        assert_eq!(bus.read_u32(counter_slot), 5);
        for index in 0..8u32 {
            assert_eq!(bus.read_u32(destination + index * 4), 0x1000_0000 | index);
        }
    }

    #[test]
    fn fast_forwards_br2_post_vs_record_copy_from_store_mid_path_noop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_record_copy_loop(&mut bus);

        let sp = 0x8000_8000;
        let counter_slot = sp + BR2_POST_VS_RECORD_COPY_COUNTER_STACK_OFFSET;
        let counter = 0x0056_da6e;
        let remaining = 12;
        let source = 0x057e_a510;
        let destination = 0x057e_3348;
        bus.write_u32(counter_slot, counter);

        let mut cpu = Cpu::default();
        cpu.pc = 0x8031_5544;
        cpu.next_pc = 0x8031_5548;
        cpu.regs[3] = source;
        cpu.regs[17] = destination;
        cpu.regs[19] = counter + remaining;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x8031_5544);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(remaining) * BR2_POST_VS_RECORD_COPY_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_RECORD_COPY_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_RECORD_COPY_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.regs[3], source + remaining * 0x10);
        assert_eq!(cpu.regs[10], counter + remaining);
        assert_eq!(cpu.regs[17], destination + remaining * 0x10);
        assert_eq!(bus.read_u32(counter_slot), counter + remaining);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_post_vs_record_copy_after_cursor_increment_noop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_record_copy_loop(&mut bus);

        let sp = 0x8000_8000;
        let counter_slot = sp + BR2_POST_VS_RECORD_COPY_COUNTER_STACK_OFFSET;
        let counter = 7;
        let remaining = 3;
        let source = 0x057e_a510;
        let destination = 0x057e_3348;
        bus.write_u32(counter_slot, counter);

        let mut cpu = Cpu::default();
        cpu.pc = 0x8031_5558;
        cpu.next_pc = 0x8031_555c;
        cpu.regs[3] = source + 0x10;
        cpu.regs[10] = counter;
        cpu.regs[17] = destination + 0x10;
        cpu.regs[19] = counter + remaining;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x8031_5558);
        assert_eq!(cpu.pc, BR2_POST_VS_RECORD_COPY_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_RECORD_COPY_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[3], source + remaining * 0x10);
        assert_eq!(cpu.regs[10], counter + remaining);
        assert_eq!(cpu.regs[17], destination + remaining * 0x10);
        assert_eq!(bus.read_u32(counter_slot), counter + remaining);
    }

    #[test]
    fn fast_forwards_br2_post_vs_record_copy_with_pending_load_mid_path_noop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_record_copy_loop(&mut bus);

        let sp = 0x8000_8000;
        let counter_slot = sp + BR2_POST_VS_RECORD_COPY_COUNTER_STACK_OFFSET;
        let counter = 9;
        let remaining = 2;
        let source = 0x057e_a510;
        let destination = 0x057e_3348;
        bus.write_u32(counter_slot, counter);

        let mut cpu = Cpu::default();
        cpu.pc = 0x8031_5530;
        cpu.next_pc = 0x8031_5534;
        cpu.pending_load = Some((10, 0));
        cpu.regs[3] = source;
        cpu.regs[17] = destination;
        cpu.regs[19] = counter + remaining;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x8031_5530);
        assert_eq!(cpu.pc, BR2_POST_VS_RECORD_COPY_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_RECORD_COPY_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[3], source + remaining * 0x10);
        assert_eq!(cpu.regs[10], counter + remaining);
        assert_eq!(cpu.regs[17], destination + remaining * 0x10);
        assert_eq!(bus.read_u32(counter_slot), counter + remaining);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn caps_br2_post_vs_record_copy_noop_loop_before_vblank_irq() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_record_copy_loop(&mut bus);
        bus.io.irq.mask = 1;
        bus.tick(565_000);
        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let expected_iterations =
            ((cycles_until_vblank - 1) / BR2_POST_VS_RECORD_COPY_CYCLES_PER_ITERATION) as u32;
        assert!(expected_iterations > 0);

        let sp = 0x8000_8000;
        let counter_slot = sp + BR2_POST_VS_RECORD_COPY_COUNTER_STACK_OFFSET;
        let counter = 0x0056_d000;
        bus.write_u32(counter_slot, counter);

        let mut cpu = Cpu::default();
        cpu.cp0[CP0_STATUS] = STATUS_IE | CAUSE_IP2;
        cpu.pc = BR2_POST_VS_RECORD_COPY_LOOP_START;
        cpu.next_pc = BR2_POST_VS_RECORD_COPY_LOOP_START + 4;
        cpu.regs[3] = 0x057e_a4b0;
        cpu.regs[17] = 0x057e_32f8;
        cpu.regs[19] = 0xa000_e004;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_RECORD_COPY_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(expected_iterations) * BR2_POST_VS_RECORD_COPY_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_RECORD_COPY_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_POST_VS_RECORD_COPY_LOOP_START + 4);
        assert_eq!(cpu.regs[2], 1);
        assert_eq!(cpu.regs[3], 0x057e_a4b0 + expected_iterations * 0x10);
        assert_eq!(cpu.regs[10], counter + expected_iterations);
        assert_eq!(cpu.regs[17], 0x057e_32f8 + expected_iterations * 0x10);
        assert_eq!(bus.read_u32(counter_slot), counter + expected_iterations);
    }

    #[test]
    fn fast_forwards_br2_post_vs_huge_expansion_record_copy_to_exit() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_record_copy_loop(&mut bus);
        bus.io.irq.mask = 1;
        bus.tick(565_000);

        let sp = 0x8000_8000;
        let counter_slot = sp + BR2_POST_VS_RECORD_COPY_COUNTER_STACK_OFFSET;
        let counter = 0x0097_fe64;
        let remaining = BR2_POST_VS_RECORD_COPY_HUGE_NOOP_MIN_ITERATIONS + 123;
        let limit = counter + remaining;
        bus.write_u32(counter_slot, counter);

        let mut cpu = Cpu::default();
        cpu.cp0[CP0_STATUS] = STATUS_IE | CAUSE_IP2;
        cpu.pc = BR2_POST_VS_RECORD_COPY_LOOP_START;
        cpu.next_pc = BR2_POST_VS_RECORD_COPY_LOOP_START + 4;
        cpu.regs[3] = 0x0980_0008;
        cpu.regs[17] = 0x0980_1008;
        cpu.regs[19] = limit;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_RECORD_COPY_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(BR2_POST_VS_RECORD_COPY_MAX_CHARGED_NOOP_ITERATIONS)
                * BR2_POST_VS_RECORD_COPY_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_RECORD_COPY_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_RECORD_COPY_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(
            cpu.regs[3],
            0x0980_0008u32.wrapping_add(remaining.wrapping_mul(16))
        );
        assert_eq!(
            cpu.regs[17],
            0x0980_1008u32.wrapping_add(remaining.wrapping_mul(16))
        );
        assert_eq!(cpu.regs[10], limit);
        assert_eq!(bus.read_u32(counter_slot), limit);
    }

    #[test]
    fn fast_forwards_br2_post_vs_strided_pointer_copy_ram_loop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_strided_pointer_copy_loop(&mut bus);
        let source = 0x8000_1000;
        let destination = 0x8000_2000;
        let pointer_table = 0x8000_3000;
        let count = 3u32;

        for offset in 0..count * 8 {
            bus.write_u8(source + offset, 0x40 + offset as u8);
        }
        for offset in 0..count * 16 {
            bus.write_u8(destination + offset, 0xee);
        }
        for offset in 0..count * 8 {
            bus.write_u8(pointer_table + offset, 0xbb);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START + 4;
        cpu.regs[3] = count;
        cpu.regs[4] = destination;
        cpu.regs[6] = source;
        cpu.regs[7] = pointer_table;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(count) * BR2_POST_VS_STRIDED_POINTER_COPY_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], destination + 40);
        assert_eq!(cpu.regs[3], 0);
        assert_eq!(cpu.regs[4], destination + 48);
        assert_eq!(cpu.regs[6], source + 24);
        assert_eq!(cpu.regs[7], pointer_table + 24);
        assert_eq!(cpu.regs[8], 0x5352_5150);
        assert_eq!(cpu.regs[9], 0x5756_5554);

        for index in 0..count {
            for offset in 0..8 {
                assert_eq!(
                    bus.read_u8(destination + index * 16 + offset),
                    0x40 + (index * 8 + offset) as u8
                );
            }
            assert_eq!(bus.read_u8(destination + index * 16 + 8), 0xee);
            assert_eq!(
                bus.read_u32(pointer_table + index * 8),
                destination + index * 16 + 8
            );
            assert_eq!(bus.read_u32(pointer_table + index * 8 + 4), 0xbbbb_bbbb);
        }
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_post_vs_strided_pointer_copy_from_boot_snapshot_after_code_corruption() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.set_trace_context(BR2_BOOT_WORD_COPY_LOOP_START, 1);
        install_br2_post_vs_strided_pointer_copy_loop(&mut bus);
        bus.clear_trace_context();
        bus.write_u32(BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START + 4, 0x5555_5555);

        let source = 0x8000_1000;
        let destination = 0x8000_2000;
        let pointer_table = 0x8000_3000;
        for offset in 0..8 {
            bus.write_u8(source + offset, 0x90 + offset as u8);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START + 4;
        cpu.regs[3] = 1;
        cpu.regs[4] = destination;
        cpu.regs[6] = source;
        cpu.regs[7] = pointer_table;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            BR2_POST_VS_STRIDED_POINTER_COPY_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_EXIT);
        assert_eq!(cpu.regs[3], 0);
        assert_eq!(cpu.regs[8], 0x9392_9190);
        assert_eq!(cpu.regs[9], 0x9796_9594);
    }

    #[test]
    fn fast_forwards_br2_post_vs_strided_pointer_copy_huge_expansion_noop_loop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_strided_pointer_copy_loop(&mut bus);
        let source = 0x8118_fae4;
        let destination = 0x8118_fadc;
        let pointer_table = 0xf786_4cb8;
        let count = BR2_POST_VS_STRIDED_POINTER_COPY_HUGE_NOOP_MIN_ITERATIONS + 123;

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START + 4;
        cpu.regs[3] = count;
        cpu.regs[4] = destination;
        cpu.regs[6] = source;
        cpu.regs[7] = pointer_table;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START);
        assert_eq!(cpu.pc, BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_EXIT + 4);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(BR2_POST_VS_STRIDED_POINTER_COPY_MAX_CHARGED_NOOP_ITERATIONS)
                * BR2_POST_VS_STRIDED_POINTER_COPY_CYCLES_PER_ITERATION
        );
        assert_eq!(
            cpu.regs[2],
            destination.wrapping_add(count.wrapping_sub(1).wrapping_mul(16).wrapping_add(8))
        );
        assert_eq!(cpu.regs[3], 0);
        assert_eq!(
            cpu.regs[4],
            destination.wrapping_add(count.wrapping_mul(16))
        );
        assert_eq!(cpu.regs[6], source.wrapping_add(count.wrapping_mul(8)));
        assert_eq!(
            cpu.regs[7],
            pointer_table.wrapping_add(count.wrapping_mul(8))
        );
        assert_eq!(cpu.regs[8], 0);
        assert_eq!(cpu.regs[9], 0);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_post_vs_strided_pointer_copy_source_noop_preserves_ram() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_strided_pointer_copy_loop(&mut bus);
        let source = 0xf707_7908;
        let destination = 0x8036_647c;
        let pointer_table = 0x8036_6600;
        let count = 2u32;

        for offset in 0..count * 16 {
            bus.write_u8(destination + offset, 0xa0 + offset as u8);
        }
        for offset in 0..count * 8 {
            bus.write_u8(pointer_table + offset, 0xc0 + offset as u8);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START + 4;
        cpu.regs[3] = count;
        cpu.regs[4] = destination;
        cpu.regs[6] = source;
        cpu.regs[7] = pointer_table;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_START);
        assert_eq!(cpu.pc, BR2_POST_VS_STRIDED_POINTER_COPY_LOOP_EXIT);
        assert_eq!(cpu.regs[3], 0);
        assert_eq!(cpu.regs[4], destination + count * 16);
        assert_eq!(cpu.regs[6], source + count * 8);
        assert_eq!(cpu.regs[7], pointer_table + count * 8);
        assert_eq!(cpu.regs[8], 0);
        assert_eq!(cpu.regs[9], 0);
        for offset in 0..count * 16 {
            assert_eq!(bus.read_u8(destination + offset), 0xa0 + offset as u8);
        }
        for offset in 0..count * 8 {
            assert_eq!(bus.read_u8(pointer_table + offset), 0xc0 + offset as u8);
        }
    }

    #[test]
    fn fast_forwards_br2_post_vs_alt_strided_pointer_copy_ram_loop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_alt_strided_pointer_copy_loop(&mut bus);
        let source = 0x8000_1400;
        let destination = 0x8000_2400;
        let pointer_table = 0x8000_3400;
        let count = 3u32;

        for offset in 0..count * 8 {
            bus.write_u8(source + offset, 0x60 + offset as u8);
        }
        for offset in 0..count * 16 {
            bus.write_u8(destination + offset, 0xdd);
        }
        for offset in 0..count * 8 {
            bus.write_u8(pointer_table + offset, 0xcc);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_START;
        cpu.next_pc = BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_START + 4;
        cpu.regs[3] = source;
        cpu.regs[4] = destination;
        cpu.regs[5] = pointer_table;
        cpu.regs[6] = count;

        let report = cpu.step_report(&mut bus);

        assert_eq!(
            report.start_pc,
            BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_START
        );
        assert_eq!(
            report.cycles_elapsed,
            u64::from(count) * BR2_POST_VS_STRIDED_POINTER_COPY_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_EXIT);
        assert_eq!(
            cpu.next_pc,
            BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_EXIT + 4
        );
        assert_eq!(cpu.regs[2], destination + 40);
        assert_eq!(cpu.regs[3], source + 24);
        assert_eq!(cpu.regs[4], destination + 48);
        assert_eq!(cpu.regs[5], pointer_table + 24);
        assert_eq!(cpu.regs[6], 0);
        assert_eq!(cpu.regs[8], 0x7372_7170);
        assert_eq!(cpu.regs[9], 0x7776_7574);

        for index in 0..count {
            for offset in 0..8 {
                assert_eq!(
                    bus.read_u8(destination + index * 16 + offset),
                    0x60 + (index * 8 + offset) as u8
                );
            }
            assert_eq!(bus.read_u8(destination + index * 16 + 8), 0xdd);
            assert_eq!(
                bus.read_u32(pointer_table + index * 8),
                destination + index * 16 + 8
            );
            assert_eq!(bus.read_u32(pointer_table + index * 8 + 4), 0xcccc_cccc);
        }
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_post_vs_alt_strided_pointer_copy_huge_expansion_noop_loop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_alt_strided_pointer_copy_loop(&mut bus);
        let source = 0xfdb4_7490;
        let destination = 0xb2c4_2b1c;
        let pointer_table = 0xfdb4_7490;
        let count = BR2_POST_VS_STRIDED_POINTER_COPY_HUGE_NOOP_MIN_ITERATIONS + 321;

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_START;
        cpu.next_pc = BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_START + 4;
        cpu.regs[3] = source;
        cpu.regs[4] = destination;
        cpu.regs[5] = pointer_table;
        cpu.regs[6] = count;

        let report = cpu.step_report(&mut bus);

        assert_eq!(
            report.start_pc,
            BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_START
        );
        assert_eq!(cpu.pc, BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_EXIT);
        assert_eq!(
            cpu.next_pc,
            BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_EXIT + 4
        );
        assert_eq!(
            report.cycles_elapsed,
            u64::from(BR2_POST_VS_STRIDED_POINTER_COPY_MAX_CHARGED_NOOP_ITERATIONS)
                * BR2_POST_VS_STRIDED_POINTER_COPY_CYCLES_PER_ITERATION
        );
        assert_eq!(
            cpu.regs[2],
            destination.wrapping_add(count.wrapping_sub(1).wrapping_mul(16).wrapping_add(8))
        );
        assert_eq!(cpu.regs[3], source.wrapping_add(count.wrapping_mul(8)));
        assert_eq!(
            cpu.regs[4],
            destination.wrapping_add(count.wrapping_mul(16))
        );
        assert_eq!(
            cpu.regs[5],
            pointer_table.wrapping_add(count.wrapping_mul(8))
        );
        assert_eq!(cpu.regs[6], 0);
        assert_eq!(cpu.regs[8], 0);
        assert_eq!(cpu.regs[9], 0);
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_post_vs_alt_strided_pointer_copy_source_noop_preserves_ram() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_alt_strided_pointer_copy_loop(&mut bus);
        let source = 0xf707_7908;
        let destination = 0x8036_647c;
        let pointer_table = 0x8036_6600;
        let count = 2u32;

        for offset in 0..count * 16 {
            bus.write_u8(destination + offset, 0xb0 + offset as u8);
        }
        for offset in 0..count * 8 {
            bus.write_u8(pointer_table + offset, 0xd0 + offset as u8);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_START;
        cpu.next_pc = BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_START + 4;
        cpu.regs[3] = source;
        cpu.regs[4] = destination;
        cpu.regs[5] = pointer_table;
        cpu.regs[6] = count;

        let report = cpu.step_report(&mut bus);

        assert_eq!(
            report.start_pc,
            BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_START
        );
        assert_eq!(cpu.pc, BR2_POST_VS_ALT_STRIDED_POINTER_COPY_LOOP_EXIT);
        assert_eq!(cpu.regs[3], source + count * 8);
        assert_eq!(cpu.regs[4], destination + count * 16);
        assert_eq!(cpu.regs[5], pointer_table + count * 8);
        assert_eq!(cpu.regs[6], 0);
        assert_eq!(cpu.regs[8], 0);
        assert_eq!(cpu.regs[9], 0);
        for offset in 0..count * 16 {
            assert_eq!(bus.read_u8(destination + offset), 0xb0 + offset as u8);
        }
        for offset in 0..count * 8 {
            assert_eq!(bus.read_u8(pointer_table + offset), 0xd0 + offset as u8);
        }
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_packet_scan_unmapped_gap_bulk_run_to_exit() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x8040_0000;
        let limit = 100_000u32;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[10], limit);
        assert_eq!(cpu.regs[11], limit);
        assert_eq!(cpu.regs[20], cursor + limit * 4);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            limit
        );
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_packet_scan_scratchpad_hole_gap_to_io_boundary() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x9f80_043c;
        let limit = 10_000u32;
        let expected_packets = ((BR2_PSX_HW_IO_START - 0x1f80_043c) / 4).saturating_sub(1);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4);
        assert_eq!(cpu.regs[10], expected_packets);
        assert_eq!(cpu.regs[11], limit);
        assert_eq!(cpu.regs[20], cursor + expected_packets * 4);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            expected_packets
        );
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_packet_scan_scratchpad_hole_gap_to_exit() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x9f80_043c;
        let limit = 32u32;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[10], limit);
        assert_eq!(cpu.regs[11], limit);
        assert_eq!(cpu.regs[20], cursor + limit * 4);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            limit
        );
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_packet_scan_io_window_zero_metadata_to_exit() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x9f80_143c;
        let limit = 16u32;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[10], limit);
        assert_eq!(cpu.regs[11], limit);
        assert_eq!(cpu.regs[20], cursor + limit * 4);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            limit
        );
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_packet_scan_high_peripheral_gap_to_exit() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x9f80_2000;
        let limit = 100_000u32;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[10], limit);
        assert_eq!(cpu.regs[11], limit);
        assert_eq!(cpu.regs[20], cursor + limit * 4);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            limit
        );
    }

    #[test]
    fn caps_br2_post_vs_stack_packet_scan_high_gap_before_alias_wrap() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x9fff_ff00;
        let limit = 1000u32;
        let expected_packets = ((0x2000_0000u32 - 0x1fff_ff00u32) / 4).saturating_sub(1);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4);
        assert_eq!(cpu.regs[10], expected_packets);
        assert_eq!(cpu.regs[11], limit);
        assert_eq!(cpu.regs[20], cursor + expected_packets * 4);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            expected_packets
        );
        assert_eq!(bus.read_u32(sp + 0x154), 0);
    }

    #[test]
    fn caps_br2_post_vs_stack_packet_scan_mapped_ram_verification_budget() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x8008_0000;
        let limit = BR2_POST_VS_STACK_PACKET_SCAN_MAX_VERIFIED_RAM_PACKETS + 10;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4);
        assert_eq!(
            cpu.regs[10],
            BR2_POST_VS_STACK_PACKET_SCAN_MAX_VERIFIED_RAM_PACKETS
        );
        assert_eq!(
            cpu.regs[20],
            cursor + BR2_POST_VS_STACK_PACKET_SCAN_MAX_VERIFIED_RAM_PACKETS * 4
        );
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            BR2_POST_VS_STACK_PACKET_SCAN_MAX_VERIFIED_RAM_PACKETS
        );
    }

    #[test]
    fn does_not_fast_forward_br2_post_vs_stack_packet_scan_for_handled_tag() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor = 0x8008_0000;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 0);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, 4);
        bus.write_u32(cursor, 8);
        bus.write_u32(cursor + 4, 1);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4;
        cpu.regs[20] = cursor;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START);
        assert_eq!(report.instruction, Some(0x8e84_0000));
        assert_eq!(report.cycles_elapsed, 2);
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 4);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_START + 8);
        assert_eq!(cpu.pending_load, Some((4, 8)));
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            0
        );
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_packet_scan_tail_limit_to_exit() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_stack_packet_scan_loop(&mut bus);
        let sp = 0x803f_fd30;
        let cursor_after_header = 0x8008_0004;
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LENGTH_STACK_OFFSET, 1);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET, 5);
        bus.write_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_LIMIT_STACK_OFFSET, 6);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LIMIT_LOAD;
        cpu.next_pc = BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LIMIT_LOAD + 4;
        cpu.pending_load = Some((10, 5));
        cpu.regs[9] = 1;
        cpu.regs[20] = cursor_after_header;
        cpu.regs[29] = sp;

        let report = cpu.step_report(&mut bus);

        assert_eq!(
            report.start_pc,
            BR2_POST_VS_STACK_PACKET_SCAN_TAIL_LIMIT_LOAD
        );
        assert_eq!(report.instruction, Some(0x8fab_0120));
        assert_eq!(
            report.cycles_elapsed,
            BR2_POST_VS_STACK_PACKET_SCAN_TAIL_COMPLETION_CYCLES
        );
        assert_eq!(cpu.pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_STACK_PACKET_SCAN_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[10], 6);
        assert_eq!(cpu.regs[11], 6);
        assert_eq!(cpu.regs[20], cursor_after_header + 4);
        assert_eq!(
            bus.read_u32(sp + BR2_POST_VS_STACK_PACKET_SCAN_INDEX_STACK_OFFSET),
            6
        );
        assert_eq!(cpu.pending_load, None);
    }

    #[test]
    fn caps_br2_post_vs_table_select_group_loop_before_vblank_irq() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_select_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let outer_index = 0u32;
        let outer_limit = 100_000u32;
        bus.write_u32(owner + 0x0c, 4);
        bus.write_u32(owner + 0x14, 0x1234_5678);
        bus.write_u32(owner + 0x24, outer_limit);
        bus.io.irq.mask = 1;
        bus.tick(550_000);
        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let expected_iterations = ((cycles_until_vblank - 1)
            / BR2_POST_VS_TABLE_SELECT_GROUP_CYCLES_PER_ITERATION)
            as u32;
        assert!(expected_iterations > 0);
        assert!(expected_iterations < outer_limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START + 4;
        cpu.cp0[CP0_STATUS] = STATUS_IE | CAUSE_IP2;
        cpu.regs[4] = owner;
        cpu.regs[5] = 0x10;
        cpu.regs[6] = 1;
        cpu.regs[7] = outer_index;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(expected_iterations) * BR2_POST_VS_TABLE_SELECT_GROUP_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START + 4);
        assert_eq!(cpu.regs[5], 0x10 + expected_iterations * 0x14);
        assert_eq!(cpu.regs[7], expected_iterations);
        assert_eq!(bus.vblank_count(), 0);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn completes_br2_post_vs_table_select_group_noop_gap_across_vblank_budget() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_select_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let outer_index = 0x0172_d136u32;
        let outer_limit = outer_index + 100_000;
        let start_record_offset = 0x0100_0000u32;
        bus.write_u32(owner + 0x0c, 4);
        bus.write_u32(owner + 0x14, 0x1234_5678);
        bus.write_u32(owner + 0x24, outer_limit);
        bus.io.irq.mask = 1;
        bus.tick(550_000);
        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let expected_charged_iterations = ((cycles_until_vblank - 1)
            / BR2_POST_VS_TABLE_SELECT_GROUP_CYCLES_PER_ITERATION)
            as u32;
        assert!(expected_charged_iterations > 0);
        assert!(expected_charged_iterations < outer_limit - outer_index);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START + 4;
        cpu.cp0[CP0_STATUS] = STATUS_IE | CAUSE_IP2;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_record_offset;
        cpu.regs[6] = 1;
        cpu.regs[7] = outer_index;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(expected_charged_iterations)
                * BR2_POST_VS_TABLE_SELECT_GROUP_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(
            cpu.regs[5],
            start_record_offset + (outer_limit - outer_index) * 0x14
        );
        assert_eq!(cpu.regs[7], outer_limit);
        assert_eq!(bus.vblank_count(), 0);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn fast_forwards_br2_post_vs_table_select_group_loop_with_tiny_vblank_budget() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_select_group_loop(&mut bus);
        let owner = 0x8001_0000;
        let outer_limit = 100_000u32;
        bus.write_u32(owner + 0x0c, 4);
        bus.write_u32(owner + 0x14, 0x1234_5678);
        bus.write_u32(owner + 0x24, outer_limit);
        bus.io.irq.mask = 1;
        bus.tick(565_990);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START + 4;
        cpu.cp0[CP0_STATUS] = STATUS_IE | CAUSE_IP2;
        cpu.regs[4] = owner;
        cpu.regs[5] = 0x10;
        cpu.regs[6] = 1;
        cpu.regs[7] = 0;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START);
        assert_eq!(report.instruction, Some(0x8c82_000c));
        assert_eq!(report.cycles_elapsed, 2);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START + 4);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START + 8);
        assert_eq!(cpu.regs[7], 0);
        assert_eq!(cpu.pending_load, Some((2, 4)));
        assert_eq!(bus.vblank_count(), 0);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn does_not_fast_forward_br2_post_vs_table_select_group_inner_path() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_select_group_loop(&mut bus);
        let owner = 0x8001_0000;
        bus.write_u32(owner + 0x0c, 2);
        bus.write_u32(owner + 0x14, 1);
        bus.write_u32(owner + 0x24, 100);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START;
        cpu.next_pc = BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START + 4;
        cpu.regs[4] = owner;
        cpu.regs[6] = 1;
        cpu.regs[7] = 0;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START);
        assert_eq!(report.instruction, Some(0x8c82_000c));
        assert_eq!(report.cycles_elapsed, 2);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START + 4);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_SELECT_GROUP_LOOP_START + 8);
        assert_eq!(cpu.regs[7], 0);
    }

    #[test]
    fn fast_forwards_br2_post_vs_completed_tail_to_exit() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let current_index = 99u32;
        let limit = 100u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, 0x8003_0000);

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
        assert_eq!(report.cycles_elapsed, 4);
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
    fn fast_forwards_br2_post_vs_live_render_table_without_writes() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let table_base = 0x8000_0000 | BR2_POST_VS_LIVE_RENDER_RAM_NOOP_START;
        let start_index = 0u32;
        let limit = BR2_POST_VS_TABLE_ACCUM_MIN_SKIP_ITERATIONS;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, table_base);
        bus.write_u32(table_base, 0x0000_0010);
        bus.write_u32(table_base + 4, 0x0000_0020);

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
            u64::from(limit) * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], limit);
        assert_eq!(bus.read_u32(table_base), 0x0000_0010);
        assert_eq!(bus.read_u32(table_base + 4), 0x0000_0020);
    }

    #[test]
    fn fast_forwards_br2_post_vs_code_patch_table_without_writes() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_2000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let table_base = 0x802c_c100;
        let start_index = 0u32;
        let limit = BR2_POST_VS_TABLE_ACCUM_MIN_SKIP_ITERATIONS;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, table_base);
        bus.write_u32(table_base, 0x27bd_ffe0);
        bus.write_u32(table_base + 4, 0x3c04_1f80);
        bus.write_u32(table_base + 8, 0x8c85_0000);

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
            u64::from(limit) * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], limit);
        assert_eq!(bus.read_u32(table_base), 0x27bd_ffe0);
        assert_eq!(bus.read_u32(table_base + 4), 0x3c04_1f80);
        assert_eq!(bus.read_u32(table_base + 8), 0x8c85_0000);
    }

    #[test]
    fn interprets_br2_post_vs_code_patch_store_without_corrupting_code() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let table_base = 0x802c_c100;
        bus.write_u32(table_base, 0x27bd_ffe8);
        bus.write_u32(table_base + 4, 0x3c04_1f80);
        bus.write_u32(table_base + 8, 0x3484_00d0);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x18;
        cpu.next_pc = cpu.pc + 4;
        cpu.regs[2] = 0xa7be_ffe8;
        cpu.regs[3] = table_base;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x18);
        assert_eq!(report.instruction, Some(0xac62_0000));
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x1c);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x20);
        assert_eq!(bus.read_u32(table_base), 0x27bd_ffe8);
        assert_eq!(bus.read_u32(table_base + 4), 0x3c04_1f80);
        assert_eq!(bus.read_u32(table_base + 8), 0x3484_00d0);
    }

    #[test]
    fn interprets_br2_post_vs_runtime_code_store_without_corrupting_loop_tail() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let target = BR2_POST_VS_TABLE_ACCUM_LOOP_START;
        bus.write_u32(target, BR2_POST_VS_TABLE_ACCUM_LOOP_INSTRUCTIONS[0]);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x18;
        cpu.next_pc = cpu.pc + 4;
        cpu.regs[2] = 0x0c43_0004;
        cpu.regs[3] = target;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x18);
        assert_eq!(report.instruction, Some(0xac62_0000));
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x1c);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x20);
        assert_eq!(
            bus.read_u32(target),
            BR2_POST_VS_TABLE_ACCUM_LOOP_INSTRUCTIONS[0]
        );
    }

    #[test]
    fn interprets_br2_post_vs_runtime_code_store_after_signature_damage() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let target = BR2_POST_VS_TABLE_ACCUM_LOOP_START;
        let damaged_word_address = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 4;
        bus.write_u32(damaged_word_address, 0xdead_beef);
        bus.write_u32(target, BR2_POST_VS_TABLE_ACCUM_LOOP_INSTRUCTIONS[0]);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x18;
        cpu.next_pc = cpu.pc + 4;
        cpu.regs[2] = 0x0c43_0004;
        cpu.regs[3] = target;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x18);
        assert_eq!(report.instruction, Some(0xac62_0000));
        assert_eq!(
            bus.read_u32(target),
            BR2_POST_VS_TABLE_ACCUM_LOOP_INSTRUCTIONS[0]
        );
        assert_eq!(bus.read_u32(damaged_word_address), 0xdead_beef);
    }

    #[test]
    fn interprets_br2_post_vs_live_render_store_without_corrupting_primitives() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let target = 0x8038_4000;
        bus.write_u32(target, 0x09ff_ffff);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x18;
        cpu.next_pc = cpu.pc + 4;
        cpu.regs[2] = 0x89ff_ffff;
        cpu.regs[3] = target;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x18);
        assert_eq!(report.instruction, Some(0xac62_0000));
        assert_eq!(bus.read_u32(target), 0x09ff_ffff);
    }

    #[test]
    fn ignores_br2_post_vs_stack_guard_store_without_corrupting_return_address() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let stack_ra_slot = 0x803f_ff74;
        bus.write_u32(stack_ra_slot, 0x8034_068c);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x18;
        cpu.next_pc = cpu.pc + 4;
        cpu.regs[2] = 0x0002_9572;
        cpu.regs[3] = stack_ra_slot;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x18);
        assert_eq!(report.instruction, Some(0xac62_0000));
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(bus.read_u32(stack_ra_slot), 0x8034_068c);
    }

    #[test]
    fn interprets_br2_post_vs_regular_table_store() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let table_base = 0x8020_1000;
        bus.write_u32(table_base, 0x0000_0010);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x18;
        cpu.next_pc = cpu.pc + 4;
        cpu.regs[2] = 0x8001_0010;
        cpu.regs[3] = table_base;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x18);
        assert_eq!(report.instruction, Some(0xac62_0000));
        assert_eq!(bus.read_u32(table_base), 0x8001_0010);
    }

    #[test]
    fn fast_forwards_br2_post_vs_stack_guard_table_without_writes() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_4df8;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let stack_ra_slot = 0x803f_ff74;
        let limit = 3u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, stack_ra_slot);
        bus.write_u32(stack_ra_slot, 0x8034_068c);

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
            u64::from(limit) * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], limit);
        assert_eq!(bus.read_u32(stack_ra_slot), 0x8034_068c);
    }

    #[test]
    fn fast_forwards_br2_post_vs_code_patch_huge_physical_table_without_writes() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let table_base = 0x002c_c100;
        let start_index = 0u32;
        let limit = 0x5000_0000u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, table_base);
        bus.write_u32(table_base, 0x27bd_ffe8);
        bus.write_u32(table_base + 4, 0x3c04_1f80);
        bus.write_u32(table_base + 8, 0x3484_00d0);

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
            u64::from(BR2_POST_VS_TABLE_ACCUM_MAX_CHARGED_NOOP_ITERATIONS)
                * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], limit);
        assert_eq!(bus.read_u32(table_base), 0x27bd_ffe8);
        assert_eq!(bus.read_u32(table_base + 4), 0x3c04_1f80);
        assert_eq!(bus.read_u32(table_base + 8), 0x3484_00d0);
    }

    #[test]
    fn fast_forwards_br2_post_vs_exception_vector_table_without_writes() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let table_base = 0x8000_0080;
        let start_index = 0u32;
        let limit = BR2_POST_VS_TABLE_ACCUM_MIN_SKIP_ITERATIONS;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, table_base);
        bus.write_u32(table_base, 0x3c1a_0000);
        bus.write_u32(table_base + 4, 0x275a_0c80);
        bus.write_u32(table_base + 8, 0x0340_0008);
        bus.write_u32(table_base + 12, 0x0000_0000);

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
            u64::from(limit) * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], limit);
        assert_eq!(bus.read_u32(table_base), 0x3c1a_0000);
        assert_eq!(bus.read_u32(table_base + 4), 0x275a_0c80);
        assert_eq!(bus.read_u32(table_base + 8), 0x0340_0008);
        assert_eq!(bus.read_u32(table_base + 12), 0x0000_0000);
    }

    #[test]
    fn fast_forwards_br2_post_vs_low_bios_irq_code_without_writes() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let table_base = 0x8000_04ec;
        let start_index = 0u32;
        let limit = BR2_POST_VS_TABLE_ACCUM_MIN_SKIP_ITERATIONS;
        let protected_words = [
            (0x8000_04ec, 0x8c42_0004),
            (0x8000_0500, 0xbc09_0000),
            (0x8000_0548, 0x8c42_0000),
            (0x8000_05cc, 0x8c62_0000),
        ];
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, table_base);
        for (address, instruction) in protected_words {
            bus.write_u32(address, instruction);
        }

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
            u64::from(limit) * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], limit);
        for (address, instruction) in protected_words {
            assert_eq!(bus.read_u32(address), instruction);
        }
    }

    #[test]
    fn fast_forwards_br2_post_vs_runtime_code_table_without_writes() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let table_base = 0x8030_8344;
        let start_index = 0u32;
        let limit = BR2_POST_VS_TABLE_ACCUM_MIN_SKIP_ITERATIONS;
        let protected_words = [
            (0x8030_8344, 0xac82_0000),
            (0x8030_8374, 0x0c0c_209e),
            (0x8030_8390, 0xac62_0000),
        ];
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, table_base);
        for (address, instruction) in protected_words {
            bus.write_u32(address, instruction);
        }

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
            u64::from(limit) * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], limit);
        for (address, instruction) in protected_words {
            assert_eq!(bus.read_u32(address), instruction);
        }
    }

    #[test]
    fn fast_forwards_br2_post_vs_code_patch_chunk_when_remaining_exceeds_ram() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_2000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let table_base = 0x802c_c100;
        let start_index = 0u32;
        let limit = BR2_POST_VS_TABLE_ACCUM_MAX_SKIP_ITERATIONS + 1;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, table_base);
        bus.write_u32(table_base, 0x27bd_ffe0);
        bus.write_u32(table_base + 4, 0x3c04_1f80);

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
            u64::from(BR2_POST_VS_TABLE_ACCUM_MAX_CHARGED_NOOP_ITERATIONS)
                * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], limit);
        assert_eq!(bus.read_u32(table_base), 0x27bd_ffe0);
        assert_eq!(bus.read_u32(table_base + 4), 0x3c04_1f80);
    }

    #[test]
    fn fast_forwards_br2_post_vs_unaligned_ram_table_without_writes() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_2000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let table_base = 0x802c_c13f;
        let start_index = 0u32;
        let limit = BR2_POST_VS_TABLE_ACCUM_MIN_SKIP_ITERATIONS;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, table_base);
        for offset in 0..8 {
            bus.write_u8(table_base + offset, 0xa0 + offset as u8);
        }

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
            u64::from(limit) * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], limit);
        for offset in 0..8 {
            assert_eq!(bus.read_u8(table_base + offset), 0xa0 + offset as u8);
        }
    }

    #[test]
    fn fast_forwards_short_br2_post_vs_unaligned_ram_tail_without_writes() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let table_base = 0x802c_c13b;
        let start_index = 1u32;
        let limit = 3u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.write_u32(count_address + 4, table_base);
        for offset in 0..16 {
            bus.write_u8(table_base + offset, 0xc0 + offset as u8);
        }

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
            u64::from(limit - start_index) * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], limit);
        for offset in 0..16 {
            assert_eq!(bus.read_u8(table_base + offset), 0xc0 + offset as u8);
        }
    }

    #[test]
    fn fast_forwards_br2_post_vs_inner_unaligned_load_without_exception_or_writes() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let target = 0x8002_ff77;
        let start_index = 42u32;
        let limit = start_index + 3;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        for offset in 0..16 {
            bus.write_u8(target + offset, 0xd0 + offset as u8);
        }

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x0c;
        cpu.next_pc = cpu.pc + 4;
        cpu.regs[3] = target;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_index;
        cpu.regs[6] = 0x10;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x0c);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], count_address);
        assert_eq!(cpu.regs[3], table_meta_offset);
        assert_eq!(cpu.regs[5], limit);
        for offset in 0..16 {
            assert_eq!(bus.read_u8(target + offset), 0xd0 + offset as u8);
        }
    }

    #[test]
    fn fast_forwards_br2_post_vs_inner_unmapped_low_memory_load_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let target = 0x0044_dd12;
        let start_index = 1024u32;
        let limit = start_index + 5;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x0c;
        cpu.next_pc = cpu.pc + 4;
        cpu.regs[3] = target;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_index;
        cpu.regs[6] = 0x10;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x0c);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], count_address);
        assert_eq!(cpu.regs[3], table_meta_offset);
        assert_eq!(cpu.regs[5], limit);
    }

    #[test]
    fn caps_br2_post_vs_inner_unaligned_load_noop_cycles() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let target = 0x8300_0001;
        let start_index = 0u32;
        let limit = BR2_POST_VS_TABLE_ACCUM_MAX_SKIP_ITERATIONS + 1;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x0c;
        cpu.next_pc = cpu.pc + 4;
        cpu.regs[3] = target;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_index;
        cpu.regs[6] = 0x10;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x0c);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(BR2_POST_VS_TABLE_ACCUM_MAX_CHARGED_NOOP_ITERATIONS)
                * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], count_address);
        assert_eq!(cpu.regs[3], table_meta_offset);
        assert_eq!(cpu.regs[5], limit);
    }

    #[test]
    fn fast_forwards_br2_post_vs_inner_unmapped_peripheral_gap_load_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let target = 0x7f89_7765;
        let start_index = 4096u32;
        let limit = start_index + 7;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x0c;
        cpu.next_pc = cpu.pc + 4;
        cpu.regs[3] = target;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_index;
        cpu.regs[6] = 0x10;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x0c);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], count_address);
        assert_eq!(cpu.regs[3], table_meta_offset);
        assert_eq!(cpu.regs[5], limit);
    }

    #[test]
    fn fast_forwards_br2_post_vs_inner_unmapped_high_peripheral_gap_load_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let target = 0xbffe_dfee;
        let start_index = 4096u32;
        let limit = start_index + 7;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x0c;
        cpu.next_pc = cpu.pc + 4;
        cpu.regs[3] = target;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_index;
        cpu.regs[6] = 0x10;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x0c);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], count_address);
        assert_eq!(cpu.regs[3], table_meta_offset);
        assert_eq!(cpu.regs[5], limit);
    }

    #[test]
    fn fast_forwards_br2_post_vs_inner_bios_rom_alias_load_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let target = 0x3fc0_0def;
        let start_index = 4096u32;
        let limit = start_index + 7;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x0c;
        cpu.next_pc = cpu.pc + 4;
        cpu.regs[3] = target;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_index;
        cpu.regs[6] = 0x10;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x0c);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], count_address);
        assert_eq!(cpu.regs[3], table_meta_offset);
        assert_eq!(cpu.regs[5], limit);
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
    fn hle_returns_from_br2_post_vs_bios_irq_handler_for_loop_inner_epc() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_bios_c80_kernel_handler_prefix(&mut bus);
        bus.io.irq.status = 1;
        bus.io.irq.mask = 9;

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_0c94;
        cpu.next_pc = 0x0000_0c98;
        cpu.cp0[CP0_STATUS] = 0x4000_0404;
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2;
        cpu.cp0[CP0_EPC] = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x28;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_0c94);
        assert_eq!(report.instruction, None);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x28);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x2c);
        assert_eq!(cpu.cp0[CP0_STATUS], 0x4000_0401);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, 0);
        assert_eq!(bus.io.irq.status & 9, 0);
    }

    #[test]
    fn hle_returns_from_blank_c80_bios_irq_handler_after_vector_entry() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        for (index, instruction) in BIOS_EXCEPTION_VECTOR_TO_C80_STUB
            .iter()
            .copied()
            .enumerate()
        {
            bus.write_u32(EXCEPTION_VECTOR + (index as u32) * 4, instruction);
        }
        bus.io.irq.status = 1;
        bus.io.irq.mask = 1;

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_0c80;
        cpu.next_pc = 0x0000_0c84;
        cpu.cp0[CP0_STATUS] = 0x4000_0410;
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2;
        cpu.cp0[CP0_EPC] = 0x802d_081c;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_0c80);
        assert_eq!(report.instruction, None);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, 0x802d_081c);
        assert_eq!(cpu.next_pc, 0x802d_0820);
        assert_eq!(cpu.cp0[CP0_STATUS], 0x4000_0404);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, 0);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn hle_returns_from_br2_draw_sync_c80_kernel_irq_handler() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_bios_c80_kernel_handler_prefix(&mut bus);
        bus.io.irq.status = 1;
        bus.io.irq.mask = 9;

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_0c80;
        cpu.next_pc = 0x0000_0c84;
        cpu.cp0[CP0_STATUS] = 0x4000_0404;
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2;
        cpu.cp0[CP0_EPC] = BR2_DRAW_SYNC_WAIT_LOOP_EXIT;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_0c80);
        assert_eq!(report.instruction, None);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, BR2_DRAW_SYNC_WAIT_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_DRAW_SYNC_WAIT_LOOP_EXIT + 4);
        assert_eq!(cpu.cp0[CP0_STATUS], 0x4000_0401);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, 0);
        assert_eq!(bus.io.irq.status & 9, 0);
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
    fn hle_returns_from_br2_runtime_bios_irq_dispatch_loop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_bios_irq_dispatch_loop_signature(&mut bus);
        install_bios_exception_context(&mut bus, 0x803f_fef0, 0x8035_dbec);
        bus.io.irq.status = 1;
        bus.io.irq.mask = 1;

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_1b80;
        cpu.next_pc = 0x0000_1b84;
        cpu.regs[16] = 0x0000_1234;
        cpu.regs[18] = 0x0000_5678;
        cpu.regs[29] = 0x0000_8b30;
        cpu.regs[31] = 0x0000_18d0;
        cpu.hi = 0xaaaa_aaaa;
        cpu.lo = 0xbbbb_bbbb;
        cpu.cp0[CP0_STATUS] = 0x4000_0404;
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2;
        cpu.cp0[CP0_EPC] = 0x8035_db48;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_1b80);
        assert_eq!(report.instruction, None);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, 0x8035_db48);
        assert_eq!(cpu.next_pc, 0x8035_db4c);
        assert_eq!(cpu.cp0[CP0_STATUS], 0x4000_0401);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, 0);
        assert_eq!(cpu.regs[16], 0x1111_0000);
        assert_eq!(cpu.regs[18], 0x2222_0000);
        assert_eq!(cpu.regs[29], 0x803f_fef0);
        assert_eq!(cpu.regs[31], 0x8035_dbec);
        assert_eq!(cpu.lo, 0x3333_0000);
        assert_eq!(cpu.hi, 0x4444_0000);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn hle_does_not_return_from_bios_irq_dispatch_loop_to_low_bios_alias() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_bios_irq_dispatch_loop_signature(&mut bus);
        install_bios_exception_context(&mut bus, 0x803f_fef0, 0x8035_dbec);
        bus.io.irq.status = 1;
        bus.io.irq.mask = 1;

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_1b80;
        cpu.next_pc = 0x0000_1b84;
        cpu.cp0[CP0_STATUS] = 0x4000_0404;
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2;
        cpu.cp0[CP0_EPC] = 0x8000_1e6c;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_1b80);
        assert_eq!(report.instruction, Some(0x0000_0000));
        assert_eq!(cpu.pc, 0x0000_1b84);
        assert_eq!(cpu.next_pc, 0x0000_1b88);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, CAUSE_IP2);
        assert_eq!(bus.io.irq.status & 1, 1);
    }

    #[test]
    fn hle_returns_from_br2_low_bios_irq_handler() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.io.irq.status = 1;
        bus.io.irq.mask = 1;

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_05cc;
        cpu.next_pc = 0x0000_05d0;
        cpu.cp0[CP0_STATUS] = 0x4000_0410;
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2;
        cpu.cp0[CP0_EPC] = 0x8030_8374;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_05cc);
        assert_eq!(report.instruction, None);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, 0x8030_8374);
        assert_eq!(cpu.next_pc, 0x8030_8378);
        assert_eq!(cpu.cp0[CP0_STATUS], 0x4000_0404);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, 0);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn hle_returns_from_br2_kseg0_low_bios_irq_prologue() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.io.irq.status = 1;
        bus.io.irq.mask = 1;
        bus.write_u32(0x8000_0470, 0x3fc1_61d0);

        let mut cpu = Cpu::default();
        cpu.pc = 0x8000_0470;
        cpu.next_pc = 0x8000_0474;
        cpu.cp0[CP0_STATUS] = 0x4000_0410;
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2;
        cpu.cp0[CP0_EPC] = 0x8035_6f00;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x8000_0470);
        assert_eq!(report.instruction, None);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, 0x8035_6f00);
        assert_eq!(cpu.next_pc, 0x8035_6f04);
        assert_eq!(cpu.cp0[CP0_STATUS], 0x4000_0404);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, 0);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn hle_returns_from_br2_kseg0_low_bios_irq_entry() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.io.irq.status = 1;
        bus.io.irq.mask = 1;
        bus.write_u32(0x8000_0420, 0x3fc1_b148);

        let mut cpu = Cpu::default();
        cpu.pc = 0x8000_0420;
        cpu.next_pc = 0x8000_0424;
        cpu.cp0[CP0_STATUS] = 0x4000_0410;
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2;
        cpu.cp0[CP0_EPC] = 0x8035_6ec4;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x8000_0420);
        assert_eq!(report.instruction, None);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, 0x8035_6ec4);
        assert_eq!(cpu.next_pc, 0x8035_6ec8);
        assert_eq!(cpu.cp0[CP0_STATUS], 0x4000_0404);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, 0);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn hle_does_not_return_from_br2_low_bios_irq_handler_after_ip2_clear() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.io.irq.status = 0;
        bus.io.irq.mask = 1;
        bus.write_u32(0x0000_05e4, 0);

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_05e4;
        cpu.next_pc = 0x0000_05e8;
        cpu.cp0[CP0_STATUS] = 0x4000_0404;
        cpu.cp0[CP0_CAUSE] = 0;
        cpu.cp0[CP0_EPC] = 0x8034_c94c;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_05e4);
        assert_eq!(report.instruction, Some(0));
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, 0x0000_05e8);
        assert_eq!(cpu.next_pc, 0x0000_05ec);
        assert_eq!(cpu.cp0[CP0_STATUS], 0x4000_0404);
        assert_eq!(cpu.cp0[CP0_EPC], 0x8034_c94c);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, 0);
    }

    #[test]
    fn hle_returns_from_br2_virtual_low_bios_irq_vector() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.io.irq.status = 1;
        bus.io.irq.mask = 1;

        let mut cpu = Cpu::default();
        cpu.pc = EXCEPTION_VECTOR;
        cpu.next_pc = EXCEPTION_VECTOR + 4;
        cpu.cp0[CP0_STATUS] = 0x4000_0404;
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2;
        cpu.cp0[CP0_EPC] = 0x8035_6f1c;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, EXCEPTION_VECTOR);
        assert_eq!(report.instruction, None);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(cpu.pc, 0x8035_6f1c);
        assert_eq!(cpu.next_pc, 0x8035_6f20);
        assert_eq!(cpu.cp0[CP0_STATUS], 0x4000_0401);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, 0);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn hle_irq_return_preserves_interrupted_epilogue_delayed_load() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.io.irq.status = 1;
        bus.io.irq.mask = 1;
        bus.write_u32(0x8034_2c1c, 0x8fb1_0014); // lw s1,0x14(sp)
        bus.write_u32(0x803f_ff24, 0x1111_2222);

        let mut cpu = Cpu::default();
        cpu.pc = EXCEPTION_VECTOR;
        cpu.next_pc = EXCEPTION_VECTOR + 4;
        cpu.regs[29] = 0x803f_ff10;
        cpu.regs[31] = 0x8034_2b8c;
        cpu.pending_load = Some((31, 0x1f80_0000));
        cpu.cp0[CP0_STATUS] = 0x4000_0404;
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2;
        cpu.cp0[CP0_EPC] = 0x8034_2c1c;

        let irq_report = cpu.step_report(&mut bus);

        assert_eq!(irq_report.start_pc, EXCEPTION_VECTOR);
        assert_eq!(irq_report.instruction, None);
        assert_eq!(cpu.pc, 0x8034_2c1c);
        assert_eq!(cpu.next_pc, 0x8034_2c20);
        assert_eq!(cpu.pending_load, Some((31, 0x1f80_0000)));
        assert_eq!(cpu.regs[31], 0x8034_2b8c);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, 0);

        let load_report = cpu.step_report(&mut bus);

        assert_eq!(load_report.start_pc, 0x8034_2c1c);
        assert_eq!(load_report.instruction, Some(0x8fb1_0014));
        assert_eq!(cpu.regs[31], 0x1f80_0000);
        assert_eq!(cpu.pending_load, Some((17, 0x1111_2222)));
    }

    #[test]
    fn hle_does_not_return_from_low_bios_irq_vector_to_low_bios_epc() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.io.irq.status = 1;
        bus.io.irq.mask = 1;

        let mut cpu = Cpu::default();
        cpu.pc = EXCEPTION_VECTOR;
        cpu.next_pc = EXCEPTION_VECTOR + 4;
        cpu.cp0[CP0_STATUS] = 0x4000_0404;
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2;
        cpu.cp0[CP0_EPC] = 0x8000_1e6c;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, EXCEPTION_VECTOR);
        assert_eq!(report.instruction, Some(0));
        assert_eq!(cpu.pc, EXCEPTION_VECTOR + 4);
        assert_eq!(cpu.next_pc, EXCEPTION_VECTOR + 8);
        assert_eq!(cpu.cp0[CP0_EPC], 0x8000_1e6c);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, CAUSE_IP2);
        assert_eq!(bus.io.irq.status & 1, 1);
    }

    #[test]
    fn hle_delivers_br2_bios_b0_wait_event_loop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_bios_b0_wait_event_signature(&mut bus);
        let event_base = 0x8000_e200;
        bus.write_u32(event_base + 4, BR2_BIOS_B0_WAIT_EVENT_ENABLED);

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_1e6c;
        cpu.next_pc = 0x0000_1e70;
        cpu.regs[2] = event_base;
        cpu.regs[31] = BR2_BIOS_B0_WAIT_EVENT_RETURN_PC;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_1e6c);
        assert_eq!(report.instruction, None);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(
            bus.read_u32(event_base + 4),
            BR2_BIOS_B0_WAIT_EVENT_DELIVERED
        );
        assert_eq!(cpu.regs[2], 1);
        assert_eq!(cpu.pc, BR2_BIOS_B0_WAIT_EVENT_RETURN_PC);
        assert_eq!(cpu.next_pc, BR2_BIOS_B0_WAIT_EVENT_RETURN_PC + 4);
    }

    #[test]
    fn hle_does_not_deliver_br2_bios_b0_wait_event_with_wrong_return_pc() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_bios_b0_wait_event_signature(&mut bus);
        let event_base = 0x8000_e200;
        bus.write_u32(event_base + 4, BR2_BIOS_B0_WAIT_EVENT_ENABLED);

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_1e6c;
        cpu.next_pc = 0x0000_1e70;
        cpu.regs[2] = event_base;
        cpu.regs[31] = 0x8034_c978;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_1e6c);
        assert_eq!(report.instruction, Some(0x8c49_0004));
        assert_eq!(bus.read_u32(event_base + 4), BR2_BIOS_B0_WAIT_EVENT_ENABLED);
        assert_eq!(cpu.pc, 0x0000_1e70);
        assert_eq!(cpu.next_pc, 0x0000_1e74);
    }

    #[test]
    fn hle_does_not_deliver_br2_bios_b0_wait_event_with_wrong_signature() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let event_base = 0x8000_e200;
        bus.write_u32(event_base + 4, BR2_BIOS_B0_WAIT_EVENT_ENABLED);
        bus.write_u32(0x0000_1e6c, 0x8c49_0004);

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_1e6c;
        cpu.next_pc = 0x0000_1e70;
        cpu.regs[2] = event_base;
        cpu.regs[31] = BR2_BIOS_B0_WAIT_EVENT_RETURN_PC;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_1e6c);
        assert_eq!(report.instruction, Some(0x8c49_0004));
        assert_eq!(bus.read_u32(event_base + 4), BR2_BIOS_B0_WAIT_EVENT_ENABLED);
        assert_eq!(cpu.pc, 0x0000_1e70);
        assert_eq!(cpu.next_pc, 0x0000_1e74);
    }

    #[test]
    fn hle_delivers_br2_bios_b0_test_event_loop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_bios_b0_test_event_signature(&mut bus);
        let event_base = 0x8000_e200;
        bus.write_u32(BR2_BIOS_EVENT_TABLE_POINTER_PHYSICAL, event_base);
        bus.write_u32(event_base + 4, BR2_BIOS_B0_WAIT_EVENT_ENABLED);

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_1e8c;
        cpu.next_pc = 0x0000_1e90;
        cpu.regs[4] = 0;
        cpu.regs[31] = BR2_BIOS_B0_TEST_EVENT_RETURN_PC;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_1e8c);
        assert_eq!(report.instruction, None);
        assert_eq!(report.cycles_elapsed, 1);
        assert_eq!(
            bus.read_u32(event_base + 4),
            BR2_BIOS_B0_WAIT_EVENT_DELIVERED
        );
        assert_eq!(cpu.regs[2], 1);
        assert_eq!(cpu.pc, BR2_BIOS_B0_TEST_EVENT_RETURN_PC);
        assert_eq!(cpu.next_pc, BR2_BIOS_B0_TEST_EVENT_RETURN_PC + 4);
    }

    #[test]
    fn hle_does_not_deliver_br2_bios_b0_test_event_with_wrong_return_pc() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_bios_b0_test_event_signature(&mut bus);
        let event_base = 0x8000_e200;
        bus.write_u32(BR2_BIOS_EVENT_TABLE_POINTER_PHYSICAL, event_base);
        bus.write_u32(event_base + 4, BR2_BIOS_B0_WAIT_EVENT_ENABLED);

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_1e8c;
        cpu.next_pc = 0x0000_1e90;
        cpu.regs[4] = 0;
        cpu.regs[31] = 0x8034_cf80;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_1e8c);
        assert_eq!(report.instruction, Some(0x3084_ffff));
        assert_eq!(bus.read_u32(event_base + 4), BR2_BIOS_B0_WAIT_EVENT_ENABLED);
        assert_eq!(cpu.pc, 0x0000_1e90);
        assert_eq!(cpu.next_pc, 0x0000_1e94);
    }

    #[test]
    fn hle_does_not_deliver_br2_bios_b0_test_event_with_wrong_signature() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let event_base = 0x8000_e200;
        bus.write_u32(BR2_BIOS_EVENT_TABLE_POINTER_PHYSICAL, event_base);
        bus.write_u32(event_base + 4, BR2_BIOS_B0_WAIT_EVENT_ENABLED);
        bus.write_u32(0x0000_1e8c, 0x3084_ffff);

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_1e8c;
        cpu.next_pc = 0x0000_1e90;
        cpu.regs[4] = 0;
        cpu.regs[31] = BR2_BIOS_B0_TEST_EVENT_RETURN_PC;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_1e8c);
        assert_eq!(report.instruction, Some(0x3084_ffff));
        assert_eq!(bus.read_u32(event_base + 4), BR2_BIOS_B0_WAIT_EVENT_ENABLED);
        assert_eq!(cpu.pc, 0x0000_1e90);
        assert_eq!(cpu.next_pc, 0x0000_1e94);
    }

    #[test]
    fn hle_br2_bios_b0_return_only_dispatch_returns_to_runtime_ra() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_00b0;
        cpu.next_pc = 0x0000_00b4;
        cpu.regs[9] = 0x35;
        cpu.regs[31] = 0x8033_e2ac;
        cpu.pending_load = Some((2, 0xdead_beef));

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_00b0);
        assert_eq!(report.instruction, None);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(cpu.pc, 0x8033_e2ac);
        assert_eq!(cpu.next_pc, 0x8033_e2b0);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.pending_load, None);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_EXCODE_MASK, 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
    }

    #[test]
    fn hle_br2_bios_b0_reset_entry_int_dispatch_returns_to_runtime_ra() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(0x0000_0f20, 0x0082_0029);

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_00b0;
        cpu.next_pc = 0x0000_00b4;
        cpu.regs[9] = BR2_BIOS_B0_RESET_ENTRY_INT_FUNCTION;
        cpu.regs[31] = 0x8034_a648;
        cpu.pending_load = Some((2, 0xdead_beef));
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2 | (4 << 2);
        cpu.cp0[CP0_EPC] = 0x8033_7800;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_00b0);
        assert_eq!(report.instruction, None);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(cpu.pc, 0x8034_a648);
        assert_eq!(cpu.next_pc, 0x8034_a64c);
        assert_eq!(cpu.regs[2], 0);
        assert_eq!(cpu.pending_load, None);
        assert_eq!(cpu.cp0[CP0_EPC], 0x8033_7800);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, CAUSE_IP2);
    }

    #[test]
    fn hle_br2_bios_b0_wait_event_dispatch_returns_to_runtime_ra() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let event_base = 0x8000_e200;
        bus.write_u32(BR2_BIOS_EVENT_TABLE_POINTER_PHYSICAL, event_base);
        bus.write_u32(event_base + 4, BR2_BIOS_B0_WAIT_EVENT_ENABLED);

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_00b0;
        cpu.next_pc = 0x0000_00b4;
        cpu.regs[4] = 0xf100_0000;
        cpu.regs[9] = 0x0a;
        cpu.regs[31] = BR2_BIOS_B0_WAIT_EVENT_RETURN_PC;
        cpu.pending_load = Some((2, 0xdead_beef));

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_00b0);
        assert_eq!(report.instruction, None);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(
            bus.read_u32(event_base + 4),
            BR2_BIOS_B0_WAIT_EVENT_DELIVERED
        );
        assert_eq!(cpu.pc, BR2_BIOS_B0_WAIT_EVENT_RETURN_PC);
        assert_eq!(cpu.next_pc, BR2_BIOS_B0_WAIT_EVENT_RETURN_PC + 4);
        assert_eq!(cpu.regs[2], 1);
        assert_eq!(cpu.pending_load, None);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_EXCODE_MASK, 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
    }

    #[test]
    fn hle_br2_bios_b0_dispatch_wins_over_low_irq_vector_alias() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let event_base = 0x8000_e200;
        bus.write_u32(BR2_BIOS_EVENT_TABLE_POINTER_PHYSICAL, event_base);
        bus.write_u32(event_base + 4, BR2_BIOS_B0_WAIT_EVENT_ENABLED);
        bus.io.irq.status = 1;
        bus.io.irq.mask = 1;

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_00b0;
        cpu.next_pc = 0x0000_00b4;
        cpu.regs[4] = 0xf100_0000;
        cpu.regs[9] = 0x0a;
        cpu.regs[31] = BR2_BIOS_B0_WAIT_EVENT_RETURN_PC;
        cpu.cp0[CP0_STATUS] = 0x4000_0410;
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2 | (4 << 2);
        cpu.cp0[CP0_EPC] = 0x8033_7800;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_00b0);
        assert_eq!(report.instruction, None);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(
            bus.read_u32(event_base + 4),
            BR2_BIOS_B0_WAIT_EVENT_DELIVERED
        );
        assert_eq!(cpu.pc, BR2_BIOS_B0_WAIT_EVENT_RETURN_PC);
        assert_eq!(cpu.next_pc, BR2_BIOS_B0_WAIT_EVENT_RETURN_PC + 4);
        assert_eq!(cpu.regs[2], 1);
    }

    #[test]
    fn hle_br2_bios_b0_pending_call_is_not_low_irq_return() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let event_base = 0x8000_e200;
        bus.write_u32(BR2_BIOS_EVENT_TABLE_POINTER_PHYSICAL, event_base);
        bus.write_u32(event_base + 4, 0);
        bus.write_u32(0x0000_00b0, i_type(0x09, 0, 2, 7));
        bus.io.irq.status = 1;
        bus.io.irq.mask = 1;

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_00b0;
        cpu.next_pc = 0x0000_00b4;
        cpu.regs[4] = 0xf100_0000;
        cpu.regs[9] = 0x0a;
        cpu.regs[31] = BR2_BIOS_B0_WAIT_EVENT_RETURN_PC;
        cpu.cp0[CP0_STATUS] = 0x4000_0410;
        cpu.cp0[CP0_CAUSE] = CAUSE_IP2 | (4 << 2);
        cpu.cp0[CP0_EPC] = 0x8033_7800;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_00b0);
        assert_eq!(report.instruction, Some(i_type(0x09, 0, 2, 7)));
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(cpu.pc, 0x0000_00b4);
        assert_eq!(cpu.next_pc, 0x0000_00b8);
        assert_eq!(cpu.regs[2], 7);
        assert_eq!(cpu.cp0[CP0_EPC], 0x8033_7800);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_IP2, CAUSE_IP2);
    }

    #[test]
    fn hle_br2_bios_b0_wait_event_dispatch_requires_event_status() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let event_base = 0x8000_e200;
        bus.write_u32(BR2_BIOS_EVENT_TABLE_POINTER_PHYSICAL, event_base);
        bus.write_u32(event_base + 4, 0);
        bus.write_u32(0x0000_00b0, i_type(0x09, 0, 2, 7));

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_00b0;
        cpu.next_pc = 0x0000_00b4;
        cpu.regs[4] = 0xf100_0000;
        cpu.regs[9] = 0x0a;
        cpu.regs[31] = BR2_BIOS_B0_WAIT_EVENT_RETURN_PC;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_00b0);
        assert_eq!(report.instruction, Some(i_type(0x09, 0, 2, 7)));
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(bus.read_u32(event_base + 4), 0);
        assert_eq!(cpu.pc, 0x0000_00b4);
        assert_eq!(cpu.next_pc, 0x0000_00b8);
        assert_eq!(cpu.regs[2], 7);
    }

    #[test]
    fn hle_br2_bios_b0_dispatch_does_not_intercept_unknown_functions() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(0x0000_00b0, i_type(0x09, 0, 2, 7));

        let mut cpu = Cpu::default();
        cpu.pc = 0x0000_00b0;
        cpu.next_pc = 0x0000_00b4;
        cpu.regs[9] = 0x34;
        cpu.regs[31] = 0x8033_e2ac;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x0000_00b0);
        assert_eq!(report.instruction, Some(i_type(0x09, 0, 2, 7)));
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(cpu.pc, 0x0000_00b4);
        assert_eq!(cpu.next_pc, 0x0000_00b8);
        assert_eq!(cpu.regs[2], 7);
    }

    #[test]
    fn rfe_status_pops_all_interrupt_mode_bits() {
        assert_eq!(rfe_status(0x4000_0410), 0x4000_0404);
        assert_eq!(rfe_status(0x4000_0404), 0x4000_0401);
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
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[5], limit);
        assert_eq!(bus.vblank_count(), 0);
        assert_eq!(bus.io.irq.status & 1, 0);
    }

    #[test]
    fn caps_br2_post_vs_unaligned_load_hle_before_vblank_irq() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        install_br2_post_vs_table_accum_loop(&mut bus);
        let owner = 0x8001_0000;
        let table_meta_offset = 0x0002_0338;
        let count_address = 0x0002_0348;
        let target = 0x8300_0001;
        let start_index = 0u32;
        let limit = 0x0303_0303u32;
        bus.write_u32(owner + 0x7c, table_meta_offset);
        bus.write_u32(count_address, limit);
        bus.io.irq.mask = 1;
        bus.tick(550_000);
        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let expected_iterations =
            ((cycles_until_vblank - 1) / BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION) as u32;

        let mut cpu = Cpu::default();
        cpu.pc = BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x0c;
        cpu.next_pc = cpu.pc + 4;
        cpu.cp0[CP0_STATUS] = 1 | CAUSE_IP2;
        cpu.regs[3] = target;
        cpu.regs[4] = owner;
        cpu.regs[5] = start_index;
        cpu.regs[6] = 0x10;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_START + 0x0c);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(expected_iterations) * BR2_POST_VS_TABLE_ACCUM_CYCLES_PER_ITERATION
        );
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT);
        assert_eq!(cpu.next_pc, BR2_POST_VS_TABLE_ACCUM_LOOP_EXIT + 4);
        assert_eq!(cpu.regs[2], count_address);
        assert_eq!(cpu.regs[3], table_meta_offset);
        assert_eq!(cpu.regs[5], limit);
        assert_eq!(bus.vblank_count(), 0);
        assert_eq!(bus.io.irq.status & 1, 0);
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
    fn caps_br2_boot_word_copy_loop_before_host_visible_vblank() {
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
        for index in 0..128u32 {
            bus.write_u32(0x8000_1000 + index * 4, 0x1000_0000 | index);
        }
        bus.io.irq.mask = 1;
        bus.tick(565_000);

        let cycles_until_vblank = bus.cycles_until_next_vblank();
        let expected_words = ((cycles_until_vblank - 1) / WORD_COPY_LOOP_CYCLES_PER_WORD) as u32;
        assert!(expected_words > 0);
        assert!(expected_words < 128);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_BOOT_WORD_COPY_LOOP_START;
        cpu.next_pc = BR2_BOOT_WORD_COPY_LOOP_START + 4;
        cpu.regs[4] = 0x8000_1000;
        cpu.regs[5] = 0x8000_2000;
        cpu.regs[6] = 512;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, BR2_BOOT_WORD_COPY_LOOP_START);
        assert_eq!(
            report.cycles_elapsed,
            u64::from(expected_words) * WORD_COPY_LOOP_CYCLES_PER_WORD
        );
        assert_eq!(cpu.pc, BR2_BOOT_WORD_COPY_LOOP_START);
        assert_eq!(cpu.next_pc, BR2_BOOT_WORD_COPY_LOOP_START + 4);
        assert_eq!(cpu.regs[4], 0x8000_1000 + expected_words * 4);
        assert_eq!(cpu.regs[5], 0x8000_2000 + expected_words * 4);
        assert_eq!(cpu.regs[6], 512 - expected_words * 4);
        assert_eq!(cpu.regs[7], 0x1000_0000 | (expected_words - 1));
        assert_eq!(
            bus.read_u32(0x8000_2000 + (expected_words - 1) * 4),
            0x1000_0000 | (expected_words - 1)
        );
        assert_eq!(bus.read_u32(0x8000_2000 + expected_words * 4), 0);
        assert_eq!(bus.vblank_count(), 0);
        assert_eq!(bus.io.irq.status & 1, 0);
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
            "{\"pc\":2147483776,\"next_pc\":2147483780,\"cycles\":4,\"halted\":true,\"status\":0,\"cause\":36,\"epc\":532676620,\"badvaddr\":0,\"r2\":42,\"r3\":0,\"r4\":7,\"r5\":0,\"r6\":0,\"r8\":0,\"r9\":0,\"r10\":0,\"r11\":0,\"r16\":0,\"r17\":0,\"r18\":0,\"r19\":0,\"r20\":0,\"r21\":0,\"r22\":0,\"r23\":0,\"r29\":0,\"r31\":0,\"gte_command_counts\":[]}"
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
    fn executes_cache_opcode_as_noop() {
        let rom = program(&[
            0xbc03_803b,                // cache 3, -0x7fc5(zero)
            i_type(0x09, 0, 2, 0x002a), // addiu v0, zero, 42
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        let cache = cpu.step_report(&mut bus);
        let next = cpu.step_report(&mut bus);

        assert_eq!(cache.outcome, StepOutcome::Continue);
        assert_eq!(cache.instruction, Some(0xbc03_803b));
        assert_eq!(cpu.regs[2], 42);
        assert_eq!(next.outcome, StepOutcome::Continue);
    }

    #[test]
    fn ignores_cop1_memory_opcodes_without_fpu_side_effects() {
        let rom = program(&[
            0xd702_0000,                // ldc1 f2, 0(t8)
            0xe702_0000,                // swc1 f2, 0(t8)
            i_type(0x09, 0, 2, 0x002a), // addiu v0, zero, 42
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();
        cpu.regs[24] = 4;

        let load = cpu.step_report(&mut bus);
        let store = cpu.step_report(&mut bus);
        let next = cpu.step_report(&mut bus);

        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.instruction, Some(0xd702_0000));
        assert_eq!(store.outcome, StepOutcome::Continue);
        assert_eq!(store.instruction, Some(0xe702_0000));
        assert_eq!(next.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[2], 42);
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
    fn traps_unaligned_word_load_with_bad_vaddr() {
        let rom = program(&[
            i_type(0x23, 0, 8, 1),    // lw t0, 1(zero)
            r_type(0, 0, 0, 0, 0x0d), // break
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);

        assert_eq!(cpu.regs[8], 0);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 1);
        assert_eq!(cpu.cp0[CP0_CAUSE], 4 << 2);
        assert_eq!(cpu.cp0[CP0_EPC], 0x1fc0_0000);
        assert_eq!(cpu.pc, EXCEPTION_VECTOR);
    }

    #[test]
    fn hle_br2_runtime_unaligned_word_load_reads_ram_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_WORD_LOAD_PC,
            i_type(0x23, 16, 8, 0), // lw t0, 0(s0)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_WORD_LOAD_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );
        bus.write_u8(0x8009_0092, 0x11);
        bus.write_u8(0x8009_0093, 0x22);
        bus.write_u8(0x8009_0094, 0x33);
        bus.write_u8(0x8009_0095, 0x44);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_WORD_LOAD_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_WORD_LOAD_PC + 4;
        cpu.regs[16] = 0x8009_0092;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, BR2_RUNTIME_UNALIGNED_WORD_LOAD_PC);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[8], 0);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[8], 0x4433_2211);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_WORD_LOAD_PC + 8);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_UNALIGNED_WORD_LOAD_PC + 12);
    }

    #[test]
    fn hle_br2_runtime_unaligned_render_source_word_load_reads_ram_mirror_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_RENDER_SOURCE_WORD_LOAD_PC,
            i_type(0x23, 4, 8, 0), // lw t0, 0(a0)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_RENDER_SOURCE_WORD_LOAD_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );
        bus.write_u8(0xe00d_f3f9, 0x11);
        bus.write_u8(0xe00d_f3fa, 0x22);
        bus.write_u8(0xe00d_f3fb, 0x33);
        bus.write_u8(0xe00d_f3fc, 0x44);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_RENDER_SOURCE_WORD_LOAD_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_RENDER_SOURCE_WORD_LOAD_PC + 4;
        cpu.regs[4] = 0xe00d_f3f9;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(
            load.start_pc,
            BR2_RUNTIME_UNALIGNED_RENDER_SOURCE_WORD_LOAD_PC
        );
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[8], 0);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[8], 0x4433_2211);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_RENDER_SOURCE_WORD_LOAD_PC + 8);
        assert_eq!(
            cpu.next_pc,
            BR2_RUNTIME_UNALIGNED_RENDER_SOURCE_WORD_LOAD_PC + 12
        );
    }

    #[test]
    fn hle_br2_runtime_unaligned_render_source_tail_word_load_reads_ram_mirror_without_exception() {
        let tail_pc = BR2_RUNTIME_UNALIGNED_RENDER_SOURCE_WORD_LOAD_PC + 8;
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(tail_pc, i_type(0x23, 4, 10, 8)); // lw t2, 8(a0)
        bus.write_u32(tail_pc + 4, r_type(0, 0, 0, 0, 0x00));
        bus.write_u8(0xe00d_f401, 0xaa);
        bus.write_u8(0xe00d_f402, 0xbb);
        bus.write_u8(0xe00d_f403, 0xcc);
        bus.write_u8(0xe00d_f404, 0xdd);

        let mut cpu = Cpu::default();
        cpu.pc = tail_pc;
        cpu.next_pc = tail_pc + 4;
        cpu.regs[4] = 0xe00d_f3f9;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, tail_pc);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[10], 0);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[10], 0xddcc_bbaa);
        assert_eq!(cpu.pc, tail_pc + 8);
        assert_eq!(cpu.next_pc, tail_pc + 12);
    }

    #[test]
    fn hle_br2_runtime_unaligned_render_source_fourth_word_load_reads_ram_mirror_without_exception()
    {
        let tail_pc = BR2_RUNTIME_UNALIGNED_RENDER_SOURCE_WORD_LOAD_PC + 12;
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(tail_pc, i_type(0x23, 4, 11, 12)); // lw t3, 12(a0)
        bus.write_u32(tail_pc + 4, r_type(0, 0, 0, 0, 0x00));
        bus.write_u8(0xe00d_f405, 0x01);
        bus.write_u8(0xe00d_f406, 0x23);
        bus.write_u8(0xe00d_f407, 0x45);
        bus.write_u8(0xe00d_f408, 0x67);

        let mut cpu = Cpu::default();
        cpu.pc = tail_pc;
        cpu.next_pc = tail_pc + 4;
        cpu.regs[4] = 0xe00d_f3f9;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, tail_pc);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[11], 0);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[11], 0x6745_2301);
        assert_eq!(cpu.pc, tail_pc + 8);
        assert_eq!(cpu.next_pc, tail_pc + 12);
    }

    #[test]
    fn hle_br2_runtime_unaligned_render_source_post_gte_word_load_reads_ram_mirror_without_exception()
     {
        let post_gte_pc = 0x8033_c898;
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(post_gte_pc, i_type(0x23, 4, 8, 0x14)); // lw t0, 0x14(a0)
        bus.write_u32(post_gte_pc + 4, r_type(0, 0, 0, 0, 0x00));
        bus.write_u8(0xe00d_f40d, 0x8d);
        bus.write_u8(0xe00d_f40e, 0x00);
        bus.write_u8(0xe00d_f40f, 0x1c);
        bus.write_u8(0xe00d_f410, 0x27);

        let mut cpu = Cpu::default();
        cpu.pc = post_gte_pc;
        cpu.next_pc = post_gte_pc + 4;
        cpu.regs[4] = 0xe00d_f3f9;
        cpu.regs[8] = 0xfeed_beef;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, post_gte_pc);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[8], 0xfeed_beef);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[8], 0x271c_008d);
        assert_eq!(cpu.pc, post_gte_pc + 8);
        assert_eq!(cpu.next_pc, post_gte_pc + 12);
    }

    #[test]
    fn hle_br2_runtime_unaligned_render_source_second_post_gte_word_load_reads_ram_mirror_without_exception()
     {
        let post_gte_pc = 0x8033_c89c;
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(post_gte_pc, i_type(0x23, 4, 9, 0x18)); // lw t1, 0x18(a0)
        bus.write_u32(post_gte_pc + 4, r_type(0, 0, 0, 0, 0x00));
        bus.write_u8(0xe00d_f411, 0x34);
        bus.write_u8(0xe00d_f412, 0x12);
        bus.write_u8(0xe00d_f413, 0x00);
        bus.write_u8(0xe00d_f414, 0x00);

        let mut cpu = Cpu::default();
        cpu.pc = post_gte_pc;
        cpu.next_pc = post_gte_pc + 4;
        cpu.regs[4] = 0xe00d_f3f9;
        cpu.regs[9] = 0xfeed_beef;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, post_gte_pc);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[9], 0xfeed_beef);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[9], 0x0000_1234);
        assert_eq!(cpu.pc, post_gte_pc + 8);
        assert_eq!(cpu.next_pc, post_gte_pc + 12);
    }

    #[test]
    fn hle_br2_runtime_unaligned_render_source_third_post_gte_word_load_reads_ram_mirror_without_exception()
     {
        let post_gte_pc = 0x8033_c8a0;
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(post_gte_pc, i_type(0x23, 4, 10, 0x1c)); // lw t2, 0x1c(a0)
        bus.write_u32(post_gte_pc + 4, r_type(0, 0, 0, 0, 0x00));
        bus.write_u8(0xe00d_f415, 0xdc);
        bus.write_u8(0xe00d_f416, 0xfd);
        bus.write_u8(0xe00d_f417, 0xff);
        bus.write_u8(0xe00d_f418, 0xff);

        let mut cpu = Cpu::default();
        cpu.pc = post_gte_pc;
        cpu.next_pc = post_gte_pc + 4;
        cpu.regs[4] = 0xe00d_f3f9;
        cpu.regs[10] = 0xfeed_beef;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, post_gte_pc);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[10], 0xfeed_beef);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[10], 0xffff_fddc);
        assert_eq!(cpu.pc, post_gte_pc + 8);
        assert_eq!(cpu.next_pc, post_gte_pc + 12);
    }

    #[test]
    fn hle_br2_runtime_unaligned_vertex_word_load_reads_ram_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC,
            i_type(0x23, 4, 9, 4), // lw t1, 4(a0)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC + 4,
            i_type(0x23, 4, 10, 12), // lw t2, 12(a0)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC + 8,
            r_type(0, 0, 0, 0, 0x00),
        );
        bus.write_u8(0xa02a_db36, 0x11);
        bus.write_u8(0xa02a_db37, 0x22);
        bus.write_u8(0xa02a_db38, 0x33);
        bus.write_u8(0xa02a_db39, 0x44);
        bus.write_u8(0xa02a_db3e, 0x55);
        bus.write_u8(0xa02a_db3f, 0x66);
        bus.write_u8(0xa02a_db40, 0x77);
        bus.write_u8(0xa02a_db41, 0x88);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC + 4;
        cpu.regs[4] = 0xa02a_db32;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[9], 0);
        assert_eq!(cpu.regs[10], 0);

        let second = cpu.step_report(&mut bus);
        assert_eq!(second.outcome, StepOutcome::Continue);
        assert_eq!(
            second.start_pc,
            BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC + 4
        );
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[9], 0x4433_2211);
        assert_eq!(cpu.regs[10], 0);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC + 8);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC + 12);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[9], 0x4433_2211);
        assert_eq!(cpu.regs[10], 0x8877_6655);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC + 12);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC + 16);
    }

    #[test]
    fn hle_br2_runtime_unaligned_vertex_word_load_noops_high_gap_pointer_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC,
            i_type(0x23, 4, 9, 4), // lw t1, 4(a0)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC + 4;
        cpu.regs[4] = 0xff00_8103;
        cpu.regs[9] = 0xfeed_beef;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[9], 0xfeed_beef);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[9], 0);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC + 8);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_UNALIGNED_VERTEX_WORD_LOAD_PC + 12);
    }

    #[test]
    fn hle_br2_runtime_unaligned_vertex_halfword_load_reads_ram_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC,
            i_type(0x25, 4, 8, 0), // lhu t0, 0(a0)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );
        bus.write_u8(0xc01a_e787, 0x34);
        bus.write_u8(0xc01a_e788, 0x12);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC + 4;
        cpu.regs[4] = 0xc01a_e787;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[8], 0);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[8], 0x1234);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC + 8);
        assert_eq!(
            cpu.next_pc,
            BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC + 12
        );
    }

    #[test]
    fn hle_br2_runtime_unaligned_vertex_halfword_load_noops_high_gap_pointer_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC,
            i_type(0x25, 4, 8, 0), // lhu t0, 0(a0)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC + 4;
        cpu.regs[4] = 0xff00_8103;
        cpu.regs[8] = 0xfeed_beef;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[8], 0xfeed_beef);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[8], 0);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC + 8);
        assert_eq!(
            cpu.next_pc,
            BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC + 12
        );
    }

    #[test]
    fn hle_br2_runtime_unaligned_vertex_signed_halfword_load_sign_extends_without_exception() {
        let signed_halfword_pc = BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_PC + 0x30;
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(signed_halfword_pc, i_type(0x21, 4, 10, 0x0e)); // lh t2, 0x0e(a0)
        bus.write_u32(signed_halfword_pc + 4, r_type(0, 0, 0, 0, 0x00));
        bus.write_u8(0xc01a_e795, 0xfe);
        bus.write_u8(0xc01a_e796, 0xff);

        let mut cpu = Cpu::default();
        cpu.pc = signed_halfword_pc;
        cpu.next_pc = signed_halfword_pc + 4;
        cpu.regs[4] = 0xc01a_e787;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, signed_halfword_pc);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[10], 0);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[10], 0xffff_fffe);
        assert_eq!(cpu.pc, signed_halfword_pc + 8);
        assert_eq!(cpu.next_pc, signed_halfword_pc + 12);

        let tail_pc = BR2_RUNTIME_UNALIGNED_VERTEX_HALFWORD_LOAD_END_PC;
        bus.write_u32(tail_pc, i_type(0x25, 4, 8, 4)); // lhu t0, 4(a0)
        bus.write_u32(tail_pc + 4, r_type(0, 0, 0, 0, 0x00));
        bus.write_u8(0xc01a_e78b, 0x78);
        bus.write_u8(0xc01a_e78c, 0x56);

        let mut tail_cpu = Cpu::default();
        tail_cpu.pc = tail_pc;
        tail_cpu.next_pc = tail_pc + 4;
        tail_cpu.regs[4] = 0xc01a_e787;

        let load = tail_cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, tail_pc);
        assert_eq!(tail_cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(tail_cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(tail_cpu.cp0[CP0_EPC], 0);

        let delay = tail_cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(tail_cpu.regs[8], 0x5678);
    }

    #[test]
    fn hle_br2_runtime_unaligned_vertex_second_word_load_reads_ram_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_SECOND_WORD_LOAD_PC,
            i_type(0x23, 4, 9, 8), // lw t1, 8(a0)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_SECOND_WORD_LOAD_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );
        bus.write_u8(0xa02a_db3a, 0x11);
        bus.write_u8(0xa02a_db3b, 0x22);
        bus.write_u8(0xa02a_db3c, 0x33);
        bus.write_u8(0xa02a_db3d, 0x44);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_VERTEX_SECOND_WORD_LOAD_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_VERTEX_SECOND_WORD_LOAD_PC + 4;
        cpu.regs[4] = 0xa02a_db32;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(
            load.start_pc,
            BR2_RUNTIME_UNALIGNED_VERTEX_SECOND_WORD_LOAD_PC
        );
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[9], 0);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[9], 0x4433_2211);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_VERTEX_SECOND_WORD_LOAD_PC + 8);
        assert_eq!(
            cpu.next_pc,
            BR2_RUNTIME_UNALIGNED_VERTEX_SECOND_WORD_LOAD_PC + 12
        );

        let tail_pc = BR2_RUNTIME_UNALIGNED_VERTEX_SECOND_WORD_LOAD_PC + 0x30;
        bus.write_u32(tail_pc, i_type(0x23, 4, 9, 8)); // lw t1, 8(a0)
        bus.write_u32(tail_pc + 4, r_type(0, 0, 0, 0, 0x00));

        let mut tail_cpu = Cpu::default();
        tail_cpu.pc = tail_pc;
        tail_cpu.next_pc = tail_pc + 4;
        tail_cpu.regs[4] = 0xa02a_db32;

        let load = tail_cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, tail_pc);
        assert_eq!(tail_cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(tail_cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(tail_cpu.cp0[CP0_EPC], 0);
        assert_eq!(tail_cpu.regs[9], 0);

        let delay = tail_cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(tail_cpu.regs[9], 0x4433_2211);
        assert_eq!(tail_cpu.pc, tail_pc + 8);
        assert_eq!(tail_cpu.next_pc, tail_pc + 12);

        let tail_end_pc = BR2_RUNTIME_UNALIGNED_VERTEX_SECOND_WORD_LOAD_PC + 0x34;
        bus.write_u32(tail_end_pc, i_type(0x23, 4, 10, 16)); // lw t2, 16(a0)
        bus.write_u32(tail_end_pc + 4, r_type(0, 0, 0, 0, 0x00));
        bus.write_u8(0xa02a_db42, 0x55);
        bus.write_u8(0xa02a_db43, 0x66);
        bus.write_u8(0xa02a_db44, 0x77);
        bus.write_u8(0xa02a_db45, 0x88);

        let mut tail_end_cpu = Cpu::default();
        tail_end_cpu.pc = tail_end_pc;
        tail_end_cpu.next_pc = tail_end_pc + 4;
        tail_end_cpu.regs[4] = 0xa02a_db32;

        let load = tail_end_cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, tail_end_pc);
        assert_eq!(tail_end_cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(tail_end_cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(tail_end_cpu.cp0[CP0_EPC], 0);
        assert_eq!(tail_end_cpu.regs[10], 0);

        let delay = tail_end_cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(tail_end_cpu.regs[10], 0x8877_6655);
        assert_eq!(tail_end_cpu.pc, tail_end_pc + 8);
        assert_eq!(tail_end_cpu.next_pc, tail_end_pc + 12);
    }

    #[test]
    fn hle_br2_runtime_unaligned_render_callback_word_load_reads_ram_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_PC,
            i_type(0x23, 16, 4, 0x48), // lw a0, 0x48(s0)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );
        bus.write_u8(0xa02a_db76, 0x21);
        bus.write_u8(0xa02a_db77, 0x43);
        bus.write_u8(0xa02a_db78, 0x65);
        bus.write_u8(0xa02a_db79, 0x87);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_PC + 4;
        cpu.regs[16] = 0xa02a_db2e;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(
            load.start_pc,
            BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_PC
        );
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[4], 0);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[4], 0x8765_4321);
        assert_eq!(
            cpu.pc,
            BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_PC + 8
        );
        assert_eq!(
            cpu.next_pc,
            BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_PC + 12
        );
    }

    #[test]
    fn hle_br2_runtime_unaligned_render_callback_tail_word_load_reads_ram_without_exception() {
        let tail_load_pc = BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_PC + 0x10;
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(tail_load_pc, i_type(0x23, 16, 17, 0x4c)); // lw s1, 0x4c(s0)
        bus.write_u32(tail_load_pc + 4, r_type(0, 0, 0, 0, 0x00));
        bus.write_u8(0xa02a_db7a, 0x12);
        bus.write_u8(0xa02a_db7b, 0x34);
        bus.write_u8(0xa02a_db7c, 0x56);
        bus.write_u8(0xa02a_db7d, 0x78);

        let mut cpu = Cpu::default();
        cpu.pc = tail_load_pc;
        cpu.next_pc = tail_load_pc + 4;
        cpu.regs[16] = 0xa02a_db2e;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, tail_load_pc);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[17], 0);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[17], 0x7856_3412);
        assert_eq!(cpu.pc, tail_load_pc + 8);
        assert_eq!(cpu.next_pc, tail_load_pc + 12);
    }

    #[test]
    fn hle_br2_runtime_unaligned_render_callback_final_word_load_reads_ram_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_END_PC,
            i_type(0x23, 16, 4, 0x44), // lw a0, 0x44(s0)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_END_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );
        bus.write_u8(0xa02a_db72, 0xde);
        bus.write_u8(0xa02a_db73, 0xad);
        bus.write_u8(0xa02a_db74, 0xbe);
        bus.write_u8(0xa02a_db75, 0xef);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_END_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_END_PC + 4;
        cpu.regs[16] = 0xa02a_db2e;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(
            load.start_pc,
            BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_END_PC
        );
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[4], 0);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[4], 0xefbe_adde);
        assert_eq!(
            cpu.pc,
            BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_END_PC + 8
        );
        assert_eq!(
            cpu.next_pc,
            BR2_RUNTIME_UNALIGNED_RENDER_CALLBACK_WORD_LOAD_END_PC + 12
        );
    }

    #[test]
    fn hle_br2_runtime_unaligned_render_accum_word_load_reads_ram_without_exception() {
        let inner_load_pc = BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_PC + 0x28;
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(inner_load_pc, i_type(0x23, 2, 8, 0x10)); // lw t0, 0x10(v0)
        bus.write_u32(inner_load_pc + 4, r_type(0, 0, 0, 0, 0x00));
        bus.write_u8(0x8036_f56f, 0x11);
        bus.write_u8(0x8036_f570, 0x22);
        bus.write_u8(0x8036_f571, 0x33);
        bus.write_u8(0x8036_f572, 0x44);

        let mut cpu = Cpu::default();
        cpu.pc = inner_load_pc;
        cpu.next_pc = inner_load_pc + 4;
        cpu.regs[2] = 0x8036_f55f;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, inner_load_pc);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[8], 0);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[8], 0x4433_2211);
        assert_eq!(cpu.pc, inner_load_pc + 8);
        assert_eq!(cpu.next_pc, inner_load_pc + 12);
    }

    #[test]
    fn hle_br2_runtime_unaligned_render_accum_prefix_word_load_reads_ram_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_PC,
            i_type(0x23, 4, 8, 0x34), // lw t0, 0x34(a0)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );
        bus.write_u8(0xc01a_e7b7, 0x88);
        bus.write_u8(0xc01a_e7b8, 0x99);
        bus.write_u8(0xc01a_e7b9, 0xaa);
        bus.write_u8(0xc01a_e7ba, 0xbb);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_PC + 4;
        cpu.regs[4] = 0xc01a_e783;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(
            load.start_pc,
            BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_PC
        );
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[8], 0xbbaa_9988);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_PC + 8);
        assert_eq!(
            cpu.next_pc,
            BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_PC + 12
        );
    }

    #[test]
    fn hle_br2_runtime_unaligned_render_accum_tail_word_load_reads_ram_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_END_PC,
            i_type(0x23, 2, 4, 0x48), // lw a0, 0x48(v0)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_END_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );
        bus.write_u8(0x8036_f5a7, 0xaa);
        bus.write_u8(0x8036_f5a8, 0xbb);
        bus.write_u8(0x8036_f5a9, 0xcc);
        bus.write_u8(0x8036_f5aa, 0xdd);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_END_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_END_PC + 4;
        cpu.regs[2] = 0x8036_f55f;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(
            load.start_pc,
            BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_END_PC
        );
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.regs[4], 0);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[4], 0xddcc_bbaa);
        assert_eq!(
            cpu.pc,
            BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_END_PC + 8
        );
        assert_eq!(
            cpu.next_pc,
            BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_LOAD_END_PC + 12
        );
    }

    #[test]
    fn hle_br2_runtime_unaligned_render_accum_word_store_writes_ram_without_exception() {
        let inner_store_pc = BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_STORE_PC + 0x28;
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(inner_store_pc, i_type(0x2b, 2, 8, 0x10)); // sw t0, 0x10(v0)
        bus.write_u32(inner_store_pc + 4, i_type(0x2b, 2, 9, 0x14)); // sw t1, 0x14(v0)
        bus.write_u32(inner_store_pc + 8, i_type(0x2b, 2, 10, 0x18)); // sw t2, 0x18(v0)
        bus.write_u32(inner_store_pc + 12, r_type(0, 0, 0, 0, 0x00));

        let mut cpu = Cpu::default();
        cpu.pc = inner_store_pc;
        cpu.next_pc = inner_store_pc + 4;
        cpu.regs[2] = 0x8036_f55f;
        cpu.regs[8] = 0x1122_3344;
        cpu.regs[9] = 0x5566_7788;
        cpu.regs[10] = 0x99aa_bbcc;

        for index in 0..3 {
            let report = cpu.step_report(&mut bus);
            assert_eq!(report.outcome, StepOutcome::Continue);
            assert_eq!(report.start_pc, inner_store_pc + index * 4);
            assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
            assert_eq!(cpu.cp0[CP0_CAUSE], 0);
            assert_eq!(cpu.cp0[CP0_EPC], 0);
        }

        assert_eq!(bus.read_u8(0x8036_f56f), 0x44);
        assert_eq!(bus.read_u8(0x8036_f570), 0x33);
        assert_eq!(bus.read_u8(0x8036_f571), 0x22);
        assert_eq!(bus.read_u8(0x8036_f572), 0x11);
        assert_eq!(bus.read_u8(0x8036_f573), 0x88);
        assert_eq!(bus.read_u8(0x8036_f574), 0x77);
        assert_eq!(bus.read_u8(0x8036_f575), 0x66);
        assert_eq!(bus.read_u8(0x8036_f576), 0x55);
        assert_eq!(bus.read_u8(0x8036_f577), 0xcc);
        assert_eq!(bus.read_u8(0x8036_f578), 0xbb);
        assert_eq!(bus.read_u8(0x8036_f579), 0xaa);
        assert_eq!(bus.read_u8(0x8036_f57a), 0x99);
    }

    #[test]
    fn hle_br2_runtime_unaligned_render_accum_prefix_word_store_writes_ram_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_STORE_PC,
            i_type(0x2b, 4, 8, 0x34), // sw t0, 0x34(a0)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_STORE_PC + 4,
            i_type(0x2b, 4, 9, 0x38), // sw t1, 0x38(a0)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_STORE_PC + 8,
            r_type(0, 0, 0, 0, 0x00),
        );

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_STORE_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_STORE_PC + 4;
        cpu.regs[4] = 0xc01a_e783;
        cpu.regs[8] = 0x1122_3344;
        cpu.regs[9] = 0x5566_7788;

        for index in 0..2 {
            let report = cpu.step_report(&mut bus);
            assert_eq!(report.outcome, StepOutcome::Continue);
            assert_eq!(
                report.start_pc,
                BR2_RUNTIME_UNALIGNED_RENDER_ACCUM_WORD_STORE_PC + index * 4
            );
            assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
            assert_eq!(cpu.cp0[CP0_CAUSE], 0);
            assert_eq!(cpu.cp0[CP0_EPC], 0);
        }

        assert_eq!(bus.read_u8(0xc01a_e7b7), 0x44);
        assert_eq!(bus.read_u8(0xc01a_e7b8), 0x33);
        assert_eq!(bus.read_u8(0xc01a_e7b9), 0x22);
        assert_eq!(bus.read_u8(0xc01a_e7ba), 0x11);
        assert_eq!(bus.read_u8(0xc01a_e7bb), 0x88);
        assert_eq!(bus.read_u8(0xc01a_e7bc), 0x77);
        assert_eq!(bus.read_u8(0xc01a_e7bd), 0x66);
        assert_eq!(bus.read_u8(0xc01a_e7be), 0x55);
    }

    #[test]
    fn hle_br2_runtime_unaligned_gte_load_sequence_reads_ram_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC,
            i_type(0x32, 5, 0, 0), // lwc2 vxy0, 0(a1)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 4,
            i_type(0x32, 5, 1, 4), // lwc2 vz0, 4(a1)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 8,
            i_type(0x32, 5, 2, 8), // lwc2 vxy1, 8(a1)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 12,
            r_type(0, 0, 0, 0, 0x00),
        );
        bus.write_u8(0xa02a_db46, 0x11);
        bus.write_u8(0xa02a_db47, 0x22);
        bus.write_u8(0xa02a_db48, 0x33);
        bus.write_u8(0xa02a_db49, 0x44);
        bus.write_u8(0xa02a_db4a, 0x55);
        bus.write_u8(0xa02a_db4b, 0x66);
        bus.write_u8(0xa02a_db4c, 0x77);
        bus.write_u8(0xa02a_db4d, 0x88);
        bus.write_u8(0xa02a_db4e, 0x99);
        bus.write_u8(0xa02a_db4f, 0xaa);
        bus.write_u8(0xa02a_db50, 0xbb);
        bus.write_u8(0xa02a_db51, 0xcc);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 4;
        cpu.regs[5] = 0xa02a_db46;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.cop2_data[0], 0x4433_2211);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 4);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 8);

        let second = cpu.step_report(&mut bus);
        assert_eq!(second.outcome, StepOutcome::Continue);
        assert_eq!(second.start_pc, BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 4);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.cop2_data[1], 0x6655);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 8);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 12);

        let third = cpu.step_report(&mut bus);
        assert_eq!(third.outcome, StepOutcome::Continue);
        assert_eq!(third.start_pc, BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 8);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.cop2_data[2], 0xccbb_aa99);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 12);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 16);
    }

    #[test]
    fn hle_br2_runtime_unaligned_gte_load_noops_high_gap_pointer_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC,
            i_type(0x32, 5, 0, 0), // lwc2 vxy0, 0(a1)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 4;
        cpu.regs[5] = 0xff00_8117;
        cpu.cop2_data[0] = 0xfeed_beef;

        let load = cpu.step_report(&mut bus);
        assert_eq!(load.outcome, StepOutcome::Continue);
        assert_eq!(load.start_pc, BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.cop2_data[0], 0);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 4);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_UNALIGNED_GTE_LOAD_PC + 8);
    }

    #[test]
    fn hle_br2_runtime_unaligned_word_store_sequence_writes_ram_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_WORD_STORE_PC,
            i_type(0x2b, 6, 8, 0), // sw t0, 0(a2)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_WORD_STORE_PC + 4,
            i_type(0x2b, 6, 9, 4), // sw t1, 4(a2)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_WORD_STORE_PC + 8,
            i_type(0x2b, 6, 10, 8), // sw t2, 8(a2)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_WORD_STORE_PC + 12,
            r_type(0, 0, 0, 0, 0x00),
        );

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_WORD_STORE_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_WORD_STORE_PC + 4;
        cpu.regs[6] = 0xa02a_db62;
        cpu.regs[8] = 0x1122_3344;
        cpu.regs[9] = 0x5566_7788;
        cpu.regs[10] = 0x99aa_bbcc;

        for index in 0..3 {
            let report = cpu.step_report(&mut bus);
            assert_eq!(report.outcome, StepOutcome::Continue);
            assert_eq!(
                report.start_pc,
                BR2_RUNTIME_UNALIGNED_WORD_STORE_PC + index * 4
            );
            assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
            assert_eq!(cpu.cp0[CP0_CAUSE], 0);
            assert_eq!(cpu.cp0[CP0_EPC], 0);
        }

        assert_eq!(bus.read_u8(0xa02a_db62), 0x44);
        assert_eq!(bus.read_u8(0xa02a_db63), 0x33);
        assert_eq!(bus.read_u8(0xa02a_db64), 0x22);
        assert_eq!(bus.read_u8(0xa02a_db65), 0x11);
        assert_eq!(bus.read_u8(0xa02a_db66), 0x88);
        assert_eq!(bus.read_u8(0xa02a_db67), 0x77);
        assert_eq!(bus.read_u8(0xa02a_db68), 0x66);
        assert_eq!(bus.read_u8(0xa02a_db69), 0x55);
        assert_eq!(bus.read_u8(0xa02a_db6a), 0xcc);
        assert_eq!(bus.read_u8(0xa02a_db6b), 0xbb);
        assert_eq!(bus.read_u8(0xa02a_db6c), 0xaa);
        assert_eq!(bus.read_u8(0xa02a_db6d), 0x99);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_WORD_STORE_PC + 12);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_UNALIGNED_WORD_STORE_PC + 16);
    }

    #[test]
    fn hle_br2_runtime_unaligned_word_store_noops_high_gap_pointer_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_WORD_STORE_PC,
            i_type(0x2b, 6, 8, 0), // sw t0, 0(a2)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_WORD_STORE_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_WORD_STORE_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_WORD_STORE_PC + 4;
        cpu.regs[6] = 0xff00_8133;
        cpu.regs[8] = 0x1122_3344;

        let store = cpu.step_report(&mut bus);
        assert_eq!(store.outcome, StepOutcome::Continue);
        assert_eq!(store.start_pc, BR2_RUNTIME_UNALIGNED_WORD_STORE_PC);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_WORD_STORE_PC + 4);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_UNALIGNED_WORD_STORE_PC + 8);
    }

    #[test]
    fn hle_br2_runtime_unaligned_vertex_result_store_writes_ram_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_PC,
            i_type(0x2b, 5, 14, 0), // sw t6, 0(a1)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_PC + 4;
        cpu.regs[5] = 0xa02a_db4e;
        cpu.regs[14] = 0x1122_3344;

        let report = cpu.step_report(&mut bus);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(
            report.start_pc,
            BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_PC
        );
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(bus.read_u8(0xa02a_db4e), 0x44);
        assert_eq!(bus.read_u8(0xa02a_db4f), 0x33);
        assert_eq!(bus.read_u8(0xa02a_db50), 0x22);
        assert_eq!(bus.read_u8(0xa02a_db51), 0x11);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_PC + 4);
        assert_eq!(
            cpu.next_pc,
            BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_PC + 8
        );
    }

    #[test]
    fn hle_br2_runtime_unaligned_vertex_result_tail_store_writes_ram_without_exception() {
        let tail_store_pc = BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_PC + 0x10;
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(tail_store_pc, i_type(0x2b, 5, 24, 12)); // sw t8, 12(a1)
        bus.write_u32(tail_store_pc + 4, r_type(0, 0, 0, 0, 0x00));

        let mut cpu = Cpu::default();
        cpu.pc = tail_store_pc;
        cpu.next_pc = tail_store_pc + 4;
        cpu.regs[5] = 0xa02a_db4e;
        cpu.regs[24] = 0x5566_7788;

        let report = cpu.step_report(&mut bus);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(report.start_pc, tail_store_pc);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(bus.read_u8(0xa02a_db5a), 0x88);
        assert_eq!(bus.read_u8(0xa02a_db5b), 0x77);
        assert_eq!(bus.read_u8(0xa02a_db5c), 0x66);
        assert_eq!(bus.read_u8(0xa02a_db5d), 0x55);
        assert_eq!(cpu.pc, tail_store_pc + 4);
        assert_eq!(cpu.next_pc, tail_store_pc + 8);
    }

    #[test]
    fn hle_br2_runtime_unaligned_vertex_result_mid_store_writes_ram_without_exception() {
        let mid_store_pc = BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_PC + 0x28;
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(mid_store_pc, i_type(0x2b, 5, 8, 4)); // sw t0, 4(a1)
        bus.write_u32(mid_store_pc + 4, r_type(0, 0, 0, 0, 0x00));

        let mut cpu = Cpu::default();
        cpu.pc = mid_store_pc;
        cpu.next_pc = mid_store_pc + 4;
        cpu.regs[5] = 0xa02a_db4e;
        cpu.regs[8] = 0x99aa_bbcc;

        let report = cpu.step_report(&mut bus);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(report.start_pc, mid_store_pc);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(bus.read_u8(0xa02a_db52), 0xcc);
        assert_eq!(bus.read_u8(0xa02a_db53), 0xbb);
        assert_eq!(bus.read_u8(0xa02a_db54), 0xaa);
        assert_eq!(bus.read_u8(0xa02a_db55), 0x99);
        assert_eq!(cpu.pc, mid_store_pc + 4);
        assert_eq!(cpu.next_pc, mid_store_pc + 8);
    }

    #[test]
    fn hle_br2_runtime_unaligned_vertex_result_final_store_writes_ram_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_END_PC,
            i_type(0x2b, 5, 9, 8), // sw t1, 8(a1)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_END_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_END_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_END_PC + 4;
        cpu.regs[5] = 0xa02a_db4e;
        cpu.regs[9] = 0xddeeff00;

        let report = cpu.step_report(&mut bus);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(
            report.start_pc,
            BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_END_PC
        );
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(bus.read_u8(0xa02a_db56), 0x00);
        assert_eq!(bus.read_u8(0xa02a_db57), 0xff);
        assert_eq!(bus.read_u8(0xa02a_db58), 0xee);
        assert_eq!(bus.read_u8(0xa02a_db59), 0xdd);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_END_PC + 4);
        assert_eq!(
            cpu.next_pc,
            BR2_RUNTIME_UNALIGNED_VERTEX_RESULT_STORE_END_PC + 8
        );
    }

    #[test]
    fn hle_br2_runtime_unaligned_vertex_gte_store_writes_ram_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC,
            i_type(0x3a, 5, 11, 16), // swc2 ir3, 16(a1)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC + 4;
        cpu.regs[5] = 0xa02a_db4e;
        cpu.cop2_data[11] = 0x0000_ff80;

        let report = cpu.step_report(&mut bus);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(report.start_pc, BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(bus.read_u8(0xa02a_db5e), 0x80);
        assert_eq!(bus.read_u8(0xa02a_db5f), 0xff);
        assert_eq!(bus.read_u8(0xa02a_db60), 0xff);
        assert_eq!(bus.read_u8(0xa02a_db61), 0xff);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC + 4);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC + 8);
    }

    #[test]
    fn hle_br2_runtime_unaligned_vertex_gte_store_noops_high_gap_pointer_without_exception() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC,
            i_type(0x3a, 5, 11, 16), // swc2 ir3, 16(a1)
        );
        bus.write_u32(
            BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC + 4,
            r_type(0, 0, 0, 0, 0x00),
        );

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC;
        cpu.next_pc = BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC + 4;
        cpu.regs[5] = 0xff00_811f;
        cpu.cop2_data[11] = 0x0000_ff80;

        let report = cpu.step_report(&mut bus);
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(report.start_pc, BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC);
        assert_eq!(cpu.cp0[CP0_BADVADDR], 0);
        assert_eq!(cpu.cp0[CP0_CAUSE], 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
        assert_eq!(cpu.pc, BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC + 4);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_UNALIGNED_VERTEX_GTE_STORE_PC + 8);
    }

    #[test]
    fn hle_br2_runtime_null_callback_jalr_skips_zero_target() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_NULL_CALLBACK_JALR_PC,
            r_type(2, 0, 31, 0, 0x09), // jalr ra, v0
        );
        bus.write_u32(
            BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 4,
            i_type(0x09, 29, 4, 0x10), // addiu a0, sp, 0x10
        );
        bus.write_u32(
            BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8,
            r_type(0, 0, 0, 0, 0x00),
        );

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_NULL_CALLBACK_JALR_PC;
        cpu.next_pc = BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 4;
        cpu.regs[2] = 0;
        cpu.regs[29] = 0x803f_fef0;

        let call = cpu.step_report(&mut bus);
        assert_eq!(call.outcome, StepOutcome::Continue);
        assert_eq!(cpu.pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 4);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8);
        assert_eq!(
            cpu.delay_slot_branch_pc,
            Some(BR2_RUNTIME_NULL_CALLBACK_JALR_PC)
        );
        assert_eq!(cpu.regs[31], BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[4], 0x803f_ff00);
        assert_eq!(cpu.pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 12);
        assert_eq!(cpu.delay_slot_branch_pc, None);
    }

    #[test]
    fn hle_br2_runtime_null_callback_jalr_skips_unmapped_target() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_NULL_CALLBACK_JALR_PC,
            r_type(2, 0, 31, 0, 0x09), // jalr ra, v0
        );
        bus.write_u32(
            BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 4,
            i_type(0x09, 29, 4, 0x10), // addiu a0, sp, 0x10
        );
        bus.write_u32(
            BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8,
            r_type(0, 0, 0, 0, 0x00),
        );

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_NULL_CALLBACK_JALR_PC;
        cpu.next_pc = BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 4;
        cpu.regs[2] = 0x00ff_0010;
        cpu.regs[29] = 0x803f_fef0;

        let call = cpu.step_report(&mut bus);
        assert_eq!(call.outcome, StepOutcome::Continue);
        assert_eq!(cpu.pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 4);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8);
        assert_eq!(
            cpu.delay_slot_branch_pc,
            Some(BR2_RUNTIME_NULL_CALLBACK_JALR_PC)
        );
        assert_eq!(cpu.regs[31], BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[4], 0x803f_ff00);
        assert_eq!(cpu.pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 12);
        assert_eq!(cpu.delay_slot_branch_pc, None);
    }

    #[test]
    fn hle_br2_runtime_null_callback_jalr_skips_low_ram_data_target() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_NULL_CALLBACK_JALR_PC,
            r_type(2, 0, 31, 0, 0x09), // jalr ra, v0
        );
        bus.write_u32(
            BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 4,
            i_type(0x09, 29, 4, 0x10), // addiu a0, sp, 0x10
        );
        bus.write_u32(
            BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8,
            r_type(0, 0, 0, 0, 0x00),
        );
        bus.write_u32(0x0003_0228, 0x57ed_06c1);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_NULL_CALLBACK_JALR_PC;
        cpu.next_pc = BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 4;
        cpu.regs[2] = 0x0003_0228;
        cpu.regs[29] = 0x803f_fef0;

        let call = cpu.step_report(&mut bus);
        assert_eq!(call.outcome, StepOutcome::Continue);
        assert_eq!(cpu.pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 4);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8);
        assert_eq!(
            cpu.delay_slot_branch_pc,
            Some(BR2_RUNTIME_NULL_CALLBACK_JALR_PC)
        );
        assert_eq!(cpu.regs[31], BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[4], 0x803f_ff00);
        assert_eq!(cpu.pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 12);
        assert_eq!(cpu.delay_slot_branch_pc, None);
    }

    #[test]
    fn hle_br2_runtime_null_callback_jalr_skips_unaligned_low_runtime_target() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_NULL_CALLBACK_JALR_PC,
            r_type(2, 0, 31, 0, 0x09), // jalr ra, v0
        );
        bus.write_u32(
            BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 4,
            i_type(0x09, 29, 4, 0x10), // addiu a0, sp, 0x10
        );
        bus.write_u32(
            BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8,
            r_type(0, 0, 0, 0, 0x00),
        );
        bus.write_u32(0x8000_0400, 0xd0bf_5600);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_NULL_CALLBACK_JALR_PC;
        cpu.next_pc = BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 4;
        cpu.regs[2] = 0x8000_0401;
        cpu.regs[29] = 0x803f_fef0;

        let call = cpu.step_report(&mut bus);
        assert_eq!(call.outcome, StepOutcome::Continue);
        assert_eq!(cpu.pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 4);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8);
        assert_eq!(
            cpu.delay_slot_branch_pc,
            Some(BR2_RUNTIME_NULL_CALLBACK_JALR_PC)
        );
        assert_eq!(cpu.regs[31], BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[4], 0x803f_ff00);
        assert_eq!(cpu.pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 12);
        assert_eq!(cpu.delay_slot_branch_pc, None);
    }

    #[test]
    fn hle_br2_runtime_null_callback_jalr_preserves_executable_target() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_NULL_CALLBACK_JALR_PC,
            r_type(2, 0, 31, 0, 0x09), // jalr ra, v0
        );
        bus.write_u32(
            BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 4,
            i_type(0x09, 29, 4, 0x10), // addiu a0, sp, 0x10
        );
        bus.write_u32(0x8001_0000, r_type(0, 0, 0, 0, 0x00));

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_NULL_CALLBACK_JALR_PC;
        cpu.next_pc = BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 4;
        cpu.regs[2] = 0x8001_0000;
        cpu.regs[29] = 0x803f_fef0;

        let call = cpu.step_report(&mut bus);
        assert_eq!(call.outcome, StepOutcome::Continue);
        assert_eq!(cpu.pc, BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 4);
        assert_eq!(cpu.next_pc, 0x8001_0000);
        assert_eq!(
            cpu.delay_slot_branch_pc,
            Some(BR2_RUNTIME_NULL_CALLBACK_JALR_PC)
        );
        assert_eq!(cpu.regs[31], BR2_RUNTIME_NULL_CALLBACK_JALR_PC + 8);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[4], 0x803f_ff00);
        assert_eq!(cpu.pc, 0x8001_0000);
        assert_eq!(cpu.next_pc, 0x8001_0004);
        assert_eq!(cpu.delay_slot_branch_pc, None);
    }

    #[test]
    fn hle_br2_runtime_render_callback_jalr_skips_invalid_target_after_delay_slot() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_RENDER_CALLBACK_JALR_PC,
            r_type(8, 0, 31, 0, 0x09), // jalr ra, t0
        );
        bus.write_u32(
            BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 4,
            i_type(0x09, 22, 22, -1), // addiu s22, s22, -1
        );
        bus.write_u32(
            BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 8,
            r_type(0, 0, 0, 0, 0x00),
        );

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_RENDER_CALLBACK_JALR_PC;
        cpu.next_pc = BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 4;
        cpu.regs[8] = 0x0000_fff6;
        cpu.regs[22] = 3;

        let call = cpu.step_report(&mut bus);
        assert_eq!(call.outcome, StepOutcome::Continue);
        assert_eq!(cpu.pc, BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 4);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 8);
        assert_eq!(
            cpu.delay_slot_branch_pc,
            Some(BR2_RUNTIME_RENDER_CALLBACK_JALR_PC)
        );
        assert_eq!(cpu.regs[31], BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 8);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[22], 2);
        assert_eq!(cpu.pc, BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 8);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 12);
        assert_eq!(cpu.delay_slot_branch_pc, None);
    }

    #[test]
    fn hle_br2_runtime_render_callback_jalr_clamps_huge_low_data_target_loop() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(
            BR2_RUNTIME_RENDER_CALLBACK_JALR_PC,
            r_type(8, 0, 31, 0, 0x09), // jalr ra, t0
        );
        bus.write_u32(
            BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 4,
            i_type(0x09, 22, 22, -1), // addiu s22, s22, -1
        );
        bus.write_u32(
            BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 8,
            r_type(0, 0, 0, 0, 0x00),
        );
        bus.write_u32(BR2_RUNTIME_RENDER_CALLBACK_MIN_TARGET_PC - 4, 0x002b_fff7);

        let mut cpu = Cpu::default();
        cpu.pc = BR2_RUNTIME_RENDER_CALLBACK_JALR_PC;
        cpu.next_pc = BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 4;
        cpu.regs[8] = BR2_RUNTIME_RENDER_CALLBACK_MIN_TARGET_PC - 4;
        cpu.regs[22] = BR2_RUNTIME_RENDER_CALLBACK_LOOP_MAX_REAL_ITERATIONS + 1;

        let call = cpu.step_report(&mut bus);
        assert_eq!(call.outcome, StepOutcome::Continue);
        assert_eq!(cpu.pc, BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 4);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 8);
        assert_eq!(cpu.regs[22], 1);
        assert_eq!(cpu.regs[31], BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 8);

        let delay = cpu.step_report(&mut bus);
        assert_eq!(delay.outcome, StepOutcome::Continue);
        assert_eq!(cpu.regs[22], 0);
        assert_eq!(cpu.pc, BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 8);
        assert_eq!(cpu.next_pc, BR2_RUNTIME_RENDER_CALLBACK_JALR_PC + 12);
        assert_eq!(cpu.delay_slot_branch_pc, None);
    }

    #[test]
    fn traps_unaligned_word_store_without_mutating_memory() {
        let rom = program(&[
            i_type(0x0f, 0, 8, 0x1122), // lui t0, 0x1122
            i_type(0x0d, 8, 8, 0x3344), // ori t0, t0, 0x3344
            i_type(0x2b, 0, 8, 1),      // sw t0, 1(zero)
            r_type(0, 0, 0, 0, 0x0d),   // break
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        for offset in 0..8 {
            bus.write_u8(offset, 0xa0 + offset as u8);
        }
        let mut cpu = Cpu::default();

        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);

        assert_eq!(cpu.cp0[CP0_BADVADDR], 1);
        assert_eq!(cpu.cp0[CP0_CAUSE], 5 << 2);
        assert_eq!(cpu.cp0[CP0_EPC], 0x1fc0_0008);
        assert_eq!(cpu.pc, EXCEPTION_VECTOR);
        for offset in 0..8 {
            assert_eq!(bus.read_u8(offset), 0xa0 + offset as u8);
        }
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
    fn hle_br2_enter_critical_section_syscall_returns_without_bios_dispatch() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(0x8033_d650, r_type(0, 0, 0, 0, 0x0c));

        let mut cpu = Cpu::default();
        cpu.pc = 0x8033_d650;
        cpu.next_pc = 0x8033_d654;
        cpu.regs[4] = BR2_BIOS_KERNEL_SYSCALL_ENTER_CRITICAL_SECTION;
        cpu.cp0[CP0_STATUS] = STATUS_IE | CAUSE_IP2;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x8033_d650);
        assert_eq!(report.instruction, Some(0x0000_000c));
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(cpu.pc, 0x8033_d654);
        assert_eq!(cpu.next_pc, 0x8033_d658);
        assert_eq!(cpu.regs[2], 1);
        assert_eq!(cpu.cp0[CP0_STATUS] & STATUS_IE, 0);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_EXCODE_MASK, 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
    }

    #[test]
    fn hle_br2_exit_critical_section_syscall_returns_without_bios_dispatch() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(0x8033_d650, r_type(0, 0, 0, 0, 0x0c));

        let mut cpu = Cpu::default();
        cpu.pc = 0x8033_d650;
        cpu.next_pc = 0x8033_d654;
        cpu.regs[4] = BR2_BIOS_KERNEL_SYSCALL_EXIT_CRITICAL_SECTION;
        cpu.cp0[CP0_STATUS] = CAUSE_IP2;

        let report = cpu.step_report(&mut bus);

        assert_eq!(report.start_pc, 0x8033_d650);
        assert_eq!(report.instruction, Some(0x0000_000c));
        assert_eq!(report.outcome, StepOutcome::Continue);
        assert_eq!(cpu.pc, 0x8033_d654);
        assert_eq!(cpu.next_pc, 0x8033_d658);
        assert_eq!(cpu.regs[2], 1);
        assert_eq!(cpu.cp0[CP0_STATUS] & STATUS_IE, STATUS_IE);
        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_EXCODE_MASK, 0);
        assert_eq!(cpu.cp0[CP0_EPC], 0);
    }

    #[test]
    fn hle_br2_kernel_syscall_does_not_intercept_low_bios_syscalls() {
        let rom = program(&[
            i_type(
                0x09,
                0,
                4,
                BR2_BIOS_KERNEL_SYSCALL_ENTER_CRITICAL_SECTION as i16,
            ),
            r_type(0, 0, 0, 0, 0x0c),
        ]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);
        let mut cpu = Cpu::default();

        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);

        assert_eq!(cpu.cp0[CP0_CAUSE] & CAUSE_EXCODE_MASK, 8 << 2);
        assert_eq!(cpu.cp0[CP0_EPC], 0x1fc0_0004);
        assert_eq!(cpu.pc, EXCEPTION_VECTOR);
        assert_eq!(cpu.next_pc, EXCEPTION_VECTOR + 4);
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
