use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use crate::action::ActionButtons;
use crate::native::io::{
    DMA_GPU_BCR, DMA_GPU_CHCR, DMA_GPU_MADR, DMA_MDEC_IN_BCR, DMA_MDEC_IN_CHCR, DMA_MDEC_IN_MADR,
    DMA_MDEC_OUT_BCR, DMA_MDEC_OUT_CHCR, DMA_MDEC_OUT_MADR, DMA_OTC_BCR, DMA_OTC_CHCR,
    DMA_OTC_MADR, DMA_REGION_END, DMA_REGION_START, GPU_GP0, GpuCommandSource, IO_REGION_END,
    IO_REGION_START, IRQ_STATUS, Io, NativeGpuDisplayCandidate, NativeGpuDrawCapture,
    gp0_command_word_count, io_access_for,
};
use crate::native::platform::{NativePlatformOps, PreferredNativePlatform};

const DMA_CHANNEL_COUNT: usize = 7;
const DMA_MDEC_IN_CHANNEL: usize = 0;
const DMA_MDEC_OUT_CHANNEL: usize = 1;
const DMA_GPU_CHANNEL: usize = 2;
const DMA_OTC_CHANNEL: usize = 6;
const DMA_DIRECTION_FROM_RAM: u32 = 1 << 0;
const DMA_STEP_DECREMENT: u32 = 1 << 1;
const DMA_LINKED_LIST_MODE: u32 = 1 << 10;
const DMA_MDEC_COMPLETION_DELAY_CYCLES: u64 = 1_024;
const DMA_GPU_COMPLETION_DELAY_CYCLES: u64 = 4_096;
const DMA_OTC_COMPLETION_DELAY_CYCLES: u64 = 512;
const VBLANK_CYCLES: u64 = 566_000;
const GPU_LINKED_LIST_NODE_LIMIT: u32 = 65_536;
const BR2_DRAW_SYNC_FLAG_VIRTUAL: u32 = 0x803a_2210;
const BR2_DRAW_SYNC_FLAG_PHYSICAL: u32 = 0x003a_2210;
const BR2_PRIMITIVE_RAM_START: u32 = 0x0038_0000;
const BR2_PRIMITIVE_RAM_END: u32 = 0x003c_0000;
const PRIMITIVE_RAM_RECENT_LIMIT: usize = 24;
const GPU_LINKED_LIST_RECENT_COMMAND_LIMIT: usize = 32;
const GPU_LINKED_LIST_NODE_SAMPLE_LIMIT: usize = 16;
const GPU_LINKED_LIST_NONEMPTY_NODE_SAMPLE_LIMIT: usize = 32;
const PRIMITIVE_PACKET_SCAN_SAMPLE_LIMIT: usize = 24;
const PRIMITIVE_PACKET_MAX_WORDS: u32 = 64;
const DMA_ACTIVITY_RECENT_LIMIT: usize = 64;
const BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW: u64 = 1;
const BR2_UNLINKED_PRIMITIVE_REPLAY_PACKET_LIMIT: usize = 8;
const BR2_UNLINKED_PRIMITIVE_REPLAY_SPARSE_NODE_LIMIT: u32 = 32;
const BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_LINKED_NODES: u32 = 512;
const BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_DRAW_PACKETS: u32 = 8;
const BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_RECENT_HEADERS: u64 = 32;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeInputActivity {
    pub p1_input_reads: u64,
    pub p1_up_active_reads: u64,
    pub p1_down_active_reads: u64,
    pub p1_left_active_reads: u64,
    pub p1_right_active_reads: u64,
    pub p1_start_active_reads: u64,
    pub p1_punch_active_reads: u64,
    pub p1_kick_active_reads: u64,
    pub p1_beast_active_reads: u64,
    pub p3_input_reads: u64,
    pub p3_guard_active_reads: u64,
    pub system_input_reads: u64,
    pub system_coin_active_reads: u64,
    pub system_start_active_reads: u64,
    pub coin_register_reads: u64,
    pub coin_register_active_reads: u64,
}

impl NativeInputActivity {
    pub fn has_play_control_activity(self) -> bool {
        self.p1_punch_active_reads > 0
            && self.p1_kick_active_reads > 0
            && self.p1_beast_active_reads > 0
            && self.p3_guard_active_reads > 0
            && self.system_coin_active_reads > 0
            && (self.system_start_active_reads > 0 || self.p1_start_active_reads > 0)
    }

    pub fn has_direction_activity(self) -> bool {
        self.p1_up_active_reads > 0
            && self.p1_down_active_reads > 0
            && self.p1_left_active_reads > 0
            && self.p1_right_active_reads > 0
    }

    pub fn has_full_control_activity(self) -> bool {
        self.has_direction_activity() && self.has_play_control_activity()
    }

    pub fn saturating_added(self, other: Self) -> Self {
        Self {
            p1_input_reads: self.p1_input_reads.saturating_add(other.p1_input_reads),
            p1_up_active_reads: self
                .p1_up_active_reads
                .saturating_add(other.p1_up_active_reads),
            p1_down_active_reads: self
                .p1_down_active_reads
                .saturating_add(other.p1_down_active_reads),
            p1_left_active_reads: self
                .p1_left_active_reads
                .saturating_add(other.p1_left_active_reads),
            p1_right_active_reads: self
                .p1_right_active_reads
                .saturating_add(other.p1_right_active_reads),
            p1_start_active_reads: self
                .p1_start_active_reads
                .saturating_add(other.p1_start_active_reads),
            p1_punch_active_reads: self
                .p1_punch_active_reads
                .saturating_add(other.p1_punch_active_reads),
            p1_kick_active_reads: self
                .p1_kick_active_reads
                .saturating_add(other.p1_kick_active_reads),
            p1_beast_active_reads: self
                .p1_beast_active_reads
                .saturating_add(other.p1_beast_active_reads),
            p3_input_reads: self.p3_input_reads.saturating_add(other.p3_input_reads),
            p3_guard_active_reads: self
                .p3_guard_active_reads
                .saturating_add(other.p3_guard_active_reads),
            system_input_reads: self
                .system_input_reads
                .saturating_add(other.system_input_reads),
            system_coin_active_reads: self
                .system_coin_active_reads
                .saturating_add(other.system_coin_active_reads),
            system_start_active_reads: self
                .system_start_active_reads
                .saturating_add(other.system_start_active_reads),
            coin_register_reads: self
                .coin_register_reads
                .saturating_add(other.coin_register_reads),
            coin_register_active_reads: self
                .coin_register_active_reads
                .saturating_add(other.coin_register_active_reads),
        }
    }

    pub fn saturating_subtracted(self, baseline: Self) -> Self {
        Self {
            p1_input_reads: self.p1_input_reads.saturating_sub(baseline.p1_input_reads),
            p1_up_active_reads: self
                .p1_up_active_reads
                .saturating_sub(baseline.p1_up_active_reads),
            p1_down_active_reads: self
                .p1_down_active_reads
                .saturating_sub(baseline.p1_down_active_reads),
            p1_left_active_reads: self
                .p1_left_active_reads
                .saturating_sub(baseline.p1_left_active_reads),
            p1_right_active_reads: self
                .p1_right_active_reads
                .saturating_sub(baseline.p1_right_active_reads),
            p1_start_active_reads: self
                .p1_start_active_reads
                .saturating_sub(baseline.p1_start_active_reads),
            p1_punch_active_reads: self
                .p1_punch_active_reads
                .saturating_sub(baseline.p1_punch_active_reads),
            p1_kick_active_reads: self
                .p1_kick_active_reads
                .saturating_sub(baseline.p1_kick_active_reads),
            p1_beast_active_reads: self
                .p1_beast_active_reads
                .saturating_sub(baseline.p1_beast_active_reads),
            p3_input_reads: self.p3_input_reads.saturating_sub(baseline.p3_input_reads),
            p3_guard_active_reads: self
                .p3_guard_active_reads
                .saturating_sub(baseline.p3_guard_active_reads),
            system_input_reads: self
                .system_input_reads
                .saturating_sub(baseline.system_input_reads),
            system_coin_active_reads: self
                .system_coin_active_reads
                .saturating_sub(baseline.system_coin_active_reads),
            system_start_active_reads: self
                .system_start_active_reads
                .saturating_sub(baseline.system_start_active_reads),
            coin_register_reads: self
                .coin_register_reads
                .saturating_sub(baseline.coin_register_reads),
            coin_register_active_reads: self
                .coin_register_active_reads
                .saturating_sub(baseline.coin_register_active_reads),
        }
    }

    pub fn json(self) -> String {
        format!(
            "{{\"p1_input_reads\":{},\"p1_up_active_reads\":{},\"p1_down_active_reads\":{},\"p1_left_active_reads\":{},\"p1_right_active_reads\":{},\"p1_start_active_reads\":{},\"p1_punch_active_reads\":{},\"p1_kick_active_reads\":{},\"p1_beast_active_reads\":{},\"p3_input_reads\":{},\"p3_guard_active_reads\":{},\"system_input_reads\":{},\"system_coin_active_reads\":{},\"system_start_active_reads\":{},\"coin_register_reads\":{},\"coin_register_active_reads\":{},\"has_direction_activity\":{},\"has_play_control_activity\":{},\"has_full_control_activity\":{}}}",
            self.p1_input_reads,
            self.p1_up_active_reads,
            self.p1_down_active_reads,
            self.p1_left_active_reads,
            self.p1_right_active_reads,
            self.p1_start_active_reads,
            self.p1_punch_active_reads,
            self.p1_kick_active_reads,
            self.p1_beast_active_reads,
            self.p3_input_reads,
            self.p3_guard_active_reads,
            self.system_input_reads,
            self.system_coin_active_reads,
            self.system_start_active_reads,
            self.coin_register_reads,
            self.coin_register_active_reads,
            self.has_direction_activity(),
            self.has_play_control_activity(),
            self.has_full_control_activity()
        )
    }
}

#[derive(Clone, Debug)]
struct GpuLinkedListDmaStats {
    calls: u64,
    last_start: u32,
    last_first_node: u32,
    last_nodes: u32,
    last_words: u32,
    last_nonempty_nodes: u32,
    last_max_node_words: u32,
    last_min_command_address: Option<u32>,
    last_max_command_address: Option<u32>,
    last_command_opcode_counts: [u32; 256],
    last_recent_commands: Vec<GpuLinkedListCommandSample>,
    last_visited_nodes: Vec<u32>,
    last_first_node_samples: Vec<GpuLinkedListNodeSample>,
    last_tail_node_samples: Vec<GpuLinkedListNodeSample>,
    last_nonempty_node_samples: Vec<GpuLinkedListNodeSample>,
    recent_runs: Vec<GpuLinkedListDmaRunSummary>,
    last_terminated: bool,
    last_hit_node_limit: bool,
    max_nodes: u32,
    max_words: u32,
    max_nonempty_nodes: u32,
    max_node_words: u32,
    node_limit_hits: u64,
}

#[derive(Clone, Debug, Default)]
struct BankedRomReadStats {
    reads: u64,
    bytes: u64,
    bank_reads: [u64; 4],
    last_bank: Option<u8>,
    last_address: Option<u32>,
    last_offset: Option<usize>,
    last_width: u8,
    last_value: u32,
}

#[derive(Clone, Copy, Debug, Default)]
struct PrimitiveRamWriteSample {
    address: u32,
    value: u32,
    pc: Option<u32>,
    vblank: u64,
    cycles: u64,
}

#[derive(Clone, Debug)]
struct PrimitiveRamWriteStats {
    writes: u64,
    command_like_writes: u64,
    header_like_writes: u64,
    current_vblank_writes: u64,
    current_vblank_command_like_writes: u64,
    current_vblank_header_like_writes: u64,
    last_vblank_writes: u64,
    last_vblank_command_like_writes: u64,
    last_vblank_header_like_writes: u64,
    opcode_counts: [u64; 256],
    current_vblank_opcode_counts: [u64; 256],
    last_vblank_opcode_counts: [u64; 256],
    header_write_vblank_by_address: HashMap<u32, u64>,
    last_address: Option<u32>,
    last_value: u32,
    last_pc: Option<u32>,
    recent_command_like_writes: Vec<PrimitiveRamWriteSample>,
    recent_header_like_writes: Vec<PrimitiveRamWriteSample>,
}

#[derive(Clone, Debug)]
struct DmaActivitySample {
    kind: &'static str,
    channel: usize,
    register: Option<&'static str>,
    address: Option<u32>,
    value: Option<u32>,
    madr: u32,
    bcr: u32,
    chcr: u32,
    start: Option<u32>,
    end: Option<u32>,
    words: u32,
    nodes: u32,
    nonempty_nodes: u32,
    pc: Option<u32>,
    vblank: u64,
    cycles: u64,
}

#[derive(Clone, Debug)]
struct UnlinkedPrimitiveReplayStats {
    attempts: u64,
    conditional_replays: u64,
    forced_replays: u64,
    skipped: u64,
    total_packets: u64,
    total_words: u64,
    last_vblank: Option<u64>,
    last_reason: &'static str,
    last_candidate_headers: usize,
    last_linked_nodes: u32,
    last_linked_nonempty_nodes: u32,
    last_linked_words: u32,
    last_packets: usize,
    last_words: usize,
}

impl DmaActivitySample {
    fn json(&self) -> String {
        format!(
            "{{\"kind\":\"{}\",\"channel\":{},\"register\":{},\"address\":{},\"address_hex\":{},\"value\":{},\"value_hex\":{},\"madr\":{},\"madr_hex\":\"0x{:08x}\",\"bcr\":{},\"bcr_hex\":\"0x{:08x}\",\"chcr\":{},\"chcr_hex\":\"0x{:08x}\",\"start\":{},\"start_hex\":{},\"end\":{},\"end_hex\":{},\"words\":{},\"nodes\":{},\"nonempty_nodes\":{},\"pc\":{},\"pc_hex\":{},\"vblank\":{},\"cycles\":{}}}",
            self.kind,
            self.channel,
            optional_str_json(self.register),
            optional_u32_json(self.address),
            optional_u32_hex_json(self.address),
            optional_u32_json(self.value),
            optional_u32_hex_json(self.value),
            self.madr,
            self.madr,
            self.bcr,
            self.bcr,
            self.chcr,
            self.chcr,
            optional_u32_json(self.start),
            optional_u32_hex_json(self.start),
            optional_u32_json(self.end),
            optional_u32_hex_json(self.end),
            self.words,
            self.nodes,
            self.nonempty_nodes,
            optional_u32_json(self.pc),
            optional_u32_hex_json(self.pc),
            self.vblank,
            self.cycles
        )
    }
}

impl Default for UnlinkedPrimitiveReplayStats {
    fn default() -> Self {
        Self {
            attempts: 0,
            conditional_replays: 0,
            forced_replays: 0,
            skipped: 0,
            total_packets: 0,
            total_words: 0,
            last_vblank: None,
            last_reason: "never",
            last_candidate_headers: 0,
            last_linked_nodes: 0,
            last_linked_nonempty_nodes: 0,
            last_linked_words: 0,
            last_packets: 0,
            last_words: 0,
        }
    }
}

impl UnlinkedPrimitiveReplayStats {
    fn record_skip(
        &mut self,
        vblank: u64,
        reason: &'static str,
        candidate_headers: usize,
        linked: &GpuLinkedListDmaRunStats,
    ) {
        self.attempts = self.attempts.saturating_add(1);
        self.skipped = self.skipped.saturating_add(1);
        self.last_vblank = Some(vblank);
        self.last_reason = reason;
        self.last_candidate_headers = candidate_headers;
        self.last_linked_nodes = linked.last_nodes;
        self.last_linked_nonempty_nodes = linked.last_nonempty_nodes;
        self.last_linked_words = linked.last_words;
        self.last_packets = 0;
        self.last_words = 0;
    }

    fn record_replay(
        &mut self,
        vblank: u64,
        reason: &'static str,
        candidate_headers: usize,
        linked: &GpuLinkedListDmaRunStats,
        packets: usize,
        words: usize,
    ) {
        self.attempts = self.attempts.saturating_add(1);
        if reason == "forced" {
            self.forced_replays = self.forced_replays.saturating_add(1);
        } else {
            self.conditional_replays = self.conditional_replays.saturating_add(1);
        }
        self.total_packets = self.total_packets.saturating_add(packets as u64);
        self.total_words = self.total_words.saturating_add(words as u64);
        self.last_vblank = Some(vblank);
        self.last_reason = reason;
        self.last_candidate_headers = candidate_headers;
        self.last_linked_nodes = linked.last_nodes;
        self.last_linked_nonempty_nodes = linked.last_nonempty_nodes;
        self.last_linked_words = linked.last_words;
        self.last_packets = packets;
        self.last_words = words;
    }

    fn json(&self) -> String {
        format!(
            "{{\"attempts\":{},\"conditional_replays\":{},\"forced_replays\":{},\"skipped\":{},\"total_packets\":{},\"total_words\":{},\"last_vblank\":{},\"last_reason\":\"{}\",\"last_candidate_headers\":{},\"last_linked_nodes\":{},\"last_linked_nonempty_nodes\":{},\"last_linked_words\":{},\"last_packets\":{},\"last_words\":{}}}",
            self.attempts,
            self.conditional_replays,
            self.forced_replays,
            self.skipped,
            self.total_packets,
            self.total_words,
            optional_u64_json(self.last_vblank),
            self.last_reason,
            self.last_candidate_headers,
            self.last_linked_nodes,
            self.last_linked_nonempty_nodes,
            self.last_linked_words,
            self.last_packets,
            self.last_words
        )
    }
}

#[derive(Clone, Debug, Default)]
struct PrimitivePacketCandidateSample {
    address: u32,
    header: u32,
    word_count: u32,
    next: u32,
    linked: bool,
    first_command: u32,
    header_write_vblank: Option<u64>,
}

#[derive(Clone, Debug)]
struct GpuLinkedListDmaRunSummary {
    call: u64,
    start: u32,
    first_node: u32,
    nodes: u32,
    words: u32,
    nonempty_nodes: u32,
    max_node_words: u32,
    min_command_address: Option<u32>,
    max_command_address: Option<u32>,
    command_opcode_counts: [u32; 256],
    terminated: bool,
    hit_node_limit: bool,
}

impl GpuLinkedListDmaRunSummary {
    fn from_run(call: u64, run: &GpuLinkedListDmaRunStats) -> Self {
        Self {
            call,
            start: run.last_start,
            first_node: run.last_first_node,
            nodes: run.last_nodes,
            words: run.last_words,
            nonempty_nodes: run.last_nonempty_nodes,
            max_node_words: run.last_max_node_words,
            min_command_address: run.last_min_command_address,
            max_command_address: run.last_max_command_address,
            command_opcode_counts: run.command_opcode_counts,
            terminated: run.terminated,
            hit_node_limit: run.hit_node_limit,
        }
    }

    fn json(&self) -> String {
        format!(
            "{{\"call\":{},\"start\":{},\"start_hex\":\"0x{:08x}\",\"first_node\":{},\"first_node_hex\":\"0x{:08x}\",\"nodes\":{},\"words\":{},\"nonempty_nodes\":{},\"max_node_words\":{},\"min_command_address\":{},\"min_command_address_hex\":{},\"max_command_address\":{},\"max_command_address_hex\":{},\"command_opcode_counts\":[{}],\"terminated\":{},\"hit_node_limit\":{}}}",
            self.call,
            self.start,
            self.start,
            self.first_node,
            self.first_node,
            self.nodes,
            self.words,
            self.nonempty_nodes,
            self.max_node_words,
            optional_u32_json(self.min_command_address),
            optional_u32_hex_json(self.min_command_address),
            optional_u32_json(self.max_command_address),
            optional_u32_hex_json(self.max_command_address),
            command_opcode_counts_json(&self.command_opcode_counts),
            self.terminated,
            self.hit_node_limit
        )
    }
}

impl PrimitivePacketCandidateSample {
    fn json(&self) -> String {
        format!(
            "{{\"address\":{},\"address_hex\":\"0x{:08x}\",\"header\":{},\"header_hex\":\"0x{:08x}\",\"word_count\":{},\"next\":{},\"next_hex\":\"0x{:06x}\",\"linked\":{},\"first_command\":{},\"first_command_hex\":\"0x{:08x}\",\"opcode\":{},\"opcode_hex\":\"0x{:02x}\",\"header_write_vblank\":{}}}",
            self.address,
            self.address,
            self.header,
            self.header,
            self.word_count,
            self.next,
            self.next,
            self.linked,
            self.first_command,
            self.first_command,
            self.first_command >> 24,
            self.first_command >> 24,
            self.header_write_vblank
                .map_or_else(|| "null".to_string(), |value| value.to_string())
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct GpuLinkedListNodeSample {
    address: u32,
    header: u32,
    word_count: u32,
    next: u32,
}

impl GpuLinkedListNodeSample {
    fn new(address: u32, header: u32) -> Self {
        Self {
            address,
            header,
            word_count: header >> 24,
            next: header & 0x00ff_ffff,
        }
    }

    fn json(&self) -> String {
        format!(
            "{{\"address\":{},\"address_hex\":\"0x{:08x}\",\"header\":{},\"header_hex\":\"0x{:08x}\",\"word_count\":{},\"next\":{},\"next_hex\":\"0x{:06x}\"}}",
            self.address,
            self.address,
            self.header,
            self.header,
            self.word_count,
            self.next,
            self.next
        )
    }
}

#[derive(Clone, Debug, Default)]
struct GpuLinkedListCommandSample {
    address: u32,
    opcode: u8,
    words: Vec<u32>,
}

impl GpuLinkedListCommandSample {
    fn new(address: u32, words: Vec<u32>) -> Self {
        Self {
            address,
            opcode: (words.first().copied().unwrap_or(0) >> 24) as u8,
            words,
        }
    }

    fn json(&self) -> String {
        let words = self
            .words
            .iter()
            .map(|word| format!("\"0x{word:08x}\""))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"address\":{},\"address_hex\":\"0x{:08x}\",\"opcode\":{},\"opcode_hex\":\"0x{:02x}\",\"word_count\":{},\"words\":[{}]}}",
            self.address,
            self.address,
            self.opcode,
            self.opcode,
            self.words.len(),
            words
        )
    }
}

impl Default for PrimitiveRamWriteStats {
    fn default() -> Self {
        Self {
            writes: 0,
            command_like_writes: 0,
            header_like_writes: 0,
            current_vblank_writes: 0,
            current_vblank_command_like_writes: 0,
            current_vblank_header_like_writes: 0,
            last_vblank_writes: 0,
            last_vblank_command_like_writes: 0,
            last_vblank_header_like_writes: 0,
            opcode_counts: [0; 256],
            current_vblank_opcode_counts: [0; 256],
            last_vblank_opcode_counts: [0; 256],
            header_write_vblank_by_address: HashMap::new(),
            last_address: None,
            last_value: 0,
            last_pc: None,
            recent_command_like_writes: Vec::new(),
            recent_header_like_writes: Vec::new(),
        }
    }
}

impl PrimitiveRamWriteStats {
    fn record(&mut self, address: u32, value: u32, pc: Option<u32>, vblank: u64, cycles: u64) {
        self.writes = self.writes.saturating_add(1);
        self.current_vblank_writes = self.current_vblank_writes.saturating_add(1);
        self.last_address = Some(address);
        self.last_value = value;
        self.last_pc = pc;

        let packet_words = value >> 24;
        let packet_next = value & 0x00ff_ffff;
        if (1..=PRIMITIVE_PACKET_MAX_WORDS).contains(&packet_words)
            && primitive_packet_next_plausible(packet_next)
        {
            self.header_like_writes = self.header_like_writes.saturating_add(1);
            self.current_vblank_header_like_writes =
                self.current_vblank_header_like_writes.saturating_add(1);
            self.recent_header_like_writes
                .push(PrimitiveRamWriteSample {
                    address,
                    value,
                    pc,
                    vblank,
                    cycles,
                });
            self.header_write_vblank_by_address.insert(address, vblank);
            if self.recent_header_like_writes.len() > PRIMITIVE_RAM_RECENT_LIMIT {
                let overflow = self.recent_header_like_writes.len() - PRIMITIVE_RAM_RECENT_LIMIT;
                self.recent_header_like_writes.drain(0..overflow);
            }
        }

        let opcode = (value >> 24) as u8;
        if !looks_like_gp0_command_opcode(opcode) {
            return;
        }

        let opcode_index = opcode as usize;
        self.command_like_writes = self.command_like_writes.saturating_add(1);
        self.current_vblank_command_like_writes =
            self.current_vblank_command_like_writes.saturating_add(1);
        self.opcode_counts[opcode_index] = self.opcode_counts[opcode_index].saturating_add(1);
        self.current_vblank_opcode_counts[opcode_index] =
            self.current_vblank_opcode_counts[opcode_index].saturating_add(1);

        self.recent_command_like_writes
            .push(PrimitiveRamWriteSample {
                address,
                value,
                pc,
                vblank,
                cycles,
            });
        if self.recent_command_like_writes.len() > PRIMITIVE_RAM_RECENT_LIMIT {
            let overflow = self.recent_command_like_writes.len() - PRIMITIVE_RAM_RECENT_LIMIT;
            self.recent_command_like_writes.drain(0..overflow);
        }
    }

    fn advance_vblank(&mut self) {
        self.last_vblank_writes = self.current_vblank_writes;
        self.last_vblank_command_like_writes = self.current_vblank_command_like_writes;
        self.last_vblank_header_like_writes = self.current_vblank_header_like_writes;
        self.last_vblank_opcode_counts = self.current_vblank_opcode_counts;
        self.current_vblank_writes = 0;
        self.current_vblank_command_like_writes = 0;
        self.current_vblank_header_like_writes = 0;
        self.current_vblank_opcode_counts = [0; 256];
    }

    fn header_write_vblank(&self, address: u32) -> Option<u64> {
        self.header_write_vblank_by_address.get(&address).copied()
    }

    fn header_addresses_written_since(&self, min_vblank: u64) -> Vec<(u64, u32)> {
        self.header_write_vblank_by_address
            .iter()
            .filter_map(|(address, vblank)| (*vblank >= min_vblank).then_some((*vblank, *address)))
            .collect()
    }

    fn json(&self) -> String {
        format!(
            "{{\"range_start\":\"0x{:08x}\",\"range_end\":\"0x{:08x}\",\"writes\":{},\"command_like_writes\":{},\"header_like_writes\":{},\"tracked_header_addresses\":{},\"current_vblank_writes\":{},\"current_vblank_command_like_writes\":{},\"current_vblank_header_like_writes\":{},\"last_vblank_writes\":{},\"last_vblank_command_like_writes\":{},\"last_vblank_header_like_writes\":{},\"opcode_counts\":[{}],\"current_vblank_opcode_counts\":[{}],\"last_vblank_opcode_counts\":[{}],\"last_address\":{},\"last_address_hex\":{},\"last_value\":{},\"last_value_hex\":\"0x{:08x}\",\"last_pc\":{},\"last_pc_hex\":{},\"recent_command_like_writes\":[{}],\"recent_header_like_writes\":[{}]}}",
            BR2_PRIMITIVE_RAM_START,
            BR2_PRIMITIVE_RAM_END,
            self.writes,
            self.command_like_writes,
            self.header_like_writes,
            self.header_write_vblank_by_address.len(),
            self.current_vblank_writes,
            self.current_vblank_command_like_writes,
            self.current_vblank_header_like_writes,
            self.last_vblank_writes,
            self.last_vblank_command_like_writes,
            self.last_vblank_header_like_writes,
            u64_command_opcode_counts_json(&self.opcode_counts),
            u64_command_opcode_counts_json(&self.current_vblank_opcode_counts),
            u64_command_opcode_counts_json(&self.last_vblank_opcode_counts),
            optional_u32_json(self.last_address),
            optional_u32_hex_json(self.last_address),
            self.last_value,
            self.last_value,
            optional_u32_json(self.last_pc),
            optional_u32_hex_json(self.last_pc),
            primitive_ram_write_samples_json(&self.recent_command_like_writes),
            primitive_ram_write_samples_json(&self.recent_header_like_writes)
        )
    }
}

impl BankedRomReadStats {
    fn record(&mut self, bank: u8, address: u32, offset: usize, width: usize, value: u32) {
        self.reads = self.reads.saturating_add(1);
        self.bytes = self.bytes.saturating_add(width as u64);
        if let Some(count) = self.bank_reads.get_mut(bank as usize) {
            *count = count.saturating_add(1);
        }
        self.last_bank = Some(bank);
        self.last_address = Some(address);
        self.last_offset = Some(offset);
        self.last_width = width as u8;
        self.last_value = value;
    }

    fn json(&self) -> String {
        let bank_reads = self
            .bank_reads
            .iter()
            .enumerate()
            .map(|(bank, reads)| format!("{{\"bank\":{},\"reads\":{}}}", bank, reads))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"reads\":{},\"bytes\":{},\"bank_reads\":[{}],\"last_bank\":{},\"last_address\":{},\"last_address_hex\":{},\"last_offset\":{},\"last_offset_hex\":{},\"last_width\":{},\"last_value\":{},\"last_value_hex\":\"0x{:08x}\"}}",
            self.reads,
            self.bytes,
            bank_reads,
            optional_u8_json(self.last_bank),
            optional_u32_json(self.last_address),
            optional_u32_hex_json(self.last_address),
            optional_usize_json(self.last_offset),
            optional_usize_hex_json(self.last_offset),
            self.last_width,
            self.last_value,
            self.last_value
        )
    }
}

impl Default for GpuLinkedListDmaStats {
    fn default() -> Self {
        Self {
            calls: 0,
            last_start: 0,
            last_first_node: 0,
            last_nodes: 0,
            last_words: 0,
            last_nonempty_nodes: 0,
            last_max_node_words: 0,
            last_min_command_address: None,
            last_max_command_address: None,
            last_command_opcode_counts: [0; 256],
            last_recent_commands: Vec::new(),
            last_visited_nodes: Vec::new(),
            last_first_node_samples: Vec::new(),
            last_tail_node_samples: Vec::new(),
            last_nonempty_node_samples: Vec::new(),
            recent_runs: Vec::new(),
            last_terminated: false,
            last_hit_node_limit: false,
            max_nodes: 0,
            max_words: 0,
            max_nonempty_nodes: 0,
            max_node_words: 0,
            node_limit_hits: 0,
        }
    }
}

impl GpuLinkedListDmaStats {
    fn merge_last(&mut self, last: GpuLinkedListDmaRunStats) {
        self.calls = self.calls.saturating_add(1);
        self.recent_runs
            .push(GpuLinkedListDmaRunSummary::from_run(self.calls, &last));
        if self.recent_runs.len() > GPU_LINKED_LIST_RECENT_COMMAND_LIMIT {
            let overflow = self.recent_runs.len() - GPU_LINKED_LIST_RECENT_COMMAND_LIMIT;
            self.recent_runs.drain(0..overflow);
        }
        self.last_start = last.last_start;
        self.last_first_node = last.last_first_node;
        self.last_nodes = last.last_nodes;
        self.last_words = last.last_words;
        self.last_nonempty_nodes = last.last_nonempty_nodes;
        self.last_max_node_words = last.last_max_node_words;
        self.last_min_command_address = last.last_min_command_address;
        self.last_max_command_address = last.last_max_command_address;
        self.last_command_opcode_counts = last.command_opcode_counts;
        self.last_recent_commands = last.recent_commands;
        self.last_visited_nodes = last.visited_nodes;
        self.last_first_node_samples = last.first_node_samples;
        self.last_tail_node_samples = last.tail_node_samples;
        self.last_nonempty_node_samples = last.nonempty_node_samples;
        self.last_terminated = last.terminated;
        self.last_hit_node_limit = last.hit_node_limit;
        self.max_nodes = self.max_nodes.max(last.last_nodes);
        self.max_words = self.max_words.max(last.last_words);
        self.max_nonempty_nodes = self.max_nonempty_nodes.max(last.last_nonempty_nodes);
        self.max_node_words = self.max_node_words.max(last.last_max_node_words);
        if last.hit_node_limit {
            self.node_limit_hits = self.node_limit_hits.saturating_add(1);
        }
    }

    fn json(&self) -> String {
        let recent_commands = self
            .last_recent_commands
            .iter()
            .map(GpuLinkedListCommandSample::json)
            .collect::<Vec<_>>()
            .join(",");
        let recent_runs = self
            .recent_runs
            .iter()
            .map(GpuLinkedListDmaRunSummary::json)
            .collect::<Vec<_>>()
            .join(",");
        let first_node_samples = gpu_linked_list_node_samples_json(&self.last_first_node_samples);
        let tail_node_samples = gpu_linked_list_node_samples_json(&self.last_tail_node_samples);
        let nonempty_node_samples =
            gpu_linked_list_node_samples_json(&self.last_nonempty_node_samples);
        format!(
            "{{\"calls\":{},\"last_start\":{},\"last_start_hex\":\"0x{:08x}\",\"last_first_node\":{},\"last_first_node_hex\":\"0x{:08x}\",\"last_nodes\":{},\"last_words\":{},\"last_nonempty_nodes\":{},\"last_max_node_words\":{},\"last_min_command_address\":{},\"last_min_command_address_hex\":{},\"last_max_command_address\":{},\"last_max_command_address_hex\":{},\"last_command_opcode_counts\":[{}],\"last_recent_commands\":[{}],\"last_first_node_samples\":[{}],\"last_tail_node_samples\":[{}],\"last_nonempty_node_samples\":[{}],\"recent_runs\":[{}],\"last_terminated\":{},\"last_hit_node_limit\":{},\"node_limit\":{},\"max_nodes\":{},\"max_words\":{},\"max_nonempty_nodes\":{},\"max_node_words\":{},\"node_limit_hits\":{}}}",
            self.calls,
            self.last_start,
            self.last_start,
            self.last_first_node,
            self.last_first_node,
            self.last_nodes,
            self.last_words,
            self.last_nonempty_nodes,
            self.last_max_node_words,
            optional_u32_json(self.last_min_command_address),
            optional_u32_hex_json(self.last_min_command_address),
            optional_u32_json(self.last_max_command_address),
            optional_u32_hex_json(self.last_max_command_address),
            command_opcode_counts_json(&self.last_command_opcode_counts),
            recent_commands,
            first_node_samples,
            tail_node_samples,
            nonempty_node_samples,
            recent_runs,
            self.last_terminated,
            self.last_hit_node_limit,
            GPU_LINKED_LIST_NODE_LIMIT,
            self.max_nodes,
            self.max_words,
            self.max_nonempty_nodes,
            self.max_node_words,
            self.node_limit_hits
        )
    }
}

#[derive(Clone, Debug)]
struct GpuLinkedListDmaRunStats {
    last_start: u32,
    last_first_node: u32,
    last_nodes: u32,
    last_words: u32,
    last_nonempty_nodes: u32,
    last_max_node_words: u32,
    last_min_command_address: Option<u32>,
    last_max_command_address: Option<u32>,
    command_opcode_counts: [u32; 256],
    recent_commands: Vec<GpuLinkedListCommandSample>,
    visited_nodes: Vec<u32>,
    first_node_samples: Vec<GpuLinkedListNodeSample>,
    tail_node_samples: Vec<GpuLinkedListNodeSample>,
    nonempty_node_samples: Vec<GpuLinkedListNodeSample>,
    terminated: bool,
    hit_node_limit: bool,
}

impl GpuLinkedListDmaRunStats {
    fn started(start_address: u32, first_node: u32) -> Self {
        Self {
            last_start: start_address,
            last_first_node: first_node,
            last_nodes: 0,
            last_words: 0,
            last_nonempty_nodes: 0,
            last_max_node_words: 0,
            last_min_command_address: None,
            last_max_command_address: None,
            command_opcode_counts: [0; 256],
            recent_commands: Vec::new(),
            visited_nodes: Vec::new(),
            first_node_samples: Vec::new(),
            tail_node_samples: Vec::new(),
            nonempty_node_samples: Vec::new(),
            terminated: false,
            hit_node_limit: false,
        }
    }

    fn record_node(&mut self, address: u32, header: u32) {
        let words = (header >> 24).min(1024);
        let sample = GpuLinkedListNodeSample::new(address, header);
        self.last_nodes = self.last_nodes.saturating_add(1);
        self.last_words = self.last_words.saturating_add(words);
        self.visited_nodes.push(address);
        if self.first_node_samples.len() < GPU_LINKED_LIST_NODE_SAMPLE_LIMIT {
            self.first_node_samples.push(sample);
        }
        self.tail_node_samples.push(sample);
        if self.tail_node_samples.len() > GPU_LINKED_LIST_NODE_SAMPLE_LIMIT {
            let overflow = self.tail_node_samples.len() - GPU_LINKED_LIST_NODE_SAMPLE_LIMIT;
            self.tail_node_samples.drain(0..overflow);
        }
        if words != 0 {
            self.last_nonempty_nodes = self.last_nonempty_nodes.saturating_add(1);
            self.last_max_node_words = self.last_max_node_words.max(words);
            if self.nonempty_node_samples.len() < GPU_LINKED_LIST_NONEMPTY_NODE_SAMPLE_LIMIT {
                self.nonempty_node_samples.push(sample);
            }
        }
    }

    fn record_command(&mut self, address: u32, command: u32) {
        self.last_min_command_address = Some(
            self.last_min_command_address
                .map_or(address, |current| current.min(address)),
        );
        self.last_max_command_address = Some(
            self.last_max_command_address
                .map_or(address, |current| current.max(address)),
        );
        let opcode = (command >> 24) as usize;
        self.command_opcode_counts[opcode] = self.command_opcode_counts[opcode].saturating_add(1);
    }

    fn record_command_group(&mut self, commands: &[(u32, u32)], range: std::ops::Range<usize>) {
        let Some((address, _)) = commands.get(range.start) else {
            return;
        };
        let words = commands[range]
            .iter()
            .map(|(_, command)| *command)
            .collect::<Vec<_>>();
        self.recent_commands
            .push(GpuLinkedListCommandSample::new(*address, words));
        if self.recent_commands.len() > GPU_LINKED_LIST_RECENT_COMMAND_LIMIT {
            let overflow = self.recent_commands.len() - GPU_LINKED_LIST_RECENT_COMMAND_LIMIT;
            self.recent_commands.drain(0..overflow);
        }
    }
}

#[derive(Clone, Debug)]
pub struct Bus {
    ram: Vec<u8>,
    scratchpad: Vec<u8>,
    rom: Vec<u8>,
    banked_roms: Vec<u8>,
    zn_board: ZnBoard,
    cache_control: u32,
    cache_isolated: bool,
    cache_isolation_transitions: u64,
    cache_isolated_write_count: u64,
    cache_isolated_write_bytes: u64,
    cache_isolated_last_address: Option<u32>,
    cache_isolated_last_width: u8,
    cache_isolated_last_value: u32,
    pending_dma_completion_cycles: [u64; DMA_CHANNEL_COUNT],
    vblank_cycle_accumulator: u64,
    vblank_count: u64,
    vblank_draw_sync_clears: u64,
    draw_sync_game_set_writes: u64,
    draw_sync_game_clear_writes: u64,
    draw_sync_game_other_writes: u64,
    draw_sync_last_game_write_value: Option<u32>,
    draw_sync_last_game_write_pc: Option<u32>,
    gpu_linked_list_dma: GpuLinkedListDmaStats,
    primitive_ram_writes: PrimitiveRamWriteStats,
    unlinked_primitive_replay: UnlinkedPrimitiveReplayStats,
    dma_activity: Vec<DmaActivitySample>,
    banked_rom_reads: RefCell<BankedRomReadStats>,
    board_asset_status: NativeBoardAssetStatus,
    pub io: Io,
    access_trace_limit: usize,
    access_trace_watch_only: bool,
    access_trace_watch_ranges: Vec<BusTraceWatchRange>,
    trace_pc: Cell<Option<u32>>,
    trace_cycles: Cell<u64>,
    access_trace: RefCell<Vec<BusAccessTraceEvent>>,
}

impl Bus {
    pub fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Self::with_banked_roms(rom, Vec::new(), ram_size)
    }

    pub fn with_banked_roms(rom: Vec<u8>, banked_roms: Vec<u8>, ram_size: usize) -> Self {
        Self::with_board_assets(rom, banked_roms, ram_size, NativeBoardAssets::default())
    }

    pub fn with_board_assets(
        rom: Vec<u8>,
        banked_roms: Vec<u8>,
        ram_size: usize,
        board_assets: NativeBoardAssets,
    ) -> Self {
        let board_asset_status = NativeBoardAssetStatus::from_assets(&board_assets);
        let mut bus = Self {
            ram: vec![0; ram_size],
            scratchpad: vec![0; 1024],
            rom,
            banked_roms,
            zn_board: ZnBoard::with_at28c16(board_assets.at28c16),
            cache_control: 0,
            cache_isolated: false,
            cache_isolation_transitions: 0,
            cache_isolated_write_count: 0,
            cache_isolated_write_bytes: 0,
            cache_isolated_last_address: None,
            cache_isolated_last_width: 0,
            cache_isolated_last_value: 0,
            pending_dma_completion_cycles: [0; DMA_CHANNEL_COUNT],
            vblank_cycle_accumulator: 0,
            vblank_count: 0,
            vblank_draw_sync_clears: 0,
            draw_sync_game_set_writes: 0,
            draw_sync_game_clear_writes: 0,
            draw_sync_game_other_writes: 0,
            draw_sync_last_game_write_value: None,
            draw_sync_last_game_write_pc: None,
            gpu_linked_list_dma: GpuLinkedListDmaStats::default(),
            primitive_ram_writes: PrimitiveRamWriteStats::default(),
            unlinked_primitive_replay: UnlinkedPrimitiveReplayStats::default(),
            dma_activity: Vec::new(),
            banked_rom_reads: RefCell::new(BankedRomReadStats::default()),
            board_asset_status,
            io: Io::default(),
            access_trace_limit: 0,
            access_trace_watch_only: false,
            access_trace_watch_ranges: Vec::new(),
            trace_pc: Cell::new(None),
            trace_cycles: Cell::new(0),
            access_trace: RefCell::new(Vec::new()),
        };
        bus.io
            .controller
            .set_cat702_transforms(board_assets.cat702_1, board_assets.cat702_2);
        if let Some(response) = zn_security_response_from_bios(&bus.rom) {
            bus.io.controller.set_security_response(response);
        }
        bus
    }

    pub fn read_u8(&self, address: u32) -> u8 {
        if cache_control_address(address) {
            let value = self.cache_control as u8;
            self.record_access_trace("read", "cache_control", address, 1, value as u32);
            return value;
        }

        if mapped_zn_board_address(address).is_some() {
            let value = self.zn_board.read(address, 1) as u8;
            self.record_access_trace("read", "zn_board", address, 1, value as u32);
            return value;
        }

        if let Some(io_address) = mapped_io_address(address, 1) {
            let value = self.io.read_u8(io_address);
            self.record_access_trace("read", "io", address, 1, value as u32);
            return value;
        }

        self.read_bytes(address, 1)[0]
    }

    pub fn read_u16(&self, address: u32) -> u16 {
        if cache_control_address(address) {
            let value = self.cache_control as u16;
            self.record_access_trace("read", "cache_control", address, 2, value as u32);
            return value;
        }

        if mapped_zn_board_address(address).is_some() {
            let value = self.zn_board.read(address, 2) as u16;
            self.record_access_trace("read", "zn_board", address, 2, value as u32);
            return value;
        }

        if let Some(io_address) = mapped_io_address(address, 2) {
            let value = self.io.read_u16(io_address);
            self.record_access_trace("read", "io", address, 2, value as u32);
            return value;
        }

        let bytes = self.read_bytes(address, 2);
        PreferredNativePlatform::read_le_u16(&bytes)
    }

    pub fn read_u32(&self, address: u32) -> u32 {
        if cache_control_address(address) {
            let value = self.cache_control;
            self.record_access_trace("read", "cache_control", address, 4, value);
            return value;
        }

        if mapped_zn_board_address(address).is_some() {
            let value = self.zn_board.read(address, 4);
            self.record_access_trace("read", "zn_board", address, 4, value);
            return value;
        }

        if let Some(io_address) = mapped_io_address(address, 4) {
            let value = self.io.read_u32(io_address);
            self.record_access_trace("read", "io", address, 4, value);
            return value;
        }

        let bytes = self.read_bytes(address, 4);
        PreferredNativePlatform::read_le_u32(&bytes)
    }

    pub fn write_u8(&mut self, address: u32, value: u8) {
        if cache_control_address(address) {
            self.cache_control = board_write_lane(
                self.cache_control,
                address & !0x03,
                address,
                value as u32,
                1,
            );
            self.record_access_trace("write", "cache_control", address, 1, value as u32);
            return;
        }

        if mapped_zn_board_address(address).is_some() {
            self.zn_board.write(address, value as u32, 1);
            self.sync_security_selects();
            self.record_access_trace("write", "zn_board", address, 1, value as u32);
            return;
        }

        if let Some(io_address) = mapped_io_address(address, 1) {
            self.io.write_u8(io_address, value);
            if io_address == IRQ_STATUS {
                self.raise_dma_irq_if_pending();
            }
            self.record_access_trace("write", "io", address, 1, value as u32);
            return;
        }

        self.write_bytes(address, &[value]);
    }

    pub fn write_u16(&mut self, address: u32, value: u16) {
        if cache_control_address(address) {
            self.cache_control = board_write_lane(
                self.cache_control,
                address & !0x03,
                address,
                value as u32,
                2,
            );
            self.record_access_trace("write", "cache_control", address, 2, value as u32);
            return;
        }

        if mapped_zn_board_address(address).is_some() {
            self.zn_board.write(address, value as u32, 2);
            self.sync_security_selects();
            self.record_access_trace("write", "zn_board", address, 2, value as u32);
            return;
        }

        if let Some(io_address) = mapped_io_address(address, 2) {
            self.io.write_u16(io_address, value);
            if io_address == IRQ_STATUS {
                self.raise_dma_irq_if_pending();
            }
            self.record_access_trace("write", "io", address, 2, value as u32);
            return;
        }

        self.write_bytes(address, &PreferredNativePlatform::write_le_u16(value));
    }

    pub fn write_u32(&mut self, address: u32, value: u32) {
        if cache_control_address(address) {
            self.cache_control = value;
            self.record_access_trace("write", "cache_control", address, 4, value);
            return;
        }

        if mapped_zn_board_address(address).is_some() {
            self.zn_board.write(address, value, 4);
            self.sync_security_selects();
            self.record_access_trace("write", "zn_board", address, 4, value);
            return;
        }

        if let Some(io_address) = mapped_io_address(address, 4) {
            if io_address == GPU_GP0 {
                self.io.gpu.write_gp0_with_source(
                    value,
                    GpuCommandSource::cpu_io(address, self.trace_pc.get()),
                );
                self.record_access_trace("write", "io", address, 4, value);
                return;
            }
            let dma_state_may_change = dma_io_address(io_address);
            self.io.write_u32(io_address, value);
            self.record_dma_register_write(io_address, value);
            self.process_dma_transfer(io_address, value);
            if dma_state_may_change {
                self.sync_dma_irq();
            } else if io_address == IRQ_STATUS {
                self.raise_dma_irq_if_pending();
            }
            self.record_access_trace("write", "io", address, 4, value);
            return;
        }

        let bytes = PreferredNativePlatform::write_le_u32(value);
        self.write_bytes(address, &bytes);
    }

    pub fn try_copy_aligned_words(
        &mut self,
        source: u32,
        destination: u32,
        byte_count: u32,
    ) -> Option<(u32, u32)> {
        if byte_count == 0
            || byte_count & 0x03 != 0
            || source & 0x03 != 0
            || destination & 0x03 != 0
        {
            return None;
        }
        let byte_len = byte_count as usize;
        if !self.word_copy_readable_range(source, byte_len)
            || !self.word_copy_writable_range(destination, byte_len)
        {
            return None;
        }

        let words = byte_count / 4;
        let mut last_word = 0;
        for index in 0..words {
            let offset = index.saturating_mul(4);
            let value = self.read_u32(source.wrapping_add(offset));
            self.write_u32(destination.wrapping_add(offset), value);
            last_word = value;
        }

        Some((words, last_word))
    }

    pub fn try_copy_bytes(
        &mut self,
        source: u32,
        destination: u32,
        byte_count: u32,
    ) -> Option<Vec<u8>> {
        if byte_count == 0 || self.cache_isolated() && cacheable_address(destination) {
            return None;
        }
        let byte_len = byte_count as usize;
        if !self.word_copy_readable_range(source, byte_len)
            || !self.word_copy_writable_range(destination, byte_len)
        {
            return None;
        }

        let bytes = self.read_bytes(source, byte_len);
        self.write_bytes(destination, &bytes);
        Some(bytes)
    }

    pub fn try_copy_halfwords(
        &mut self,
        source: u32,
        destination: u32,
        halfword_count: u32,
    ) -> Option<u16> {
        if halfword_count == 0 || self.cache_isolated() && cacheable_address(destination) {
            return None;
        }
        let byte_len = (halfword_count as usize).checked_mul(2)?;
        if !self.word_copy_readable_range(source, byte_len)
            || !self.word_copy_writable_range(destination, byte_len)
        {
            return None;
        }

        let mut last = 0;
        for index in 0..halfword_count {
            let offset = index.saturating_mul(2);
            let value = self.read_u16(source.wrapping_add(offset));
            self.write_u16(destination.wrapping_add(offset), value);
            last = value;
        }

        Some(last)
    }

    pub fn try_fill_aligned_words(
        &mut self,
        destination: u32,
        byte_count: u32,
        value: u32,
    ) -> Option<u32> {
        if byte_count == 0 || byte_count & 0x03 != 0 || destination & 0x03 != 0 {
            return None;
        }
        let byte_len = byte_count as usize;
        if !self.word_copy_writable_range(destination, byte_len) {
            return None;
        }

        let words = byte_count / 4;
        for index in 0..words {
            self.write_u32(destination.wrapping_add(index.saturating_mul(4)), value);
        }

        Some(words)
    }

    pub fn rom_len(&self) -> usize {
        self.rom.len()
    }

    pub fn ram_len(&self) -> usize {
        self.ram.len()
    }

    pub fn scratchpad_len(&self) -> usize {
        self.scratchpad.len()
    }

    pub fn banked_rom_len(&self) -> usize {
        self.banked_roms.len()
    }

    fn word_copy_readable_range(&self, address: u32, byte_len: usize) -> bool {
        ram_offset(address, self.ram.len(), byte_len).is_some()
            || scratchpad_offset(address, self.scratchpad.len(), byte_len).is_some()
            || rom_offset(address, self.rom.len(), byte_len).is_some()
            || banked_rom_offset(
                address,
                self.banked_roms.len(),
                byte_len,
                self.zn_board.rom_bank,
            )
            .is_some()
    }

    fn word_copy_writable_range(&self, address: u32, byte_len: usize) -> bool {
        ram_offset(address, self.ram.len(), byte_len).is_some()
            || scratchpad_offset(address, self.scratchpad.len(), byte_len).is_some()
    }

    pub fn set_cache_isolated(&mut self, isolated: bool) {
        if self.cache_isolated != isolated {
            self.cache_isolation_transitions = self.cache_isolation_transitions.saturating_add(1);
        }
        self.cache_isolated = isolated;
    }

    pub fn cache_isolated(&self) -> bool {
        self.cache_isolated
    }

    pub fn tick(&mut self, cycles: u64) {
        let timer_irqs = self.io.tick(cycles);
        self.io.irq.status |= timer_irqs;
        self.tick_pending_dma(cycles);
        self.vblank_cycle_accumulator = self.vblank_cycle_accumulator.saturating_add(cycles);
        while self.vblank_cycle_accumulator >= VBLANK_CYCLES {
            self.vblank_cycle_accumulator -= VBLANK_CYCLES;
            self.vblank_count = self.vblank_count.saturating_add(1);
            self.primitive_ram_writes.advance_vblank();
            self.io.gpu.capture_vblank_presented_frame();
            self.io.irq.status |= 1;
            self.complete_draw_sync_on_vblank();
        }
    }

    pub fn vblank_count(&self) -> u64 {
        self.vblank_count
    }

    pub fn cycles_until_next_vblank(&self) -> u64 {
        VBLANK_CYCLES.saturating_sub(self.vblank_cycle_accumulator)
    }

    pub fn zn_board_json(&self) -> String {
        format!(
            "{{\"state\":{},\"assets\":{}}}",
            self.zn_board.json(),
            self.board_asset_status.json()
        )
    }

    pub fn native_sync_json(&self) -> String {
        format!(
            "{{\"br2_draw_sync_flag\":{},\"vblank_count\":{},\"vblank_cycle_accumulator\":{},\"vblank_draw_sync_clears\":{},\"game_set_writes\":{},\"game_clear_writes\":{},\"game_other_writes\":{},\"last_game_write_value\":{},\"last_game_write_pc\":{},\"cache\":{},\"banked_rom_reads\":{},\"dma_activity\":[{}],\"gpu_linked_list_dma\":{},\"primitive_ram_writes\":{},\"unlinked_primitive_replay\":{},\"primitive_packet_scan\":{}}}",
            self.read_ram_u32_physical(BR2_DRAW_SYNC_FLAG_PHYSICAL)
                .unwrap_or(0),
            self.vblank_count,
            self.vblank_cycle_accumulator,
            self.vblank_draw_sync_clears,
            self.draw_sync_game_set_writes,
            self.draw_sync_game_clear_writes,
            self.draw_sync_game_other_writes,
            optional_u32_json(self.draw_sync_last_game_write_value),
            optional_u32_hex_json(self.draw_sync_last_game_write_pc),
            self.cache_json(),
            self.banked_rom_reads.borrow().json(),
            self.dma_activity_json(),
            self.gpu_linked_list_dma.json(),
            self.primitive_ram_writes.json(),
            self.unlinked_primitive_replay.json(),
            self.primitive_packet_scan_json()
        )
    }

    fn native_sync_compact_json(&self) -> String {
        format!(
            "{{\"br2_draw_sync_flag\":{},\"vblank_count\":{},\"vblank_cycle_accumulator\":{},\"vblank_draw_sync_clears\":{},\"game_set_writes\":{},\"game_clear_writes\":{},\"game_other_writes\":{},\"last_game_write_value\":{},\"last_game_write_pc\":{},\"cache_isolated\":{},\"cache_isolation_transitions\":{},\"dma_irq_pending\":{},\"pending_dma_completion_cycles\":[{}],\"banked_rom_reads\":{},\"gpu_linked_list_dma\":{{\"calls\":{},\"last_start_hex\":\"0x{:08x}\",\"last_nodes\":{},\"last_words\":{},\"last_nonempty_nodes\":{},\"last_terminated\":{},\"last_hit_node_limit\":{},\"node_limit_hits\":{}}},\"primitive_ram_writes\":{{\"writes\":{},\"command_like_writes\":{},\"header_like_writes\":{},\"current_vblank_header_like_writes\":{},\"last_vblank_header_like_writes\":{}}},\"unlinked_primitive_replay\":{{\"attempts\":{},\"conditional_replays\":{},\"forced_replays\":{},\"skipped\":{},\"last_reason\":\"{}\",\"last_packets\":{},\"last_words\":{}}}}}",
            self.read_ram_u32_physical(BR2_DRAW_SYNC_FLAG_PHYSICAL)
                .unwrap_or(0),
            self.vblank_count,
            self.vblank_cycle_accumulator,
            self.vblank_draw_sync_clears,
            self.draw_sync_game_set_writes,
            self.draw_sync_game_clear_writes,
            self.draw_sync_game_other_writes,
            optional_u32_json(self.draw_sync_last_game_write_value),
            optional_u32_hex_json(self.draw_sync_last_game_write_pc),
            self.cache_isolated,
            self.cache_isolation_transitions,
            self.io.dma.irq_pending(),
            self.pending_dma_completion_cycles
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(","),
            self.banked_rom_reads.borrow().json(),
            self.gpu_linked_list_dma.calls,
            self.gpu_linked_list_dma.last_start,
            self.gpu_linked_list_dma.last_nodes,
            self.gpu_linked_list_dma.last_words,
            self.gpu_linked_list_dma.last_nonempty_nodes,
            self.gpu_linked_list_dma.last_terminated,
            self.gpu_linked_list_dma.last_hit_node_limit,
            self.gpu_linked_list_dma.node_limit_hits,
            self.primitive_ram_writes.writes,
            self.primitive_ram_writes.command_like_writes,
            self.primitive_ram_writes.header_like_writes,
            self.primitive_ram_writes.current_vblank_header_like_writes,
            self.primitive_ram_writes.last_vblank_header_like_writes,
            self.unlinked_primitive_replay.attempts,
            self.unlinked_primitive_replay.conditional_replays,
            self.unlinked_primitive_replay.forced_replays,
            self.unlinked_primitive_replay.skipped,
            self.unlinked_primitive_replay.last_reason,
            self.unlinked_primitive_replay.last_packets,
            self.unlinked_primitive_replay.last_words
        )
    }

    fn dma_activity_json(&self) -> String {
        self.dma_activity
            .iter()
            .map(DmaActivitySample::json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn primitive_packet_scan_json(&self) -> String {
        let linked_nodes = self
            .gpu_linked_list_dma
            .last_visited_nodes
            .iter()
            .map(|address| address & 0x00ff_fffc)
            .collect::<HashSet<_>>();
        let mut candidates = 0u64;
        let mut linked_candidates = 0u64;
        let mut unlinked_candidates = 0u64;
        let mut candidate_words = 0u64;
        let mut linked_words = 0u64;
        let mut unlinked_words = 0u64;
        let mut opcode_counts = [0u64; 256];
        let mut linked_opcode_counts = [0u64; 256];
        let mut unlinked_opcode_counts = [0u64; 256];
        let mut linked_samples = Vec::new();
        let mut unlinked_samples = Vec::new();
        let current_vblank = self.vblank_count;
        let previous_vblank = self.vblank_count.saturating_sub(1);
        let mut current_vblank_candidates = 0u64;
        let mut previous_vblank_candidates = 0u64;
        let mut current_vblank_linked_candidates = 0u64;
        let mut previous_vblank_linked_candidates = 0u64;

        let mut address = BR2_PRIMITIVE_RAM_START;
        while address.saturating_add(8) <= BR2_PRIMITIVE_RAM_END {
            if let Some(sample) = self.primitive_packet_candidate_sample(address, &linked_nodes) {
                let opcode_index = (sample.first_command >> 24) as usize;
                let words = u64::from(sample.word_count);
                candidates = candidates.saturating_add(1);
                candidate_words = candidate_words.saturating_add(words);
                opcode_counts[opcode_index] = opcode_counts[opcode_index].saturating_add(1);
                if sample.header_write_vblank == Some(current_vblank) {
                    current_vblank_candidates = current_vblank_candidates.saturating_add(1);
                    if sample.linked {
                        current_vblank_linked_candidates =
                            current_vblank_linked_candidates.saturating_add(1);
                    }
                }
                if sample.header_write_vblank == Some(previous_vblank) {
                    previous_vblank_candidates = previous_vblank_candidates.saturating_add(1);
                    if sample.linked {
                        previous_vblank_linked_candidates =
                            previous_vblank_linked_candidates.saturating_add(1);
                    }
                }
                if sample.linked {
                    linked_candidates = linked_candidates.saturating_add(1);
                    linked_words = linked_words.saturating_add(words);
                    linked_opcode_counts[opcode_index] =
                        linked_opcode_counts[opcode_index].saturating_add(1);
                    if linked_samples.len() < PRIMITIVE_PACKET_SCAN_SAMPLE_LIMIT {
                        linked_samples.push(sample);
                    }
                } else {
                    unlinked_candidates = unlinked_candidates.saturating_add(1);
                    unlinked_words = unlinked_words.saturating_add(words);
                    unlinked_opcode_counts[opcode_index] =
                        unlinked_opcode_counts[opcode_index].saturating_add(1);
                    if unlinked_samples.len() < PRIMITIVE_PACKET_SCAN_SAMPLE_LIMIT {
                        unlinked_samples.push(sample);
                    }
                }
            }
            address = address.saturating_add(4);
        }

        let linked_samples_json = linked_samples
            .iter()
            .map(PrimitivePacketCandidateSample::json)
            .collect::<Vec<_>>()
            .join(",");
        let unlinked_samples_json = unlinked_samples
            .iter()
            .map(PrimitivePacketCandidateSample::json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"range_start\":\"0x{:08x}\",\"range_end\":\"0x{:08x}\",\"max_packet_words\":{},\"current_vblank\":{},\"previous_vblank\":{},\"last_dma_visited_nodes\":{},\"candidates\":{},\"linked_candidates\":{},\"unlinked_candidates\":{},\"current_vblank_candidates\":{},\"current_vblank_linked_candidates\":{},\"previous_vblank_candidates\":{},\"previous_vblank_linked_candidates\":{},\"candidate_words\":{},\"linked_words\":{},\"unlinked_words\":{},\"opcode_counts\":[{}],\"linked_opcode_counts\":[{}],\"unlinked_opcode_counts\":[{}],\"linked_samples\":[{}],\"unlinked_samples\":[{}]}}",
            BR2_PRIMITIVE_RAM_START,
            BR2_PRIMITIVE_RAM_END,
            PRIMITIVE_PACKET_MAX_WORDS,
            current_vblank,
            previous_vblank,
            linked_nodes.len(),
            candidates,
            linked_candidates,
            unlinked_candidates,
            current_vblank_candidates,
            current_vblank_linked_candidates,
            previous_vblank_candidates,
            previous_vblank_linked_candidates,
            candidate_words,
            linked_words,
            unlinked_words,
            u64_command_opcode_counts_json(&opcode_counts),
            u64_command_opcode_counts_json(&linked_opcode_counts),
            u64_command_opcode_counts_json(&unlinked_opcode_counts),
            linked_samples_json,
            unlinked_samples_json
        )
    }

    fn primitive_packet_candidate_sample(
        &self,
        address: u32,
        linked_nodes: &HashSet<u32>,
    ) -> Option<PrimitivePacketCandidateSample> {
        let header = self.read_ram_u32_physical(address)?;
        let word_count = header >> 24;
        if !(1..=PRIMITIVE_PACKET_MAX_WORDS).contains(&word_count) {
            return None;
        }

        let packet_end = address
            .checked_add(4)?
            .checked_add(word_count.checked_mul(4)?)?;
        if packet_end > BR2_PRIMITIVE_RAM_END {
            return None;
        }

        let next = header & 0x00ff_ffff;
        if !primitive_packet_next_plausible(next) {
            return None;
        }

        let first_command = self.read_ram_u32_physical(address + 4)?;
        let opcode = (first_command >> 24) as u8;
        if !looks_like_gp0_command_opcode(opcode) {
            return None;
        }

        if !self.primitive_packet_words_plausible(address, word_count) {
            return None;
        }

        Some(PrimitivePacketCandidateSample {
            address,
            header,
            word_count,
            next,
            linked: linked_nodes.contains(&(address & 0x00ff_fffc)),
            first_command,
            header_write_vblank: self.primitive_ram_writes.header_write_vblank(address),
        })
    }

    fn primitive_packet_words_plausible(&self, address: u32, word_count: u32) -> bool {
        let mut commands = Vec::with_capacity(word_count as usize);
        for index in 0..word_count {
            let Some(command) = self.read_ram_u32_physical(address + 4 + index * 4) else {
                return false;
            };
            commands.push(command);
        }

        let mut offset = 0usize;
        while offset < commands.len() {
            let Some(command_words) = gp0_command_word_count(&commands[offset..]) else {
                return false;
            };
            if command_words == 0 || offset + command_words > commands.len() {
                return false;
            }
            offset += command_words;
        }
        true
    }

    fn primitive_packet_has_playfield_draw_bounds(&self, address: u32, word_count: u32) -> bool {
        let mut commands = Vec::with_capacity(word_count as usize);
        for index in 0..word_count {
            let Some(command) = self.read_ram_u32_physical(address + 4 + index * 4) else {
                return false;
            };
            commands.push(command);
        }

        let mut offset = 0usize;
        while offset < commands.len() {
            let Some(command_words) = gp0_command_word_count(&commands[offset..]) else {
                return false;
            };
            if command_words == 0 || offset + command_words > commands.len() {
                return false;
            }
            if gp0_command_has_playfield_draw_bounds(&commands[offset..offset + command_words]) {
                return true;
            }
            offset += command_words;
        }
        false
    }

    fn cache_json(&self) -> String {
        format!(
            "{{\"control\":{},\"control_hex\":\"0x{:08x}\",\"isolated\":{},\"isolation_transitions\":{},\"isolated_write_count\":{},\"isolated_write_bytes\":{},\"isolated_last_address\":{},\"isolated_last_address_hex\":{},\"isolated_last_width\":{},\"isolated_last_value\":{},\"isolated_last_value_hex\":\"0x{:08x}\"}}",
            self.cache_control,
            self.cache_control,
            self.cache_isolated,
            self.cache_isolation_transitions,
            self.cache_isolated_write_count,
            self.cache_isolated_write_bytes,
            optional_u32_json(self.cache_isolated_last_address),
            optional_u32_hex_json(self.cache_isolated_last_address),
            self.cache_isolated_last_width,
            self.cache_isolated_last_value,
            self.cache_isolated_last_value
        )
    }

    pub fn io_json(&self) -> String {
        self.io.json()
    }

    pub fn io_compact_json(&self) -> String {
        self.io.compact_json()
    }

    pub fn runtime_probe_json(&self) -> String {
        format!(
            "{{\"io\":{},\"zn_board\":{{\"state\":{},\"assets\":{}}},\"native_sync\":{}}}",
            self.io.runtime_probe_json(),
            self.zn_board.runtime_probe_json(),
            self.board_asset_status.json(),
            self.native_sync_json()
        )
    }

    pub fn runtime_compact_probe_json(&self) -> String {
        format!(
            "{{\"io\":{},\"zn_board\":{{\"state\":{},\"assets\":{}}},\"native_sync\":{}}}",
            self.io.runtime_compact_probe_json(),
            self.zn_board.runtime_probe_json(),
            self.board_asset_status.json(),
            self.native_sync_compact_json()
        )
    }

    pub fn native_playability_json(&self) -> String {
        self.io.native_playability_json()
    }

    pub fn native_playable_candidate(&self) -> bool {
        self.io.native_playable_candidate()
    }

    pub fn display_rgb_frame(&self) -> (usize, usize, Vec<u32>) {
        self.io.display_rgb_frame()
    }

    pub fn set_gpu_draw_capture_range(&mut self, start: u64, end: u64) {
        self.io.gpu.set_draw_capture_range(start, end);
    }

    pub fn gpu_draw_captures(&self) -> &[NativeGpuDrawCapture] {
        self.io.gpu.draw_captures()
    }

    pub fn gpu_display_candidates(&self) -> Vec<NativeGpuDisplayCandidate> {
        self.io.gpu.display_candidate_pngs()
    }

    pub fn set_input(&mut self, buttons: ActionButtons) {
        self.io.set_input(buttons);
        self.zn_board.set_input(buttons);
    }

    pub fn input_activity(&self) -> NativeInputActivity {
        self.zn_board.input_activity()
    }

    pub fn set_access_trace_limit(&mut self, limit: usize) {
        self.access_trace_limit = limit;
        self.access_trace.get_mut().clear();
    }

    pub fn set_access_trace_watch_ranges(&mut self, ranges: Vec<(u32, u32)>) {
        self.access_trace_watch_ranges = ranges
            .into_iter()
            .filter_map(|(address, len)| BusTraceWatchRange::new(address, len))
            .collect();
        self.access_trace.get_mut().clear();
    }

    pub fn set_access_trace_watch_only(&mut self, watch_only: bool) {
        self.access_trace_watch_only = watch_only;
        self.access_trace.get_mut().clear();
    }

    pub fn set_trace_context(&self, pc: u32, cycles: u64) {
        self.trace_pc.set(Some(pc));
        self.trace_cycles.set(cycles);
    }

    pub fn clear_trace_context(&self) {
        self.trace_pc.set(None);
    }

    pub fn access_trace_json(&self) -> String {
        self.access_trace
            .borrow()
            .iter()
            .map(BusAccessTraceEvent::json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn record_access_trace(
        &self,
        operation: &'static str,
        region: &'static str,
        address: u32,
        width: u8,
        value: u32,
    ) {
        if self.access_trace_limit == 0 {
            return;
        }
        if self.access_trace_watch_only && !self.watch_matches(address, width as usize) {
            return;
        }

        let mut events = self.access_trace.borrow_mut();
        events.push(BusAccessTraceEvent {
            operation,
            region,
            address,
            physical_address: physical_address(address),
            width,
            value,
            pc: self.trace_pc.get(),
            cycles: self.trace_cycles.get(),
        });
        if events.len() > self.access_trace_limit {
            events.remove(0);
        }
    }

    fn record_watch_trace(
        &self,
        operation: &'static str,
        region: &'static str,
        address: u32,
        width: usize,
        value: u32,
    ) {
        if self.watch_matches(address, width) {
            self.record_access_trace(operation, region, address, width as u8, value);
        }
    }

    fn watch_matches(&self, address: u32, len: usize) -> bool {
        if self.access_trace_watch_ranges.is_empty() || len == 0 {
            return false;
        }

        let start = physical_address(address);
        let end = start.saturating_add(len as u32);
        self.access_trace_watch_ranges
            .iter()
            .any(|range| ranges_overlap(start, end, range.start, range.end))
    }

    fn sync_security_selects(&mut self) {
        self.io.controller.set_security_selects(
            self.zn_board.cat702_1_select(),
            self.zn_board.cat702_2_select(),
            self.zn_board.zn_mcu_analog_read(),
            self.zn_board.zn_mcu_trackball_read(),
            self.zn_board.zn_mcu_selected(),
        );
    }

    fn sync_dma_irq(&mut self) {
        if self.io.dma.irq_pending() {
            self.io.irq.status |= 1 << 3;
        } else {
            self.io.irq.status &= !(1 << 3);
        }
    }

    pub fn acknowledge_hle_bios_irq_sources(&mut self, pending: u32) {
        if pending & (1 << 3) != 0 {
            self.io.dma.acknowledge_pending_irq_flags();
        }
        self.io.irq.status &= !pending;
        self.sync_dma_irq();
    }

    fn raise_dma_irq_if_pending(&mut self) {
        if self.io.dma.irq_pending() {
            self.io.irq.status |= 1 << 3;
        }
    }

    fn record_dma_register_write(&mut self, io_address: u32, value: u32) {
        let Some((channel, register)) = dma_activity_register_metadata(io_address) else {
            return;
        };
        let (madr, bcr, chcr) = self.dma_channel_snapshot(channel);
        self.push_dma_activity(DmaActivitySample {
            kind: "register_write",
            channel,
            register: Some(register),
            address: Some(io_address),
            value: Some(value),
            madr,
            bcr,
            chcr,
            start: None,
            end: None,
            words: 0,
            nodes: 0,
            nonempty_nodes: 0,
            pc: self.trace_pc.get(),
            vblank: self.vblank_count,
            cycles: self.trace_cycles.get(),
        });
    }

    fn record_gpu_linked_list_dma_activity(
        &mut self,
        start_address: u32,
        stats: &GpuLinkedListDmaRunStats,
    ) {
        let (madr, bcr, chcr) = self.dma_channel_snapshot(DMA_GPU_CHANNEL);
        self.push_dma_activity(DmaActivitySample {
            kind: "gpu_linked_list",
            channel: DMA_GPU_CHANNEL,
            register: None,
            address: None,
            value: None,
            madr,
            bcr,
            chcr,
            start: Some(start_address & 0x00ff_fffc),
            end: stats.last_max_command_address,
            words: stats.last_words,
            nodes: stats.last_nodes,
            nonempty_nodes: stats.last_nonempty_nodes,
            pc: self.trace_pc.get(),
            vblank: self.vblank_count,
            cycles: self.trace_cycles.get(),
        });
    }

    fn record_otc_dma_activity(&mut self, start_address: u32, words: u32) {
        let (madr, bcr, chcr) = self.dma_channel_snapshot(DMA_OTC_CHANNEL);
        let start = start_address & 0x00ff_fffc;
        let end = if words == 0 {
            None
        } else {
            Some(start.wrapping_sub(words.saturating_sub(1).saturating_mul(4)) & 0x00ff_fffc)
        };
        self.push_dma_activity(DmaActivitySample {
            kind: "otc_clear",
            channel: DMA_OTC_CHANNEL,
            register: None,
            address: None,
            value: None,
            madr,
            bcr,
            chcr,
            start: Some(start),
            end,
            words,
            nodes: words,
            nonempty_nodes: 0,
            pc: self.trace_pc.get(),
            vblank: self.vblank_count,
            cycles: self.trace_cycles.get(),
        });
    }

    fn dma_channel_snapshot(&self, channel: usize) -> (u32, u32, u32) {
        self.io
            .dma
            .channel_state(channel)
            .map_or((0, 0, 0), |state| (state.madr, state.bcr, state.chcr))
    }

    fn push_dma_activity(&mut self, sample: DmaActivitySample) {
        self.dma_activity.push(sample);
        if self.dma_activity.len() > DMA_ACTIVITY_RECENT_LIMIT {
            let overflow = self.dma_activity.len() - DMA_ACTIVITY_RECENT_LIMIT;
            self.dma_activity.drain(0..overflow);
        }
    }

    fn tick_pending_dma(&mut self, cycles: u64) {
        if cycles == 0 {
            return;
        }

        let mut completed_dma = false;
        for channel in 0..self.pending_dma_completion_cycles.len() {
            let remaining = &mut self.pending_dma_completion_cycles[channel];
            if *remaining == 0 {
                continue;
            }

            *remaining = (*remaining).saturating_sub(cycles);
            if *remaining == 0 {
                self.io.dma.complete_channel(channel);
                completed_dma = true;
            }
        }

        if completed_dma {
            self.sync_dma_irq();
        }
    }

    fn schedule_dma_completion(&mut self, channel: usize, delay_cycles: u64) {
        if let Some(remaining) = self.pending_dma_completion_cycles.get_mut(channel) {
            *remaining = delay_cycles.max(1);
        }
    }

    fn complete_draw_sync_on_vblank(&mut self) {
        let Some(value) = self.read_ram_u32_physical(BR2_DRAW_SYNC_FLAG_PHYSICAL) else {
            return;
        };
        if value == 0 {
            return;
        }

        if self.write_ram_u32_physical(BR2_DRAW_SYNC_FLAG_PHYSICAL, 0) {
            self.vblank_draw_sync_clears += 1;
            self.record_watch_trace("write", "ram", BR2_DRAW_SYNC_FLAG_VIRTUAL, 4, 0);
        }
    }

    fn process_dma_transfer(&mut self, io_address: u32, control: u32) {
        if control & (1 << 24) == 0 {
            return;
        }

        match io_address {
            DMA_MDEC_IN_CHCR => self.process_mdec_in_dma(control),
            DMA_MDEC_OUT_CHCR => self.process_mdec_out_dma(control),
            DMA_GPU_CHCR => self.process_gpu_dma(control),
            DMA_OTC_CHCR => self.process_otc_dma(),
            _ => {}
        }
    }

    fn process_mdec_in_dma(&mut self, control: u32) {
        if control & DMA_DIRECTION_FROM_RAM == 0 {
            return;
        }

        let Some(channel) = self.io.dma.channel_state(DMA_MDEC_IN_CHANNEL) else {
            return;
        };
        let words = dma_word_count(channel.bcr).min(self.ram.len() as u32 / 4);
        let mut address = channel.madr & 0x00ff_fffc;
        let step = dma_address_step(control);
        for _ in 0..words {
            let word = self.read_u32(address);
            self.io.mdec.write_dma_input(word);
            address = address.wrapping_add(step);
        }
        self.schedule_dma_completion(DMA_MDEC_IN_CHANNEL, DMA_MDEC_COMPLETION_DELAY_CYCLES);
    }

    fn process_mdec_out_dma(&mut self, control: u32) {
        if control & DMA_DIRECTION_FROM_RAM != 0 {
            return;
        }

        let Some(channel) = self.io.dma.channel_state(DMA_MDEC_OUT_CHANNEL) else {
            return;
        };
        let words = dma_word_count(channel.bcr).min(self.ram.len() as u32 / 4);
        let mut address = channel.madr & 0x00ff_fffc;
        let step = dma_address_step(control);
        for _ in 0..words {
            let word = self.io.mdec.read_dma_output();
            self.write_dma_u32(address, word);
            address = address.wrapping_add(step);
        }
        self.schedule_dma_completion(DMA_MDEC_OUT_CHANNEL, DMA_MDEC_COMPLETION_DELAY_CYCLES);
    }

    fn process_gpu_dma(&mut self, control: u32) {
        let Some(channel) = self.io.dma.channel_state(2) else {
            return;
        };

        if control & DMA_DIRECTION_FROM_RAM == 0 {
            self.process_gpu_read_dma(channel.madr, channel.bcr, control);
        } else if control & DMA_LINKED_LIST_MODE != 0 {
            self.process_gpu_linked_list_dma(channel.madr);
            self.io.gpu.capture_vblank_presented_frame();
        } else {
            self.process_gpu_block_dma(channel.madr, channel.bcr, control);
            self.io.gpu.capture_vblank_presented_frame();
        }
        self.schedule_dma_completion(DMA_GPU_CHANNEL, DMA_GPU_COMPLETION_DELAY_CYCLES);
    }

    fn process_gpu_linked_list_dma(&mut self, start_address: u32) {
        let mut address = start_address & 0x00ff_fffc;
        let mut stats = GpuLinkedListDmaRunStats::started(start_address, address);
        let reverse_nodes = reverse_gpu_linked_list_nodes();
        let reverse_command_groups = reverse_gpu_linked_list_command_groups();
        let mut deferred_nodes = Vec::new();
        for _ in 0..GPU_LINKED_LIST_NODE_LIMIT {
            let header = self.read_u32(address);
            let words = (header >> 24).min(1024);
            stats.record_node(address, header);
            let mut node_commands = Vec::new();
            for index in 0..words {
                let command_address = address.wrapping_add(4 + index * 4);
                let command = self.read_u32(command_address);
                stats.record_command(command_address, command);
                node_commands.push((command_address, command));
                if !reverse_nodes {
                    self.io.gpu.write_gp0_with_source(
                        command,
                        GpuCommandSource::dma_linked_list(command_address, self.trace_pc.get()),
                    );
                }
            }
            for range in gpu_linked_list_command_ranges(&node_commands) {
                stats.record_command_group(&node_commands, range);
            }
            if reverse_nodes && !node_commands.is_empty() {
                deferred_nodes.push(node_commands);
            }

            let next = header & 0x00ff_ffff;
            if gpu_linked_list_terminator(next) {
                stats.terminated = true;
                break;
            }
            address = next & 0x00ff_fffc;
        }
        if !stats.terminated {
            stats.hit_node_limit = true;
        }
        if reverse_nodes {
            for node in deferred_nodes.iter().rev() {
                if reverse_command_groups {
                    for range in gpu_linked_list_command_ranges(node).into_iter().rev() {
                        for (command_address, command) in &node[range] {
                            self.write_gpu_dma_linked_list_word(*command_address, *command);
                        }
                    }
                } else {
                    for (command_address, command) in node {
                        self.write_gpu_dma_linked_list_word(*command_address, *command);
                    }
                }
            }
        }
        let replay_decision = self.unlinked_primitive_replay_decision(&stats);
        if replay_decision.enabled {
            let linked_nodes = stats
                .visited_nodes
                .iter()
                .map(|address| address & 0x00ff_fffc)
                .collect::<HashSet<_>>();
            let (packets, words) = self.replay_recent_unlinked_primitive_packets(&linked_nodes);
            self.unlinked_primitive_replay.record_replay(
                self.vblank_count,
                replay_decision.reason,
                replay_decision.candidate_headers,
                &stats,
                packets,
                words,
            );
        } else {
            self.unlinked_primitive_replay.record_skip(
                self.vblank_count,
                replay_decision.reason,
                replay_decision.candidate_headers,
                &stats,
            );
        }
        self.record_gpu_linked_list_dma_activity(start_address, &stats);
        self.gpu_linked_list_dma.merge_last(stats);
    }

    fn write_gpu_dma_linked_list_word(&mut self, command_address: u32, command: u32) {
        self.io.gpu.write_gp0_with_source(
            command,
            GpuCommandSource::dma_linked_list(command_address, self.trace_pc.get()),
        );
    }

    fn unlinked_primitive_replay_decision(
        &self,
        stats: &GpuLinkedListDmaRunStats,
    ) -> UnlinkedPrimitiveReplayDecision {
        let min_vblank = self
            .vblank_count
            .saturating_sub(BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW);
        let recent_header_count = self
            .primitive_ram_writes
            .header_addresses_written_since(min_vblank)
            .len();

        if std::env::var_os("BR2_NATIVE_DISABLE_UNLINKED_PRIMITIVE_REPLAY").is_some() {
            return UnlinkedPrimitiveReplayDecision::disabled("disabled", recent_header_count);
        }

        if std::env::var_os("BR2_NATIVE_ENABLE_UNLINKED_PRIMITIVE_REPLAY").is_some() {
            return UnlinkedPrimitiveReplayDecision::enabled("forced", recent_header_count);
        }

        if std::env::var_os("BR2_NATIVE_AUTO_UNLINKED_PRIMITIVE_REPLAY").is_none() {
            return UnlinkedPrimitiveReplayDecision::disabled(
                "disabled_by_default",
                recent_header_count,
            );
        }

        if self.unlinked_primitive_replay.last_vblank == Some(self.vblank_count)
            && self.unlinked_primitive_replay.last_packets > 0
        {
            return UnlinkedPrimitiveReplayDecision::disabled(
                "already_replayed_this_vblank",
                recent_header_count,
            );
        }

        let recent_header_writes = self
            .primitive_ram_writes
            .current_vblank_header_like_writes
            .saturating_add(self.primitive_ram_writes.last_vblank_header_like_writes);
        let recent_draw_writes = recent_draw_primitive_writes(&self.primitive_ram_writes);
        let has_recent_primitive_stream = recent_header_count as u64
            >= BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_RECENT_HEADERS
            || recent_header_writes >= BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_RECENT_HEADERS;
        let has_any_recent_headers = recent_header_count > 0 || recent_header_writes > 0;
        let has_recent_draw_stream =
            recent_draw_writes >= u64::from(BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_DRAW_PACKETS);

        if stats.last_nodes < BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_LINKED_NODES {
            if stats.last_nonempty_nodes <= BR2_UNLINKED_PRIMITIVE_REPLAY_SPARSE_NODE_LIMIT
                && has_recent_draw_stream
                && (has_recent_primitive_stream || has_any_recent_headers)
            {
                return UnlinkedPrimitiveReplayDecision::enabled(
                    "short_linked_list_recent_primitive_stream",
                    recent_header_count,
                );
            }
            return UnlinkedPrimitiveReplayDecision::disabled(
                "linked_list_too_short",
                recent_header_count,
            );
        }

        if stats.last_nonempty_nodes > BR2_UNLINKED_PRIMITIVE_REPLAY_SPARSE_NODE_LIMIT {
            return UnlinkedPrimitiveReplayDecision::disabled(
                "linked_list_not_sparse",
                recent_header_count,
            );
        }

        let linked_draw_packets = draw_primitive_count(&stats.command_opcode_counts);
        if linked_draw_packets < BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_DRAW_PACKETS {
            return UnlinkedPrimitiveReplayDecision::disabled(
                "not_enough_linked_draw_packets",
                recent_header_count,
            );
        }

        if recent_header_writes < BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_RECENT_HEADERS {
            return UnlinkedPrimitiveReplayDecision::disabled(
                "not_enough_recent_headers",
                recent_header_count,
            );
        }

        if recent_header_count == 0 {
            return UnlinkedPrimitiveReplayDecision::disabled("no_recent_headers", 0);
        }

        UnlinkedPrimitiveReplayDecision::enabled(
            "sparse_recent_primitive_headers",
            recent_header_count,
        )
    }

    fn replay_recent_unlinked_primitive_packets(
        &mut self,
        linked_nodes: &HashSet<u32>,
    ) -> (usize, usize) {
        let min_vblank = self
            .vblank_count
            .saturating_sub(BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW);
        let mut packet_addresses = self
            .primitive_ram_writes
            .header_addresses_written_since(min_vblank);
        let mut seen = packet_addresses
            .iter()
            .map(|(_, address)| *address)
            .collect::<HashSet<_>>();
        for (vblank, address) in self.primitive_ram_writes.header_addresses_written_since(0) {
            if seen.contains(&address) {
                continue;
            }
            let Some(sample) = self.primitive_packet_candidate_sample(address, linked_nodes) else {
                continue;
            };
            if !sample.linked
                && looks_like_draw_primitive_opcode((sample.first_command >> 24) as u8)
                && self.primitive_packet_has_playfield_draw_bounds(address, sample.word_count)
            {
                packet_addresses.push((vblank, address));
                seen.insert(address);
            }
        }
        packet_addresses.sort_unstable_by(|left, right| {
            right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1))
        });

        let mut replayed_packets = 0usize;
        let mut replayed_words = 0usize;
        for (_, address) in packet_addresses {
            if replayed_packets >= BR2_UNLINKED_PRIMITIVE_REPLAY_PACKET_LIMIT {
                break;
            }
            let Some(sample) = self.primitive_packet_candidate_sample(address, linked_nodes) else {
                continue;
            };
            if sample.linked {
                continue;
            }
            let opcode = (sample.first_command >> 24) as u8;
            if !looks_like_draw_primitive_opcode(opcode) {
                continue;
            }

            for index in 0..sample.word_count {
                let command_address = sample.address + 4 + index * 4;
                if let Some(command) = self.read_ram_u32_physical(command_address) {
                    self.write_gpu_dma_linked_list_word(command_address, command);
                    replayed_words = replayed_words.saturating_add(1);
                }
            }
            replayed_packets = replayed_packets.saturating_add(1);
        }
        (replayed_packets, replayed_words)
    }

    fn process_gpu_block_dma(&mut self, start_address: u32, bcr: u32, control: u32) {
        let words = dma_word_count(bcr).min(self.ram.len() as u32 / 4);
        let mut address = start_address & 0x00ff_fffc;
        let step = dma_address_step(control);
        for _ in 0..words {
            let command = self.read_u32(address);
            self.io.gpu.write_gp0_with_source(
                command,
                GpuCommandSource::dma_block(address, self.trace_pc.get()),
            );
            address = address.wrapping_add(step);
        }
    }

    fn process_gpu_read_dma(&mut self, start_address: u32, bcr: u32, control: u32) {
        let words = dma_word_count(bcr).min(self.ram.len() as u32 / 4);
        let mut address = start_address & 0x00ff_fffc;
        let step = dma_address_step(control);
        for _ in 0..words {
            self.write_dma_u32(address, self.io.gpu.gp0_read);
            address = address.wrapping_add(step);
        }
    }

    fn process_otc_dma(&mut self) {
        let Some(channel) = self.io.dma.channel_state(6) else {
            return;
        };

        let words = (channel.bcr & 0xffff).min(4096);
        let mut address = channel.madr & 0x00ff_fffc;
        for index in 0..words {
            let next = if index + 1 == words {
                0x00ff_ffff
            } else {
                address.wrapping_sub(4) & 0x00ff_fffc
            };
            self.write_dma_u32(address, next);
            address = address.wrapping_sub(4);
        }
        self.record_otc_dma_activity(channel.madr, words);
        self.schedule_dma_completion(DMA_OTC_CHANNEL, DMA_OTC_COMPLETION_DELAY_CYCLES);
    }

    fn read_bytes(&self, address: u32, len: usize) -> Vec<u8> {
        if let Some(offset) = ram_offset(address, self.ram.len(), len) {
            let bytes = self.ram[offset..offset + len].to_vec();
            self.record_watch_trace("read", "ram", address, len, bytes_to_le_u32(&bytes));
            return bytes;
        }

        if let Some(offset) = scratchpad_offset(address, self.scratchpad.len(), len) {
            let bytes = self.scratchpad[offset..offset + len].to_vec();
            self.record_watch_trace("read", "scratchpad", address, len, bytes_to_le_u32(&bytes));
            return bytes;
        }

        if let Some(offset) = rom_offset(address, self.rom.len(), len) {
            return self.rom[offset..offset + len].to_vec();
        }

        if let Some(offset) =
            banked_rom_offset(address, self.banked_roms.len(), len, self.zn_board.rom_bank)
        {
            let bytes = self.banked_roms[offset..offset + len].to_vec();
            let value = bytes_to_le_u32(&bytes);
            self.banked_rom_reads.borrow_mut().record(
                self.zn_board.rom_bank,
                address,
                offset,
                len,
                value,
            );
            self.record_watch_trace("read", "banked_rom", address, len, value);
            return bytes;
        }

        self.record_access_trace("read", "unmapped", address, len as u8, 0);
        vec![0; len]
    }

    pub fn read_ram_u32_physical(&self, physical: u32) -> Option<u32> {
        let offset = physical as usize;
        let bytes = self.ram.get(offset..offset.checked_add(4)?)?;
        Some(PreferredNativePlatform::read_le_u32(bytes))
    }

    fn write_ram_u32_physical(&mut self, physical: u32, value: u32) -> bool {
        let offset = physical as usize;
        let Some(end) = offset.checked_add(4) else {
            return false;
        };
        let Some(bytes) = self.ram.get_mut(offset..end) else {
            return false;
        };
        bytes.copy_from_slice(&PreferredNativePlatform::write_le_u32(value));
        true
    }

    fn write_dma_u32(&mut self, address: u32, value: u32) {
        let bytes = PreferredNativePlatform::write_le_u32(value);
        if let Some(offset) = ram_offset(address, self.ram.len(), bytes.len()) {
            self.ram[offset..offset + bytes.len()].copy_from_slice(&bytes);
            self.record_watch_trace("write", "ram_dma", address, bytes.len(), value);
        } else {
            self.record_access_trace("write", "dma_unmapped", address, bytes.len() as u8, value);
        }
    }

    fn write_bytes(&mut self, address: u32, bytes: &[u8]) {
        if self.cache_isolated && cacheable_address(address) {
            self.cache_isolated_write_count = self.cache_isolated_write_count.saturating_add(1);
            self.cache_isolated_write_bytes = self
                .cache_isolated_write_bytes
                .saturating_add(bytes.len() as u64);
            self.cache_isolated_last_address = Some(address);
            self.cache_isolated_last_width = bytes.len() as u8;
            self.cache_isolated_last_value = bytes_to_le_u32(bytes);
            self.record_access_trace(
                "write",
                "cache_isolated",
                address,
                bytes.len() as u8,
                self.cache_isolated_last_value,
            );
            return;
        }

        if let Some(offset) = ram_offset(address, self.ram.len(), bytes.len()) {
            self.ram[offset..offset + bytes.len()].copy_from_slice(bytes);
            self.record_primitive_ram_write(address, bytes);
            self.record_draw_sync_game_write(address, bytes);
            self.record_watch_trace("write", "ram", address, bytes.len(), bytes_to_le_u32(bytes));
        } else if let Some(offset) = scratchpad_offset(address, self.scratchpad.len(), bytes.len())
        {
            self.scratchpad[offset..offset + bytes.len()].copy_from_slice(bytes);
            self.record_watch_trace(
                "write",
                "scratchpad",
                address,
                bytes.len(),
                bytes_to_le_u32(bytes),
            );
        } else if banked_rom_offset(
            address,
            self.banked_roms.len(),
            bytes.len(),
            self.zn_board.rom_bank,
        )
        .is_some()
        {
            self.record_access_trace(
                "write",
                "banked_rom",
                address,
                bytes.len() as u8,
                bytes_to_le_u32(bytes),
            );
        } else {
            self.record_access_trace(
                "write",
                "unmapped",
                address,
                bytes.len() as u8,
                bytes_to_le_u32(bytes),
            );
        }
    }

    fn record_draw_sync_game_write(&mut self, address: u32, bytes: &[u8]) {
        if bytes.len() != 4 || physical_address(address) != BR2_DRAW_SYNC_FLAG_PHYSICAL {
            return;
        }

        let value = bytes_to_le_u32(bytes);
        match value {
            0 => {
                self.draw_sync_game_clear_writes =
                    self.draw_sync_game_clear_writes.saturating_add(1);
            }
            1 => {
                self.draw_sync_game_set_writes = self.draw_sync_game_set_writes.saturating_add(1);
            }
            _ => {
                self.draw_sync_game_other_writes =
                    self.draw_sync_game_other_writes.saturating_add(1);
            }
        }
        self.draw_sync_last_game_write_value = Some(value);
        self.draw_sync_last_game_write_pc = self.trace_pc.get();
    }

    fn record_primitive_ram_write(&mut self, address: u32, bytes: &[u8]) {
        if bytes.len() != 4 {
            return;
        }
        let physical = physical_address(address);
        if !(BR2_PRIMITIVE_RAM_START..BR2_PRIMITIVE_RAM_END).contains(&physical) {
            return;
        }
        self.primitive_ram_writes.record(
            physical,
            bytes_to_le_u32(bytes),
            self.trace_pc.get(),
            self.vblank_count,
            self.trace_cycles.get(),
        );
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeBoardAssets {
    pub cat702_1: Option<[u8; 8]>,
    pub cat702_2: Option<[u8; 8]>,
    pub at28c16: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct NativeBoardAssetStatus {
    cat702_1_loaded: bool,
    cat702_2_loaded: bool,
    at28c16_loaded: bool,
}

impl NativeBoardAssetStatus {
    fn from_assets(assets: &NativeBoardAssets) -> Self {
        Self {
            cat702_1_loaded: assets.cat702_1.is_some(),
            cat702_2_loaded: assets.cat702_2.is_some(),
            at28c16_loaded: assets.at28c16.is_some(),
        }
    }

    fn json(self) -> String {
        format!(
            "{{\"cat702_1_loaded\":{},\"cat702_2_loaded\":{},\"at28c16_loaded\":{}}}",
            self.cat702_1_loaded, self.cat702_2_loaded, self.at28c16_loaded
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BusAccessTraceEvent {
    pub operation: &'static str,
    pub region: &'static str,
    pub address: u32,
    pub physical_address: u32,
    pub width: u8,
    pub value: u32,
    pub pc: Option<u32>,
    pub cycles: u64,
}

impl BusAccessTraceEvent {
    fn json(&self) -> String {
        format!(
            "{{\"operation\":\"{}\",\"region\":\"{}\",\"address\":{},\"address_hex\":\"0x{:08x}\",\"physical_address\":{},\"physical_address_hex\":\"0x{:08x}\",\"width\":{},\"value\":{},\"value_hex\":\"0x{:08x}\",\"pc\":{},\"pc_hex\":{},\"cycles\":{}}}",
            self.operation,
            self.region,
            self.address,
            self.address,
            self.physical_address,
            self.physical_address,
            self.width,
            self.value,
            self.value,
            optional_u32_json(self.pc),
            optional_u32_hex_json(self.pc),
            self.cycles
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BusTraceWatchRange {
    start: u32,
    end: u32,
}

impl BusTraceWatchRange {
    fn new(address: u32, len: u32) -> Option<Self> {
        if len == 0 {
            return None;
        }

        let start = physical_address(address);
        let end = start.saturating_add(len);
        (start < end).then_some(Self { start, end })
    }
}

fn ranges_overlap(left_start: u32, left_end: u32, right_start: u32, right_end: u32) -> bool {
    left_start < right_end && right_start < left_end
}

fn bytes_to_le_u32(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .take(4)
        .enumerate()
        .fold(0, |value, (index, byte)| {
            value | ((*byte as u32) << (index * 8))
        })
}

fn dma_word_count(bcr: u32) -> u32 {
    let block_size = bcr & 0xffff;
    let block_count = (bcr >> 16) & 0xffff;
    match (block_size, block_count) {
        (0, 0) => 0,
        (_, 0) => block_size,
        (0, _) => block_count,
        _ => block_size.saturating_mul(block_count),
    }
}

fn dma_address_step(control: u32) -> u32 {
    if control & DMA_STEP_DECREMENT != 0 {
        u32::MAX - 3
    } else {
        4
    }
}

fn gpu_linked_list_terminator(next: u32) -> bool {
    next & 0x0080_0000 != 0
}

fn reverse_gpu_linked_list_nodes() -> bool {
    std::env::var_os("BR2_NATIVE_REVERSE_GPU_LINKED_LIST").is_some()
}

fn reverse_gpu_linked_list_command_groups() -> bool {
    std::env::var_os("BR2_NATIVE_REVERSE_GPU_LINKED_LIST_COMMANDS").is_some()
}

fn looks_like_draw_primitive_opcode(opcode: u8) -> bool {
    matches!(opcode, 0x20..=0x7f)
}

fn gp0_command_has_playfield_draw_bounds(words: &[u32]) -> bool {
    let Some(points) = gp0_command_vertex_words(words) else {
        return false;
    };
    if points.is_empty() {
        return false;
    }

    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    let mut has_visible_x = false;
    for word in points {
        let (x, y) = gp0_signed_xy(word);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
        has_visible_x |= (-64..=576).contains(&x);
    }

    has_visible_x && max_y >= 72 && min_y <= 430 && min_y < 420
}

fn gp0_command_vertex_words(words: &[u32]) -> Option<Vec<u32>> {
    let opcode = (*words.first()? >> 24) as u8;
    let vertices = match opcode {
        0x20..=0x23 if words.len() >= 4 => vec![words[1], words[2], words[3]],
        0x24..=0x27 if words.len() >= 7 => vec![words[1], words[3], words[5]],
        0x28..=0x2b if words.len() >= 5 => vec![words[1], words[2], words[3], words[4]],
        0x2c..=0x2f if words.len() >= 9 => vec![words[1], words[3], words[5], words[7]],
        0x30..=0x33 if words.len() >= 6 => vec![words[1], words[3], words[5]],
        0x34..=0x37 if words.len() >= 9 => vec![words[1], words[4], words[7]],
        0x38..=0x3b if words.len() >= 8 => vec![words[1], words[3], words[5], words[7]],
        0x3c..=0x3f if words.len() >= 12 => vec![words[1], words[4], words[7], words[10]],
        0x40..=0x47 if words.len() >= 3 => vec![words[1], words[2]],
        0x50..=0x57 if words.len() >= 4 => vec![words[1], words[3]],
        0x60..=0x7f if words.len() >= 2 => vec![words[1]],
        _ => Vec::new(),
    };
    Some(vertices)
}

fn gp0_signed_xy(value: u32) -> (i32, i32) {
    (
        sign_extend_11_bits(value & 0x07ff),
        sign_extend_11_bits((value >> 16) & 0x07ff),
    )
}

fn sign_extend_11_bits(value: u32) -> i32 {
    if value & 0x0400 != 0 {
        (value as i32) | !0x07ff
    } else {
        value as i32
    }
}

fn draw_primitive_count(counts: &[u32; 256]) -> u32 {
    counts
        .iter()
        .enumerate()
        .filter(|(opcode, _)| looks_like_draw_primitive_opcode(*opcode as u8))
        .fold(0u32, |total, (_, count)| total.saturating_add(*count))
}

fn recent_draw_primitive_writes(stats: &PrimitiveRamWriteStats) -> u64 {
    (0x20usize..=0x7f)
        .map(|opcode| {
            stats.current_vblank_opcode_counts[opcode]
                .saturating_add(stats.last_vblank_opcode_counts[opcode])
        })
        .fold(0u64, u64::saturating_add)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UnlinkedPrimitiveReplayDecision {
    enabled: bool,
    reason: &'static str,
    candidate_headers: usize,
}

impl UnlinkedPrimitiveReplayDecision {
    fn enabled(reason: &'static str, candidate_headers: usize) -> Self {
        Self {
            enabled: true,
            reason,
            candidate_headers,
        }
    }

    fn disabled(reason: &'static str, candidate_headers: usize) -> Self {
        Self {
            enabled: false,
            reason,
            candidate_headers,
        }
    }
}

fn gpu_linked_list_command_ranges(commands: &[(u32, u32)]) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut offset = 0;
    while offset < commands.len() {
        let remaining_words = commands[offset..]
            .iter()
            .map(|(_, command)| *command)
            .collect::<Vec<_>>();
        let command_words = gp0_command_word_count(&remaining_words)
            .unwrap_or(1)
            .max(1)
            .min(commands.len() - offset);
        ranges.push(offset..offset + command_words);
        offset += command_words;
    }
    ranges
}

fn zn_security_response_from_bios(rom: &[u8]) -> Option<Vec<u8>> {
    const LICENSE_OFFSET: usize = 0x0000_baa0;
    const RESPONSE_OFFSET: usize = 0x0000_b98d;

    let license = rom.get(LICENSE_OFFSET..)?;
    let license_len = license.iter().position(|byte| *byte == 0)?;
    if license_len < 2 {
        return None;
    }

    let response_len = license_len - 1;
    let response = rom.get(RESPONSE_OFFSET..RESPONSE_OFFSET.checked_add(response_len)?)?;
    Some(response.to_vec())
}

fn ram_offset(address: u32, ram_len: usize, access_len: usize) -> Option<usize> {
    let physical = physical_address(address);
    if physical >= 0x0080_0000 || ram_len == 0 {
        return None;
    }

    let offset = physical as usize % ram_len;
    (offset + access_len <= ram_len).then_some(offset)
}

fn scratchpad_offset(address: u32, scratchpad_len: usize, access_len: usize) -> Option<usize> {
    let physical = physical_address(address);
    let base = 0x1f80_0000;
    if physical < base {
        return None;
    }

    let offset = (physical - base) as usize;
    (offset + access_len <= scratchpad_len).then_some(offset)
}

fn banked_rom_offset(
    address: u32,
    banked_rom_len: usize,
    access_len: usize,
    rom_bank: u8,
) -> Option<usize> {
    let physical = physical_address(address);
    let base = 0x1f00_0000;
    let window_len = 0x0080_0000;
    if !(base..base + window_len).contains(&physical) {
        return None;
    }

    let offset = rom_bank as usize * window_len as usize + (physical - base) as usize;
    (offset + access_len <= banked_rom_len).then_some(offset)
}

fn mapped_zn_board_address(address: u32) -> Option<u32> {
    let physical = physical_address(address);
    (zn_board_address(physical)).then_some(physical)
}

fn zn_board_address(physical: u32) -> bool {
    matches!(
        physical,
        0x1fa0_0000..=0x1fa0_0003
            | 0x1fa0_0100..=0x1fa0_0103
            | 0x1fa0_0200..=0x1fa0_0203
            | 0x1fa0_0300..=0x1fa0_0303
            | 0x1fa1_0000..=0x1fa1_0003
            | 0x1fa1_0100..=0x1fa1_0103
            | 0x1fa1_0200
            | 0x1fa1_0300
            | 0x1fa2_0000
            | 0x1fa3_0000..=0x1fa3_0003
            | 0x1fa4_0000..=0x1fa4_0003
            | 0x1faf_0000..=0x1faf_07ff
            | 0x1fb0_0004
            | 0x1fb2_0000..=0x1fb2_0007
    )
}

#[derive(Clone, Debug)]
struct ZnBoard {
    rom_bank: u8,
    znsecsel: u8,
    coin: u8,
    sound_irq_latch: u8,
    at28c16: [u8; 2048],
    input: ActionButtons,
    p1_input_reads: Cell<u64>,
    p1_up_active_reads: Cell<u64>,
    p1_down_active_reads: Cell<u64>,
    p1_left_active_reads: Cell<u64>,
    p1_right_active_reads: Cell<u64>,
    p1_start_active_reads: Cell<u64>,
    p1_punch_active_reads: Cell<u64>,
    p1_kick_active_reads: Cell<u64>,
    p1_beast_active_reads: Cell<u64>,
    p3_input_reads: Cell<u64>,
    p3_guard_active_reads: Cell<u64>,
    system_input_reads: Cell<u64>,
    system_coin_active_reads: Cell<u64>,
    system_start_active_reads: Cell<u64>,
    coin_register_reads: Cell<u64>,
    coin_register_active_reads: Cell<u64>,
    last_p1_input: Cell<u32>,
    last_p3_input: Cell<u32>,
    last_system_input: Cell<u32>,
    last_coin_register: Cell<u32>,
}

impl Default for ZnBoard {
    fn default() -> Self {
        Self::with_at28c16(None)
    }
}

impl ZnBoard {
    fn with_at28c16(default_at28c16: Option<Vec<u8>>) -> Self {
        let mut at28c16 = [0xff; 2048];
        if let Some(bytes) = default_at28c16 {
            let len = bytes.len().min(at28c16.len());
            at28c16[..len].copy_from_slice(&bytes[..len]);
        }
        Self {
            rom_bank: 0,
            znsecsel: 0,
            coin: 0,
            sound_irq_latch: 0,
            at28c16,
            input: ActionButtons::default(),
            p1_input_reads: Cell::new(0),
            p1_up_active_reads: Cell::new(0),
            p1_down_active_reads: Cell::new(0),
            p1_left_active_reads: Cell::new(0),
            p1_right_active_reads: Cell::new(0),
            p1_start_active_reads: Cell::new(0),
            p1_punch_active_reads: Cell::new(0),
            p1_kick_active_reads: Cell::new(0),
            p1_beast_active_reads: Cell::new(0),
            p3_input_reads: Cell::new(0),
            p3_guard_active_reads: Cell::new(0),
            system_input_reads: Cell::new(0),
            system_coin_active_reads: Cell::new(0),
            system_start_active_reads: Cell::new(0),
            coin_register_reads: Cell::new(0),
            coin_register_active_reads: Cell::new(0),
            last_p1_input: Cell::new(0xffff_ffff),
            last_p3_input: Cell::new(0xffff_ffff),
            last_system_input: Cell::new(0xffff_ffff),
            last_coin_register: Cell::new(0),
        }
    }
}

impl ZnBoard {
    fn read(&self, address: u32, access_len: usize) -> u32 {
        board_read_lane(
            self.read_base_u32(address),
            board_register_base(address),
            address,
            access_len,
        )
    }

    fn write(&mut self, address: u32, value: u32, access_len: usize) {
        let base = board_register_base(address);
        let merged = board_write_lane(self.read_base_u32(base), base, address, value, access_len);
        self.write_base_u32(base, merged);
    }

    fn read_base_u32(&self, address: u32) -> u32 {
        let physical = physical_address(address);
        match physical {
            0x1fa0_0000 => self.read_player1_input(),
            0x1fa0_0100 => active_low_player2_input(),
            0x1fa0_0200 => active_low_service_input(),
            0x1fa0_0300 => self.read_system_input(),
            0x1fa1_0000 => self.read_player3_input(),
            0x1fa1_0100 => active_low_player4_input(),
            0x1fa1_0200 => 0x0000_0069,
            0x1fa1_0300 => self.znsecsel as u32,
            0x1fa2_0000 => self.read_coin_register(),
            0x1fa3_0000 | 0x1fa4_0000 => 0,
            0x1faf_0000..=0x1faf_07ff => {
                let offset = (physical - 0x1faf_0000) as usize;
                self.at28c16[offset] as u32
            }
            0x1fb0_0004 => self.sound_irq_latch as u32,
            0x1fb2_0000..=0x1fb2_0007 => 0xffff,
            _ => 0,
        }
    }

    fn write_base_u32(&mut self, address: u32, value: u32) {
        let physical = physical_address(address);
        match physical {
            0x1fa1_0300 => {
                self.znsecsel = value as u8;
                self.rom_bank = self.znsecsel & 0x03;
            }
            0x1fa2_0000 => self.coin = value as u8,
            0x1faf_0000..=0x1faf_07ff => {
                let offset = (physical - 0x1faf_0000) as usize;
                self.at28c16[offset] = value as u8;
            }
            0x1fb0_0004 => self.sound_irq_latch = value as u8,
            _ => {}
        }
    }

    fn json(&self) -> String {
        format!(
            "{{\"rom_bank\":{},\"znsecsel\":{},\"coin\":{},\"sound_irq_latch\":{},\"p1_input_reads\":{},\"p1_up_active_reads\":{},\"p1_down_active_reads\":{},\"p1_left_active_reads\":{},\"p1_right_active_reads\":{},\"p1_start_active_reads\":{},\"p1_punch_active_reads\":{},\"p1_kick_active_reads\":{},\"p1_beast_active_reads\":{},\"p3_input_reads\":{},\"p3_guard_active_reads\":{},\"system_input_reads\":{},\"system_coin_active_reads\":{},\"system_start_active_reads\":{},\"coin_register_reads\":{},\"coin_register_active_reads\":{},\"last_p1_input\":{},\"last_p1_input_hex\":\"0x{:08x}\",\"last_p3_input\":{},\"last_p3_input_hex\":\"0x{:08x}\",\"last_system_input\":{},\"last_system_input_hex\":\"0x{:08x}\",\"last_coin_register\":{},\"last_coin_register_hex\":\"0x{:08x}\"}}",
            self.rom_bank,
            self.znsecsel,
            self.coin,
            self.sound_irq_latch,
            self.p1_input_reads.get(),
            self.p1_up_active_reads.get(),
            self.p1_down_active_reads.get(),
            self.p1_left_active_reads.get(),
            self.p1_right_active_reads.get(),
            self.p1_start_active_reads.get(),
            self.p1_punch_active_reads.get(),
            self.p1_kick_active_reads.get(),
            self.p1_beast_active_reads.get(),
            self.p3_input_reads.get(),
            self.p3_guard_active_reads.get(),
            self.system_input_reads.get(),
            self.system_coin_active_reads.get(),
            self.system_start_active_reads.get(),
            self.coin_register_reads.get(),
            self.coin_register_active_reads.get(),
            self.last_p1_input.get(),
            self.last_p1_input.get(),
            self.last_p3_input.get(),
            self.last_p3_input.get(),
            self.last_system_input.get(),
            self.last_system_input.get(),
            self.last_coin_register.get(),
            self.last_coin_register.get()
        )
    }

    fn runtime_probe_json(&self) -> String {
        format!(
            "{{\"rom_bank\":{},\"znsecsel\":{},\"coin\":{},\"coin_hex\":\"0x{:02x}\",\"p1_input_reads\":{},\"p1_start_active_reads\":{},\"p1_punch_active_reads\":{},\"p1_kick_active_reads\":{},\"p1_beast_active_reads\":{},\"p3_input_reads\":{},\"p3_guard_active_reads\":{},\"system_input_reads\":{},\"system_coin_active_reads\":{},\"system_start_active_reads\":{},\"coin_register_reads\":{},\"coin_register_active_reads\":{},\"last_p1_input_hex\":\"0x{:08x}\",\"last_p3_input_hex\":\"0x{:08x}\",\"last_system_input_hex\":\"0x{:08x}\",\"last_coin_register_hex\":\"0x{:08x}\"}}",
            self.rom_bank,
            self.znsecsel,
            self.coin,
            self.coin,
            self.p1_input_reads.get(),
            self.p1_start_active_reads.get(),
            self.p1_punch_active_reads.get(),
            self.p1_kick_active_reads.get(),
            self.p1_beast_active_reads.get(),
            self.p3_input_reads.get(),
            self.p3_guard_active_reads.get(),
            self.system_input_reads.get(),
            self.system_coin_active_reads.get(),
            self.system_start_active_reads.get(),
            self.coin_register_reads.get(),
            self.coin_register_active_reads.get(),
            self.last_p1_input.get(),
            self.last_p3_input.get(),
            self.last_system_input.get(),
            self.last_coin_register.get()
        )
    }

    fn cat702_1_select(&self) -> bool {
        self.znsecsel & 0x04 != 0
    }

    fn cat702_2_select(&self) -> bool {
        self.znsecsel & 0x08 != 0
    }

    fn zn_mcu_analog_read(&self) -> bool {
        self.znsecsel & 0x10 != 0
    }

    fn zn_mcu_trackball_read(&self) -> bool {
        self.znsecsel & 0x20 != 0
    }

    fn zn_mcu_selected(&self) -> bool {
        self.znsecsel & 0x8c != 0x8c
    }

    fn set_input(&mut self, input: ActionButtons) {
        self.input = input;
    }

    fn input_activity(&self) -> NativeInputActivity {
        NativeInputActivity {
            p1_input_reads: self.p1_input_reads.get(),
            p1_up_active_reads: self.p1_up_active_reads.get(),
            p1_down_active_reads: self.p1_down_active_reads.get(),
            p1_left_active_reads: self.p1_left_active_reads.get(),
            p1_right_active_reads: self.p1_right_active_reads.get(),
            p1_start_active_reads: self.p1_start_active_reads.get(),
            p1_punch_active_reads: self.p1_punch_active_reads.get(),
            p1_kick_active_reads: self.p1_kick_active_reads.get(),
            p1_beast_active_reads: self.p1_beast_active_reads.get(),
            p3_input_reads: self.p3_input_reads.get(),
            p3_guard_active_reads: self.p3_guard_active_reads.get(),
            system_input_reads: self.system_input_reads.get(),
            system_coin_active_reads: self.system_coin_active_reads.get(),
            system_start_active_reads: self.system_start_active_reads.get(),
            coin_register_reads: self.coin_register_reads.get(),
            coin_register_active_reads: self.coin_register_active_reads.get(),
        }
    }

    fn read_player1_input(&self) -> u32 {
        let value = active_low_player1_input(self.input);
        self.p1_input_reads
            .set(self.p1_input_reads.get().saturating_add(1));
        self.count_active_player1_inputs();
        self.last_p1_input.set(value);
        value
    }

    fn count_active_player1_inputs(&self) {
        increment_if(&self.p1_up_active_reads, self.input.up);
        increment_if(&self.p1_down_active_reads, self.input.down);
        increment_if(&self.p1_left_active_reads, self.input.left);
        increment_if(&self.p1_right_active_reads, self.input.right);
        increment_if(&self.p1_start_active_reads, self.input.start);
        increment_if(&self.p1_punch_active_reads, self.input.punch);
        increment_if(&self.p1_kick_active_reads, self.input.kick);
        increment_if(&self.p1_beast_active_reads, self.input.beast);
    }

    fn read_player3_input(&self) -> u32 {
        let value = active_low_player3_input(self.input);
        self.p3_input_reads
            .set(self.p3_input_reads.get().saturating_add(1));
        increment_if(&self.p3_guard_active_reads, self.input.guard);
        self.last_p3_input.set(value);
        value
    }

    fn read_system_input(&self) -> u32 {
        let value = active_low_system_input(self.input);
        self.system_input_reads
            .set(self.system_input_reads.get().saturating_add(1));
        if self.input.coin {
            self.system_coin_active_reads
                .set(self.system_coin_active_reads.get().saturating_add(1));
        }
        if self.input.start {
            self.system_start_active_reads
                .set(self.system_start_active_reads.get().saturating_add(1));
        }
        self.last_system_input.set(value);
        value
    }

    fn read_coin_register(&self) -> u32 {
        let value = self.coin as u32;
        self.coin_register_reads
            .set(self.coin_register_reads.get().saturating_add(1));
        if self.input.coin {
            self.coin_register_active_reads
                .set(self.coin_register_active_reads.get().saturating_add(1));
        }
        self.last_coin_register.set(value);
        value
    }
}

fn active_low_player1_input(input: ActionButtons) -> u32 {
    let mut value = 0xffff_ffff;
    clear_bit_if(&mut value, 0x0000_0001, input.up);
    clear_bit_if(&mut value, 0x0000_0002, input.down);
    clear_bit_if(&mut value, 0x0000_0004, input.left);
    clear_bit_if(&mut value, 0x0000_0008, input.right);
    clear_bit_if(&mut value, 0x0000_0010, input.punch);
    clear_bit_if(&mut value, 0x0000_0020, input.kick);
    clear_bit_if(&mut value, 0x0000_0040, input.beast);
    clear_bit_if(&mut value, 0x0000_0080, input.guard);
    value
}

fn active_low_player2_input() -> u32 {
    0xffff_ffff
}

fn active_low_player3_input(input: ActionButtons) -> u32 {
    let mut value = 0xffff_ffff;
    clear_bit_if(&mut value, 0x0000_0010, input.guard);
    value
}

fn active_low_player4_input() -> u32 {
    0xffff_ffff
}

fn active_low_service_input() -> u32 {
    0xffff_ffff
}

fn active_low_system_input(input: ActionButtons) -> u32 {
    let mut value = 0xffff_ffff;
    clear_bit_if(&mut value, 0x0000_0001, input.start);
    clear_bit_if(&mut value, 0x0000_0010, input.coin);
    clear_bit_if(&mut value, 0x0000_0020, input.coin);
    value
}

fn clear_bit_if(value: &mut u32, bit: u32, clear: bool) {
    if clear {
        *value &= !bit;
    }
}

fn increment_if(counter: &Cell<u64>, condition: bool) {
    if condition {
        counter.set(counter.get().saturating_add(1));
    }
}

fn board_register_base(address: u32) -> u32 {
    let physical = physical_address(address);
    match physical {
        0x1faf_0000..=0x1faf_07ff => physical,
        _ => physical & !0x03,
    }
}

fn board_read_lane(value: u32, base: u32, address: u32, access_len: usize) -> u32 {
    let shifted = value >> ((address - base) * 8);
    match access_len {
        1 => shifted & 0xff,
        2 => shifted & 0xffff,
        _ => shifted,
    }
}

fn board_write_lane(current: u32, base: u32, address: u32, value: u32, access_len: usize) -> u32 {
    let shift = (address - base) * 8;
    let mask = match access_len {
        1 => 0xff,
        2 => 0xffff,
        _ => u32::MAX,
    } << shift;
    (current & !mask) | ((value << shift) & mask)
}

fn rom_offset(address: u32, rom_len: usize, access_len: usize) -> Option<usize> {
    let masked = physical_address(address);
    let base = 0x1fc0_0000;
    if masked < base {
        return None;
    }

    let offset = (masked - base) as usize;
    (offset + access_len <= rom_len).then_some(offset)
}

fn io_address(address: u32) -> Option<u32> {
    let physical = physical_address(address);
    (IO_REGION_START..=IO_REGION_END)
        .contains(&physical)
        .then_some(physical)
}

fn mapped_io_address(address: u32, access_len: usize) -> Option<u32> {
    let physical = io_address(address)?;
    io_access_for(physical, access_len)
        .is_some()
        .then_some(physical)
}

fn dma_io_address(address: u32) -> bool {
    (DMA_REGION_START..=DMA_REGION_END).contains(&address)
}

fn cache_control_address(address: u32) -> bool {
    address == 0xfffe_0130 || physical_address(address) == 0x1ffe_0130
}

fn cacheable_address(address: u32) -> bool {
    address < 0xa000_0000
}

fn physical_address(address: u32) -> u32 {
    address & 0x1fff_ffff
}

fn dma_activity_register_metadata(address: u32) -> Option<(usize, &'static str)> {
    match address {
        DMA_MDEC_IN_MADR => Some((DMA_MDEC_IN_CHANNEL, "MDEC_IN_MADR")),
        DMA_MDEC_IN_BCR => Some((DMA_MDEC_IN_CHANNEL, "MDEC_IN_BCR")),
        DMA_MDEC_IN_CHCR => Some((DMA_MDEC_IN_CHANNEL, "MDEC_IN_CHCR")),
        DMA_MDEC_OUT_MADR => Some((DMA_MDEC_OUT_CHANNEL, "MDEC_OUT_MADR")),
        DMA_MDEC_OUT_BCR => Some((DMA_MDEC_OUT_CHANNEL, "MDEC_OUT_BCR")),
        DMA_MDEC_OUT_CHCR => Some((DMA_MDEC_OUT_CHANNEL, "MDEC_OUT_CHCR")),
        DMA_GPU_MADR => Some((DMA_GPU_CHANNEL, "GPU_MADR")),
        DMA_GPU_BCR => Some((DMA_GPU_CHANNEL, "GPU_BCR")),
        DMA_GPU_CHCR => Some((DMA_GPU_CHANNEL, "GPU_CHCR")),
        DMA_OTC_MADR => Some((DMA_OTC_CHANNEL, "OTC_MADR")),
        DMA_OTC_BCR => Some((DMA_OTC_CHANNEL, "OTC_BCR")),
        DMA_OTC_CHCR => Some((DMA_OTC_CHANNEL, "OTC_CHCR")),
        _ => None,
    }
}

fn optional_str_json(value: Option<&str>) -> String {
    value.map_or_else(|| "null".to_string(), |value| format!("\"{value}\""))
}

fn optional_u32_json(value: Option<u32>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn optional_u64_json(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn optional_u32_hex_json(value: Option<u32>) -> String {
    value.map_or_else(|| "null".to_string(), |value| format!("\"0x{value:08x}\""))
}

fn optional_u8_json(value: Option<u8>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn optional_usize_json(value: Option<usize>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn optional_usize_hex_json(value: Option<usize>) -> String {
    value.map_or_else(|| "null".to_string(), |value| format!("\"0x{value:08x}\""))
}

fn command_opcode_counts_json(counts: &[u32; 256]) -> String {
    counts
        .iter()
        .enumerate()
        .filter(|(_, count)| **count != 0)
        .map(|(opcode, count)| {
            format!(
                "{{\"opcode\":{},\"opcode_hex\":\"0x{:02x}\",\"count\":{}}}",
                opcode, opcode, count
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn u64_command_opcode_counts_json(counts: &[u64; 256]) -> String {
    counts
        .iter()
        .enumerate()
        .filter(|(_, count)| **count != 0)
        .map(|(opcode, count)| {
            format!(
                "{{\"opcode\":{},\"opcode_hex\":\"0x{:02x}\",\"count\":{}}}",
                opcode, opcode, count
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn primitive_ram_write_samples_json(samples: &[PrimitiveRamWriteSample]) -> String {
    samples
        .iter()
        .map(|sample| {
            format!(
                "{{\"address\":{},\"address_hex\":\"0x{:08x}\",\"value\":{},\"value_hex\":\"0x{:08x}\",\"opcode\":{},\"opcode_hex\":\"0x{:02x}\",\"pc\":{},\"pc_hex\":{},\"vblank\":{},\"cycles\":{}}}",
                sample.address,
                sample.address,
                sample.value,
                sample.value,
                sample.value >> 24,
                sample.value >> 24,
                optional_u32_json(sample.pc),
                optional_u32_hex_json(sample.pc),
                sample.vblank,
                sample.cycles
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn gpu_linked_list_node_samples_json(samples: &[GpuLinkedListNodeSample]) -> String {
    samples
        .iter()
        .map(GpuLinkedListNodeSample::json)
        .collect::<Vec<_>>()
        .join(",")
}

fn primitive_packet_next_plausible(next: u32) -> bool {
    if matches!(next, 0x00ff_ffff | 0x0080_0000) {
        return true;
    }
    let physical = next & 0x00ff_fffc;
    next & 0x03 == 0 && (BR2_PRIMITIVE_RAM_START..BR2_PRIMITIVE_RAM_END).contains(&physical)
}

fn looks_like_gp0_command_opcode(opcode: u8) -> bool {
    matches!(
        opcode,
        0x00..=0x02 | 0x20..=0x3f | 0x40..=0x5f | 0x60..=0x7f | 0x80 | 0xa0 | 0xc0
            | 0xe1..=0xe6
    )
}

#[cfg(test)]
mod tests {
    use super::{
        BR2_DRAW_SYNC_FLAG_VIRTUAL, BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_RECENT_HEADERS, Bus,
        DMA_GPU_COMPLETION_DELAY_CYCLES, DMA_MDEC_COMPLETION_DELAY_CYCLES, DMA_STEP_DECREMENT,
        GPU_LINKED_LIST_NODE_LIMIT, NativeInputActivity, draw_primitive_count,
        gpu_linked_list_command_ranges,
    };
    use crate::action::ActionButtons;
    use crate::native::io::{
        DMA_GPU_BCR, DMA_GPU_CHCR, DMA_GPU_MADR, DMA_INTERRUPT, DMA_MDEC_IN_BCR, DMA_MDEC_IN_CHCR,
        DMA_MDEC_IN_MADR, DMA_MDEC_OUT_BCR, DMA_MDEC_OUT_CHCR, DMA_MDEC_OUT_MADR, DMA_OTC_BCR,
        DMA_OTC_CHCR, DMA_OTC_MADR, DMA_SPU_CHCR, GPU_GP0, IRQ_MASK, IRQ_STATUS, MDEC_COMMAND,
        SIO_DATA, SPU_REGION_START, TIMER1_COUNTER, TIMER1_MODE, TIMER1_TARGET,
    };

    #[test]
    fn bus_dispatches_halfword_irq_io_accesses() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.io.irq.status = 0xffff;

        bus.write_u16(IRQ_STATUS, 0x00ff);
        bus.write_u16(0xbf80_1074, 0x0101);

        assert_eq!(bus.io.irq.status, 0x00ff);
        assert_eq!(bus.io.irq.mask, 0x0101);
        assert_eq!(bus.read_u16(IRQ_STATUS), 0x00ff);
        assert_eq!(bus.read_u16(IRQ_MASK), 0x0101);
    }

    #[test]
    fn bus_dispatches_byte_serial_controller_accesses() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.io.controller.p1_state = 0xabcd;

        bus.write_u8(SIO_DATA, 0x01);
        assert_eq!(bus.read_u8(SIO_DATA), 0xff);
        bus.write_u8(SIO_DATA, 0x5a);

        assert_eq!(bus.io.controller.last_write, 0x005a);
        assert_eq!(bus.read_u8(SIO_DATA), 0xff);
        assert_eq!(bus.read_u16(crate::native::io::SIO_STATUS) & 0x0002, 0x0002);
    }

    #[test]
    fn bus_dispatches_word_gpu_and_dma_windows() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.write_u32(GPU_GP0, 0x1234_5678);
        bus.write_u32(DMA_GPU_MADR, 0x0012_3000);

        assert_eq!(bus.io.gpu.gp0_read, 0x1234_5678);
        assert_eq!(bus.io.gpu.commands_seen, 1);
        assert_eq!(bus.read_u32(GPU_GP0), 0x1234_5678);
        assert_eq!(bus.read_u32(DMA_GPU_MADR), 0x0012_3000);
    }

    #[test]
    fn bus_maps_action_buttons_to_controller_and_board_inputs() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.set_input(ActionButtons {
            start: true,
            coin: true,
            up: true,
            punch: true,
            guard: true,
            ..ActionButtons::default()
        });

        assert_eq!(bus.io.controller.p1_state & 0x0008, 0);
        assert_eq!(bus.io.controller.p1_state & 0x0010, 0);
        assert_eq!(bus.io.controller.p1_state & 0x4000, 0);
        let p1 = bus.read_u16(0x1fa0_0000);
        assert_eq!(p1 & 0x0091, 0);
        assert_eq!(p1 & 0x0100, 0x0100);
        assert_eq!(bus.read_u8(0x1fa0_0200), 0xff);
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x11, 0);
        assert_eq!(bus.read_u8(0x1fa1_0000) & 0x10, 0);
        let board_json = bus.zn_board_json();
        assert!(board_json.contains("\"p1_up_active_reads\":1"));
        assert!(board_json.contains("\"p1_start_active_reads\":1"));
        assert!(board_json.contains("\"p1_punch_active_reads\":1"));
        assert!(board_json.contains("\"p3_guard_active_reads\":1"));
    }

    #[test]
    fn input_activity_reports_direction_and_full_control_status() {
        let no_activity = NativeInputActivity::default();
        assert!(!no_activity.has_direction_activity());
        assert!(!no_activity.has_play_control_activity());
        assert!(!no_activity.has_full_control_activity());

        let full_activity = NativeInputActivity {
            p1_input_reads: 8,
            p1_up_active_reads: 1,
            p1_down_active_reads: 1,
            p1_left_active_reads: 1,
            p1_right_active_reads: 1,
            p1_start_active_reads: 1,
            p1_punch_active_reads: 1,
            p1_kick_active_reads: 1,
            p1_beast_active_reads: 1,
            p3_input_reads: 1,
            p3_guard_active_reads: 1,
            system_input_reads: 2,
            system_coin_active_reads: 1,
            system_start_active_reads: 1,
            coin_register_reads: 1,
            coin_register_active_reads: 1,
        };

        assert!(full_activity.has_direction_activity());
        assert!(full_activity.has_play_control_activity());
        assert!(full_activity.has_full_control_activity());

        let json = full_activity.json();
        assert!(json.contains("\"has_direction_activity\":true"));
        assert!(json.contains("\"has_play_control_activity\":true"));
        assert!(json.contains("\"has_full_control_activity\":true"));
    }

    #[test]
    fn input_activity_merges_and_diffs_branch_reads_safely() {
        let baseline = NativeInputActivity {
            p1_input_reads: 8,
            p1_up_active_reads: 2,
            p1_punch_active_reads: 1,
            system_coin_active_reads: 1,
            ..NativeInputActivity::default()
        };
        let branch = NativeInputActivity {
            p1_input_reads: 13,
            p1_up_active_reads: 2,
            p1_down_active_reads: 4,
            p1_punch_active_reads: 3,
            system_coin_active_reads: 1,
            p3_guard_active_reads: 5,
            ..NativeInputActivity::default()
        };

        let delta = branch.saturating_subtracted(baseline);
        assert_eq!(delta.p1_input_reads, 5);
        assert_eq!(delta.p1_up_active_reads, 0);
        assert_eq!(delta.p1_down_active_reads, 4);
        assert_eq!(delta.p1_punch_active_reads, 2);
        assert_eq!(delta.system_coin_active_reads, 0);
        assert_eq!(delta.p3_guard_active_reads, 5);

        let merged = baseline.saturating_added(delta);
        assert_eq!(merged.p1_input_reads, 13);
        assert_eq!(merged.p1_down_active_reads, 4);
        assert_eq!(merged.p1_punch_active_reads, 3);
        assert_eq!(merged.p3_guard_active_reads, 5);
    }

    #[test]
    fn bus_preserves_mapped_but_unmodeled_register_range_state() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.write_u16(SPU_REGION_START + 2, 0xbeef);

        assert_eq!(bus.read_u16(SPU_REGION_START + 2), 0xbeef);
        assert_eq!(bus.read_u16(SPU_REGION_START + 4), 0);
    }

    #[test]
    fn bus_maps_ram_mirrors_scratchpad_and_banked_roms() {
        let mut bus = Bus::with_banked_roms(
            vec![0xaa, 0xbb, 0xcc, 0xdd],
            vec![0x11, 0x22, 0x33, 0x44],
            2 * 1024 * 1024,
        );

        bus.write_u32(0x0020_0000, 0x1234_5678);
        bus.write_u32(0x1f80_0000, 0xfeed_beef);

        assert_eq!(bus.read_u32(0), 0x1234_5678);
        assert_eq!(bus.read_u32(0x8000_0000), 0x1234_5678);
        assert_eq!(bus.read_u32(0x1f80_0000), 0xfeed_beef);
        assert_eq!(bus.read_u32(0x1f00_0000), 0x4433_2211);
        assert_eq!(bus.read_u32(0x1fc0_0000), 0xddcc_bbaa);
    }

    #[test]
    fn bus_access_trace_records_io_and_unmapped_accesses_only() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.set_access_trace_limit(3);

        bus.write_u32(GPU_GP0, 0x1234_5678);
        let _ = bus.read_u32(GPU_GP0);
        let _ = bus.read_u32(0x1ffe_0130);
        bus.write_u32(0x1ffe_0130, 0x0000_0804);

        let json = bus.access_trace_json();
        assert!(!json.contains("\"address_hex\":\"0x00000000\""));
        assert!(json.contains("\"operation\":\"read\""));
        assert!(json.contains("\"operation\":\"write\""));
        assert!(json.contains("\"region\":\"io\""));
        assert!(json.contains("\"region\":\"cache_control\""));
        assert!(json.contains("\"address_hex\":\"0x1ffe0130\""));
        assert_eq!(json.matches("\"operation\"").count(), 3);
    }

    #[test]
    fn bus_access_trace_records_watched_ram_with_cpu_context() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.set_access_trace_limit(8);
        bus.set_access_trace_watch_ranges(vec![(0x803a_2210, 4)]);
        bus.set_access_trace_watch_only(true);
        bus.set_trace_context(0x802d_080c, 1234);

        bus.write_u32(0x803a_2210, 1);
        let _ = bus.read_u32(0x003a_2210);
        let _ = bus.read_u32(GPU_GP0);
        bus.write_u32(0x803a_2220, 2);

        let json = bus.access_trace_json();
        assert!(json.contains("\"region\":\"ram\""));
        assert!(json.contains("\"physical_address_hex\":\"0x003a2210\""));
        assert!(json.contains("\"pc_hex\":\"0x802d080c\""));
        assert!(json.contains("\"cycles\":1234"));
        assert_eq!(json.matches("\"operation\"").count(), 2);
    }

    #[test]
    fn bus_records_banked_rom_read_stats_and_watch_trace() {
        let mut banked = vec![0; 0x0100_0004];
        banked[0x0080_0000..0x0080_0004].copy_from_slice(&[0x78, 0x56, 0x34, 0x12]);
        let mut bus = Bus::with_banked_roms(Vec::new(), banked, 4 * 1024 * 1024);
        bus.set_access_trace_limit(4);
        bus.set_access_trace_watch_ranges(vec![(0x1f00_0000, 4)]);
        bus.set_access_trace_watch_only(true);
        bus.set_trace_context(0x8020_0000, 99);

        bus.write_u8(0x1fa1_0300, 0x01);
        assert_eq!(bus.read_u32(0x1f00_0000), 0x1234_5678);

        let sync_json = bus.native_sync_json();
        assert!(sync_json.contains("\"banked_rom_reads\""));
        assert!(sync_json.contains("\"bank\":1,\"reads\":1"));
        assert!(sync_json.contains("\"last_offset_hex\":\"0x00800000\""));

        let trace_json = bus.access_trace_json();
        assert!(trace_json.contains("\"region\":\"banked_rom\""));
        assert!(trace_json.contains("\"pc_hex\":\"0x80200000\""));
        assert!(trace_json.contains("\"cycles\":99"));
    }

    #[test]
    fn bus_suppresses_cacheable_ram_writes_while_cache_is_isolated() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.write_u32(0x0000_0500, 0x1234_5678);

        bus.set_cache_isolated(true);
        bus.write_u32(0x8000_0500, 0xdead_beef);
        bus.write_u32(0xa000_0504, 0xcafe_babe);

        assert_eq!(bus.read_u32(0x0000_0500), 0x1234_5678);
        assert_eq!(bus.read_u32(0x0000_0504), 0xcafe_babe);
        assert!(bus.cache_isolated());
    }

    #[test]
    fn dma_to_ram_bypasses_cache_isolated_cpu_store_suppression() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.write_u32(0x0000_2008, 0x1111_1111);
        bus.write_u32(0x0000_3000, 0x2222_2222);
        bus.io.gpu.gp0_read = 0xfeed_cafe;

        bus.set_cache_isolated(true);
        bus.write_u32(0x8000_2008, 0xdead_beef);
        bus.write_u32(DMA_OTC_MADR, 0x0000_2008);
        bus.write_u32(DMA_OTC_BCR, 3);
        bus.write_u32(DMA_OTC_CHCR, 0x1100_0002);
        bus.write_u32(DMA_GPU_MADR, 0x0000_3000);
        bus.write_u32(DMA_GPU_BCR, 1);
        bus.write_u32(DMA_GPU_CHCR, 1 << 24);

        assert_eq!(bus.read_u32(0x0000_2008), 0x0000_2004);
        assert_eq!(bus.read_u32(0x0000_2004), 0x0000_2000);
        assert_eq!(bus.read_u32(0x0000_2000), 0x00ff_ffff);
        assert_eq!(bus.read_u32(0x0000_3000), 0xfeed_cafe);
        assert!(
            bus.native_sync_json()
                .contains("\"isolated_write_count\":1")
        );
    }

    #[test]
    fn bus_tick_advances_timers_and_raises_vblank_irq() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.tick(127);
        assert_eq!(bus.io.timers.0[1].counter, 0);
        assert_eq!(bus.io.irq.status & 1, 0);

        bus.tick(1);
        assert_eq!(bus.io.timers.0[1].counter, 1);

        bus.tick(566_000);
        assert_ne!(bus.io.timers.0[1].counter, 1);
        assert_eq!(bus.io.irq.status & 1, 1);
    }

    #[test]
    fn bus_tick_raises_timer_irq_on_target() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.write_u16(TIMER1_COUNTER, 0);
        bus.write_u16(TIMER1_TARGET, 2);
        bus.write_u16(TIMER1_MODE, (1 << 3) | (1 << 4) | (1 << 6));

        bus.tick(128);
        assert_eq!(bus.read_u16(TIMER1_COUNTER), 1);
        assert_eq!(bus.io.irq.status & (1 << 5), 0);

        bus.tick(128);
        assert_eq!(bus.io.irq.status & (1 << 5), 1 << 5);
        assert_eq!(bus.read_u16(TIMER1_COUNTER), 2);
        assert_ne!(bus.read_u16(TIMER1_MODE) & (1 << 11), 0);
    }

    #[test]
    fn vblank_clears_bloody_roar_draw_sync_flag() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);

        bus.write_u32(BR2_DRAW_SYNC_FLAG_VIRTUAL, 1);
        bus.tick(566_000);

        assert_eq!(bus.read_u32(BR2_DRAW_SYNC_FLAG_VIRTUAL), 0);
        assert!(
            bus.native_sync_json()
                .contains("\"vblank_draw_sync_clears\":1")
        );
    }

    #[test]
    fn draw_sync_json_tracks_game_writes_separately_from_vblank_clears() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);

        bus.write_u32(BR2_DRAW_SYNC_FLAG_VIRTUAL, 1);
        bus.write_u32(BR2_DRAW_SYNC_FLAG_VIRTUAL, 0);
        bus.write_u32(BR2_DRAW_SYNC_FLAG_VIRTUAL, 7);

        let json = bus.native_sync_json();
        assert!(json.contains("\"game_set_writes\":1"));
        assert!(json.contains("\"game_clear_writes\":1"));
        assert!(json.contains("\"game_other_writes\":1"));
        assert!(json.contains("\"last_game_write_value\":7"));
    }

    #[test]
    fn bus_raises_dma_irq_when_enabled_channel_completes() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.write_u32(DMA_INTERRUPT, (1 << 23) | (1 << 20));
        bus.write_u32(DMA_SPU_CHCR, 1 << 24);

        assert_eq!(bus.io.irq.status & (1 << 3), 1 << 3);
        assert!(bus.io.dma.irq_pending());
    }

    #[test]
    fn bus_clears_dma_irq_status_when_dma_source_is_acknowledged_late() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.write_u32(DMA_INTERRUPT, (1 << 23) | (1 << 20));
        bus.write_u32(DMA_SPU_CHCR, 1 << 24);

        assert_eq!(bus.io.irq.status & (1 << 3), 1 << 3);
        bus.write_u32(IRQ_STATUS, !(1 << 3));
        assert_eq!(bus.io.irq.status & (1 << 3), 1 << 3);

        bus.write_u32(DMA_INTERRUPT, (1 << 28) | (1 << 23) | (1 << 20));

        assert!(!bus.io.dma.irq_pending());
        assert_eq!(bus.io.irq.status & (1 << 3), 0);
    }

    #[test]
    fn bus_blank_bios_irq_acknowledges_dma_source_flag() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.write_u32(DMA_INTERRUPT, (1 << 23) | (1 << 20));
        bus.write_u32(DMA_SPU_CHCR, 1 << 24);

        assert!(bus.io.dma.irq_pending());
        assert_eq!(bus.io.irq.status & (1 << 3), 1 << 3);

        bus.acknowledge_hle_bios_irq_sources(1 << 3);

        assert!(!bus.io.dma.irq_pending());
        assert_eq!(bus.io.irq.status & (1 << 3), 0);
        assert_eq!(
            bus.io.dma.interrupt & ((1 << 23) | (1 << 20)),
            (1 << 23) | (1 << 20)
        );
        assert_eq!(bus.io.dma.interrupt & (1 << 28), 0);
    }

    #[test]
    fn gpu_linked_list_dma_feeds_gp0_commands() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.write_u32(0x0000_1000, 0x0200_ffff);
        bus.write_u32(0x0000_1004, 0xe100_0400);
        bus.write_u32(0x0000_1008, 0xe600_0000);
        bus.write_u32(DMA_INTERRUPT, (1 << 23) | (1 << 18));

        bus.write_u32(DMA_GPU_MADR, 0x0000_1000);
        bus.write_u32(DMA_GPU_CHCR, 0x0100_0401);

        assert_eq!(bus.io.gpu.gp0_read, 0xe600_0000);
        assert_eq!(bus.io.gpu.commands_seen, 2);
        assert_eq!(bus.io.irq.status & (1 << 3), 0);
        assert_eq!(bus.read_u32(DMA_GPU_CHCR) & (1 << 24), 1 << 24);

        bus.tick(DMA_GPU_COMPLETION_DELAY_CYCLES - 1);
        assert_eq!(bus.io.irq.status & (1 << 3), 0);
        assert_eq!(bus.read_u32(DMA_GPU_CHCR) & (1 << 24), 1 << 24);

        bus.tick(1);
        assert_eq!(bus.io.irq.status & (1 << 3), 1 << 3);
        assert_eq!(bus.read_u32(DMA_GPU_CHCR) & (1 << 24), 0);
    }

    #[test]
    fn dma_activity_json_tracks_gpu_and_otc_heads() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.write_u32(0x0000_2008, 0x1111_1111);
        bus.write_u32(0x0000_1000, 0x01ff_ffff);
        bus.write_u32(0x0000_1004, 0xe100_0400);

        bus.write_u32(DMA_OTC_MADR, 0x0000_2008);
        bus.write_u32(DMA_OTC_BCR, 3);
        bus.write_u32(DMA_OTC_CHCR, 0x1100_0002);
        bus.write_u32(DMA_GPU_MADR, 0x0000_1000);
        bus.write_u32(DMA_GPU_CHCR, 0x0100_0401);

        let sync_json = bus.native_sync_json();
        assert!(sync_json.contains("\"dma_activity\""));
        assert!(sync_json.contains("\"register\":\"OTC_CHCR\""));
        assert!(sync_json.contains("\"kind\":\"otc_clear\""));
        assert!(sync_json.contains("\"start_hex\":\"0x00002008\""));
        assert!(sync_json.contains("\"end_hex\":\"0x00002000\""));
        assert!(sync_json.contains("\"register\":\"GPU_CHCR\""));
        assert!(sync_json.contains("\"kind\":\"gpu_linked_list\""));
        assert!(sync_json.contains("\"start_hex\":\"0x00001000\""));
        assert!(sync_json.contains("\"nonempty_nodes\":1"));
    }

    #[test]
    fn gpu_linked_list_dma_stops_on_address_bit_23_terminator() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.write_u32(0x0000_1000, 0x0180_0000);
        bus.write_u32(0x0000_1004, 0xe100_0400);
        bus.write_u32(0x0000_0000, 0x0100_ffff);
        bus.write_u32(0x0000_0004, 0xe600_0000);

        bus.write_u32(DMA_GPU_MADR, 0x0000_1000);
        bus.write_u32(DMA_GPU_CHCR, 0x0100_0401);

        assert_eq!(bus.io.gpu.gp0_read, 0xe100_0400);
        assert_eq!(bus.io.gpu.commands_seen, 1);
    }

    #[test]
    fn gpu_linked_list_dma_groups_gp0_primitives_without_reversing_words() {
        let packet = [
            (0x0000_1004, 0xe100_0400),
            (0x0000_1008, 0x2c40_4040),
            (0x0000_100c, 0x000a_000a),
            (0x0000_1010, 0x0000_0000),
            (0x0000_1014, 0x000a_000c),
            (0x0000_1018, 0x0000_0001),
            (0x0000_101c, 0x000c_000a),
            (0x0000_1020, 0x0000_0100),
            (0x0000_1024, 0x000c_000c),
            (0x0000_1028, 0x0000_0101),
            (0x0000_102c, 0xe600_0000),
        ];

        let ranges = gpu_linked_list_command_ranges(&packet);
        let reversed_words = ranges
            .iter()
            .rev()
            .flat_map(|range| packet[range.clone()].iter().map(|(_, command)| *command))
            .collect::<Vec<_>>();

        assert_eq!(
            ranges
                .iter()
                .map(|range| range.end - range.start)
                .collect::<Vec<_>>(),
            vec![1, 9, 1]
        );
        assert_eq!(reversed_words[0], 0xe600_0000);
        assert_eq!(reversed_words[1], 0x2c40_4040);
        assert_eq!(reversed_words[9], 0x0000_0101);
        assert_eq!(reversed_words[10], 0xe100_0400);
    }

    #[test]
    fn gpu_linked_list_dma_reaches_commands_after_large_ordering_table() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let base = 0x0000_1000;
        let empty_nodes = 4_096_u32;
        for index in 0..empty_nodes {
            let node = base + index * 4;
            bus.write_u32(node, (node + 4) & 0x00ff_ffff);
        }
        let command_node = base + empty_nodes * 4;
        bus.write_u32(command_node, 0x0180_0000);
        bus.write_u32(command_node + 4, 0xe100_0400);

        bus.write_u32(DMA_GPU_MADR, base);
        bus.write_u32(DMA_GPU_CHCR, 0x0100_0401);

        assert_eq!(bus.io.gpu.gp0_read, 0xe100_0400);
        assert_eq!(bus.io.gpu.commands_seen, 1);
        let sync_json = bus.native_sync_json();
        assert!(sync_json.contains("\"last_nodes\":4097"));
        assert!(sync_json.contains("\"last_hit_node_limit\":false"));
        assert!(GPU_LINKED_LIST_NODE_LIMIT > empty_nodes);
    }

    #[test]
    fn primitive_packet_scan_distinguishes_linked_and_unlinked_packets() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(0x003a_1000, 0x01ff_ffff);
        bus.write_u32(0x003a_1004, 0xe100_0400);
        bus.write_u32(0x003a_1100, 0x01ff_ffff);
        bus.write_u32(0x003a_1104, 0xe600_0000);

        bus.write_u32(DMA_GPU_MADR, 0x003a_1000);
        bus.write_u32(DMA_GPU_CHCR, 0x0100_0401);

        let sync_json = bus.native_sync_json();
        assert!(sync_json.contains("\"primitive_packet_scan\""));
        assert!(sync_json.contains("\"last_dma_visited_nodes\":1"));
        assert!(sync_json.contains("\"candidates\":2"));
        assert!(sync_json.contains("\"linked_candidates\":1"));
        assert!(sync_json.contains("\"unlinked_candidates\":1"));
        assert!(sync_json.contains("\"address_hex\":\"0x003a1100\""));
    }

    #[test]
    fn gpu_linked_list_dma_skips_recent_unlinked_br2_primitive_packets_by_default() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(0x003a_1000, 0x01ff_ffff);
        bus.write_u32(0x003a_1004, 0xe100_0400);

        for index in 0..BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_RECENT_HEADERS {
            let base = 0x0038_1000 + (index as u32) * 0x20;
            bus.write_u32(base, 0x05ff_ffff);
            bus.write_u32(base + 4, 0x2800_ff00);
            bus.write_u32(base + 8, 0x0000_0000);
            bus.write_u32(base + 12, 0x0000_0008);
            bus.write_u32(base + 16, 0x0008_0000);
            bus.write_u32(base + 20, 0x0008_0008);
        }

        bus.write_u32(DMA_GPU_MADR, 0x003a_1000);
        bus.write_u32(DMA_GPU_CHCR, 0x0100_0401);

        assert_eq!(bus.io.gpu.commands_seen, 1);
        let sync_json = bus.native_sync_json();
        assert!(sync_json.contains("\"conditional_replays\":0"));
        assert!(sync_json.contains("\"last_reason\":\"disabled_by_default\""));
    }

    #[test]
    fn gpu_unlinked_replay_counts_all_gp0_draw_primitive_opcodes() {
        let mut counts = [0u32; 256];
        counts[0x29] = 3;
        counts[0x39] = 5;
        counts[0xe1] = 99;

        assert_eq!(draw_primitive_count(&counts), 8);
    }

    #[test]
    fn mdec_input_dma_feeds_command_data() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.write_u32(0x0000_3000, 0x1111_2222);
        bus.write_u32(0x0000_3004, 0x3333_4444);
        bus.write_u32(MDEC_COMMAND, 0x4000_0001);

        bus.write_u32(DMA_MDEC_IN_MADR, 0x0000_3000);
        bus.write_u32(DMA_MDEC_IN_BCR, 2);
        bus.write_u32(DMA_MDEC_IN_CHCR, (1 << 24) | 1);

        assert_eq!(bus.io.mdec.dma_input_words(), 2);
        assert_eq!(bus.io.mdec.input_words_remaining(), 30);
        assert_eq!(bus.read_u32(DMA_MDEC_IN_CHCR) & (1 << 24), 1 << 24);

        bus.tick(DMA_MDEC_COMPLETION_DELAY_CYCLES);

        assert_eq!(bus.read_u32(DMA_MDEC_IN_CHCR) & (1 << 24), 0);
    }

    #[test]
    fn mdec_input_dma_can_complete_large_decode_payload() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        let payload_words = 4097_u32;
        bus.write_u32(MDEC_COMMAND, (1 << 29) | payload_words);
        for index in 0..payload_words {
            bus.write_u32(0x0001_0000 + index * 4, index);
        }

        bus.write_u32(DMA_MDEC_IN_MADR, 0x0001_0000);
        bus.write_u32(DMA_MDEC_IN_BCR, payload_words);
        bus.write_u32(DMA_MDEC_IN_CHCR, (1 << 24) | 1);

        assert_eq!(bus.io.mdec.dma_input_words(), payload_words as u64);
        assert_eq!(bus.io.mdec.input_words_remaining(), 0);
    }

    #[test]
    fn mdec_output_dma_writes_deterministic_placeholder_words() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.write_u32(DMA_MDEC_OUT_MADR, 0x0000_3000);
        bus.write_u32(DMA_MDEC_OUT_BCR, 2);
        bus.write_u32(DMA_MDEC_OUT_CHCR, 1 << 24);

        assert_eq!(bus.io.mdec.dma_output_words(), 2);
        assert_eq!(bus.read_u32(0x0000_3000), 0);
        assert_eq!(bus.read_u32(0x0000_3004), 0);
        assert_eq!(bus.read_u32(DMA_MDEC_OUT_CHCR) & (1 << 24), 1 << 24);

        bus.tick(DMA_MDEC_COMPLETION_DELAY_CYCLES);

        assert_eq!(bus.read_u32(DMA_MDEC_OUT_CHCR) & (1 << 24), 0);
    }

    #[test]
    fn gpu_block_dma_from_ram_feeds_gp0_commands() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.write_u32(0x0000_3000, 0xe100_0400);

        bus.write_u32(DMA_GPU_MADR, 0x0000_3000);
        bus.write_u32(DMA_GPU_BCR, 1);
        bus.write_u32(DMA_GPU_CHCR, (1 << 24) | 1);

        assert_eq!(bus.io.gpu.gp0_read, 0xe100_0400);
        assert_eq!(bus.io.gpu.commands_seen, 1);
    }

    #[test]
    fn gpu_block_dma_can_complete_large_image_upload_payload() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.write_u32(GPU_GP0, 0xa000_0000);
        bus.write_u32(GPU_GP0, 0x0000_0380);
        bus.write_u32(GPU_GP0, 0x0100_0040);
        for index in 0..8192 {
            bus.write_u32(0x0001_0014 + index * 4, 0);
        }

        bus.write_u32(DMA_GPU_MADR, 0x0001_0014);
        bus.write_u32(DMA_GPU_BCR, 0x0200_0010);
        bus.write_u32(DMA_GPU_CHCR, (1 << 24) | (1 << 9) | 1);

        assert_eq!(bus.io.gpu.gp0_pending_words(), 0);
        assert!(bus.io_json().contains("\"gpu_image_upload_commands\":1"));
    }

    #[test]
    fn gpu_block_dma_from_ram_honors_decrement_step() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.write_u32(0x0000_3000, 0xe100_0400);
        bus.write_u32(0x0000_2ffc, 0xe600_0000);

        bus.write_u32(DMA_GPU_MADR, 0x0000_3000);
        bus.write_u32(DMA_GPU_BCR, 2);
        bus.write_u32(DMA_GPU_CHCR, (1 << 24) | 0x03);

        assert_eq!(bus.io.gpu.gp0_read, 0xe600_0000);
        assert_eq!(bus.io.gpu.commands_seen, 2);
    }

    #[test]
    fn gpu_block_dma_to_ram_does_not_feed_gp0_commands() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.write_u32(0x0000_3000, 0xe100_0400);
        bus.io.gpu.gp0_read = 0xdead_beef;

        bus.write_u32(DMA_GPU_MADR, 0x0000_3000);
        bus.write_u32(DMA_GPU_BCR, 2);
        bus.write_u32(DMA_GPU_CHCR, 1 << 24);

        assert_eq!(bus.io.gpu.commands_seen, 0);
        assert_eq!(bus.read_u32(0x0000_3000), 0xdead_beef);
        assert_eq!(bus.read_u32(0x0000_3004), 0xdead_beef);
    }

    #[test]
    fn gpu_block_dma_to_ram_honors_decrement_step() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.io.gpu.gp0_read = 0xfeed_cafe;

        bus.write_u32(DMA_GPU_MADR, 0x0000_3000);
        bus.write_u32(DMA_GPU_BCR, 2);
        bus.write_u32(DMA_GPU_CHCR, (1 << 24) | DMA_STEP_DECREMENT);

        assert_eq!(bus.io.gpu.commands_seen, 0);
        assert_eq!(bus.read_u32(0x0000_3000), 0xfeed_cafe);
        assert_eq!(bus.read_u32(0x0000_2ffc), 0xfeed_cafe);
    }

    #[test]
    fn otc_dma_initializes_reverse_ordering_table() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.write_u32(DMA_OTC_MADR, 0x0000_2008);
        bus.write_u32(DMA_OTC_BCR, 3);
        bus.write_u32(DMA_OTC_CHCR, 0x1100_0002);

        assert_eq!(bus.read_u32(0x0000_2008), 0x0000_2004);
        assert_eq!(bus.read_u32(0x0000_2004), 0x0000_2000);
        assert_eq!(bus.read_u32(0x0000_2000), 0x00ff_ffff);
    }

    #[test]
    fn bus_models_raizing_zn_board_config_and_bank_select() {
        let mut banked = vec![0; 0x0180_0000];
        banked[0] = 0x11;
        banked[0x0080_0000] = 0x22;
        banked[0x0100_0000] = 0x33;
        let mut bus = Bus::with_banked_roms(Vec::new(), banked, 4 * 1024 * 1024);

        assert_eq!(bus.read_u8(0x1fa1_0200), 0x69);
        assert_eq!(bus.read_u8(0x1f00_0000), 0x11);

        bus.write_u8(0x1fa1_0300, 0x01);
        assert_eq!(bus.read_u8(0x1fa1_0300), 0x01);
        assert_eq!(bus.read_u8(0x1f00_0000), 0x22);

        bus.write_u8(0x1fa1_0300, 0x02);
        assert_eq!(bus.read_u8(0x1f00_0000), 0x33);

        assert_eq!(bus.read_u8(0x1faf_0000), 0xff);
        assert_eq!(bus.read_u16(0x1fb2_0000), 0xffff);
        assert!(bus.zn_board_json().contains("\"rom_bank\":2"));
    }

    #[test]
    fn bus_routes_znsecsel_to_zn_mcu_sio_response() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.write_u8(0x1fa1_0300, 0x8c);
        bus.write_u8(SIO_DATA, 0);

        assert_eq!(bus.read_u8(SIO_DATA), 0xff);
        assert!(bus.runtime_probe_json().contains("\"selected\":false"));

        bus.write_u8(0x1fa1_0300, 0x00);
        bus.write_u8(SIO_DATA, 0);

        assert_eq!(bus.read_u8(SIO_DATA), 0x1f);
        assert!(bus.runtime_probe_json().contains("\"selected\":true"));

        bus.write_u8(0x1fa1_0300, 0x10);
        bus.write_u8(SIO_DATA, 0);

        assert_eq!(bus.read_u8(SIO_DATA), 0x8f);
    }

    #[test]
    fn bus_derives_zn_security_response_from_local_bios_bytes() {
        let mut rom = vec![0; 0x0000_bad8];
        rom[0x0000_baa0..0x0000_baa4].copy_from_slice(b"TEST");
        rom[0x0000_b98d..0x0000_b990].copy_from_slice(&[0x12, 0x34, 0x56]);
        let mut bus = Bus::new(rom, 2 * 1024 * 1024);

        bus.write_u16(crate::native::io::SIO_CONTROL, 0x2003);
        bus.write_u8(SIO_DATA, b'T');
        assert_eq!(bus.read_u8(SIO_DATA), 0xff);
        bus.write_u8(SIO_DATA, b'E');
        assert_eq!(bus.read_u8(SIO_DATA), 0x12);
        bus.write_u8(SIO_DATA, b'S');
        assert_eq!(bus.read_u8(SIO_DATA), 0x34);
        bus.write_u8(SIO_DATA, b'T');
        assert_eq!(bus.read_u8(SIO_DATA), 0x56);
    }
}
