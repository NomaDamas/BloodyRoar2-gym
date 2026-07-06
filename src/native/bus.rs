use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::env;

use crate::action::ActionButtons;
use crate::native::io::{
    DMA_GPU_BCR, DMA_GPU_CHCR, DMA_GPU_MADR, DMA_MDEC_IN_BCR, DMA_MDEC_IN_CHCR, DMA_MDEC_IN_MADR,
    DMA_MDEC_OUT_BCR, DMA_MDEC_OUT_CHCR, DMA_MDEC_OUT_MADR, DMA_OTC_BCR, DMA_OTC_CHCR,
    DMA_OTC_MADR, DMA_REGION_END, DMA_REGION_START, GPU_GP0, GPU_GP1, GpuCommandSource,
    IO_REGION_END, IO_REGION_START, IRQ_STATUS, Io, NativeGpuDisplayCandidate,
    NativeGpuDrawCapture, NativeGpuDrawCapturePredicate, gp0_command_word_count, io_access_for,
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
const GPU_VBLANK_PRESENTATION_CAPTURE_INTERVAL: u64 = 60;
const GPU_LINKED_LIST_NODE_LIMIT: u32 = 65_536;
const BR2_DRAW_SYNC_FLAG_VIRTUAL: u32 = 0x803a_2210;
const BR2_DRAW_SYNC_FLAG_PHYSICAL: u32 = 0x003a_2210;
const BR2_PRIMITIVE_RAM_START: u32 = 0x0038_0000;
const BR2_PRIMITIVE_RAM_END: u32 = 0x003c_0000;
const BR2_BOOT_WORD_COPY_LOOP_PHYSICAL: u32 = 0x0001_011c;
const BR2_RUNTIME_CODE_SNAPSHOT_START: u32 = 0x002c_0000;
const BR2_RUNTIME_CODE_SNAPSHOT_END: u32 = 0x0037_0000;
const BR2_RUNTIME_CODE_SNAPSHOT_LEN: usize =
    (BR2_RUNTIME_CODE_SNAPSHOT_END - BR2_RUNTIME_CODE_SNAPSHOT_START) as usize;
const BR2_BOOT_GLOBAL_SNAPSHOT_FALLBACK_RANGES: [(u32, u32); 2] =
    [(0x0036_643c, 0x0036_6480), (0x0036_658c, 0x0036_6590)];
const BR2_CODE_PATCH_SNAPSHOT_START: u32 = 0x002c_c100;
const BR2_CODE_PATCH_SNAPSHOT_END: u32 = 0x002c_c220;
const BR2_CODE_PATCH_SNAPSHOT_LEN: usize =
    (BR2_CODE_PATCH_SNAPSHOT_END - BR2_CODE_PATCH_SNAPSHOT_START) as usize;
const PRIMITIVE_RAM_RECENT_LIMIT: usize = 4096;
const GPU_LINKED_LIST_RECENT_COMMAND_LIMIT: usize = 32;
const GPU_LINKED_LIST_NODE_SAMPLE_LIMIT: usize = 16;
const GPU_LINKED_LIST_NONEMPTY_NODE_SAMPLE_LIMIT: usize = 32;
const PRIMITIVE_RECENT_HEADER_RELATION_LIMIT: usize = 24;
const PRIMITIVE_PACKET_SCAN_SAMPLE_LIMIT: usize = 24;
const PRIMITIVE_PACKET_MAX_WORDS: u32 = 64;
const DMA_ACTIVITY_RECENT_LIMIT: usize = 64;
const DMA_OTC_RECENT_RANGE_LIMIT: usize = 8;
const DMA_GPU_RECENT_REGISTER_WRITE_LIMIT: usize = 8;
const ZN_BOARD_INPUT_READ_PORTS: [u32; 5] = [
    0x1fa0_0000,
    0x1fa0_0200,
    0x1fa0_0300,
    0x1fa1_0000,
    0x1fa2_0000,
];
const ZN_BOARD_INPUT_READ_RECENT_LIMIT: usize = 128;
const ZN_BOARD_INPUT_ACTIVE_READ_RECENT_LIMIT: usize = 1024;
const ZN_BOARD_INPUT_COMPACT_ACTIVE_READ_LIMIT: usize = 32;
const LEGACY_ZINC_SYSTEM_EDGE_LATCH_READS: u8 = 120;
const BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW: u64 = 4;
const BR2_UNLINKED_PRIMITIVE_REPLAY_PACKET_LIMIT: usize = 512;
const BR2_UNLINKED_PRIMITIVE_REPLAY_SPARSE_NODE_LIMIT: u32 = 32;
const BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_LINKED_NODES: u32 = 512;
const BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_DRAW_PACKETS: u32 = 8;
const BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_RECENT_HEADERS: u64 = 32;
const BR2_UNLINKED_PRIMITIVE_REPLAY_STALE_SCAN_MIN_PACKETS: u32 = 8;
const BR2_UNLINKED_PRIMITIVE_REPLAY_STATE_CHAIN_LIMIT: usize = 8;
const BR2_UNLINKED_PRIMITIVE_REPLAY_REJECT_COOLDOWN_VBLANKS: u64 = 60;
const BR2_UNLINKED_PRIMITIVE_REPLAY_FULL_VALIDATION_COOLDOWN_VBLANKS: u64 = 300;
const BR2_UNLINKED_PRIMITIVE_REPLAY_SPARSE_SCENE_SCAN_INTERVAL: u64 = 15;
const BR2_UNLINKED_PRIMITIVE_REPLAY_INTERVAL_ENV: &str =
    "BR2_NATIVE_UNLINKED_PRIMITIVE_REPLAY_INTERVAL";
const ZN_BOARD_RECENT_COIN_REGISTER_WRITES: usize = 16;
const NATIVE_COIN_MAPPING_ENV: &str = "BLOODYROAR2_NATIVE_COIN_MAPPING";
const BR2_INPUT_SCRATCHPAD_WORD: u32 = 0x1f80_006c;
const BR2_INPUT_SCRATCHPAD_WRITE_PC_PHYSICAL: u32 = 0x002c_ff84;
const BR2_INPUT_SCRATCHPAD_EDGE_WORD: u32 = 0x1f80_0068;
const BR2_INPUT_SCRATCHPAD_EDGE_MIRROR_WORD: u32 = 0x1f80_0074;
const BR2_INPUT_SCRATCHPAD_EDGE_WRITE_PC_PHYSICAL: u32 = 0x002c_ff94;
const BR2_INPUT_SCRATCHPAD_EDGE_MIRROR_WRITE_PC_PHYSICAL: u32 = 0x002c_ff98;
const BR2_CREDIT_STATE_BASE: u32 = 0x803b_fa00;
const BR2_CREDIT_FREEPLAY_FLAG_OFFSET: u32 = 0x00;
const BR2_CREDIT_PLAYER_MODE_OFFSET: u32 = 0x01;
const BR2_CREDIT_REQUIRED_P1_OFFSET: u32 = 0x08;
const BR2_CREDIT_REQUIRED_P2_OFFSET: u32 = 0x09;
const BR2_CREDIT_SHARED_SLOT_OFFSET: u32 = 0x18;
const BR2_NATIVE_CREDIT_INPUT_BIT: u32 = 0x0000_0008;
const BR2_NATIVE_CREDIT_INPUT_BIT_ENV: &str = "BLOODYROAR2_NATIVE_CREDIT_INPUT_BIT";
const BR2_NATIVE_CREDIT_PROJECTION_ENV: &str = "BLOODYROAR2_NATIVE_CREDIT_PROJECTION";
const BR2_NATIVE_CREDIT_ADAPTER_PENDING_WRITES: u8 = 16;
const BR2_NATIVE_CREDIT_ADAPTER_EDGE_PROJECTION_WRITES: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeCreditProjectionBucket {
    Current,
    Edge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeCreditProjectionRule {
    address: u32,
    access_len: usize,
    pc: Option<u32>,
    mask: Option<u32>,
    bucket: NativeCreditProjectionBucket,
}

impl NativeCreditProjectionRule {
    fn new(
        address: u32,
        access_len: usize,
        pc: Option<u32>,
        mask: Option<u32>,
        bucket: NativeCreditProjectionBucket,
    ) -> Self {
        Self {
            address: physical_address(address),
            access_len,
            pc: pc.map(physical_address),
            mask,
            bucket,
        }
    }

    fn matches(self, address: u32, access_len: usize, pc: Option<u32>) -> bool {
        self.address == physical_address(address)
            && self.access_len == access_len
            && self
                .pc
                .map(|expected| Some(expected) == pc.map(physical_address))
                .unwrap_or(true)
    }

    fn effective_mask(self, default_mask: u32) -> u32 {
        self.mask.unwrap_or(default_mask)
    }

    fn json(self) -> String {
        format!(
            "{{\"address\":{},\"address_hex\":\"0x{:08x}\",\"access_len\":{},\"pc\":{},\"pc_hex\":{},\"mask\":{},\"mask_hex\":{},\"bucket\":\"{}\"}}",
            self.address,
            self.address,
            self.access_len,
            optional_u32_json(self.pc),
            optional_u32_hex_json(self.pc),
            optional_u32_json(self.mask),
            optional_u32_hex_json(self.mask),
            match self.bucket {
                NativeCreditProjectionBucket::Current => "current",
                NativeCreditProjectionBucket::Edge => "edge",
            }
        )
    }
}

fn native_credit_default_projection_rules() -> Vec<NativeCreditProjectionRule> {
    vec![
        NativeCreditProjectionRule::new(
            BR2_INPUT_SCRATCHPAD_WORD,
            4,
            Some(BR2_INPUT_SCRATCHPAD_WRITE_PC_PHYSICAL),
            None,
            NativeCreditProjectionBucket::Current,
        ),
        NativeCreditProjectionRule::new(
            BR2_INPUT_SCRATCHPAD_EDGE_WORD,
            4,
            Some(BR2_INPUT_SCRATCHPAD_EDGE_WRITE_PC_PHYSICAL),
            None,
            NativeCreditProjectionBucket::Edge,
        ),
        NativeCreditProjectionRule::new(
            BR2_INPUT_SCRATCHPAD_EDGE_MIRROR_WORD,
            4,
            Some(BR2_INPUT_SCRATCHPAD_EDGE_MIRROR_WRITE_PC_PHYSICAL),
            None,
            NativeCreditProjectionBucket::Edge,
        ),
    ]
}

fn native_credit_projection_rules_from_env() -> Vec<NativeCreditProjectionRule> {
    env::var(BR2_NATIVE_CREDIT_PROJECTION_ENV)
        .ok()
        .and_then(|value| parse_native_credit_projection_rules(&value))
        .unwrap_or_else(native_credit_default_projection_rules)
}

fn parse_native_credit_projection_rules(value: &str) -> Option<Vec<NativeCreditProjectionRule>> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
        return Some(native_credit_default_projection_rules());
    }
    if trimmed.eq_ignore_ascii_case("none") || trimmed.eq_ignore_ascii_case("off") {
        return Some(Vec::new());
    }

    let mut rules = Vec::new();
    for token in trimmed.split(|ch: char| ch == ',' || ch == ';' || ch.is_whitespace()) {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let (target, mask) = parse_native_credit_projection_token_mask(token)?;
        append_native_credit_projection_target_rules(&mut rules, target, mask)?;
    }
    Some(rules)
}

fn parse_native_credit_projection_token_mask(token: &str) -> Option<(&str, Option<u32>)> {
    let Some((target, mask)) = token.split_once('=') else {
        return Some((token, None));
    };
    let parsed_mask = parse_native_u32_env_value(mask)?;
    if parsed_mask == 0 {
        return None;
    }
    Some((target, Some(parsed_mask)))
}

fn append_native_credit_projection_target_rules(
    rules: &mut Vec<NativeCreditProjectionRule>,
    target: &str,
    mask: Option<u32>,
) -> Option<()> {
    let normalized = target.trim().to_ascii_lowercase().replace(['-', '_'], "");
    match normalized.as_str() {
        "" | "auto" | "default" | "inputedge" => {
            rules.extend(
                native_credit_default_projection_rules()
                    .into_iter()
                    .map(|rule| NativeCreditProjectionRule { mask, ..rule }),
            );
        }
        "current" | "input" | "word" | "inputword" => {
            rules.push(NativeCreditProjectionRule::new(
                BR2_INPUT_SCRATCHPAD_WORD,
                4,
                Some(BR2_INPUT_SCRATCHPAD_WRITE_PC_PHYSICAL),
                mask,
                NativeCreditProjectionBucket::Current,
            ));
        }
        "previous" | "prev" | "last" => {
            rules.push(NativeCreditProjectionRule::new(
                0x1f80_0070,
                4,
                None,
                mask,
                NativeCreditProjectionBucket::Current,
            ));
        }
        "edge" => {
            rules.push(NativeCreditProjectionRule::new(
                BR2_INPUT_SCRATCHPAD_EDGE_WORD,
                4,
                Some(BR2_INPUT_SCRATCHPAD_EDGE_WRITE_PC_PHYSICAL),
                mask,
                NativeCreditProjectionBucket::Edge,
            ));
        }
        "edgemirror" | "mirror" => {
            rules.push(NativeCreditProjectionRule::new(
                BR2_INPUT_SCRATCHPAD_EDGE_MIRROR_WORD,
                4,
                Some(BR2_INPUT_SCRATCHPAD_EDGE_MIRROR_WRITE_PC_PHYSICAL),
                mask,
                NativeCreditProjectionBucket::Edge,
            ));
        }
        "copies" | "copy" | "mirrors" => {
            append_native_credit_copy_projection_rules(rules, mask);
        }
        "wide" | "all" => {
            rules.extend(
                native_credit_default_projection_rules()
                    .into_iter()
                    .map(|rule| NativeCreditProjectionRule { mask, ..rule }),
            );
            rules.push(NativeCreditProjectionRule::new(
                0x1f80_0070,
                4,
                None,
                mask,
                NativeCreditProjectionBucket::Current,
            ));
            append_native_credit_copy_projection_rules(rules, mask);
            append_native_credit_watch_projection_rules(rules, mask);
        }
        _ => {
            let (address, access_len) = parse_native_credit_projection_address_target(target)?;
            rules.push(NativeCreditProjectionRule::new(
                address,
                access_len,
                None,
                mask,
                NativeCreditProjectionBucket::Current,
            ));
        }
    }
    Some(())
}

fn append_native_credit_copy_projection_rules(
    rules: &mut Vec<NativeCreditProjectionRule>,
    mask: Option<u32>,
) {
    for (address, access_len) in [
        (0x1f80_007c, 4),
        (0x1f80_007c, 2),
        (0x1f80_007e, 2),
        (0x1f80_0080, 4),
    ] {
        rules.push(NativeCreditProjectionRule::new(
            address,
            access_len,
            None,
            mask,
            NativeCreditProjectionBucket::Current,
        ));
    }
}

fn append_native_credit_watch_projection_rules(
    rules: &mut Vec<NativeCreditProjectionRule>,
    mask: Option<u32>,
) {
    for (address, access_len) in [
        (0x1f80_0078, 4),
        (0x1f80_0078, 1),
        (0x1f80_0079, 1),
        (0x1f80_009e, 2),
        (0x1f80_00b8, 4),
        (0x1f80_00ce, 2),
    ] {
        rules.push(NativeCreditProjectionRule::new(
            address,
            access_len,
            None,
            mask,
            NativeCreditProjectionBucket::Current,
        ));
    }
}

fn parse_native_credit_projection_address_target(target: &str) -> Option<(u32, usize)> {
    let (address, access_len) = target.split_once('/').unwrap_or((target, "4"));
    let parsed_address = parse_native_u32_env_value(address)?;
    let parsed_len = access_len.parse::<usize>().ok()?;
    matches!(parsed_len, 1 | 2 | 4).then_some((parsed_address, parsed_len))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeCoinInputMapping {
    Mame,
    LegacyZinc,
    System20,
    System10System20,
    Service01,
    Service02,
    Service10,
    Service02Service10,
    CoinRegisterBit0,
    All,
}

impl NativeCoinInputMapping {
    fn default_for_compat(legacy_zinc_input_compat: bool) -> Self {
        if legacy_zinc_input_compat {
            Self::LegacyZinc
        } else {
            Self::Mame
        }
    }

    fn from_env(legacy_zinc_input_compat: bool) -> Self {
        env::var(NATIVE_COIN_MAPPING_ENV)
            .ok()
            .as_deref()
            .and_then(Self::parse)
            .unwrap_or_else(|| Self::default_for_compat(legacy_zinc_input_compat))
    }

    fn parse(value: &str) -> Option<Self> {
        let normalized = value
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_', '+'], "");
        match normalized.as_str() {
            "" | "auto" => None,
            "mame" | "system10" | "sys10" | "coin1" => Some(Self::Mame),
            "legacy" | "zinc" | "legacyzinc" | "zinclegacy" => Some(Self::LegacyZinc),
            "system20" | "sys20" | "coin2" => Some(Self::System20),
            "system1020" | "sys1020" | "system10system20" | "coin12" => {
                Some(Self::System10System20)
            }
            "service01" | "svc01" => Some(Self::Service01),
            "service02" | "svc02" | "service" => Some(Self::Service02),
            "service10" | "svc10" => Some(Self::Service10),
            "service0210" | "svc0210" | "service02service10" => Some(Self::Service02Service10),
            "coinregisterbit0" | "coinregister" | "registerbit0" | "regbit0" => {
                Some(Self::CoinRegisterBit0)
            }
            "all" | "wide" => Some(Self::All),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Mame => "mame",
            Self::LegacyZinc => "legacy-zinc",
            Self::System20 => "system20",
            Self::System10System20 => "system10-system20",
            Self::Service01 => "service01",
            Self::Service02 => "service02",
            Self::Service10 => "service10",
            Self::Service02Service10 => "service02-service10",
            Self::CoinRegisterBit0 => "coin-register-bit0",
            Self::All => "all",
        }
    }

    fn clears_system_10(self) -> bool {
        matches!(
            self,
            Self::Mame | Self::LegacyZinc | Self::System10System20 | Self::All
        )
    }

    fn clears_system_20(self) -> bool {
        matches!(
            self,
            Self::LegacyZinc | Self::System20 | Self::System10System20 | Self::All
        )
    }

    fn clears_service_01(self) -> bool {
        matches!(self, Self::Service01 | Self::All)
    }

    fn clears_service_02(self) -> bool {
        matches!(
            self,
            Self::LegacyZinc | Self::Service02 | Self::Service02Service10 | Self::All
        )
    }

    fn clears_service_10(self) -> bool {
        matches!(
            self,
            Self::LegacyZinc | Self::Service10 | Self::Service02Service10 | Self::All
        )
    }

    fn mirrors_coin_register_bit0(self) -> bool {
        matches!(self, Self::LegacyZinc | Self::CoinRegisterBit0 | Self::All)
    }

    fn enables_legacy_start_compat(self) -> bool {
        matches!(self, Self::LegacyZinc | Self::All)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CoinRegisterWriteEvent {
    vblank: u64,
    cycles: u64,
    pc: Option<u32>,
    address: u32,
    access_len: usize,
    raw_value: u32,
    merged_value: u32,
    data: u8,
}

impl CoinRegisterWriteEvent {
    fn json(self) -> String {
        format!(
            "{{\"vblank\":{},\"cycles\":{},\"pc\":{},\"pc_hex\":{},\"address\":{},\"address_hex\":\"0x{:08x}\",\"access_len\":{},\"raw_value\":{},\"raw_value_hex\":\"0x{:08x}\",\"merged_value\":{},\"merged_value_hex\":\"0x{:08x}\",\"data\":{},\"data_hex\":\"0x{:02x}\"}}",
            self.vblank,
            self.cycles,
            optional_u32_json(self.pc),
            optional_u32_hex_json(self.pc),
            self.address,
            self.address,
            self.access_len,
            self.raw_value,
            self.raw_value,
            self.merged_value,
            self.merged_value,
            self.data,
            self.data
        )
    }
}

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
    pub system_service_active_reads: u64,
    pub system_start_active_reads: u64,
    pub coin_register_reads: u64,
    pub coin_register_active_reads: u64,
    pub coin_register_writes: u64,
    pub coin_insert_edges: u64,
    pub coin_counter_0_edges: u64,
    pub coin_counter_1_edges: u64,
    pub legacy_system_coin_latch_edges: u64,
    pub legacy_system_start_latch_edges: u64,
    pub native_credit_adapter_writes: u64,
    pub native_credit_adapter_edges: u64,
    pub last_system_input: u32,
    pub last_coin_register: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Br2NativeCreditHleCheck {
    pub player: u32,
    pub freeplay: bool,
    pub required: u8,
    pub credit_slot: u32,
    pub credit_before: u8,
    pub credit_after: u8,
    pub pending_coin_edges: u64,
    pub result: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Br2NativeCreditHleStats {
    calls: u64,
    accepted: u64,
    rejected: u64,
    freeplay: u64,
    injected_coin_edges: u64,
    last: Option<Br2NativeCreditHleCheck>,
}

impl Br2NativeCreditHleStats {
    fn record(&mut self, check: Br2NativeCreditHleCheck) {
        self.calls = self.calls.saturating_add(1);
        self.injected_coin_edges = self
            .injected_coin_edges
            .saturating_add(check.pending_coin_edges);
        if check.freeplay {
            self.freeplay = self.freeplay.saturating_add(1);
        }
        if check.result == u32::MAX {
            self.rejected = self.rejected.saturating_add(1);
        } else {
            self.accepted = self.accepted.saturating_add(1);
        }
        self.last = Some(check);
    }

    fn json(&self) -> String {
        let last = self.last.map_or_else(
            || "null".to_string(),
            |check| {
                format!(
                    "{{\"player\":{},\"freeplay\":{},\"required\":{},\"credit_slot\":{},\"credit_slot_hex\":\"0x{:08x}\",\"credit_before\":{},\"credit_after\":{},\"pending_coin_edges\":{},\"result\":{},\"result_hex\":\"0x{:08x}\"}}",
                    check.player,
                    check.freeplay,
                    check.required,
                    check.credit_slot,
                    check.credit_slot,
                    check.credit_before,
                    check.credit_after,
                    check.pending_coin_edges,
                    check.result,
                    check.result
                )
            },
        );
        format!(
            "{{\"calls\":{},\"accepted\":{},\"rejected\":{},\"freeplay\":{},\"injected_coin_edges\":{},\"last\":{}}}",
            self.calls, self.accepted, self.rejected, self.freeplay, self.injected_coin_edges, last
        )
    }
}

impl NativeInputActivity {
    pub fn has_play_control_activity(self) -> bool {
        self.p1_punch_active_reads > 0
            && self.p1_kick_active_reads > 0
            && self.p1_beast_active_reads > 0
            && self.p3_guard_active_reads > 0
            && self.system_coin_active_reads > 0
            && self.system_start_active_reads > 0
    }

    pub fn has_direction_activity(self) -> bool {
        self.p1_up_active_reads > 0
            && self.p1_down_active_reads > 0
            && self.p1_left_active_reads > 0
            && self.p1_right_active_reads > 0
    }

    pub fn has_any_direction_activity(self) -> bool {
        self.p1_up_active_reads > 0
            || self.p1_down_active_reads > 0
            || self.p1_left_active_reads > 0
            || self.p1_right_active_reads > 0
    }

    pub fn has_any_attack_activity(self) -> bool {
        self.p1_punch_active_reads > 0
            || self.p1_kick_active_reads > 0
            || self.p1_beast_active_reads > 0
            || self.p3_guard_active_reads > 0
    }

    pub fn has_any_play_control_activity(self) -> bool {
        self.has_any_attack_activity()
            || self.system_coin_active_reads > 0
            || self.system_start_active_reads > 0
            || self.p1_start_active_reads > 0
    }

    pub fn has_service_activity(self) -> bool {
        self.system_service_active_reads > 0
    }

    pub fn has_any_control_activity(self) -> bool {
        self.has_any_direction_activity()
            || self.has_any_play_control_activity()
            || self.has_service_activity()
    }

    pub fn has_full_control_activity(self) -> bool {
        self.has_direction_activity() && self.has_play_control_activity()
    }

    pub fn has_coin_edge_activity(self) -> bool {
        self.coin_insert_edges > 0 || self.legacy_system_coin_latch_edges > 0
    }

    pub fn has_start_edge_activity(self) -> bool {
        self.legacy_system_start_latch_edges > 0
            || self.system_start_active_reads > 0
            || self.p1_start_active_reads > 0
    }

    pub fn has_coin_probe_activity(self) -> bool {
        self.has_coin_edge_activity() && self.system_coin_active_reads > 0
    }

    pub fn has_start_probe_activity(self) -> bool {
        self.has_start_edge_activity()
            && (self.system_start_active_reads > 0 || self.p1_start_active_reads > 0)
    }

    pub fn has_credit_probe_activity(self) -> bool {
        self.has_coin_probe_activity() && self.has_start_probe_activity()
    }

    pub fn has_coin_register_active_activity(self) -> bool {
        self.coin_register_reads > 0 && self.coin_register_active_reads > 0
    }

    pub fn has_native_credit_adapter_activity(self) -> bool {
        self.native_credit_adapter_writes > 0 || self.native_credit_adapter_edges > 0
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
            system_service_active_reads: self
                .system_service_active_reads
                .saturating_add(other.system_service_active_reads),
            system_start_active_reads: self
                .system_start_active_reads
                .saturating_add(other.system_start_active_reads),
            coin_register_reads: self
                .coin_register_reads
                .saturating_add(other.coin_register_reads),
            coin_register_active_reads: self
                .coin_register_active_reads
                .saturating_add(other.coin_register_active_reads),
            coin_register_writes: self
                .coin_register_writes
                .saturating_add(other.coin_register_writes),
            coin_insert_edges: self
                .coin_insert_edges
                .saturating_add(other.coin_insert_edges),
            coin_counter_0_edges: self
                .coin_counter_0_edges
                .saturating_add(other.coin_counter_0_edges),
            coin_counter_1_edges: self
                .coin_counter_1_edges
                .saturating_add(other.coin_counter_1_edges),
            legacy_system_coin_latch_edges: self
                .legacy_system_coin_latch_edges
                .saturating_add(other.legacy_system_coin_latch_edges),
            legacy_system_start_latch_edges: self
                .legacy_system_start_latch_edges
                .saturating_add(other.legacy_system_start_latch_edges),
            native_credit_adapter_writes: self
                .native_credit_adapter_writes
                .saturating_add(other.native_credit_adapter_writes),
            native_credit_adapter_edges: self
                .native_credit_adapter_edges
                .saturating_add(other.native_credit_adapter_edges),
            last_system_input: if other.system_input_reads > 0 {
                other.last_system_input
            } else {
                self.last_system_input
            },
            last_coin_register: if other.coin_register_reads > 0 {
                other.last_coin_register
            } else {
                self.last_coin_register
            },
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
            system_service_active_reads: self
                .system_service_active_reads
                .saturating_sub(baseline.system_service_active_reads),
            system_start_active_reads: self
                .system_start_active_reads
                .saturating_sub(baseline.system_start_active_reads),
            coin_register_reads: self
                .coin_register_reads
                .saturating_sub(baseline.coin_register_reads),
            coin_register_active_reads: self
                .coin_register_active_reads
                .saturating_sub(baseline.coin_register_active_reads),
            coin_register_writes: self
                .coin_register_writes
                .saturating_sub(baseline.coin_register_writes),
            coin_insert_edges: self
                .coin_insert_edges
                .saturating_sub(baseline.coin_insert_edges),
            coin_counter_0_edges: self
                .coin_counter_0_edges
                .saturating_sub(baseline.coin_counter_0_edges),
            coin_counter_1_edges: self
                .coin_counter_1_edges
                .saturating_sub(baseline.coin_counter_1_edges),
            legacy_system_coin_latch_edges: self
                .legacy_system_coin_latch_edges
                .saturating_sub(baseline.legacy_system_coin_latch_edges),
            legacy_system_start_latch_edges: self
                .legacy_system_start_latch_edges
                .saturating_sub(baseline.legacy_system_start_latch_edges),
            native_credit_adapter_writes: self
                .native_credit_adapter_writes
                .saturating_sub(baseline.native_credit_adapter_writes),
            native_credit_adapter_edges: self
                .native_credit_adapter_edges
                .saturating_sub(baseline.native_credit_adapter_edges),
            last_system_input: self.last_system_input,
            last_coin_register: self.last_coin_register,
        }
    }

    pub fn json(self) -> String {
        format!(
            "{{\"p1_input_reads\":{},\"p1_up_active_reads\":{},\"p1_down_active_reads\":{},\"p1_left_active_reads\":{},\"p1_right_active_reads\":{},\"p1_start_active_reads\":{},\"p1_punch_active_reads\":{},\"p1_kick_active_reads\":{},\"p1_beast_active_reads\":{},\"p3_input_reads\":{},\"p3_guard_active_reads\":{},\"system_input_reads\":{},\"system_coin_active_reads\":{},\"system_service_active_reads\":{},\"system_start_active_reads\":{},\"coin_register_reads\":{},\"coin_register_active_reads\":{},\"coin_register_writes\":{},\"coin_insert_edges\":{},\"coin_counter_0_edges\":{},\"coin_counter_1_edges\":{},\"legacy_system_coin_latch_edges\":{},\"legacy_system_start_latch_edges\":{},\"native_credit_adapter_writes\":{},\"native_credit_adapter_edges\":{},\"last_system_input\":{},\"last_system_input_hex\":\"0x{:08x}\",\"last_coin_register\":{},\"last_coin_register_hex\":\"0x{:08x}\",\"has_direction_activity\":{},\"has_play_control_activity\":{},\"has_full_control_activity\":{},\"has_any_direction_activity\":{},\"has_any_attack_activity\":{},\"has_any_play_control_activity\":{},\"has_service_activity\":{},\"has_any_control_activity\":{},\"has_coin_edge_activity\":{},\"has_start_edge_activity\":{},\"has_coin_probe_activity\":{},\"has_start_probe_activity\":{},\"has_credit_probe_activity\":{},\"has_coin_register_active_activity\":{},\"has_native_credit_adapter_activity\":{}}}",
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
            self.system_service_active_reads,
            self.system_start_active_reads,
            self.coin_register_reads,
            self.coin_register_active_reads,
            self.coin_register_writes,
            self.coin_insert_edges,
            self.coin_counter_0_edges,
            self.coin_counter_1_edges,
            self.legacy_system_coin_latch_edges,
            self.legacy_system_start_latch_edges,
            self.native_credit_adapter_writes,
            self.native_credit_adapter_edges,
            self.last_system_input,
            self.last_system_input,
            self.last_coin_register,
            self.last_coin_register,
            self.has_direction_activity(),
            self.has_play_control_activity(),
            self.has_full_control_activity(),
            self.has_any_direction_activity(),
            self.has_any_attack_activity(),
            self.has_any_play_control_activity(),
            self.has_service_activity(),
            self.has_any_control_activity(),
            self.has_coin_edge_activity(),
            self.has_start_edge_activity(),
            self.has_coin_probe_activity(),
            self.has_start_probe_activity(),
            self.has_credit_probe_activity(),
            self.has_coin_register_active_activity(),
            self.has_native_credit_adapter_activity()
        )
    }
}

#[derive(Clone, Debug)]
struct GpuLinkedListDmaStats {
    calls: u64,
    last_start: u32,
    last_first_node: u32,
    last_pc: Option<u32>,
    last_vblank: u64,
    last_cycles: u64,
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
    embedded_payload_skips: u64,
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
    command_write_vblank_by_address: HashMap<u32, u64>,
    last_address: Option<u32>,
    last_value: u32,
    last_pc: Option<u32>,
    recent_command_like_writes: Vec<PrimitiveRamWriteSample>,
    recent_header_like_writes: Vec<PrimitiveRamWriteSample>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PrimitiveRamWriteRecord {
    header_like: bool,
    command_like: bool,
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

#[derive(Clone, Debug, Default)]
struct DmaChannelLifetimeStats {
    samples: u64,
    register_writes: u64,
    transfers: u64,
    gpu_linked_list: u64,
    gpu_block_write: u64,
    gpu_read: u64,
    otc_clear: u64,
    last_vblank: Option<u64>,
    last_pc: Option<u32>,
    last_activity: Option<DmaActivitySample>,
    last_register_write: Option<DmaActivitySample>,
    last_transfer: Option<DmaActivitySample>,
}

#[derive(Clone, Debug)]
struct UnlinkedPrimitiveReplayStats {
    attempts: u64,
    conditional_replays: u64,
    forced_replays: u64,
    skipped: u64,
    total_packets: u64,
    total_words: u64,
    full_validations: u64,
    last_full_validation_vblank: Option<u64>,
    last_vblank: Option<u64>,
    last_reason: &'static str,
    last_candidate_headers: usize,
    last_linked_nodes: u32,
    last_linked_nonempty_nodes: u32,
    last_linked_words: u32,
    last_packets: usize,
    last_words: usize,
    last_diagnostics: UnlinkedPrimitiveReplayDiagnostics,
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

impl DmaChannelLifetimeStats {
    fn record(&mut self, sample: &DmaActivitySample) {
        self.samples = self.samples.saturating_add(1);
        self.last_vblank = Some(sample.vblank);
        self.last_pc = sample.pc;
        self.last_activity = Some(sample.clone());

        match sample.kind {
            "register_write" => {
                self.register_writes = self.register_writes.saturating_add(1);
                self.last_register_write = Some(sample.clone());
            }
            "gpu_linked_list" => {
                self.transfers = self.transfers.saturating_add(1);
                self.gpu_linked_list = self.gpu_linked_list.saturating_add(1);
                self.last_transfer = Some(sample.clone());
            }
            "gpu_block_write" => {
                self.transfers = self.transfers.saturating_add(1);
                self.gpu_block_write = self.gpu_block_write.saturating_add(1);
                self.last_transfer = Some(sample.clone());
            }
            "gpu_read" => {
                self.transfers = self.transfers.saturating_add(1);
                self.gpu_read = self.gpu_read.saturating_add(1);
                self.last_transfer = Some(sample.clone());
            }
            "otc_clear" => {
                self.transfers = self.transfers.saturating_add(1);
                self.otc_clear = self.otc_clear.saturating_add(1);
                self.last_transfer = Some(sample.clone());
            }
            _ => {
                self.transfers = self.transfers.saturating_add(1);
                self.last_transfer = Some(sample.clone());
            }
        }
    }

    fn json(&self, channel: usize) -> String {
        format!(
            "{{\"channel\":{},\"samples\":{},\"register_writes\":{},\"transfers\":{},\"gpu_linked_list\":{},\"gpu_block_write\":{},\"gpu_read\":{},\"otc_clear\":{},\"last_vblank\":{},\"last_pc\":{},\"last_pc_hex\":{},\"last_activity\":{},\"last_register_write\":{},\"last_transfer\":{}}}",
            channel,
            self.samples,
            self.register_writes,
            self.transfers,
            self.gpu_linked_list,
            self.gpu_block_write,
            self.gpu_read,
            self.otc_clear,
            optional_u64_json(self.last_vblank),
            optional_u32_json(self.last_pc),
            optional_u32_hex_json(self.last_pc),
            self.last_activity
                .as_ref()
                .map_or_else(|| "null".to_string(), DmaActivitySample::json),
            self.last_register_write
                .as_ref()
                .map_or_else(|| "null".to_string(), DmaActivitySample::json),
            self.last_transfer
                .as_ref()
                .map_or_else(|| "null".to_string(), DmaActivitySample::json)
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
            full_validations: 0,
            last_full_validation_vblank: None,
            last_vblank: None,
            last_reason: "never",
            last_candidate_headers: 0,
            last_linked_nodes: 0,
            last_linked_nonempty_nodes: 0,
            last_linked_words: 0,
            last_packets: 0,
            last_words: 0,
            last_diagnostics: UnlinkedPrimitiveReplayDiagnostics::default(),
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
        diagnostics: UnlinkedPrimitiveReplayDiagnostics,
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
        self.last_diagnostics = diagnostics;
    }

    #[allow(clippy::too_many_arguments)]
    fn record_replay(
        &mut self,
        vblank: u64,
        reason: &'static str,
        candidate_headers: usize,
        linked: &GpuLinkedListDmaRunStats,
        diagnostics: UnlinkedPrimitiveReplayDiagnostics,
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
        self.last_diagnostics = diagnostics;
    }

    fn record_full_validation(&mut self, vblank: u64) {
        self.full_validations = self.full_validations.saturating_add(1);
        self.last_full_validation_vblank = Some(vblank);
    }

    fn json(&self) -> String {
        format!(
            "{{\"attempts\":{},\"conditional_replays\":{},\"forced_replays\":{},\"skipped\":{},\"total_packets\":{},\"total_words\":{},\"full_validations\":{},\"last_full_validation_vblank\":{},\"last_vblank\":{},\"last_reason\":\"{}\",\"last_candidate_headers\":{},\"last_linked_nodes\":{},\"last_linked_nonempty_nodes\":{},\"last_linked_words\":{},\"last_packets\":{},\"last_words\":{},\"last_diagnostics\":{}}}",
            self.attempts,
            self.conditional_replays,
            self.forced_replays,
            self.skipped,
            self.total_packets,
            self.total_words,
            self.full_validations,
            optional_u64_json(self.last_full_validation_vblank),
            optional_u64_json(self.last_vblank),
            self.last_reason,
            self.last_candidate_headers,
            self.last_linked_nodes,
            self.last_linked_nonempty_nodes,
            self.last_linked_words,
            self.last_packets,
            self.last_words,
            self.last_diagnostics.json()
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UnlinkedPrimitiveReplayDiagnostics {
    min_vblank: u64,
    recent_header_count: usize,
    recent_header_writes: u64,
    recent_draw_writes: u64,
    recent_draw_candidates: u32,
    recent_stale_draw_candidates: u32,
    stale_draw_candidates: u32,
    replay_input_candidates: u32,
    replay_ordered_candidates: u32,
    replay_linked_skips: u32,
    replay_non_draw_skips: u32,
    replay_empty_safe_range_skips: u32,
    replay_duplicate_command_skips: u32,
    replay_state_words: u32,
    replayed_draw_packets: u32,
    replayed_draw_words: u32,
    replayed_min_address: Option<u32>,
    replayed_max_address: Option<u32>,
    replayed_opcode_counts: [u32; 256],
    raw_stream_candidates: u32,
    raw_stream_rejected_incomplete: u32,
    raw_stream_rejected_unsafe: u32,
    raw_stream_replayed_packets: u32,
    raw_stream_replayed_words: u32,
    reject_no_bounds: u32,
    reject_non_playfield_bounds: u32,
    reject_unsafe_bounds: u32,
    reject_zero_texture_oversize: u32,
    reject_command_texture_contamination: u32,
    reject_primitive_pointer_contamination: u32,
    reject_linked_list_artifact: u32,
    reject_title_overlay_atlas: u32,
}

impl Default for UnlinkedPrimitiveReplayDiagnostics {
    fn default() -> Self {
        Self {
            min_vblank: 0,
            recent_header_count: 0,
            recent_header_writes: 0,
            recent_draw_writes: 0,
            recent_draw_candidates: 0,
            recent_stale_draw_candidates: 0,
            stale_draw_candidates: 0,
            replay_input_candidates: 0,
            replay_ordered_candidates: 0,
            replay_linked_skips: 0,
            replay_non_draw_skips: 0,
            replay_empty_safe_range_skips: 0,
            replay_duplicate_command_skips: 0,
            replay_state_words: 0,
            replayed_draw_packets: 0,
            replayed_draw_words: 0,
            replayed_min_address: None,
            replayed_max_address: None,
            replayed_opcode_counts: [0; 256],
            raw_stream_candidates: 0,
            raw_stream_rejected_incomplete: 0,
            raw_stream_rejected_unsafe: 0,
            raw_stream_replayed_packets: 0,
            raw_stream_replayed_words: 0,
            reject_no_bounds: 0,
            reject_non_playfield_bounds: 0,
            reject_unsafe_bounds: 0,
            reject_zero_texture_oversize: 0,
            reject_command_texture_contamination: 0,
            reject_primitive_pointer_contamination: 0,
            reject_linked_list_artifact: 0,
            reject_title_overlay_atlas: 0,
        }
    }
}

impl UnlinkedPrimitiveReplayDiagnostics {
    fn json(self) -> String {
        format!(
            "{{\"min_vblank\":{},\"recent_header_count\":{},\"recent_header_writes\":{},\"recent_draw_writes\":{},\"recent_draw_candidates\":{},\"recent_stale_draw_candidates\":{},\"stale_draw_candidates\":{},\"replay_input_candidates\":{},\"replay_ordered_candidates\":{},\"replay_linked_skips\":{},\"replay_non_draw_skips\":{},\"replay_empty_safe_range_skips\":{},\"replay_duplicate_command_skips\":{},\"replay_state_words\":{},\"replayed_draw_packets\":{},\"replayed_draw_words\":{},\"replayed_min_address\":{},\"replayed_min_address_hex\":{},\"replayed_max_address\":{},\"replayed_max_address_hex\":{},\"replayed_opcode_counts\":[{}],\"raw_stream_candidates\":{},\"raw_stream_rejected_incomplete\":{},\"raw_stream_rejected_unsafe\":{},\"raw_stream_replayed_packets\":{},\"raw_stream_replayed_words\":{},\"reject_no_bounds\":{},\"reject_non_playfield_bounds\":{},\"reject_unsafe_bounds\":{},\"reject_zero_texture_oversize\":{},\"reject_command_texture_contamination\":{},\"reject_primitive_pointer_contamination\":{},\"reject_linked_list_artifact\":{},\"reject_title_overlay_atlas\":{}}}",
            self.min_vblank,
            self.recent_header_count,
            self.recent_header_writes,
            self.recent_draw_writes,
            self.recent_draw_candidates,
            self.recent_stale_draw_candidates,
            self.stale_draw_candidates,
            self.replay_input_candidates,
            self.replay_ordered_candidates,
            self.replay_linked_skips,
            self.replay_non_draw_skips,
            self.replay_empty_safe_range_skips,
            self.replay_duplicate_command_skips,
            self.replay_state_words,
            self.replayed_draw_packets,
            self.replayed_draw_words,
            optional_u32_json(self.replayed_min_address),
            optional_u32_hex_json(self.replayed_min_address),
            optional_u32_json(self.replayed_max_address),
            optional_u32_hex_json(self.replayed_max_address),
            command_opcode_counts_json(&self.replayed_opcode_counts),
            self.raw_stream_candidates,
            self.raw_stream_rejected_incomplete,
            self.raw_stream_rejected_unsafe,
            self.raw_stream_replayed_packets,
            self.raw_stream_replayed_words,
            self.reject_no_bounds,
            self.reject_non_playfield_bounds,
            self.reject_unsafe_bounds,
            self.reject_zero_texture_oversize,
            self.reject_command_texture_contamination,
            self.reject_primitive_pointer_contamination,
            self.reject_linked_list_artifact,
            self.reject_title_overlay_atlas
        )
    }

    fn record_replayed_command(&mut self, address: u32, command: u32) {
        self.replayed_draw_words = self.replayed_draw_words.saturating_add(1);
        self.replayed_min_address = Some(
            self.replayed_min_address
                .map_or(address, |current| current.min(address)),
        );
        self.replayed_max_address = Some(
            self.replayed_max_address
                .map_or(address, |current| current.max(address)),
        );
        let opcode = (command >> 24) as usize;
        self.replayed_opcode_counts[opcode] = self.replayed_opcode_counts[opcode].saturating_add(1);
    }

    fn record_reject_reason(&mut self, reason: Gp0ReplayDrawRejectReason) {
        match reason {
            Gp0ReplayDrawRejectReason::NoBounds => {
                self.reject_no_bounds = self.reject_no_bounds.saturating_add(1);
            }
            Gp0ReplayDrawRejectReason::NonPlayfieldBounds => {
                self.reject_non_playfield_bounds =
                    self.reject_non_playfield_bounds.saturating_add(1);
            }
            Gp0ReplayDrawRejectReason::UnsafeBounds => {
                self.reject_unsafe_bounds = self.reject_unsafe_bounds.saturating_add(1);
            }
            Gp0ReplayDrawRejectReason::ZeroTextureOversize => {
                self.reject_zero_texture_oversize =
                    self.reject_zero_texture_oversize.saturating_add(1);
            }
            Gp0ReplayDrawRejectReason::CommandTextureContamination => {
                self.reject_command_texture_contamination =
                    self.reject_command_texture_contamination.saturating_add(1);
            }
            Gp0ReplayDrawRejectReason::PrimitivePointerContamination => {
                self.reject_primitive_pointer_contamination = self
                    .reject_primitive_pointer_contamination
                    .saturating_add(1);
            }
            Gp0ReplayDrawRejectReason::LinkedListArtifact => {
                self.reject_linked_list_artifact =
                    self.reject_linked_list_artifact.saturating_add(1);
            }
            Gp0ReplayDrawRejectReason::TitleOverlayAtlas => {
                self.reject_title_overlay_atlas = self.reject_title_overlay_atlas.saturating_add(1);
            }
        }
    }

    fn saturating_add_state_words(&mut self, words: usize) {
        self.replay_state_words = self
            .replay_state_words
            .saturating_add(words.min(u32::MAX as usize) as u32);
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
    command_write_vblank: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrimitiveReplayCandidate {
    address: u32,
    vblank: u64,
    priority: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrimitiveReplayStateCandidate {
    address: u32,
    vblank: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrimitiveReplayCandidateCacheKey {
    primitive_header_generation: u64,
    gpu_linked_list_generation: u64,
}

#[derive(Clone, Debug, Default)]
struct PrimitiveReplayCandidateCache {
    key: Option<PrimitiveReplayCandidateCacheKey>,
    candidates: HashMap<u32, PrimitiveReplayCandidate>,
}

fn sort_primitive_replay_candidates(candidates: &mut [PrimitiveReplayCandidate]) {
    candidates.sort_unstable_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| right.vblank.cmp(&left.vblank))
            .then_with(|| right.address.cmp(&left.address))
    });
}

#[derive(Clone, Debug)]
struct GpuLinkedListDmaRunSummary {
    call: u64,
    start: u32,
    first_node: u32,
    pc: Option<u32>,
    vblank: u64,
    cycles: u64,
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
            pc: run.pc,
            vblank: run.vblank,
            cycles: run.cycles,
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
            "{{\"call\":{},\"start\":{},\"start_hex\":\"0x{:08x}\",\"first_node\":{},\"first_node_hex\":\"0x{:08x}\",\"pc\":{},\"pc_hex\":{},\"vblank\":{},\"cycles\":{},\"nodes\":{},\"words\":{},\"nonempty_nodes\":{},\"max_node_words\":{},\"min_command_address\":{},\"min_command_address_hex\":{},\"max_command_address\":{},\"max_command_address_hex\":{},\"command_opcode_counts\":[{}],\"terminated\":{},\"hit_node_limit\":{}}}",
            self.call,
            self.start,
            self.start,
            self.first_node,
            self.first_node,
            optional_u32_json(self.pc),
            optional_u32_hex_json(self.pc),
            self.vblank,
            self.cycles,
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
            "{{\"address\":{},\"address_hex\":\"0x{:08x}\",\"header\":{},\"header_hex\":\"0x{:08x}\",\"word_count\":{},\"next\":{},\"next_hex\":\"0x{:06x}\",\"linked\":{},\"first_command\":{},\"first_command_hex\":\"0x{:08x}\",\"opcode\":{},\"opcode_hex\":\"0x{:02x}\",\"header_write_vblank\":{},\"command_write_vblank\":{}}}",
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
                .map_or_else(|| "null".to_string(), |value| value.to_string()),
            self.command_write_vblank
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
            command_write_vblank_by_address: HashMap::new(),
            last_address: None,
            last_value: 0,
            last_pc: None,
            recent_command_like_writes: Vec::new(),
            recent_header_like_writes: Vec::new(),
        }
    }
}

impl PrimitiveRamWriteStats {
    fn record(
        &mut self,
        address: u32,
        value: u32,
        pc: Option<u32>,
        vblank: u64,
        cycles: u64,
    ) -> PrimitiveRamWriteRecord {
        self.writes = self.writes.saturating_add(1);
        self.current_vblank_writes = self.current_vblank_writes.saturating_add(1);
        self.last_address = Some(address);
        self.last_value = value;
        self.last_pc = pc;
        let mut record = PrimitiveRamWriteRecord::default();

        let packet_words = value >> 24;
        let packet_next = value & 0x00ff_ffff;
        if (1..=PRIMITIVE_PACKET_MAX_WORDS).contains(&packet_words)
            && primitive_packet_next_plausible(packet_next)
        {
            record.header_like = true;
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
            return record;
        }

        record.command_like = true;
        let opcode_index = opcode as usize;
        self.command_like_writes = self.command_like_writes.saturating_add(1);
        self.current_vblank_command_like_writes =
            self.current_vblank_command_like_writes.saturating_add(1);
        self.opcode_counts[opcode_index] = self.opcode_counts[opcode_index].saturating_add(1);
        self.current_vblank_opcode_counts[opcode_index] =
            self.current_vblank_opcode_counts[opcode_index].saturating_add(1);
        self.command_write_vblank_by_address.insert(address, vblank);

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
        record
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

    fn command_write_vblank(&self, address: u32) -> Option<u64> {
        self.command_write_vblank_by_address.get(&address).copied()
    }

    fn tracked_header_addresses_written_since(&self, min_vblank: u64) -> Vec<(u64, u32)> {
        let mut headers = self
            .header_write_vblank_by_address
            .iter()
            .filter_map(|(address, vblank)| (*vblank >= min_vblank).then_some((*vblank, *address)))
            .collect::<Vec<_>>();
        headers.sort_unstable_by(|left, right| {
            right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1))
        });
        headers
    }

    fn json(&self) -> String {
        format!(
            "{{\"range_start\":\"0x{:08x}\",\"range_end\":\"0x{:08x}\",\"writes\":{},\"command_like_writes\":{},\"header_like_writes\":{},\"tracked_header_addresses\":{},\"tracked_command_addresses\":{},\"current_vblank_writes\":{},\"current_vblank_command_like_writes\":{},\"current_vblank_header_like_writes\":{},\"last_vblank_writes\":{},\"last_vblank_command_like_writes\":{},\"last_vblank_header_like_writes\":{},\"opcode_counts\":[{}],\"current_vblank_opcode_counts\":[{}],\"last_vblank_opcode_counts\":[{}],\"last_address\":{},\"last_address_hex\":{},\"last_value\":{},\"last_value_hex\":\"0x{:08x}\",\"last_pc\":{},\"last_pc_hex\":{},\"recent_command_like_writes\":[{}],\"recent_header_like_writes\":[{}]}}",
            BR2_PRIMITIVE_RAM_START,
            BR2_PRIMITIVE_RAM_END,
            self.writes,
            self.command_like_writes,
            self.header_like_writes,
            self.header_write_vblank_by_address.len(),
            self.command_write_vblank_by_address.len(),
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
            last_pc: None,
            last_vblank: 0,
            last_cycles: 0,
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
            embedded_payload_skips: 0,
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
        self.last_pc = last.pc;
        self.last_vblank = last.vblank;
        self.last_cycles = last.cycles;
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
            "{{\"calls\":{},\"last_start\":{},\"last_start_hex\":\"0x{:08x}\",\"last_first_node\":{},\"last_first_node_hex\":\"0x{:08x}\",\"last_pc\":{},\"last_pc_hex\":{},\"last_vblank\":{},\"last_cycles\":{},\"last_nodes\":{},\"last_words\":{},\"last_nonempty_nodes\":{},\"last_max_node_words\":{},\"last_min_command_address\":{},\"last_min_command_address_hex\":{},\"last_max_command_address\":{},\"last_max_command_address_hex\":{},\"last_command_opcode_counts\":[{}],\"last_recent_commands\":[{}],\"last_first_node_samples\":[{}],\"last_tail_node_samples\":[{}],\"last_nonempty_node_samples\":[{}],\"recent_runs\":[{}],\"last_terminated\":{},\"last_hit_node_limit\":{},\"node_limit\":{},\"max_nodes\":{},\"max_words\":{},\"max_nonempty_nodes\":{},\"max_node_words\":{},\"node_limit_hits\":{},\"embedded_payload_skips\":{}}}",
            self.calls,
            self.last_start,
            self.last_start,
            self.last_first_node,
            self.last_first_node,
            optional_u32_json(self.last_pc),
            optional_u32_hex_json(self.last_pc),
            self.last_vblank,
            self.last_cycles,
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
            self.node_limit_hits,
            self.embedded_payload_skips
        )
    }
}

#[derive(Clone, Debug)]
struct GpuLinkedListDmaRunStats {
    last_start: u32,
    last_first_node: u32,
    pc: Option<u32>,
    vblank: u64,
    cycles: u64,
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
            pc: None,
            vblank: 0,
            cycles: 0,
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

    fn record_context(&mut self, pc: Option<u32>, vblank: u64, cycles: u64) {
        self.pc = pc;
        self.vblank = vblank;
        self.cycles = cycles;
    }
}

#[derive(Clone, Debug)]
pub struct Bus {
    ram: Vec<u8>,
    scratchpad: Vec<u8>,
    rom: Vec<u8>,
    banked_roms: Vec<u8>,
    br2_runtime_code_snapshot: Vec<u8>,
    br2_runtime_code_snapshot_valid: Vec<bool>,
    br2_code_patch_snapshot: [u8; BR2_CODE_PATCH_SNAPSHOT_LEN],
    br2_code_patch_snapshot_valid: [bool; BR2_CODE_PATCH_SNAPSHOT_LEN],
    br2_code_patch_snapshot_frozen: bool,
    zn_board: ZnBoard,
    br2_native_credit_hle_consumed_coin_edges: u64,
    br2_native_credit_hle: Br2NativeCreditHleStats,
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
    vblank_presentation_capture_interval: Option<u64>,
    vblank_draw_sync_clears: u64,
    draw_sync_game_set_writes: u64,
    draw_sync_game_clear_writes: u64,
    draw_sync_game_other_writes: u64,
    draw_sync_last_game_write_value: Option<u32>,
    draw_sync_last_game_write_pc: Option<u32>,
    gpu_linked_list_dma: GpuLinkedListDmaStats,
    primitive_ram_writes: PrimitiveRamWriteStats,
    unlinked_primitive_replay: UnlinkedPrimitiveReplayStats,
    primitive_header_generation: u64,
    gpu_linked_list_generation: u64,
    stale_unlinked_primitive_replay_candidates: PrimitiveReplayCandidateCache,
    unlinked_primitive_replay_interval: Option<u64>,
    dma_activity: Vec<DmaActivitySample>,
    dma_lifetime_activity: Vec<DmaChannelLifetimeStats>,
    banked_rom_reads: RefCell<BankedRomReadStats>,
    recent_zn_input_reads: RefCell<Vec<ZnBoardInputReadEvent>>,
    recent_active_zn_input_reads: RefCell<Vec<ZnBoardInputReadEvent>>,
    zn_input_read_stats: RefCell<ZnBoardInputReadStats>,
    board_asset_status: NativeBoardAssetStatus,
    pub io: Io,
    access_trace_limit: usize,
    access_trace_watch_only: bool,
    access_trace_watch_ranges: Vec<BusTraceWatchRange>,
    access_trace_watch_access_hit: Cell<bool>,
    access_trace_watch_data_hit: Cell<bool>,
    access_trace_watch_write_hit: Cell<bool>,
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
            br2_runtime_code_snapshot: vec![0; BR2_RUNTIME_CODE_SNAPSHOT_LEN],
            br2_runtime_code_snapshot_valid: vec![false; BR2_RUNTIME_CODE_SNAPSHOT_LEN],
            br2_code_patch_snapshot: [0; BR2_CODE_PATCH_SNAPSHOT_LEN],
            br2_code_patch_snapshot_valid: [false; BR2_CODE_PATCH_SNAPSHOT_LEN],
            br2_code_patch_snapshot_frozen: false,
            zn_board: ZnBoard::with_board_assets(&board_assets),
            br2_native_credit_hle_consumed_coin_edges: 0,
            br2_native_credit_hle: Br2NativeCreditHleStats::default(),
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
            vblank_presentation_capture_interval: Some(GPU_VBLANK_PRESENTATION_CAPTURE_INTERVAL),
            vblank_draw_sync_clears: 0,
            draw_sync_game_set_writes: 0,
            draw_sync_game_clear_writes: 0,
            draw_sync_game_other_writes: 0,
            draw_sync_last_game_write_value: None,
            draw_sync_last_game_write_pc: None,
            gpu_linked_list_dma: GpuLinkedListDmaStats::default(),
            primitive_ram_writes: PrimitiveRamWriteStats::default(),
            unlinked_primitive_replay: UnlinkedPrimitiveReplayStats::default(),
            primitive_header_generation: 0,
            gpu_linked_list_generation: 0,
            stale_unlinked_primitive_replay_candidates: PrimitiveReplayCandidateCache::default(),
            unlinked_primitive_replay_interval: None,
            dma_activity: Vec::new(),
            dma_lifetime_activity: vec![DmaChannelLifetimeStats::default(); DMA_CHANNEL_COUNT],
            banked_rom_reads: RefCell::new(BankedRomReadStats::default()),
            recent_zn_input_reads: RefCell::new(Vec::new()),
            recent_active_zn_input_reads: RefCell::new(Vec::new()),
            zn_input_read_stats: RefCell::new(ZnBoardInputReadStats::default()),
            board_asset_status,
            io: Io::default(),
            access_trace_limit: 0,
            access_trace_watch_only: false,
            access_trace_watch_ranges: Vec::new(),
            access_trace_watch_access_hit: Cell::new(false),
            access_trace_watch_data_hit: Cell::new(false),
            access_trace_watch_write_hit: Cell::new(false),
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
        if board_asset_status.cat702_1_loaded || board_asset_status.cat702_2_loaded {
            bus.sync_security_selects();
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
            self.record_zn_board_input_read(address, 1, value as u32);
            self.record_access_trace("read", "zn_board", address, 1, value as u32);
            return value;
        }

        if let Some(io_address) = mapped_io_address(address, 1) {
            let value = self.io.read_u8(io_address);
            self.record_access_trace("read", "io", address, 1, value as u32);
            return value;
        }

        if let Some(offset) = ram_offset(address, self.ram.len(), 1) {
            let value = self
                .br2_runtime_code_snapshot_read_value(address, 1)
                .or_else(|| self.br2_code_patch_snapshot_read_value(address, 1))
                .unwrap_or_else(|| {
                    self.br2_boot_global_snapshot_fallback_value(
                        address,
                        1,
                        self.ram[offset] as u32,
                    )
                });
            self.record_watch_trace("read", "ram", address, 1, value);
            return value as u8;
        }

        if let Some(offset) = scratchpad_offset(address, self.scratchpad.len(), 1) {
            let value = self.scratchpad[offset] as u32;
            self.record_watch_trace("read", "scratchpad", address, 1, value);
            return value as u8;
        }

        if let Some(offset) = rom_offset(address, self.rom.len(), 1) {
            return self.rom[offset];
        }

        if let Some(offset) =
            banked_rom_offset(address, self.banked_roms.len(), 1, self.zn_board.rom_bank)
        {
            let value = self.banked_roms[offset] as u32;
            self.banked_rom_reads.borrow_mut().record(
                self.zn_board.rom_bank,
                address,
                offset,
                1,
                value,
            );
            self.record_watch_trace("read", "banked_rom", address, 1, value);
            return value as u8;
        }

        self.record_access_trace("read", "unmapped", address, 1, 0);
        0
    }

    pub fn read_u16(&self, address: u32) -> u16 {
        if cache_control_address(address) {
            let value = self.cache_control as u16;
            self.record_access_trace("read", "cache_control", address, 2, value as u32);
            return value;
        }

        if mapped_zn_board_address(address).is_some() {
            let value = self.zn_board.read(address, 2) as u16;
            self.record_zn_board_input_read(address, 2, value as u32);
            self.record_access_trace("read", "zn_board", address, 2, value as u32);
            return value;
        }

        if let Some(io_address) = mapped_io_address(address, 2) {
            let value = self.io.read_u16(io_address);
            self.record_access_trace("read", "io", address, 2, value as u32);
            return value;
        }

        if let Some(offset) = ram_offset(address, self.ram.len(), 2) {
            let value = self
                .br2_runtime_code_snapshot_read_value(address, 2)
                .or_else(|| self.br2_code_patch_snapshot_read_value(address, 2))
                .unwrap_or_else(|| {
                    let ram_value =
                        PreferredNativePlatform::read_le_u16(&self.ram[offset..offset + 2]) as u32;
                    self.br2_boot_global_snapshot_fallback_value(address, 2, ram_value)
                });
            self.record_watch_trace("read", "ram", address, 2, value);
            return value as u16;
        }

        if let Some(offset) = scratchpad_offset(address, self.scratchpad.len(), 2) {
            let value = PreferredNativePlatform::read_le_u16(&self.scratchpad[offset..offset + 2]);
            self.record_watch_trace("read", "scratchpad", address, 2, value as u32);
            return value;
        }

        if let Some(offset) = rom_offset(address, self.rom.len(), 2) {
            return PreferredNativePlatform::read_le_u16(&self.rom[offset..offset + 2]);
        }

        if let Some(offset) =
            banked_rom_offset(address, self.banked_roms.len(), 2, self.zn_board.rom_bank)
        {
            let value =
                PreferredNativePlatform::read_le_u16(&self.banked_roms[offset..offset + 2]) as u32;
            self.banked_rom_reads.borrow_mut().record(
                self.zn_board.rom_bank,
                address,
                offset,
                2,
                value,
            );
            self.record_watch_trace("read", "banked_rom", address, 2, value);
            return value as u16;
        }

        self.record_access_trace("read", "unmapped", address, 2, 0);
        0
    }

    pub fn read_u32(&self, address: u32) -> u32 {
        if cache_control_address(address) {
            let value = self.cache_control;
            self.record_access_trace("read", "cache_control", address, 4, value);
            return value;
        }

        if mapped_zn_board_address(address).is_some() {
            let value = self.zn_board.read(address, 4);
            self.record_zn_board_input_read(address, 4, value);
            self.record_access_trace("read", "zn_board", address, 4, value);
            return value;
        }

        if let Some(io_address) = mapped_io_address(address, 4) {
            let value = self.io.read_u32(io_address);
            self.record_access_trace("read", "io", address, 4, value);
            return value;
        }

        if let Some(offset) = ram_offset(address, self.ram.len(), 4) {
            let value = self
                .br2_runtime_code_snapshot_read_value(address, 4)
                .or_else(|| self.br2_code_patch_snapshot_read_value(address, 4))
                .unwrap_or_else(|| {
                    let ram_value =
                        PreferredNativePlatform::read_le_u32(&self.ram[offset..offset + 4]);
                    self.br2_boot_global_snapshot_fallback_value(address, 4, ram_value)
                });
            self.record_watch_trace("read", "ram", address, 4, value);
            return value;
        }

        if let Some(offset) = scratchpad_offset(address, self.scratchpad.len(), 4) {
            let value = PreferredNativePlatform::read_le_u32(&self.scratchpad[offset..offset + 4]);
            self.record_watch_trace("read", "scratchpad", address, 4, value);
            return value;
        }

        if let Some(offset) = rom_offset(address, self.rom.len(), 4) {
            return PreferredNativePlatform::read_le_u32(&self.rom[offset..offset + 4]);
        }

        if let Some(offset) =
            banked_rom_offset(address, self.banked_roms.len(), 4, self.zn_board.rom_bank)
        {
            let value = PreferredNativePlatform::read_le_u32(&self.banked_roms[offset..offset + 4]);
            self.banked_rom_reads.borrow_mut().record(
                self.zn_board.rom_bank,
                address,
                offset,
                4,
                value,
            );
            self.record_watch_trace("read", "banked_rom", address, 4, value);
            return value;
        }

        self.record_access_trace("read", "unmapped", address, 4, 0);
        0
    }

    pub fn read_u32_fast_no_trace(&self, address: u32) -> u32 {
        if let Some(offset) = ram_offset(address, self.ram.len(), 4) {
            let physical = physical_address(address);
            if self.trace_pc.get().map(physical_address) == Some(physical)
                && let Some(value) = self.br2_runtime_code_snapshot_read_value(address, 4)
            {
                return value;
            }
            if self.br2_code_patch_snapshot_frozen
                && physical >= BR2_CODE_PATCH_SNAPSHOT_START
                && physical
                    .checked_add(4)
                    .is_some_and(|end| end <= BR2_CODE_PATCH_SNAPSHOT_END)
                && let Some(value) = self.br2_code_patch_snapshot_read_value(address, 4)
            {
                return value;
            }
            return PreferredNativePlatform::read_le_u32(&self.ram[offset..offset + 4]);
        }

        if let Some(offset) = scratchpad_offset(address, self.scratchpad.len(), 4) {
            return PreferredNativePlatform::read_le_u32(&self.scratchpad[offset..offset + 4]);
        }

        if let Some(offset) = rom_offset(address, self.rom.len(), 4) {
            return PreferredNativePlatform::read_le_u32(&self.rom[offset..offset + 4]);
        }

        if let Some(offset) =
            banked_rom_offset(address, self.banked_roms.len(), 4, self.zn_board.rom_bank)
        {
            return PreferredNativePlatform::read_le_u32(&self.banked_roms[offset..offset + 4]);
        }

        self.read_u32(address)
    }

    pub fn read_u32_executable_no_trace(&self, address: u32) -> u32 {
        if let Some(offset) = ram_offset(address, self.ram.len(), 4) {
            if let Some(value) = self.br2_runtime_code_snapshot_read_value_unchecked(address, 4) {
                return value;
            }
            if let Some(value) = self.br2_code_patch_snapshot_read_value(address, 4) {
                return value;
            }
            return PreferredNativePlatform::read_le_u32(&self.ram[offset..offset + 4]);
        }

        if let Some(offset) = scratchpad_offset(address, self.scratchpad.len(), 4) {
            return PreferredNativePlatform::read_le_u32(&self.scratchpad[offset..offset + 4]);
        }

        if let Some(offset) = rom_offset(address, self.rom.len(), 4) {
            return PreferredNativePlatform::read_le_u32(&self.rom[offset..offset + 4]);
        }

        if let Some(offset) =
            banked_rom_offset(address, self.banked_roms.len(), 4, self.zn_board.rom_bank)
        {
            return PreferredNativePlatform::read_le_u32(&self.banked_roms[offset..offset + 4]);
        }

        self.read_u32_fast_no_trace(address)
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
            self.zn_board.write(
                address,
                value as u32,
                1,
                self.trace_pc.get(),
                self.vblank_count,
                self.trace_cycles.get(),
            );
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
            self.zn_board.write(
                address,
                value as u32,
                2,
                self.trace_pc.get(),
                self.vblank_count,
                self.trace_cycles.get(),
            );
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
            self.zn_board.write(
                address,
                value,
                4,
                self.trace_pc.get(),
                self.vblank_count,
                self.trace_cycles.get(),
            );
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
            if io_address == GPU_GP1 {
                self.io.gpu.write_gp1_with_source(
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
        if byte_count == 0
            || self.cache_isolated()
                && cacheable_address(destination)
                && !cache_isolated_write_suppression_disabled()
        {
            return None;
        }
        let byte_len = byte_count as usize;
        if !self.word_copy_readable_range(source, byte_len)
            || !self.word_copy_writable_range(destination, byte_len)
        {
            return None;
        }

        let mut bytes = Vec::with_capacity(byte_len);
        for offset in 0..byte_count {
            let byte = self.read_u8(source.wrapping_add(offset));
            self.write_u8(destination.wrapping_add(offset), byte);
            bytes.push(byte);
        }
        Some(bytes)
    }

    pub fn try_copy_halfwords(
        &mut self,
        source: u32,
        destination: u32,
        halfword_count: u32,
    ) -> Option<u16> {
        if halfword_count == 0
            || self.cache_isolated()
                && cacheable_address(destination)
                && !cache_isolated_write_suppression_disabled()
        {
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

    pub fn ram_snapshot(&self) -> Vec<u8> {
        self.ram.clone()
    }

    pub fn scratchpad_snapshot(&self) -> Vec<u8> {
        self.scratchpad.clone()
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
            self.io.gpu.advance_vblank_field();
            self.process_vblank_unlinked_primitive_replay();
            if self.should_capture_vblank_presented_frame() {
                self.io.gpu.capture_vblank_presented_frame();
            }
            self.primitive_ram_writes.advance_vblank();
            self.io.irq.status |= 1;
            self.complete_draw_sync_on_vblank();
        }
    }

    pub fn vblank_count(&self) -> u64 {
        self.vblank_count
    }

    pub fn vblank_presentation_capture_interval(&self) -> Option<u64> {
        self.vblank_presentation_capture_interval
    }

    pub fn set_vblank_presentation_capture_interval(&mut self, interval: Option<u64>) {
        self.vblank_presentation_capture_interval = interval.filter(|value| *value > 0);
    }

    pub fn capture_vblank_presented_frame(&mut self) {
        self.io.gpu.capture_vblank_presented_frame();
    }

    pub fn unlinked_primitive_replay_interval(&self) -> Option<u64> {
        self.unlinked_primitive_replay_interval
    }

    pub fn set_unlinked_primitive_replay_interval(&mut self, interval: Option<u64>) {
        self.unlinked_primitive_replay_interval = interval.filter(|value| *value > 0);
    }

    fn effective_unlinked_primitive_replay_interval(&self) -> Option<u64> {
        native_unlinked_primitive_replay_interval_override()
            .unwrap_or(self.unlinked_primitive_replay_interval)
    }

    fn should_capture_vblank_presented_frame(&self) -> bool {
        self.vblank_count == 1
            || self
                .vblank_presentation_capture_interval
                .is_some_and(|interval| self.vblank_count.is_multiple_of(interval))
    }

    fn should_attempt_unlinked_primitive_replay(&self) -> bool {
        if std::env::var_os("BR2_NATIVE_ENABLE_UNLINKED_PRIMITIVE_REPLAY").is_some() {
            return true;
        }
        self.effective_unlinked_primitive_replay_interval()
            .is_none_or(|interval| interval <= 1 || self.vblank_count.is_multiple_of(interval))
    }

    fn should_attempt_sparse_scene_unlinked_primitive_replay(
        &self,
        stats: &GpuLinkedListDmaRunStats,
    ) -> bool {
        let Some(interval) = self.effective_unlinked_primitive_replay_interval() else {
            return false;
        };
        if std::env::var_os("BR2_NATIVE_DISABLE_UNLINKED_PRIMITIVE_REPLAY").is_some()
            || std::env::var_os("BR2_NATIVE_DISABLE_STALE_UNLINKED_PRIMITIVE_REPLAY").is_some()
            || stats.last_nonempty_nodes > BR2_UNLINKED_PRIMITIVE_REPLAY_SPARSE_NODE_LIMIT
            || !self.io.native_sparse_scene_replay_gate()
        {
            return false;
        }

        let sparse_interval = interval
            .min(BR2_UNLINKED_PRIMITIVE_REPLAY_SPARSE_SCENE_SCAN_INTERVAL)
            .max(1);
        sparse_interval <= 1 || self.vblank_count.is_multiple_of(sparse_interval)
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
        let gpu_dma_stale_vblanks = (self.gpu_linked_list_dma.calls > 0).then_some(
            self.vblank_count
                .saturating_sub(self.gpu_linked_list_dma.last_vblank),
        );
        let last_gpu_dma_register_write = self.last_dma_register_write_json(DMA_GPU_CHANNEL);
        let recent_gpu_dma_register_writes = self
            .recent_dma_register_writes_json(DMA_GPU_CHANNEL, DMA_GPU_RECENT_REGISTER_WRITE_LIMIT);
        let recent_dma_activity_counts = self.recent_dma_activity_counts_json();
        let dma_lifetime_activity = self.dma_lifetime_activity_json();
        format!(
            "{{\"br2_draw_sync_flag\":{},\"vblank_count\":{},\"vblank_cycle_accumulator\":{},\"vblank_draw_sync_clears\":{},\"game_set_writes\":{},\"game_clear_writes\":{},\"game_other_writes\":{},\"last_game_write_value\":{},\"last_game_write_pc\":{},\"cache\":{},\"banked_rom_reads\":{},\"gpu_dma_stale_vblanks\":{},\"last_gpu_dma_register_write\":{},\"recent_gpu_dma_register_writes\":[{}],\"recent_dma_activity_counts\":[{}],\"dma_lifetime_activity\":[{}],\"dma_activity\":[{}],\"recent_otc_ranges\":[{}],\"gpu_linked_list_dma\":{},\"primitive_ram_writes\":{},\"recent_primitive_header_relations\":[{}],\"unlinked_primitive_replay\":{},\"primitive_packet_scan\":{}}}",
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
            optional_u64_json(gpu_dma_stale_vblanks),
            last_gpu_dma_register_write,
            recent_gpu_dma_register_writes,
            recent_dma_activity_counts,
            dma_lifetime_activity,
            self.dma_activity_json(),
            self.recent_otc_clear_ranges_json(DMA_OTC_RECENT_RANGE_LIMIT),
            self.gpu_linked_list_dma.json(),
            self.primitive_ram_writes.json(),
            self.recent_primitive_header_relations_json(PRIMITIVE_RECENT_HEADER_RELATION_LIMIT),
            self.unlinked_primitive_replay.json(),
            self.primitive_packet_scan_json()
        )
    }

    pub fn native_sync_compact_json(&self) -> String {
        let recent_commands = self
            .gpu_linked_list_dma
            .last_recent_commands
            .iter()
            .map(GpuLinkedListCommandSample::json)
            .collect::<Vec<_>>()
            .join(",");
        let recent_runs = self
            .gpu_linked_list_dma
            .recent_runs
            .iter()
            .map(GpuLinkedListDmaRunSummary::json)
            .collect::<Vec<_>>()
            .join(",");
        let first_node_samples =
            gpu_linked_list_node_samples_json(&self.gpu_linked_list_dma.last_first_node_samples);
        let tail_node_samples =
            gpu_linked_list_node_samples_json(&self.gpu_linked_list_dma.last_tail_node_samples);
        let nonempty_node_samples =
            gpu_linked_list_node_samples_json(&self.gpu_linked_list_dma.last_nonempty_node_samples);
        let gpu_dma_stale_vblanks = (self.gpu_linked_list_dma.calls > 0).then_some(
            self.vblank_count
                .saturating_sub(self.gpu_linked_list_dma.last_vblank),
        );
        let last_gpu_dma_register_write = self.last_dma_register_write_json(DMA_GPU_CHANNEL);
        let recent_gpu_dma_register_writes = self
            .recent_dma_register_writes_json(DMA_GPU_CHANNEL, DMA_GPU_RECENT_REGISTER_WRITE_LIMIT);
        let recent_dma_activity_counts = self.recent_dma_activity_counts_json();
        let dma_lifetime_activity = self.dma_lifetime_activity_json();
        format!(
            "{{\"br2_draw_sync_flag\":{},\"vblank_count\":{},\"vblank_cycle_accumulator\":{},\"vblank_draw_sync_clears\":{},\"game_set_writes\":{},\"game_clear_writes\":{},\"game_other_writes\":{},\"last_game_write_value\":{},\"last_game_write_pc\":{},\"cache_isolated\":{},\"cache_isolation_transitions\":{},\"dma_irq_pending\":{},\"gpu_dma_channel\":{},\"gpu_dma_stale_vblanks\":{},\"last_gpu_dma_register_write\":{},\"recent_gpu_dma_register_writes\":[{}],\"recent_dma_activity_counts\":[{}],\"dma_lifetime_activity\":[{}],\"pending_dma_completion_cycles\":[{}],\"banked_rom_reads\":{},\"recent_dma_activity\":[{}],\"recent_otc_ranges\":[{}],\"gpu_linked_list_dma\":{{\"calls\":{},\"last_start_hex\":\"0x{:08x}\",\"last_first_node_hex\":\"0x{:08x}\",\"last_pc\":{},\"last_pc_hex\":{},\"last_vblank\":{},\"last_cycles\":{},\"last_nodes\":{},\"last_words\":{},\"last_nonempty_nodes\":{},\"last_max_node_words\":{},\"last_terminated\":{},\"last_hit_node_limit\":{},\"node_limit_hits\":{},\"max_nodes\":{},\"max_words\":{},\"max_nonempty_nodes\":{},\"max_node_words\":{},\"last_recent_commands\":[{}],\"last_first_node_samples\":[{}],\"last_tail_node_samples\":[{}],\"last_nonempty_node_samples\":[{}],\"recent_runs\":[{}]}},\"primitive_ram_writes\":{{\"writes\":{},\"command_like_writes\":{},\"header_like_writes\":{},\"current_vblank_header_like_writes\":{},\"last_vblank_header_like_writes\":{}}},\"recent_primitive_header_relations\":[{}],\"unlinked_primitive_replay\":{{\"attempts\":{},\"conditional_replays\":{},\"forced_replays\":{},\"skipped\":{},\"last_reason\":\"{}\",\"last_packets\":{},\"last_words\":{},\"last_diagnostics\":{},\"top_candidates\":[{}]}},\"primitive_packet_scan\":{}}}",
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
            self.dma_channel_compact_json(DMA_GPU_CHANNEL),
            optional_u64_json(gpu_dma_stale_vblanks),
            last_gpu_dma_register_write,
            recent_gpu_dma_register_writes,
            recent_dma_activity_counts,
            dma_lifetime_activity,
            self.pending_dma_completion_cycles
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(","),
            self.banked_rom_reads.borrow().json(),
            self.dma_activity_json(),
            self.recent_otc_clear_ranges_json(DMA_OTC_RECENT_RANGE_LIMIT),
            self.gpu_linked_list_dma.calls,
            self.gpu_linked_list_dma.last_start,
            self.gpu_linked_list_dma.last_first_node,
            optional_u32_json(self.gpu_linked_list_dma.last_pc),
            optional_u32_hex_json(self.gpu_linked_list_dma.last_pc),
            self.gpu_linked_list_dma.last_vblank,
            self.gpu_linked_list_dma.last_cycles,
            self.gpu_linked_list_dma.last_nodes,
            self.gpu_linked_list_dma.last_words,
            self.gpu_linked_list_dma.last_nonempty_nodes,
            self.gpu_linked_list_dma.last_max_node_words,
            self.gpu_linked_list_dma.last_terminated,
            self.gpu_linked_list_dma.last_hit_node_limit,
            self.gpu_linked_list_dma.node_limit_hits,
            self.gpu_linked_list_dma.max_nodes,
            self.gpu_linked_list_dma.max_words,
            self.gpu_linked_list_dma.max_nonempty_nodes,
            self.gpu_linked_list_dma.max_node_words,
            recent_commands,
            first_node_samples,
            tail_node_samples,
            nonempty_node_samples,
            recent_runs,
            self.primitive_ram_writes.writes,
            self.primitive_ram_writes.command_like_writes,
            self.primitive_ram_writes.header_like_writes,
            self.primitive_ram_writes.current_vblank_header_like_writes,
            self.primitive_ram_writes.last_vblank_header_like_writes,
            self.recent_primitive_header_relations_json(8),
            self.unlinked_primitive_replay.attempts,
            self.unlinked_primitive_replay.conditional_replays,
            self.unlinked_primitive_replay.forced_replays,
            self.unlinked_primitive_replay.skipped,
            self.unlinked_primitive_replay.last_reason,
            self.unlinked_primitive_replay.last_packets,
            self.unlinked_primitive_replay.last_words,
            self.unlinked_primitive_replay.last_diagnostics.json(),
            self.unlinked_primitive_replay_candidate_diagnostics_json(16),
            self.primitive_packet_scan_compact_json()
        )
    }

    fn dma_activity_json(&self) -> String {
        self.dma_activity
            .iter()
            .map(DmaActivitySample::json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn recent_dma_register_writes_json(&self, channel: usize, limit: usize) -> String {
        if limit == 0 {
            return String::new();
        }
        let mut samples = self
            .dma_activity
            .iter()
            .rev()
            .filter(|sample| sample.channel == channel && sample.kind == "register_write")
            .take(limit)
            .collect::<Vec<_>>();
        samples.reverse();
        samples
            .into_iter()
            .map(DmaActivitySample::json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn last_dma_register_write_json(&self, channel: usize) -> String {
        self.dma_activity
            .iter()
            .rev()
            .find(|sample| sample.channel == channel && sample.kind == "register_write")
            .map(DmaActivitySample::json)
            .unwrap_or_else(|| "null".to_string())
    }

    fn recent_dma_activity_counts_json(&self) -> String {
        let mut channels = self
            .dma_activity
            .iter()
            .map(|sample| sample.channel)
            .collect::<Vec<_>>();
        channels.sort_unstable();
        channels.dedup();

        channels
            .into_iter()
            .map(|channel| {
                let mut samples = 0u64;
                let mut register_writes = 0u64;
                let mut transfers = 0u64;
                let mut gpu_linked_list = 0u64;
                let mut gpu_block_write = 0u64;
                let mut gpu_read = 0u64;
                let mut otc_clear = 0u64;
                let mut last_vblank = None;
                let mut last_pc = None;

                for sample in self
                    .dma_activity
                    .iter()
                    .filter(|sample| sample.channel == channel)
                {
                    samples = samples.saturating_add(1);
                    last_vblank = Some(sample.vblank);
                    last_pc = sample.pc;
                    match sample.kind {
                        "register_write" => register_writes = register_writes.saturating_add(1),
                        "gpu_linked_list" => {
                            transfers = transfers.saturating_add(1);
                            gpu_linked_list = gpu_linked_list.saturating_add(1);
                        }
                        "gpu_block_write" => {
                            transfers = transfers.saturating_add(1);
                            gpu_block_write = gpu_block_write.saturating_add(1);
                        }
                        "gpu_read" => {
                            transfers = transfers.saturating_add(1);
                            gpu_read = gpu_read.saturating_add(1);
                        }
                        "otc_clear" => {
                            transfers = transfers.saturating_add(1);
                            otc_clear = otc_clear.saturating_add(1);
                        }
                        _ => transfers = transfers.saturating_add(1),
                    }
                }

                format!(
                    "{{\"channel\":{},\"samples\":{},\"register_writes\":{},\"transfers\":{},\"gpu_linked_list\":{},\"gpu_block_write\":{},\"gpu_read\":{},\"otc_clear\":{},\"last_vblank\":{},\"last_pc\":{},\"last_pc_hex\":{}}}",
                    channel,
                    samples,
                    register_writes,
                    transfers,
                    gpu_linked_list,
                    gpu_block_write,
                    gpu_read,
                    otc_clear,
                    optional_u64_json(last_vblank),
                    optional_u32_json(last_pc),
                    optional_u32_hex_json(last_pc)
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    fn dma_lifetime_activity_json(&self) -> String {
        self.dma_lifetime_activity
            .iter()
            .enumerate()
            .filter(|(_, stats)| stats.samples > 0)
            .map(|(channel, stats)| stats.json(channel))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn recent_otc_clear_ranges_json(&self, limit: usize) -> String {
        let mut samples = self
            .dma_activity
            .iter()
            .rev()
            .filter(|sample| sample.kind == "otc_clear")
            .take(limit)
            .collect::<Vec<_>>();
        samples.reverse();
        samples
            .into_iter()
            .filter_map(|sample| {
                let (low, high) = dma_activity_range_bounds(sample)?;
                Some(format!(
                    "{{\"vblank\":{},\"cycles\":{},\"start\":{},\"start_hex\":\"0x{:08x}\",\"end\":{},\"end_hex\":\"0x{:08x}\",\"low\":{},\"low_hex\":\"0x{:08x}\",\"high\":{},\"high_hex\":\"0x{:08x}\",\"words\":{},\"pc\":{},\"pc_hex\":{}}}",
                    sample.vblank,
                    sample.cycles,
                    sample.start.unwrap_or_default(),
                    sample.start.unwrap_or_default(),
                    sample.end.unwrap_or_default(),
                    sample.end.unwrap_or_default(),
                    low,
                    low,
                    high,
                    high,
                    sample.words,
                    optional_u32_json(sample.pc),
                    optional_u32_hex_json(sample.pc)
                ))
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    fn primitive_address_dma_relation_json(&self, address: u32) -> String {
        let gpu_start = self.gpu_linked_list_dma.last_start & 0x00ff_fffc;
        let gpu_first_node = self.gpu_linked_list_dma.last_first_node & 0x00ff_fffc;
        let gpu_start_distance = address.abs_diff(gpu_start);
        let gpu_first_node_distance = address.abs_diff(gpu_first_node);

        let latest_otc = self
            .dma_activity
            .iter()
            .rev()
            .find(|sample| sample.kind == "otc_clear")
            .and_then(|sample| {
                let (low, high) = dma_activity_range_bounds(sample)?;
                let inside = (low..=high).contains(&address);
                Some((
                    sample,
                    low,
                    high,
                    inside,
                    distance_to_range(address, low, high),
                ))
            });
        let nearest_otc = self
            .dma_activity
            .iter()
            .rev()
            .filter(|sample| sample.kind == "otc_clear")
            .take(DMA_OTC_RECENT_RANGE_LIMIT)
            .filter_map(|sample| {
                let (low, high) = dma_activity_range_bounds(sample)?;
                let inside = (low..=high).contains(&address);
                Some((
                    sample,
                    low,
                    high,
                    inside,
                    distance_to_range(address, low, high),
                ))
            })
            .min_by_key(|(_, _, _, inside, distance)| (!*inside, *distance));

        let (
            latest_otc_vblank,
            latest_otc_low,
            latest_otc_high,
            latest_otc_inside,
            latest_otc_distance,
        ) = latest_otc.map_or(
            (None, None, None, false, None),
            |(sample, low, high, inside, distance)| {
                (
                    Some(sample.vblank),
                    Some(low),
                    Some(high),
                    inside,
                    Some(distance),
                )
            },
        );
        let (
            nearest_otc_vblank,
            nearest_otc_low,
            nearest_otc_high,
            nearest_otc_inside,
            nearest_otc_distance,
        ) = nearest_otc.map_or(
            (None, None, None, false, None),
            |(sample, low, high, inside, distance)| {
                (
                    Some(sample.vblank),
                    Some(low),
                    Some(high),
                    inside,
                    Some(distance),
                )
            },
        );

        format!(
            "{{\"last_gpu_start\":{},\"last_gpu_start_hex\":\"0x{:08x}\",\"distance_to_last_gpu_start\":{},\"last_gpu_first_node\":{},\"last_gpu_first_node_hex\":\"0x{:08x}\",\"distance_to_last_gpu_first_node\":{},\"latest_otc_vblank\":{},\"latest_otc_low\":{},\"latest_otc_low_hex\":{},\"latest_otc_high\":{},\"latest_otc_high_hex\":{},\"inside_latest_otc\":{},\"distance_to_latest_otc\":{},\"nearest_otc_vblank\":{},\"nearest_otc_low\":{},\"nearest_otc_low_hex\":{},\"nearest_otc_high\":{},\"nearest_otc_high_hex\":{},\"inside_nearest_otc\":{},\"distance_to_nearest_otc\":{}}}",
            gpu_start,
            gpu_start,
            gpu_start_distance,
            gpu_first_node,
            gpu_first_node,
            gpu_first_node_distance,
            optional_u64_json(latest_otc_vblank),
            optional_u32_json(latest_otc_low),
            optional_u32_hex_json(latest_otc_low),
            optional_u32_json(latest_otc_high),
            optional_u32_hex_json(latest_otc_high),
            latest_otc_inside,
            optional_u32_json(latest_otc_distance),
            optional_u64_json(nearest_otc_vblank),
            optional_u32_json(nearest_otc_low),
            optional_u32_hex_json(nearest_otc_low),
            optional_u32_json(nearest_otc_high),
            optional_u32_hex_json(nearest_otc_high),
            nearest_otc_inside,
            optional_u32_json(nearest_otc_distance)
        )
    }

    fn recent_primitive_header_relations_json(&self, limit: usize) -> String {
        if limit == 0 {
            return String::new();
        }
        let linked_nodes = self
            .gpu_linked_list_dma
            .last_visited_nodes
            .iter()
            .map(|address| address & 0x00ff_fffc)
            .collect::<HashSet<_>>();
        self.primitive_ram_writes
            .recent_header_like_writes
            .iter()
            .rev()
            .take(limit)
            .filter_map(|write| {
                let word_count = write.value >> 24;
                let next = write.value & 0x00ff_ffff;
                let first_command = self.read_ram_u32_physical(write.address + 4)?;
                let opcode = (first_command >> 24) as u8;
                let sample = self.primitive_packet_candidate_sample(write.address, &linked_nodes);
                let linked = sample
                    .as_ref()
                    .is_some_and(|sample| sample.linked)
                    || linked_nodes.contains(&(write.address & 0x00ff_fffc));
                let command_write_vblank = sample
                    .as_ref()
                    .and_then(|sample| sample.command_write_vblank);
                let playfield_bounds = sample.as_ref().is_some_and(|sample| {
                    self.primitive_packet_has_playfield_draw_bounds(
                        sample.address,
                        sample.word_count,
                    )
                });
                Some(format!(
                    "{{\"address\":{},\"address_hex\":\"0x{:08x}\",\"value\":{},\"value_hex\":\"0x{:08x}\",\"word_count\":{},\"next\":{},\"next_hex\":\"0x{:06x}\",\"first_command\":{},\"first_command_hex\":\"0x{:08x}\",\"opcode\":{},\"opcode_hex\":\"0x{:02x}\",\"candidate\":{},\"linked\":{},\"draw_opcode\":{},\"playfield_bounds\":{},\"header_write_vblank\":{},\"command_write_vblank\":{},\"pc\":{},\"pc_hex\":{},\"cycles\":{},\"dma_relation\":{}}}",
                    write.address,
                    write.address,
                    write.value,
                    write.value,
                    word_count,
                    next,
                    next,
                    first_command,
                    first_command,
                    opcode,
                    opcode,
                    sample.is_some(),
                    linked,
                    looks_like_draw_primitive_opcode(opcode),
                    playfield_bounds,
                    write.vblank,
                    optional_u64_json(command_write_vblank),
                    optional_u32_json(write.pc),
                    optional_u32_hex_json(write.pc),
                    write.cycles,
                    self.primitive_address_dma_relation_json(write.address)
                ))
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    fn dma_channel_compact_json(&self, channel: usize) -> String {
        let Some(state) = self.io.dma.channel_state(channel) else {
            return "null".to_string();
        };
        let pending_cycles = self
            .pending_dma_completion_cycles
            .get(channel)
            .copied()
            .unwrap_or_default();
        format!(
            "{{\"channel\":{},\"madr\":{},\"madr_hex\":\"0x{:08x}\",\"bcr\":{},\"bcr_hex\":\"0x{:08x}\",\"chcr\":{},\"chcr_hex\":\"0x{:08x}\",\"pending_completion_cycles\":{}}}",
            channel,
            state.madr,
            state.madr,
            state.bcr,
            state.bcr,
            state.chcr,
            state.chcr,
            pending_cycles
        )
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
        let mut current_vblank_command_candidates = 0u64;
        let mut previous_vblank_command_candidates = 0u64;
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
                if sample.command_write_vblank == Some(current_vblank) {
                    current_vblank_command_candidates =
                        current_vblank_command_candidates.saturating_add(1);
                }
                if sample.command_write_vblank == Some(previous_vblank) {
                    previous_vblank_command_candidates =
                        previous_vblank_command_candidates.saturating_add(1);
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
            "{{\"range_start\":\"0x{:08x}\",\"range_end\":\"0x{:08x}\",\"max_packet_words\":{},\"current_vblank\":{},\"previous_vblank\":{},\"last_dma_visited_nodes\":{},\"candidates\":{},\"linked_candidates\":{},\"unlinked_candidates\":{},\"current_vblank_candidates\":{},\"current_vblank_command_candidates\":{},\"current_vblank_linked_candidates\":{},\"previous_vblank_candidates\":{},\"previous_vblank_command_candidates\":{},\"previous_vblank_linked_candidates\":{},\"candidate_words\":{},\"linked_words\":{},\"unlinked_words\":{},\"opcode_counts\":[{}],\"linked_opcode_counts\":[{}],\"unlinked_opcode_counts\":[{}],\"linked_samples\":[{}],\"unlinked_samples\":[{}]}}",
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
            current_vblank_command_candidates,
            current_vblank_linked_candidates,
            previous_vblank_candidates,
            previous_vblank_command_candidates,
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

    fn primitive_packet_scan_compact_json(&self) -> String {
        let linked_nodes = self
            .gpu_linked_list_dma
            .last_visited_nodes
            .iter()
            .map(|address| address & 0x00ff_fffc)
            .collect::<HashSet<_>>();
        let current_vblank = self.vblank_count;
        let previous_vblank = self.vblank_count.saturating_sub(1);
        let min_vblank = self
            .vblank_count
            .saturating_sub(BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW);
        let mut candidates = 0u64;
        let mut linked_candidates = 0u64;
        let mut unlinked_candidates = 0u64;
        let mut draw_candidates = 0u64;
        let mut playfield_candidates = 0u64;
        let mut recent_command_candidates = 0u64;
        let mut current_vblank_candidates = 0u64;
        let mut previous_vblank_candidates = 0u64;
        let mut newest_header_vblank = 0u64;
        let mut newest_command_vblank = 0u64;

        let mut address = BR2_PRIMITIVE_RAM_START;
        while address.saturating_add(8) <= BR2_PRIMITIVE_RAM_END {
            if let Some(sample) = self.primitive_packet_candidate_sample(address, &linked_nodes) {
                candidates = candidates.saturating_add(1);
                if sample.linked {
                    linked_candidates = linked_candidates.saturating_add(1);
                } else {
                    unlinked_candidates = unlinked_candidates.saturating_add(1);
                }
                let opcode = (sample.first_command >> 24) as u8;
                if looks_like_draw_primitive_opcode(opcode) {
                    draw_candidates = draw_candidates.saturating_add(1);
                }
                if looks_like_draw_primitive_opcode(opcode)
                    && self.primitive_packet_has_playfield_draw_bounds(address, sample.word_count)
                {
                    playfield_candidates = playfield_candidates.saturating_add(1);
                }
                if sample
                    .command_write_vblank
                    .is_some_and(|vblank| vblank >= min_vblank)
                {
                    recent_command_candidates = recent_command_candidates.saturating_add(1);
                }
                if sample.header_write_vblank == Some(current_vblank) {
                    current_vblank_candidates = current_vblank_candidates.saturating_add(1);
                }
                if sample.header_write_vblank == Some(previous_vblank) {
                    previous_vblank_candidates = previous_vblank_candidates.saturating_add(1);
                }
                if let Some(vblank) = sample.header_write_vblank {
                    newest_header_vblank = newest_header_vblank.max(vblank);
                }
                if let Some(vblank) = sample.command_write_vblank {
                    newest_command_vblank = newest_command_vblank.max(vblank);
                }
            }
            address = address.saturating_add(4);
        }

        format!(
            "{{\"last_dma_visited_nodes\":{},\"candidates\":{},\"linked_candidates\":{},\"unlinked_candidates\":{},\"draw_candidates\":{},\"playfield_candidates\":{},\"recent_command_candidates\":{},\"current_vblank_candidates\":{},\"previous_vblank_candidates\":{},\"newest_header_vblank\":{},\"newest_command_vblank\":{},\"min_recent_vblank\":{}}}",
            linked_nodes.len(),
            candidates,
            linked_candidates,
            unlinked_candidates,
            draw_candidates,
            playfield_candidates,
            recent_command_candidates,
            current_vblank_candidates,
            previous_vblank_candidates,
            newest_header_vblank,
            newest_command_vblank,
            min_vblank
        )
    }

    fn unlinked_primitive_replay_candidate_diagnostics_json(&self, limit: usize) -> String {
        if limit == 0 {
            return String::new();
        }

        let linked_nodes = self
            .gpu_linked_list_dma
            .last_visited_nodes
            .iter()
            .map(|address| address & 0x00ff_fffc)
            .collect::<HashSet<_>>();
        let min_vblank = self
            .vblank_count
            .saturating_sub(BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW);
        let candidates =
            self.collect_unlinked_primitive_replay_candidates(&linked_nodes, Some(min_vblank));
        self.unlinked_primitive_replay_order(&candidates, &linked_nodes)
            .into_iter()
            .take(limit)
            .filter_map(|candidate| {
                self.primitive_replay_candidate_diagnostic_json(candidate, &linked_nodes)
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    fn primitive_replay_candidate_diagnostic_json(
        &self,
        candidate: PrimitiveReplayCandidate,
        linked_nodes: &HashSet<u32>,
    ) -> Option<String> {
        let sample = self.primitive_packet_candidate_sample(candidate.address, linked_nodes)?;
        let command_words = self.primitive_packet_command_words(sample.address, sample.word_count);
        let safe_draw_ranges = gp0_replay_safe_draw_command_ranges(&command_words);
        let safe_state_ranges = gp0_replay_safe_state_command_ranges(&command_words);
        let command_word_count = command_words.len();
        let command_words = command_words
            .iter()
            .map(|word| format!("\"0x{word:08x}\""))
            .collect::<Vec<_>>()
            .join(",");
        let opcode = (sample.first_command >> 24) as u8;
        Some(format!(
            "{{\"address\":{},\"address_hex\":\"0x{:08x}\",\"priority\":{},\"vblank\":{},\"linked\":{},\"word_count\":{},\"command_word_count\":{},\"next\":{},\"next_hex\":\"0x{:06x}\",\"opcode\":{},\"opcode_hex\":\"0x{:02x}\",\"draw_opcode\":{},\"textured_opcode\":{},\"playfield_bounds\":{},\"safe_draw_ranges\":[{}],\"safe_state_ranges\":[{}],\"header_write_vblank\":{},\"command_write_vblank\":{},\"dma_relation\":{},\"words\":[{}]}}",
            candidate.address,
            candidate.address,
            candidate.priority,
            candidate.vblank,
            sample.linked,
            sample.word_count,
            command_word_count,
            sample.next,
            sample.next,
            opcode,
            opcode,
            looks_like_draw_primitive_opcode(opcode),
            looks_like_textured_primitive_opcode(opcode),
            self.primitive_packet_has_playfield_draw_bounds(sample.address, sample.word_count),
            ranges_json(&safe_draw_ranges),
            ranges_json(&safe_state_ranges),
            optional_u64_json(sample.header_write_vblank),
            optional_u64_json(sample.command_write_vblank),
            self.primitive_address_dma_relation_json(sample.address),
            command_words
        ))
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
            command_write_vblank: self.packet_command_write_vblank(address, word_count),
        })
    }

    fn packet_command_write_vblank(&self, address: u32, word_count: u32) -> Option<u64> {
        let mut latest = None;
        for index in 0..word_count {
            let command_address = address + 4 + index * 4;
            if let Some(vblank) = self
                .primitive_ram_writes
                .command_write_vblank(command_address)
            {
                latest = Some(latest.map_or(vblank, |current: u64| current.max(vblank)));
            }
        }
        latest
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
            if gp0_command_is_replay_safe_draw(&commands[offset..offset + command_words]) {
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
            "{{\"io\":{},\"zn_board\":{{\"state\":{},\"assets\":{}}},\"br2_native_credit_hle\":{},\"zn_input_reads\":{},\"native_sync\":{}}}",
            self.io.runtime_probe_json(),
            self.zn_board.runtime_probe_json(),
            self.board_asset_status.json(),
            self.br2_native_credit_hle_json(),
            self.zn_input_reads_json(),
            self.native_sync_json()
        )
    }

    pub fn runtime_compact_probe_json(&self) -> String {
        format!(
            "{{\"io\":{},\"zn_board\":{{\"state\":{},\"assets\":{}}},\"br2_native_credit_hle\":{},\"zn_input_reads\":{},\"native_sync\":{}}}",
            self.io.runtime_compact_probe_json(),
            self.zn_board.runtime_probe_json(),
            self.board_asset_status.json(),
            self.br2_native_credit_hle_json(),
            self.zn_input_reads_json(),
            self.native_sync_compact_json()
        )
    }

    pub fn input_probe_json(&self) -> String {
        format!(
            "{{\"vblank_count\":{},\"zn_board\":{{\"state\":{},\"assets\":{}}},\"br2_native_credit_hle\":{},\"zn_input_reads\":{},\"input_activity\":{},\"controller\":{}}}",
            self.vblank_count(),
            self.zn_board.runtime_probe_json(),
            self.board_asset_status.json(),
            self.br2_native_credit_hle_json(),
            self.zn_input_reads_json(),
            self.input_activity().json(),
            self.io.controller.diagnostic_json()
        )
    }

    pub fn input_compact_probe_json(&self) -> String {
        format!(
            "{{\"vblank_count\":{},\"zn_board\":{{\"state\":{},\"assets\":{}}},\"br2_native_credit_hle\":{},\"zn_input_read_summary\":{},\"recent_active_zn_input_reads\":[{}],\"input_activity\":{}}}",
            self.vblank_count(),
            self.zn_board.runtime_probe_json(),
            self.board_asset_status.json(),
            self.br2_native_credit_hle_json(),
            self.zn_input_read_stats.borrow().json(),
            zn_input_tail_events_json(
                &self.recent_active_zn_input_reads.borrow(),
                ZN_BOARD_INPUT_COMPACT_ACTIVE_READ_LIMIT,
            ),
            self.input_activity().json()
        )
    }

    pub fn input_summary_json(&self) -> String {
        format!(
            "{{\"vblank_count\":{},\"zn_board\":{{\"state\":{},\"assets\":{}}},\"br2_native_credit_hle\":{},\"zn_input_read_summary\":{},\"input_activity\":{},\"controller\":{}}}",
            self.vblank_count(),
            self.zn_board.runtime_probe_json(),
            self.board_asset_status.json(),
            self.br2_native_credit_hle_json(),
            self.zn_input_read_stats.borrow().json(),
            self.input_activity().json(),
            self.io.controller.diagnostic_json()
        )
    }

    pub fn security_probe_json(&self) -> String {
        format!(
            "{{\"vblank_count\":{},\"zn_board\":{{\"state\":{},\"assets\":{}}},\"br2_native_credit_hle\":{},\"zn_input_read_summary\":{},\"input_activity\":{},\"controller\":{}}}",
            self.vblank_count(),
            self.zn_board.runtime_probe_json(),
            self.board_asset_status.json(),
            self.br2_native_credit_hle_json(),
            self.zn_input_read_stats.borrow().json(),
            self.input_activity().json(),
            self.io.controller.security_compact_json()
        )
    }

    pub fn set_coin_input_mapping_name(&mut self, value: &str) -> bool {
        self.zn_board.set_coin_input_mapping_name(value)
    }

    pub fn set_native_credit_adapter_input_bit(&mut self, value: u32) -> bool {
        self.zn_board.set_native_credit_adapter_input_bit(value)
    }

    pub fn set_native_credit_projection_name(&mut self, value: &str) -> bool {
        self.zn_board.set_native_credit_projection_name(value)
    }

    pub fn native_playability_json(&self) -> String {
        self.io.native_playability_json()
    }

    pub fn native_playability_compact_json(&self) -> String {
        self.io.native_playability_compact_json()
    }

    pub fn native_playable_candidate(&self) -> bool {
        self.io.native_playable_candidate()
    }

    pub fn native_rendered_scene_candidate(&self) -> bool {
        self.io.native_rendered_scene_candidate()
    }

    pub fn display_candidate_summary_json(&self) -> String {
        self.io.display_candidate_summary_json()
    }

    pub fn display_candidate_compact_summary_json(&self, limit: usize) -> String {
        self.io.display_candidate_compact_summary_json(limit)
    }

    pub fn display_rgb_frame(&self) -> (usize, usize, Vec<u32>) {
        self.io.display_rgb_frame()
    }

    pub fn set_gpu_draw_capture_range(&mut self, start: u64, end: u64) {
        self.io.gpu.set_draw_capture_range(start, end);
    }

    pub fn set_gpu_draw_capture_predicates(
        &mut self,
        predicates: Vec<NativeGpuDrawCapturePredicate>,
    ) {
        self.io.gpu.set_draw_capture_predicates(predicates);
    }

    pub fn gpu_draw_sequence(&self) -> u64 {
        self.io.gpu.draw_sequence()
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
        self.inject_br2_native_credit_from_coin_edges();
    }

    pub fn input_activity(&self) -> NativeInputActivity {
        let pad = self.io.controller.input_activity();
        self.zn_board
            .input_activity()
            .saturating_added(NativeInputActivity {
                p1_input_reads: pad.p1_input_reads,
                p1_up_active_reads: pad.p1_up_active_reads,
                p1_down_active_reads: pad.p1_down_active_reads,
                p1_left_active_reads: pad.p1_left_active_reads,
                p1_right_active_reads: pad.p1_right_active_reads,
                p1_start_active_reads: pad.p1_start_active_reads,
                p1_punch_active_reads: pad.p1_punch_active_reads,
                p1_kick_active_reads: pad.p1_kick_active_reads,
                p1_beast_active_reads: pad.p1_beast_active_reads,
                p3_guard_active_reads: pad.p1_guard_active_reads,
                ..NativeInputActivity::default()
            })
    }

    pub fn consume_br2_native_credit_hle_coin_edges(&mut self) -> u64 {
        let edges = self.zn_board.coin_insert_edges;
        let pending = edges.saturating_sub(self.br2_native_credit_hle_consumed_coin_edges);
        self.br2_native_credit_hle_consumed_coin_edges = edges;
        pending
    }

    fn inject_br2_native_credit_from_coin_edges(&mut self) {
        let edges = self.zn_board.coin_insert_edges;
        let pending = edges.saturating_sub(self.br2_native_credit_hle_consumed_coin_edges);
        if pending == 0 || !self.br2_native_credit_state_is_ready() {
            return;
        }

        let player = 0;
        let freeplay = self.read_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_FREEPLAY_FLAG_OFFSET) != 0;
        let required = self.read_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_REQUIRED_P1_OFFSET);
        let effective_required = required.max(1);
        let credit_slot = self.br2_credit_slot_address(player);
        let credit_before = self.read_u8(credit_slot);
        let inserted = pending
            .saturating_mul(u64::from(effective_required))
            .min(0xff) as u8;
        let credit_after = if freeplay {
            credit_before
        } else {
            credit_before.saturating_add(inserted)
        };
        if !freeplay {
            self.write_u8(credit_slot, credit_after);
        }
        self.br2_native_credit_hle_consumed_coin_edges = edges;
        self.br2_native_credit_hle.record(Br2NativeCreditHleCheck {
            player,
            freeplay,
            required,
            credit_slot,
            credit_before,
            credit_after,
            pending_coin_edges: pending,
            result: u32::from(credit_after),
        });
    }

    fn br2_native_credit_state_is_ready(&self) -> bool {
        let freeplay = self.read_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_FREEPLAY_FLAG_OFFSET) != 0;
        let required_p1 = self.read_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_REQUIRED_P1_OFFSET);
        let required_p2 = self.read_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_REQUIRED_P2_OFFSET);
        let player_mode = self.read_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_PLAYER_MODE_OFFSET);
        if required_p2 > 9 || player_mode > 2 {
            return false;
        }

        let br2_input_adapter_seen = self.zn_board.native_credit_adapter_writes > 0
            || self.zn_board.native_credit_adapter_edges > 0;
        freeplay
            || (1..=9).contains(&required_p1)
            || (required_p1 == 0
                && (self.br2_native_credit_hle_accepted_seen() || br2_input_adapter_seen))
    }

    fn br2_credit_slot_address(&self, player: u32) -> u32 {
        let player_mode = self.read_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_PLAYER_MODE_OFFSET);
        if player_mode == 1 {
            BR2_CREDIT_STATE_BASE + BR2_CREDIT_SHARED_SLOT_OFFSET + player.min(1).saturating_mul(2)
        } else {
            BR2_CREDIT_STATE_BASE + BR2_CREDIT_SHARED_SLOT_OFFSET
        }
    }

    pub fn record_br2_native_credit_hle_check(&mut self, check: Br2NativeCreditHleCheck) {
        self.br2_native_credit_hle.record(check);
    }

    pub fn br2_native_credit_hle_json(&self) -> String {
        self.br2_native_credit_hle.json()
    }

    pub fn br2_native_credit_hle_accepted_seen(&self) -> bool {
        self.br2_native_credit_hle.accepted > 0
    }

    fn record_zn_board_input_read(&self, address: u32, width: u8, value: u32) {
        if !is_zn_input_read_address(address) {
            return;
        }

        let event = ZnBoardInputReadEvent {
            vblank: self.vblank_count,
            cycles: self.trace_cycles.get(),
            pc: self.trace_pc.get(),
            address,
            width,
            value,
            input: self.zn_board.input,
        };
        self.zn_input_read_stats.borrow_mut().record(event);
        push_recent_zn_input_event(
            &self.recent_zn_input_reads,
            event,
            ZN_BOARD_INPUT_READ_RECENT_LIMIT,
        );
        if action_buttons_have_any_input(event.input) {
            push_recent_zn_input_event(
                &self.recent_active_zn_input_reads,
                event,
                ZN_BOARD_INPUT_ACTIVE_READ_RECENT_LIMIT,
            );
        }
    }

    fn zn_input_reads_json(&self) -> String {
        format!(
            "{{\"summary\":{},\"recent\":[{}],\"recent_active\":[{}]}}",
            self.zn_input_read_stats.borrow().json(),
            zn_input_events_json(&self.recent_zn_input_reads.borrow()),
            zn_input_events_json(&self.recent_active_zn_input_reads.borrow())
        )
    }

    pub fn set_access_trace_limit(&mut self, limit: usize) {
        self.access_trace_limit = limit;
        self.access_trace.get_mut().clear();
        self.access_trace_watch_access_hit.set(false);
        self.access_trace_watch_data_hit.set(false);
        self.access_trace_watch_write_hit.set(false);
    }

    pub fn set_access_trace_watch_ranges(&mut self, ranges: Vec<(u32, u32)>) {
        self.access_trace_watch_ranges = ranges
            .into_iter()
            .filter_map(|(address, len)| BusTraceWatchRange::new(address, len))
            .collect();
        self.access_trace.get_mut().clear();
        self.access_trace_watch_access_hit.set(false);
        self.access_trace_watch_data_hit.set(false);
        self.access_trace_watch_write_hit.set(false);
    }

    pub fn set_access_trace_watch_only(&mut self, watch_only: bool) {
        self.access_trace_watch_only = watch_only;
        self.access_trace.get_mut().clear();
        self.access_trace_watch_access_hit.set(false);
        self.access_trace_watch_data_hit.set(false);
        self.access_trace_watch_write_hit.set(false);
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

    pub fn access_trace_watch_write_hit(&self) -> bool {
        self.access_trace_watch_write_hit.get()
    }

    pub fn access_trace_watch_access_hit(&self) -> bool {
        self.access_trace_watch_access_hit.get()
    }

    pub fn access_trace_watch_data_hit(&self) -> bool {
        self.access_trace_watch_data_hit.get()
    }

    pub fn clear_access_trace_watch_hits(&self) {
        self.access_trace_watch_access_hit.set(false);
        self.access_trace_watch_data_hit.set(false);
        self.access_trace_watch_write_hit.set(false);
    }

    pub fn executable_pc_mapped(&self, pc: u32) -> bool {
        ram_offset(pc, self.ram.len(), 4).is_some()
            || scratchpad_offset(pc, self.scratchpad.len(), 4).is_some()
            || rom_offset(pc, self.rom.len(), 4).is_some()
            || banked_rom_offset(pc, self.banked_roms.len(), 4, self.zn_board.rom_bank).is_some()
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
        if self.watch_matches(address, width as usize) {
            self.access_trace_watch_access_hit.set(true);
            if self.trace_pc.get() != Some(address) {
                self.access_trace_watch_data_hit.set(true);
            }
            if operation == "write" {
                self.access_trace_watch_write_hit.set(true);
            }
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
            self.access_trace_watch_access_hit.set(true);
            if self.trace_pc.get() != Some(address) {
                self.access_trace_watch_data_hit.set(true);
            }
            if operation == "write" {
                self.access_trace_watch_write_hit.set(true);
            }
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

    fn record_gpu_block_dma_activity(&mut self, start_address: u32, words: u32, control: u32) {
        let (madr, bcr, chcr) = self.dma_channel_snapshot(DMA_GPU_CHANNEL);
        let start = start_address & 0x00ff_fffc;
        let end = dma_transfer_end_address(start, words, control);
        self.push_dma_activity(DmaActivitySample {
            kind: "gpu_block_write",
            channel: DMA_GPU_CHANNEL,
            register: None,
            address: None,
            value: None,
            madr,
            bcr,
            chcr,
            start: Some(start),
            end,
            words,
            nodes: 0,
            nonempty_nodes: 0,
            pc: self.trace_pc.get(),
            vblank: self.vblank_count,
            cycles: self.trace_cycles.get(),
        });
    }

    fn record_gpu_read_dma_activity(&mut self, start_address: u32, words: u32, control: u32) {
        let (madr, bcr, chcr) = self.dma_channel_snapshot(DMA_GPU_CHANNEL);
        let start = start_address & 0x00ff_fffc;
        let end = dma_transfer_end_address(start, words, control);
        self.push_dma_activity(DmaActivitySample {
            kind: "gpu_read",
            channel: DMA_GPU_CHANNEL,
            register: None,
            address: None,
            value: None,
            madr,
            bcr,
            chcr,
            start: Some(start),
            end,
            words,
            nodes: 0,
            nonempty_nodes: 0,
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
        if let Some(stats) = self.dma_lifetime_activity.get_mut(sample.channel) {
            stats.record(&sample);
        }
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
        let mdec_video_disabled = std::env::var_os("BR2_NATIVE_DISABLE_MDEC_VIDEO").is_some();
        for _ in 0..words {
            let word = if mdec_video_disabled {
                self.io.mdec.read_disabled_dma_output()
            } else {
                self.io.mdec.read_dma_output()
            };
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
            }
            for range in gpu_linked_list_command_ranges(&node_commands) {
                stats.record_command_group(&node_commands, range.clone());
                if !reverse_nodes {
                    self.write_gpu_dma_linked_list_command_range(&node_commands, range);
                }
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
                let ranges = gpu_linked_list_command_ranges(node);
                if reverse_command_groups {
                    for range in ranges.into_iter().rev() {
                        self.write_gpu_dma_linked_list_command_range(node, range);
                    }
                } else {
                    for range in ranges {
                        self.write_gpu_dma_linked_list_command_range(node, range);
                    }
                }
            }
        }
        stats.record_context(
            self.trace_pc.get(),
            self.vblank_count,
            self.trace_cycles.get(),
        );
        self.gpu_linked_list_generation = self.gpu_linked_list_generation.saturating_add(1);
        self.try_unlinked_primitive_replay(&stats);
        self.record_gpu_linked_list_dma_activity(start_address, &stats);
        self.gpu_linked_list_dma.merge_last(stats);
    }

    fn write_gpu_dma_linked_list_command_range(
        &mut self,
        commands: &[(u32, u32)],
        range: std::ops::Range<usize>,
    ) {
        let Some((range_start_address, _)) = commands.get(range.start) else {
            return;
        };
        if self.gpu_linked_list_command_start_is_embedded_payload(*range_start_address) {
            self.gpu_linked_list_dma.embedded_payload_skips = self
                .gpu_linked_list_dma
                .embedded_payload_skips
                .saturating_add(1);
            return;
        }

        let words = commands[range.clone()]
            .iter()
            .map(|(_, command)| *command)
            .collect::<Vec<_>>();
        if self.gpu_linked_list_command_range_has_stale_draw_body(commands, range.clone(), &words) {
            return;
        }
        if gp0_command_is_linked_list_artifact_draw(&words) {
            return;
        }
        for (command_address, command) in &commands[range] {
            self.write_gpu_dma_linked_list_word(*command_address, *command);
        }
    }

    fn gpu_linked_list_command_range_has_stale_draw_body(
        &self,
        commands: &[(u32, u32)],
        range: std::ops::Range<usize>,
        words: &[u32],
    ) -> bool {
        let Some((range_start_address, _)) = commands.get(range.start) else {
            return false;
        };
        let Some(header_address) = range_start_address.checked_sub(4) else {
            return false;
        };
        let header_address = header_address & 0x00ff_fffc;
        if !(BR2_PRIMITIVE_RAM_START..BR2_PRIMITIVE_RAM_END).contains(&header_address) {
            return false;
        }

        let Some(command) = words.first() else {
            return false;
        };
        if !looks_like_draw_primitive_opcode((command >> 24) as u8)
            || !gp0_command_has_playfield_draw_bounds(words)
        {
            return false;
        }

        let min_vblank = self
            .vblank_count
            .saturating_sub(BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW);
        let Some(header_vblank) = self
            .primitive_ram_writes
            .header_write_vblank(header_address)
        else {
            return false;
        };
        if header_vblank < min_vblank {
            return false;
        }

        let latest_command_vblank = commands[range]
            .iter()
            .filter_map(|(command_address, _)| {
                self.primitive_ram_writes
                    .command_write_vblank(*command_address & 0x00ff_fffc)
            })
            .max();
        let Some(latest_command_vblank) = latest_command_vblank else {
            return false;
        };
        latest_command_vblank < min_vblank
            && header_vblank.saturating_sub(latest_command_vblank)
                >= BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW
    }

    fn gpu_linked_list_command_start_is_embedded_payload(&self, command_address: u32) -> bool {
        let command_address = command_address & 0x00ff_fffc;
        if command_address <= BR2_PRIMITIVE_RAM_START {
            return false;
        }

        for lookback_words in 1..=12_u32 {
            let Some(candidate_start) = command_address.checked_sub(lookback_words * 4) else {
                break;
            };
            if candidate_start < BR2_PRIMITIVE_RAM_START {
                break;
            }

            let mut words = Vec::new();
            for index in 0..PRIMITIVE_PACKET_MAX_WORDS {
                let Some(word) = self.read_ram_u32_physical(candidate_start + index * 4) else {
                    break;
                };
                words.push(word);
                if let Some(command_words) = gp0_command_word_count(&words)
                    && command_words == words.len()
                {
                    break;
                }
            }

            let Some(command_words) = gp0_command_word_count(&words) else {
                continue;
            };
            if command_words <= lookback_words as usize || command_words > words.len() {
                continue;
            }
            let opcode = (words[0] >> 24) as u8;
            if looks_like_draw_primitive_opcode(opcode) || matches!(opcode, 0x80 | 0xa0) {
                return true;
            }
        }

        false
    }

    fn write_gpu_dma_linked_list_word(&mut self, command_address: u32, command: u32) {
        if self.io.gpu.gp0_pending_words() == 0
            && self.gpu_linked_list_command_start_is_embedded_payload(command_address)
        {
            self.gpu_linked_list_dma.embedded_payload_skips = self
                .gpu_linked_list_dma
                .embedded_payload_skips
                .saturating_add(1);
            return;
        }
        self.io.gpu.write_gp0_with_source(
            command,
            GpuCommandSource::dma_linked_list(command_address, self.trace_pc.get()),
        );
    }

    fn unlinked_primitive_replay_decision(
        &mut self,
        stats: &GpuLinkedListDmaRunStats,
    ) -> UnlinkedPrimitiveReplayDecision {
        let min_vblank = self
            .vblank_count
            .saturating_sub(BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW);
        let recent_header_count = self
            .primitive_ram_writes
            .tracked_header_addresses_written_since(min_vblank)
            .len();
        let mut diagnostics = UnlinkedPrimitiveReplayDiagnostics {
            min_vblank,
            recent_header_count,
            ..UnlinkedPrimitiveReplayDiagnostics::default()
        };

        if std::env::var_os("BR2_NATIVE_DISABLE_UNLINKED_PRIMITIVE_REPLAY").is_some() {
            return UnlinkedPrimitiveReplayDecision::disabled("disabled", recent_header_count)
                .with_diagnostics(diagnostics);
        }

        if std::env::var_os("BR2_NATIVE_ENABLE_UNLINKED_PRIMITIVE_REPLAY").is_some() {
            return UnlinkedPrimitiveReplayDecision::enabled("forced", recent_header_count)
                .with_diagnostics(diagnostics);
        }

        if self
            .effective_unlinked_primitive_replay_interval()
            .is_none()
        {
            return UnlinkedPrimitiveReplayDecision::disabled(
                "disabled_by_default",
                recent_header_count,
            )
            .with_diagnostics(diagnostics);
        }

        if self.unlinked_primitive_replay.last_vblank == Some(self.vblank_count)
            && (self.unlinked_primitive_replay.last_packets > 0
                || self.unlinked_primitive_replay.last_reason == "replay_rejected_after_validation"
                || self.unlinked_primitive_replay.last_reason == "replay_no_packets"
                || self.unlinked_primitive_replay.last_reason == "replay_full_validation_throttled"
                || self.unlinked_primitive_replay.last_reason == "already_attempted_this_vblank")
        {
            return UnlinkedPrimitiveReplayDecision::disabled(
                "already_attempted_this_vblank",
                recent_header_count,
            )
            .with_diagnostics(diagnostics);
        }
        if self.unlinked_primitive_replay.last_reason == "replay_rejected_after_validation"
            && self
                .unlinked_primitive_replay
                .last_vblank
                .is_some_and(|last_vblank| {
                    self.vblank_count.saturating_sub(last_vblank)
                        < BR2_UNLINKED_PRIMITIVE_REPLAY_REJECT_COOLDOWN_VBLANKS
                })
        {
            return UnlinkedPrimitiveReplayDecision::disabled(
                "recent_validation_reject_cooldown",
                recent_header_count,
            )
            .with_diagnostics(diagnostics);
        }

        let recent_header_writes = self
            .primitive_ram_writes
            .current_vblank_header_like_writes
            .saturating_add(self.primitive_ram_writes.last_vblank_header_like_writes);
        let recent_draw_writes = recent_draw_primitive_writes(&self.primitive_ram_writes);
        let linked_nodes = stats
            .visited_nodes
            .iter()
            .map(|address| address & 0x00ff_fffc)
            .collect::<HashSet<_>>();
        let recent_draw_candidates =
            self.recent_unlinked_draw_packet_candidates(&linked_nodes, min_vblank);
        let recent_stale_draw_candidates =
            self.recent_stale_unlinked_draw_packet_candidates(&linked_nodes, min_vblank);
        let sparse_scene_stale_scan_allowed =
            self.sparse_scene_stale_unlinked_primitive_scan_allowed(stats);
        let stale_scan_allowed =
            br2_stale_unlinked_primitive_scan_allowed() || sparse_scene_stale_scan_allowed;
        let stale_draw_candidates = if stale_scan_allowed
            && stats.last_nonempty_nodes <= BR2_UNLINKED_PRIMITIVE_REPLAY_SPARSE_NODE_LIMIT
        {
            self.stale_unlinked_draw_packet_candidates(&linked_nodes)
        } else {
            0
        };
        diagnostics.recent_header_writes = recent_header_writes;
        diagnostics.recent_draw_writes = recent_draw_writes;
        diagnostics.recent_draw_candidates = recent_draw_candidates;
        diagnostics.recent_stale_draw_candidates = recent_stale_draw_candidates;
        diagnostics.stale_draw_candidates = stale_draw_candidates;
        let has_recent_primitive_stream = recent_header_count as u64
            >= BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_RECENT_HEADERS
            || recent_header_writes >= BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_RECENT_HEADERS
            || recent_draw_candidates >= BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_DRAW_PACKETS;
        let has_any_recent_headers = recent_header_count > 0 || recent_header_writes > 0;
        let has_recent_draw_stream = recent_draw_writes
            >= u64::from(BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_DRAW_PACKETS)
            || recent_draw_candidates >= BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_DRAW_PACKETS;

        if stats.last_nodes < BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_LINKED_NODES {
            if stats.last_nonempty_nodes <= BR2_UNLINKED_PRIMITIVE_REPLAY_SPARSE_NODE_LIMIT
                && has_recent_draw_stream
                && (has_recent_primitive_stream || has_any_recent_headers)
            {
                return UnlinkedPrimitiveReplayDecision::enabled(
                    "short_linked_list_recent_primitive_stream",
                    recent_header_count,
                )
                .with_diagnostics(diagnostics);
            }
            if stats.last_nonempty_nodes <= BR2_UNLINKED_PRIMITIVE_REPLAY_SPARSE_NODE_LIMIT
                && recent_stale_draw_candidates >= BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_DRAW_PACKETS
            {
                return UnlinkedPrimitiveReplayDecision::enabled(
                    "short_linked_list_recent_stale_primitive_stream",
                    recent_header_count.max(recent_stale_draw_candidates as usize),
                )
                .with_diagnostics(diagnostics);
            }
            if stats.last_nonempty_nodes <= BR2_UNLINKED_PRIMITIVE_REPLAY_SPARSE_NODE_LIMIT
                && stale_draw_candidates >= BR2_UNLINKED_PRIMITIVE_REPLAY_STALE_SCAN_MIN_PACKETS
                && stale_scan_allowed
            {
                let reason = if sparse_scene_stale_scan_allowed
                    && !br2_stale_unlinked_primitive_scan_allowed()
                {
                    "short_linked_list_stale_sparse_scene"
                } else {
                    "short_linked_list_stale_primitive_scan"
                };
                return UnlinkedPrimitiveReplayDecision::enabled(
                    reason,
                    recent_header_count.max(stale_draw_candidates as usize),
                )
                .with_diagnostics(diagnostics);
            }
            return UnlinkedPrimitiveReplayDecision::disabled(
                "linked_list_too_short",
                recent_header_count,
            )
            .with_diagnostics(diagnostics);
        }

        if stats.last_nonempty_nodes > BR2_UNLINKED_PRIMITIVE_REPLAY_SPARSE_NODE_LIMIT {
            return UnlinkedPrimitiveReplayDecision::disabled(
                "linked_list_not_sparse",
                recent_header_count,
            )
            .with_diagnostics(diagnostics);
        }

        let linked_draw_packets = draw_primitive_count(&stats.command_opcode_counts);
        if linked_draw_packets < BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_DRAW_PACKETS {
            return UnlinkedPrimitiveReplayDecision::disabled(
                "not_enough_linked_draw_packets",
                recent_header_count,
            )
            .with_diagnostics(diagnostics);
        }

        if recent_header_writes < BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_RECENT_HEADERS {
            return UnlinkedPrimitiveReplayDecision::disabled(
                "not_enough_recent_headers",
                recent_header_count,
            )
            .with_diagnostics(diagnostics);
        }

        if recent_header_count == 0 {
            return UnlinkedPrimitiveReplayDecision::disabled("no_recent_headers", 0)
                .with_diagnostics(diagnostics);
        }

        UnlinkedPrimitiveReplayDecision::enabled(
            "sparse_recent_primitive_headers",
            recent_header_count,
        )
        .with_diagnostics(diagnostics)
    }

    fn recent_unlinked_draw_packet_candidates(
        &self,
        linked_nodes: &HashSet<u32>,
        min_vblank: u64,
    ) -> u32 {
        self.collect_unlinked_primitive_replay_candidates(linked_nodes, Some(min_vblank))
            .into_values()
            .filter(|candidate| {
                self.primitive_packet_candidate_sample(candidate.address, linked_nodes)
                    .is_some_and(|sample| {
                        let playfield = self.primitive_packet_has_playfield_draw_bounds(
                            candidate.address,
                            sample.word_count,
                        );
                        !sample.linked
                            && looks_like_draw_primitive_opcode((sample.first_command >> 24) as u8)
                            && playfield
                    })
            })
            .count()
            .min(u32::MAX as usize) as u32
    }

    fn stale_unlinked_draw_packet_candidates(&mut self, linked_nodes: &HashSet<u32>) -> u32 {
        self.collect_stale_unlinked_primitive_replay_candidates_cached(linked_nodes)
            .len()
            .min(u32::MAX as usize) as u32
    }

    fn sparse_scene_stale_unlinked_primitive_scan_allowed(
        &self,
        stats: &GpuLinkedListDmaRunStats,
    ) -> bool {
        std::env::var_os("BR2_NATIVE_DISABLE_STALE_UNLINKED_PRIMITIVE_REPLAY").is_none()
            && self
                .effective_unlinked_primitive_replay_interval()
                .is_some()
            && stats.last_nonempty_nodes <= BR2_UNLINKED_PRIMITIVE_REPLAY_SPARSE_NODE_LIMIT
            && self.io.native_sparse_scene_replay_gate()
    }

    fn should_run_full_unlinked_primitive_replay_validation(&self) -> bool {
        if std::env::var_os("BR2_NATIVE_DISABLE_UNLINKED_PRIMITIVE_REPLAY_FULL_VALIDATION_THROTTLE")
            .is_some()
        {
            return true;
        }

        self.unlinked_primitive_replay
            .last_full_validation_vblank
            .is_none_or(|last_vblank| {
                self.vblank_count.saturating_sub(last_vblank)
                    >= BR2_UNLINKED_PRIMITIVE_REPLAY_FULL_VALIDATION_COOLDOWN_VBLANKS
            })
    }

    fn recent_stale_unlinked_draw_packet_candidates(
        &self,
        linked_nodes: &HashSet<u32>,
        min_vblank: u64,
    ) -> u32 {
        self.collect_recent_stale_unlinked_primitive_replay_candidates(linked_nodes, min_vblank)
            .len()
            .min(u32::MAX as usize) as u32
    }

    fn process_vblank_unlinked_primitive_replay(&mut self) {
        if self.primitive_ram_writes.current_vblank_header_like_writes == 0
            && self.primitive_ram_writes.last_vblank_header_like_writes == 0
            && self.primitive_ram_writes.current_vblank_command_like_writes == 0
            && self.primitive_ram_writes.last_vblank_command_like_writes == 0
        {
            return;
        }
        let stats = self.last_gpu_linked_list_run_for_replay();
        self.try_unlinked_primitive_replay(&stats);
    }

    fn last_gpu_linked_list_run_for_replay(&self) -> GpuLinkedListDmaRunStats {
        let mut stats = GpuLinkedListDmaRunStats::started(
            self.gpu_linked_list_dma.last_start,
            self.gpu_linked_list_dma.last_first_node,
        );
        stats.last_nodes = self.gpu_linked_list_dma.last_nodes;
        stats.last_words = self.gpu_linked_list_dma.last_words;
        stats.last_nonempty_nodes = self.gpu_linked_list_dma.last_nonempty_nodes;
        stats.last_max_node_words = self.gpu_linked_list_dma.last_max_node_words;
        stats.last_min_command_address = self.gpu_linked_list_dma.last_min_command_address;
        stats.last_max_command_address = self.gpu_linked_list_dma.last_max_command_address;
        stats.command_opcode_counts = self.gpu_linked_list_dma.last_command_opcode_counts;
        stats.visited_nodes = self.gpu_linked_list_dma.last_visited_nodes.clone();
        stats.pc = self.gpu_linked_list_dma.last_pc;
        stats.vblank = self.gpu_linked_list_dma.last_vblank;
        stats.cycles = self.gpu_linked_list_dma.last_cycles;
        stats.terminated = self.gpu_linked_list_dma.last_terminated;
        stats.hit_node_limit = self.gpu_linked_list_dma.last_hit_node_limit;
        stats
    }

    fn try_unlinked_primitive_replay(&mut self, stats: &GpuLinkedListDmaRunStats) {
        let sparse_scene_attempt =
            self.should_attempt_sparse_scene_unlinked_primitive_replay(stats);
        if !self.should_attempt_unlinked_primitive_replay() && !sparse_scene_attempt {
            self.unlinked_primitive_replay.record_skip(
                self.vblank_count,
                "replay_throttled",
                0,
                stats,
                UnlinkedPrimitiveReplayDiagnostics::default(),
            );
            return;
        }

        let replay_decision = self.unlinked_primitive_replay_decision(stats);
        if replay_decision.enabled {
            let linked_nodes = stats
                .visited_nodes
                .iter()
                .map(|address| address & 0x00ff_fffc)
                .collect::<HashSet<_>>();
            let validate_replay = replay_decision.reason != "forced"
                && std::env::var_os("BR2_NATIVE_DISABLE_UNLINKED_PRIMITIVE_REPLAY_VALIDATION")
                    .is_none();
            let gpu_before = validate_replay.then(|| self.io.gpu.clone());
            let mut replay_diagnostics = replay_decision.diagnostics;
            let include_stale_scan = br2_stale_unlinked_primitive_scan_allowed()
                || replay_decision.reason == "short_linked_list_stale_sparse_scene"
                || replay_decision.reason == "short_linked_list_stale_primitive_scan";
            let (packets, words) = self.replay_recent_unlinked_primitive_packets_with_diagnostics(
                &linked_nodes,
                &mut replay_diagnostics,
                include_stale_scan,
            );
            if packets == 0 {
                self.unlinked_primitive_replay.record_skip(
                    self.vblank_count,
                    "replay_no_packets",
                    replay_decision.candidate_headers,
                    stats,
                    replay_diagnostics,
                );
                return;
            }
            let lightweight_replay_validated = gpu_before.as_ref().is_some_and(|before| {
                self.io
                    .native_replay_lightweight_validation_candidate(before)
            });
            let run_full_validation = validate_replay
                && !lightweight_replay_validated
                && self.should_run_full_unlinked_primitive_replay_validation();
            if validate_replay && !lightweight_replay_validated && !run_full_validation {
                if let Some(gpu_before) = gpu_before {
                    self.io.gpu = gpu_before;
                }
                self.unlinked_primitive_replay.record_skip(
                    self.vblank_count,
                    "replay_full_validation_throttled",
                    replay_decision.candidate_headers,
                    stats,
                    replay_diagnostics,
                );
                return;
            }
            let full_replay_validated = if run_full_validation {
                self.unlinked_primitive_replay
                    .record_full_validation(self.vblank_count);
                let replay_candidate = self.io.native_replay_validation_candidate();
                let replay_incremental_candidate = gpu_before.as_ref().is_some_and(|before| {
                    self.io
                        .native_replay_validation_incremental_candidate(before)
                });
                replay_candidate || replay_incremental_candidate
            } else {
                false
            };
            let replay_validated =
                !validate_replay || lightweight_replay_validated || full_replay_validated;
            if validate_replay && packets > 0 && !replay_validated {
                let debug_validation_rejects =
                    std::env::var_os("BR2_NATIVE_REPLAY_VALIDATION_DEBUG").is_some();
                let debug_min_vblank =
                    native_replay_validation_debug_min_vblank().unwrap_or(u64::MAX);
                if debug_validation_rejects
                    && (self.unlinked_primitive_replay.skipped < 16
                        || self.vblank_count >= debug_min_vblank)
                {
                    eprintln!(
                        "br2_replay_validation_reject vblank={} packets={} words={} probe={}",
                        self.vblank_count,
                        packets,
                        words,
                        self.io
                            .gpu
                            .native_replay_validation_probe_json(gpu_before.as_ref())
                    );
                }
                if let Some(gpu_before) = gpu_before {
                    self.io.gpu = gpu_before;
                }
                self.unlinked_primitive_replay.record_skip(
                    self.vblank_count,
                    "replay_rejected_after_validation",
                    replay_decision.candidate_headers,
                    stats,
                    replay_diagnostics,
                );
                return;
            }
            self.unlinked_primitive_replay.record_replay(
                self.vblank_count,
                replay_decision.reason,
                replay_decision.candidate_headers,
                stats,
                replay_diagnostics,
                packets,
                words,
            );
        } else {
            self.unlinked_primitive_replay.record_skip(
                self.vblank_count,
                replay_decision.reason,
                replay_decision.candidate_headers,
                stats,
                replay_decision.diagnostics,
            );
        }
    }

    #[cfg(test)]
    fn replay_recent_unlinked_primitive_packets(
        &mut self,
        linked_nodes: &HashSet<u32>,
    ) -> (usize, usize) {
        let mut diagnostics = UnlinkedPrimitiveReplayDiagnostics::default();
        self.replay_recent_unlinked_primitive_packets_with_diagnostics(
            linked_nodes,
            &mut diagnostics,
            br2_stale_unlinked_primitive_scan_allowed(),
        )
    }

    fn replay_recent_unlinked_primitive_packets_with_diagnostics(
        &mut self,
        linked_nodes: &HashSet<u32>,
        diagnostics: &mut UnlinkedPrimitiveReplayDiagnostics,
        include_stale_scan: bool,
    ) -> (usize, usize) {
        let min_vblank = self
            .vblank_count
            .saturating_sub(BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW);
        let mut candidates =
            self.collect_unlinked_primitive_replay_candidates(linked_nodes, Some(min_vblank));
        if candidates.len() < BR2_UNLINKED_PRIMITIVE_REPLAY_STALE_SCAN_MIN_PACKETS as usize {
            for (address, candidate) in self
                .collect_recent_stale_unlinked_primitive_replay_candidates(linked_nodes, min_vblank)
            {
                candidates.entry(address).or_insert(candidate);
            }
        }
        if candidates.len() < BR2_UNLINKED_PRIMITIVE_REPLAY_STALE_SCAN_MIN_PACKETS as usize
            && include_stale_scan
        {
            for (address, candidate) in
                self.collect_stale_unlinked_primitive_replay_candidates_cached(linked_nodes)
            {
                candidates.entry(address).or_insert(candidate);
            }
        }
        diagnostics.replay_input_candidates = candidates.len().min(u32::MAX as usize) as u32;
        let candidates = self.unlinked_primitive_replay_order(&candidates, linked_nodes);
        diagnostics.replay_ordered_candidates = candidates.len().min(u32::MAX as usize) as u32;
        let state_packets_by_next =
            self.collect_unlinked_state_replay_candidates_by_next(linked_nodes, min_vblank);

        let mut replayed_packets = 0usize;
        let mut replayed_words = 0usize;
        let mut replayed_command_addresses = HashSet::new();
        for candidate in candidates {
            if replayed_packets >= BR2_UNLINKED_PRIMITIVE_REPLAY_PACKET_LIMIT {
                break;
            }
            let address = candidate.address;
            let Some(sample) = self.primitive_packet_candidate_sample(address, linked_nodes) else {
                continue;
            };
            if sample.linked {
                diagnostics.replay_linked_skips = diagnostics.replay_linked_skips.saturating_add(1);
                continue;
            }
            let opcode = (sample.first_command >> 24) as u8;
            if !looks_like_draw_primitive_opcode(opcode) && candidate.priority > 3 {
                diagnostics.replay_non_draw_skips =
                    diagnostics.replay_non_draw_skips.saturating_add(1);
                continue;
            }

            let command_words =
                self.primitive_packet_command_words(sample.address, sample.word_count);
            record_replay_draw_rejects(&command_words, diagnostics);
            let ranges = gp0_replay_safe_draw_command_ranges(&command_words);
            if ranges.is_empty() {
                diagnostics.replay_empty_safe_range_skips =
                    diagnostics.replay_empty_safe_range_skips.saturating_add(1);
                continue;
            }

            let state_words = self.replay_unlinked_state_packet_chain_for_target(
                sample.address,
                linked_nodes,
                &state_packets_by_next,
                &mut replayed_command_addresses,
            );
            diagnostics.saturating_add_state_words(state_words);
            replayed_words = replayed_words.saturating_add(state_words);
            for range in ranges {
                for index in range {
                    let command_address = sample.address + 4 + index as u32 * 4;
                    let command = command_words[index];
                    if !replayed_command_addresses.insert(command_address) {
                        diagnostics.replay_duplicate_command_skips =
                            diagnostics.replay_duplicate_command_skips.saturating_add(1);
                        continue;
                    }
                    self.write_gpu_dma_linked_list_word(command_address, command);
                    diagnostics.record_replayed_command(command_address, command);
                    replayed_words = replayed_words.saturating_add(1);
                }
            }
            replayed_packets = replayed_packets.saturating_add(1);
            diagnostics.replayed_draw_packets = diagnostics.replayed_draw_packets.saturating_add(1);
        }

        if replayed_packets < BR2_UNLINKED_PRIMITIVE_REPLAY_PACKET_LIMIT {
            let (raw_packets, raw_words) = self.replay_recent_unlinked_draw_command_streams(
                linked_nodes,
                min_vblank,
                BR2_UNLINKED_PRIMITIVE_REPLAY_PACKET_LIMIT - replayed_packets,
                &replayed_command_addresses,
                diagnostics,
            );
            replayed_packets = replayed_packets.saturating_add(raw_packets);
            replayed_words = replayed_words.saturating_add(raw_words);
        }
        (replayed_packets, replayed_words)
    }

    fn collect_unlinked_state_replay_candidates_by_next(
        &self,
        linked_nodes: &HashSet<u32>,
        min_vblank: u64,
    ) -> HashMap<u32, Vec<PrimitiveReplayStateCandidate>> {
        let mut packets_by_next: HashMap<u32, Vec<PrimitiveReplayStateCandidate>> = HashMap::new();
        let mut seen = HashSet::new();
        for (vblank, address) in self
            .primitive_ram_writes
            .tracked_header_addresses_written_since(min_vblank)
        {
            if !seen.insert(address) {
                continue;
            }
            let Some(sample) = self.primitive_packet_candidate_sample(address, linked_nodes) else {
                continue;
            };
            if sample.linked || gpu_linked_list_terminator(sample.next) {
                continue;
            }
            let commands = self.primitive_packet_command_words(sample.address, sample.word_count);
            if gp0_replay_safe_state_command_ranges(&commands).is_empty() {
                continue;
            }
            packets_by_next
                .entry(sample.next & 0x00ff_fffc)
                .or_default()
                .push(PrimitiveReplayStateCandidate {
                    address: sample.address,
                    vblank,
                });
        }
        packets_by_next
    }

    fn replay_unlinked_state_packet_chain_for_target(
        &mut self,
        target_address: u32,
        linked_nodes: &HashSet<u32>,
        state_packets_by_next: &HashMap<u32, Vec<PrimitiveReplayStateCandidate>>,
        replayed_command_addresses: &mut HashSet<u32>,
    ) -> usize {
        let mut chain = Vec::new();
        let mut target = target_address & 0x00ff_fffc;
        let mut seen = HashSet::new();
        for _ in 0..BR2_UNLINKED_PRIMITIVE_REPLAY_STATE_CHAIN_LIMIT {
            let Some(candidates) = state_packets_by_next.get(&target) else {
                break;
            };
            let Some(candidate) = candidates
                .iter()
                .max_by_key(|candidate| (candidate.vblank, candidate.address))
                .copied()
            else {
                break;
            };
            if !seen.insert(candidate.address) {
                break;
            }
            chain.push(candidate.address);
            target = candidate.address;
        }

        let mut replayed_words = 0usize;
        for address in chain.into_iter().rev() {
            replayed_words = replayed_words.saturating_add(self.replay_unlinked_state_packet(
                address,
                linked_nodes,
                replayed_command_addresses,
            ));
        }
        replayed_words
    }

    fn replay_unlinked_state_packet(
        &mut self,
        address: u32,
        linked_nodes: &HashSet<u32>,
        replayed_command_addresses: &mut HashSet<u32>,
    ) -> usize {
        let Some(sample) = self.primitive_packet_candidate_sample(address, linked_nodes) else {
            return 0;
        };
        if sample.linked {
            return 0;
        }

        let command_words = self.primitive_packet_command_words(sample.address, sample.word_count);
        let mut replayed_words = 0usize;
        for range in gp0_replay_safe_state_command_ranges(&command_words) {
            for index in range {
                let command_address = sample.address + 4 + index as u32 * 4;
                if !replayed_command_addresses.insert(command_address) {
                    continue;
                }
                self.write_gpu_dma_linked_list_word(command_address, command_words[index]);
                replayed_words = replayed_words.saturating_add(1);
            }
        }
        replayed_words
    }

    fn replay_recent_unlinked_draw_command_streams(
        &mut self,
        linked_nodes: &HashSet<u32>,
        min_vblank: u64,
        packet_limit: usize,
        excluded_command_addresses: &HashSet<u32>,
        diagnostics: &mut UnlinkedPrimitiveReplayDiagnostics,
    ) -> (usize, usize) {
        if packet_limit == 0 {
            return (0, 0);
        }

        let mut starts = self
            .primitive_ram_writes
            .recent_command_like_writes
            .iter()
            .copied()
            .filter(|sample| {
                sample.vblank >= min_vblank
                    && !linked_nodes.contains(&(sample.address & 0x00ff_fffc))
                    && !excluded_command_addresses.contains(&sample.address)
                    && looks_like_draw_primitive_opcode((sample.value >> 24) as u8)
            })
            .collect::<Vec<_>>();
        diagnostics.raw_stream_candidates = starts.len().min(u32::MAX as usize) as u32;
        starts.sort_unstable_by(|left, right| {
            left.vblank
                .cmp(&right.vblank)
                .then_with(|| left.cycles.cmp(&right.cycles))
                .then_with(|| left.address.cmp(&right.address))
        });

        let mut replayed_packets = 0usize;
        let mut replayed_words = 0usize;
        let mut seen_addresses = HashSet::new();
        for sample in starts {
            if replayed_packets >= packet_limit {
                break;
            }
            if !seen_addresses.insert(sample.address) {
                continue;
            }
            let mut words = Vec::new();
            for index in 0..PRIMITIVE_PACKET_MAX_WORDS {
                let Some(word) = self.read_ram_u32_physical(sample.address + index * 4) else {
                    break;
                };
                words.push(word);
                if let Some(command_words) = gp0_command_word_count(&words)
                    && command_words == words.len()
                {
                    break;
                }
            }

            let Some(command_words) = gp0_command_word_count(&words) else {
                diagnostics.raw_stream_rejected_incomplete =
                    diagnostics.raw_stream_rejected_incomplete.saturating_add(1);
                continue;
            };
            if command_words == 0 || command_words > words.len() {
                diagnostics.raw_stream_rejected_incomplete =
                    diagnostics.raw_stream_rejected_incomplete.saturating_add(1);
                continue;
            }
            words.truncate(command_words);
            if !gp0_command_is_replay_safe_draw(&words) {
                if let Some(reason) = gp0_command_replay_draw_reject_reason(&words) {
                    diagnostics.record_reject_reason(reason);
                }
                diagnostics.raw_stream_rejected_unsafe =
                    diagnostics.raw_stream_rejected_unsafe.saturating_add(1);
                continue;
            }

            for (index, command) in words.into_iter().enumerate() {
                let command_address = sample.address + (index as u32) * 4;
                self.write_gpu_dma_linked_list_word(command_address, command);
                diagnostics.record_replayed_command(command_address, command);
                replayed_words = replayed_words.saturating_add(1);
            }
            replayed_packets = replayed_packets.saturating_add(1);
            diagnostics.raw_stream_replayed_packets =
                diagnostics.raw_stream_replayed_packets.saturating_add(1);
            diagnostics.raw_stream_replayed_words = diagnostics
                .raw_stream_replayed_words
                .saturating_add(command_words.min(u32::MAX as usize) as u32);
        }

        (replayed_packets, replayed_words)
    }

    fn primitive_packet_command_words(&self, address: u32, word_count: u32) -> Vec<u32> {
        let mut words = Vec::with_capacity(word_count as usize);
        for index in 0..word_count {
            if let Some(command) = self.read_ram_u32_physical(address + 4 + index * 4) {
                words.push(command);
            }
        }
        words
    }

    fn collect_unlinked_primitive_replay_candidates(
        &self,
        linked_nodes: &HashSet<u32>,
        min_vblank: Option<u64>,
    ) -> HashMap<u32, PrimitiveReplayCandidate> {
        let mut candidates = HashMap::new();
        let mut seen = HashSet::new();

        if let Some(min_vblank) = min_vblank {
            for (vblank, address) in self
                .primitive_ram_writes
                .tracked_header_addresses_written_since(min_vblank)
            {
                self.insert_unlinked_primitive_replay_candidate(
                    &mut candidates,
                    &mut seen,
                    linked_nodes,
                    address,
                    vblank,
                    Some(min_vblank),
                );
            }
            let mut address = BR2_PRIMITIVE_RAM_START;
            while address.saturating_add(8) <= BR2_PRIMITIVE_RAM_END {
                if let Some(sample) = self.primitive_packet_candidate_sample(address, linked_nodes)
                    && let Some(vblank) = sample.command_write_vblank
                    && vblank >= min_vblank
                {
                    self.insert_unlinked_primitive_replay_candidate(
                        &mut candidates,
                        &mut seen,
                        linked_nodes,
                        address,
                        vblank,
                        Some(min_vblank),
                    );
                }
                address = address.saturating_add(4);
            }
            return candidates;
        }

        let mut address = BR2_PRIMITIVE_RAM_START;
        while address.saturating_add(8) <= BR2_PRIMITIVE_RAM_END {
            let vblank = self
                .primitive_ram_writes
                .header_write_vblank(address)
                .unwrap_or(0);
            self.insert_unlinked_primitive_replay_candidate(
                &mut candidates,
                &mut seen,
                linked_nodes,
                address,
                vblank,
                None,
            );
            address = address.saturating_add(4);
        }
        candidates
    }

    fn collect_stale_unlinked_primitive_replay_candidates_cached(
        &mut self,
        linked_nodes: &HashSet<u32>,
    ) -> HashMap<u32, PrimitiveReplayCandidate> {
        let key = PrimitiveReplayCandidateCacheKey {
            primitive_header_generation: self.primitive_header_generation,
            gpu_linked_list_generation: self.gpu_linked_list_generation,
        };
        if self.stale_unlinked_primitive_replay_candidates.key == Some(key) {
            return self
                .stale_unlinked_primitive_replay_candidates
                .candidates
                .clone();
        }

        let candidates = self.collect_unlinked_primitive_replay_candidates(linked_nodes, None);
        self.stale_unlinked_primitive_replay_candidates = PrimitiveReplayCandidateCache {
            key: Some(key),
            candidates: candidates.clone(),
        };
        candidates
    }

    fn collect_recent_stale_unlinked_primitive_replay_candidates(
        &self,
        linked_nodes: &HashSet<u32>,
        min_vblank: u64,
    ) -> HashMap<u32, PrimitiveReplayCandidate> {
        let mut candidates = HashMap::new();
        let mut seen = HashSet::new();

        for sample in self.primitive_ram_writes.recent_command_like_writes.iter() {
            if sample.vblank < min_vblank {
                continue;
            }
            if !looks_like_draw_primitive_opcode((sample.value >> 24) as u8) {
                continue;
            }
            let Some(header_address) = sample.address.checked_sub(4) else {
                continue;
            };
            if !(BR2_PRIMITIVE_RAM_START..BR2_PRIMITIVE_RAM_END).contains(&header_address) {
                continue;
            }
            self.insert_unlinked_primitive_replay_candidate(
                &mut candidates,
                &mut seen,
                linked_nodes,
                header_address,
                sample.vblank,
                None,
            );
        }

        candidates
    }

    fn insert_unlinked_primitive_replay_candidate(
        &self,
        candidates: &mut HashMap<u32, PrimitiveReplayCandidate>,
        seen: &mut HashSet<u32>,
        linked_nodes: &HashSet<u32>,
        address: u32,
        vblank: u64,
        min_vblank: Option<u64>,
    ) {
        if !seen.insert(address) {
            return;
        }
        let Some(sample) = self.primitive_packet_candidate_sample(address, linked_nodes) else {
            return;
        };
        if sample.linked {
            return;
        }
        let opcode = (sample.first_command >> 24) as u8;
        let draw = looks_like_draw_primitive_opcode(opcode);
        let textured = looks_like_textured_primitive_opcode(opcode);
        let playfield =
            draw && self.primitive_packet_has_playfield_draw_bounds(address, sample.word_count);
        if let Some(min_vblank) = min_vblank {
            let command_fresh = sample
                .command_write_vblank
                .is_some_and(|command_vblank| command_vblank >= min_vblank);
            let header_fresh = sample
                .header_write_vblank
                .is_some_and(|header_vblank| header_vblank >= min_vblank);
            let command_body_stale = sample
                .command_write_vblank
                .is_some_and(|command_vblank| command_vblank < min_vblank);
            if command_body_stale {
                return;
            }
            if !(command_fresh || header_fresh && playfield) {
                return;
            }
        }
        let priority = match (textured, playfield, draw) {
            (true, true, _) => 0,
            (false, true, _) => 2,
            _ => return,
        };
        candidates.insert(
            address,
            PrimitiveReplayCandidate {
                address,
                vblank,
                priority,
            },
        );
    }

    fn unlinked_primitive_replay_order(
        &self,
        candidates: &HashMap<u32, PrimitiveReplayCandidate>,
        linked_nodes: &HashSet<u32>,
    ) -> Vec<PrimitiveReplayCandidate> {
        let mut pointed_to = HashSet::new();
        for address in candidates.keys() {
            let Some(sample) = self.primitive_packet_candidate_sample(*address, linked_nodes)
            else {
                continue;
            };
            if gpu_linked_list_terminator(sample.next) {
                continue;
            }
            let next = sample.next & 0x00ff_fffc;
            if candidates.contains_key(&next) {
                pointed_to.insert(next);
            }
        }

        let mut heads = candidates
            .values()
            .filter(|candidate| !pointed_to.contains(&candidate.address))
            .copied()
            .collect::<Vec<_>>();
        sort_primitive_replay_candidates(&mut heads);

        let mut ordered = Vec::with_capacity(candidates.len());
        let mut visited = HashSet::new();
        for head in heads {
            let mut address = head.address;
            while let Some(candidate) = candidates.get(&address).copied() {
                if !visited.insert(address) {
                    break;
                }
                ordered.push(candidate);

                let Some(sample) = self.primitive_packet_candidate_sample(address, linked_nodes)
                else {
                    break;
                };
                if gpu_linked_list_terminator(sample.next) {
                    break;
                }
                let next = sample.next & 0x00ff_fffc;
                if !candidates.contains_key(&next) {
                    break;
                }
                address = next;
            }
        }

        let mut orphans = candidates
            .values()
            .filter(|candidate| !visited.contains(&candidate.address))
            .copied()
            .collect::<Vec<_>>();
        sort_primitive_replay_candidates(&mut orphans);
        ordered.extend(orphans);
        ordered
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
        self.record_gpu_block_dma_activity(start_address, words, control);
    }

    fn process_gpu_read_dma(&mut self, start_address: u32, bcr: u32, control: u32) {
        let words = dma_word_count(bcr).min(self.ram.len() as u32 / 4);
        let mut address = start_address & 0x00ff_fffc;
        let step = dma_address_step(control);
        for _ in 0..words {
            self.write_dma_u32(address, self.io.gpu.gp0_read);
            address = address.wrapping_add(step);
        }
        self.record_gpu_read_dma_activity(start_address, words, control);
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

    #[allow(dead_code)]
    fn read_bytes(&self, address: u32, len: usize) -> Vec<u8> {
        if let Some(offset) = ram_offset(address, self.ram.len(), len) {
            let bytes = self
                .br2_runtime_code_snapshot_read(address, len)
                .or_else(|| self.br2_code_patch_snapshot_read(address, len))
                .unwrap_or_else(|| self.ram[offset..offset + len].to_vec());
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
        let encoded = PreferredNativePlatform::write_le_u32(value);
        bytes.copy_from_slice(&encoded);
        self.record_br2_runtime_code_snapshot_write(physical, &encoded);
        self.record_br2_code_patch_snapshot_write(physical, &encoded);
        true
    }

    fn write_dma_u32(&mut self, address: u32, value: u32) {
        let bytes = PreferredNativePlatform::write_le_u32(value);
        if let Some(offset) = ram_offset(address, self.ram.len(), bytes.len()) {
            self.ram[offset..offset + bytes.len()].copy_from_slice(&bytes);
            self.record_br2_runtime_code_snapshot_write(address, &bytes);
            self.record_br2_code_patch_snapshot_write(address, &bytes);
            self.record_watch_trace("write", "ram_dma", address, bytes.len(), value);
        } else {
            self.record_access_trace("write", "dma_unmapped", address, bytes.len() as u8, value);
        }
    }

    fn write_bytes(&mut self, address: u32, bytes: &[u8]) {
        let physical = physical_address(address);
        let cache_isolated_primitive_passthrough =
            cache_isolated_primitive_ram_write_passthrough_allowed()
                && (BR2_PRIMITIVE_RAM_START..BR2_PRIMITIVE_RAM_END).contains(&physical);
        if self.cache_isolated
            && cacheable_address(address)
            && !cache_isolated_write_suppression_disabled()
            && !cache_isolated_primitive_passthrough
        {
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
            self.record_br2_runtime_code_snapshot_write(address, bytes);
            self.record_br2_code_patch_snapshot_write(address, bytes);
            self.record_primitive_ram_write(address, bytes);
            self.record_draw_sync_game_write(address, bytes);
            self.record_watch_trace("write", "ram", address, bytes.len(), bytes_to_le_u32(bytes));
        } else if let Some(offset) = scratchpad_offset(address, self.scratchpad.len(), bytes.len())
        {
            let adapted_bytes;
            let bytes_to_store = if let Some(value) =
                self.native_credit_adapter_scratchpad_write_value(address, bytes)
            {
                adapted_bytes = PreferredNativePlatform::write_le_u32(value);
                &adapted_bytes[..bytes.len()]
            } else {
                bytes
            };
            self.scratchpad[offset..offset + bytes.len()].copy_from_slice(bytes_to_store);
            self.record_watch_trace(
                "write",
                "scratchpad",
                address,
                bytes.len(),
                bytes_to_le_u32(bytes_to_store),
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

    fn native_credit_adapter_scratchpad_write_value(
        &mut self,
        address: u32,
        bytes: &[u8],
    ) -> Option<u32> {
        if !matches!(bytes.len(), 1 | 2 | 4) {
            return None;
        }

        self.zn_board.native_credit_adapter_scratchpad_write_value(
            physical_address(address),
            bytes_to_le_u32(bytes),
            bytes.len(),
            self.trace_pc.get(),
            self.vblank_count,
            self.trace_cycles.get(),
        )
    }

    fn record_br2_runtime_code_snapshot_write(&mut self, address: u32, bytes: &[u8]) {
        if bytes.is_empty()
            || self.trace_pc.get().map(physical_address) != Some(BR2_BOOT_WORD_COPY_LOOP_PHYSICAL)
        {
            return;
        }

        for (index, byte) in bytes.iter().copied().enumerate() {
            let physical = physical_address(address.wrapping_add(index as u32));
            if !(BR2_RUNTIME_CODE_SNAPSHOT_START..BR2_RUNTIME_CODE_SNAPSHOT_END).contains(&physical)
            {
                continue;
            }
            let snapshot_index = (physical - BR2_RUNTIME_CODE_SNAPSHOT_START) as usize;
            self.br2_runtime_code_snapshot[snapshot_index] = byte;
            self.br2_runtime_code_snapshot_valid[snapshot_index] = true;
        }
    }

    #[allow(dead_code)]
    fn br2_runtime_code_snapshot_read(&self, address: u32, len: usize) -> Option<Vec<u8>> {
        if len == 0 {
            return None;
        }

        let start = physical_address(address);
        if self.trace_pc.get().map(physical_address) != Some(start) {
            return None;
        }

        let end = start.checked_add(len as u32)?;
        if start < BR2_RUNTIME_CODE_SNAPSHOT_START || end > BR2_RUNTIME_CODE_SNAPSHOT_END {
            return None;
        }

        let offset = (start - BR2_RUNTIME_CODE_SNAPSHOT_START) as usize;
        let end_offset = offset.checked_add(len)?;
        self.br2_runtime_code_snapshot_valid
            .get(offset..end_offset)?
            .iter()
            .all(|valid| *valid)
            .then(|| self.br2_runtime_code_snapshot[offset..end_offset].to_vec())
    }

    fn br2_runtime_code_snapshot_read_value(&self, address: u32, len: usize) -> Option<u32> {
        if len == 0 || len > 4 {
            return None;
        }

        let start = physical_address(address);
        if self.trace_pc.get().map(physical_address) != Some(start) {
            return None;
        }

        self.br2_runtime_code_snapshot_read_value_unchecked(address, len)
    }

    fn br2_runtime_code_snapshot_read_value_unchecked(
        &self,
        address: u32,
        len: usize,
    ) -> Option<u32> {
        if len == 0 || len > 4 {
            return None;
        }

        let start = physical_address(address);

        let end = start.checked_add(len as u32)?;
        if start < BR2_RUNTIME_CODE_SNAPSHOT_START || end > BR2_RUNTIME_CODE_SNAPSHOT_END {
            return None;
        }

        let offset = (start - BR2_RUNTIME_CODE_SNAPSHOT_START) as usize;
        let end_offset = offset.checked_add(len)?;
        let valid = self
            .br2_runtime_code_snapshot_valid
            .get(offset..end_offset)?;
        if !valid.iter().all(|valid| *valid) {
            return None;
        }

        Some(bytes_to_le_u32(
            &self.br2_runtime_code_snapshot[offset..end_offset],
        ))
    }

    fn br2_boot_global_snapshot_fallback_value(
        &self,
        address: u32,
        len: usize,
        ram_value: u32,
    ) -> u32 {
        if ram_value != 0 || len == 0 || len > 4 {
            return ram_value;
        }

        let start = physical_address(address);
        let Some(end) = start.checked_add(len as u32) else {
            return ram_value;
        };
        if !BR2_BOOT_GLOBAL_SNAPSHOT_FALLBACK_RANGES
            .iter()
            .any(|(range_start, range_end)| start >= *range_start && end <= *range_end)
        {
            return ram_value;
        }

        self.br2_runtime_code_snapshot_read_value_unchecked(address, len)
            .filter(|value| *value != 0)
            .unwrap_or(ram_value)
    }

    fn record_br2_code_patch_snapshot_write(&mut self, address: u32, bytes: &[u8]) {
        if self.br2_code_patch_snapshot_frozen || bytes.is_empty() {
            return;
        }

        for (index, byte) in bytes.iter().copied().enumerate() {
            let physical = physical_address(address.wrapping_add(index as u32));
            if !(BR2_CODE_PATCH_SNAPSHOT_START..BR2_CODE_PATCH_SNAPSHOT_END).contains(&physical) {
                continue;
            }
            let snapshot_index = (physical - BR2_CODE_PATCH_SNAPSHOT_START) as usize;
            self.br2_code_patch_snapshot[snapshot_index] = byte;
            self.br2_code_patch_snapshot_valid[snapshot_index] = true;
        }

        self.br2_code_patch_snapshot_frozen = self
            .br2_code_patch_snapshot_valid
            .iter()
            .all(|valid| *valid);
    }

    #[allow(dead_code)]
    fn br2_code_patch_snapshot_read(&self, address: u32, len: usize) -> Option<Vec<u8>> {
        if !self.br2_code_patch_snapshot_frozen || len == 0 {
            return None;
        }

        let start = physical_address(address);
        let end = start.checked_add(len as u32)?;
        if start < BR2_CODE_PATCH_SNAPSHOT_START || end > BR2_CODE_PATCH_SNAPSHOT_END {
            return None;
        }

        let offset = (start - BR2_CODE_PATCH_SNAPSHOT_START) as usize;
        let end_offset = offset.checked_add(len)?;
        Some(self.br2_code_patch_snapshot[offset..end_offset].to_vec())
    }

    fn br2_code_patch_snapshot_read_value(&self, address: u32, len: usize) -> Option<u32> {
        if !self.br2_code_patch_snapshot_frozen || len == 0 || len > 4 {
            return None;
        }

        let start = physical_address(address);
        let end = start.checked_add(len as u32)?;
        if start < BR2_CODE_PATCH_SNAPSHOT_START || end > BR2_CODE_PATCH_SNAPSHOT_END {
            return None;
        }

        let offset = (start - BR2_CODE_PATCH_SNAPSHOT_START) as usize;
        let end_offset = offset.checked_add(len)?;
        Some(bytes_to_le_u32(
            &self.br2_code_patch_snapshot[offset..end_offset],
        ))
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
        let write_record = self.primitive_ram_writes.record(
            physical,
            bytes_to_le_u32(bytes),
            self.trace_pc.get(),
            self.vblank_count,
            self.trace_cycles.get(),
        );
        if write_record.header_like {
            self.primitive_header_generation = self.primitive_header_generation.saturating_add(1);
        }
        if write_record.command_like {
            self.stale_unlinked_primitive_replay_candidates.key = None;
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeBoardAssets {
    pub cat702_1: Option<[u8; 8]>,
    pub cat702_2: Option<[u8; 8]>,
    pub at28c16: Option<Vec<u8>>,
    pub at28c16_blank_default: bool,
    pub legacy_zinc_input_compat: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct NativeBoardAssetStatus {
    cat702_1_loaded: bool,
    cat702_2_loaded: bool,
    at28c16_loaded: bool,
    at28c16_blank_default: bool,
    legacy_zinc_input_compat: bool,
}

impl NativeBoardAssetStatus {
    fn from_assets(assets: &NativeBoardAssets) -> Self {
        Self {
            cat702_1_loaded: assets.cat702_1.is_some(),
            cat702_2_loaded: assets.cat702_2.is_some(),
            at28c16_loaded: assets.at28c16.is_some(),
            at28c16_blank_default: assets.at28c16_blank_default,
            legacy_zinc_input_compat: assets.legacy_zinc_input_compat,
        }
    }

    fn json(self) -> String {
        format!(
            "{{\"cat702_1_loaded\":{},\"cat702_2_loaded\":{},\"at28c16_loaded\":{},\"at28c16_blank_default\":{},\"legacy_zinc_input_compat\":{}}}",
            self.cat702_1_loaded,
            self.cat702_2_loaded,
            self.at28c16_loaded,
            self.at28c16_blank_default,
            self.legacy_zinc_input_compat
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
struct ZnBoardInputReadEvent {
    vblank: u64,
    cycles: u64,
    pc: Option<u32>,
    address: u32,
    width: u8,
    value: u32,
    input: ActionButtons,
}

impl ZnBoardInputReadEvent {
    fn json(&self) -> String {
        format!(
            "{{\"vblank\":{},\"cycles\":{},\"pc\":{},\"pc_hex\":{},\"address\":{},\"address_hex\":\"0x{:08x}\",\"physical_address\":{},\"physical_address_hex\":\"0x{:08x}\",\"width\":{},\"value\":{},\"value_hex\":\"0x{:08x}\",\"input\":{}}}",
            self.vblank,
            self.cycles,
            optional_u32_json(self.pc),
            optional_u32_hex_json(self.pc),
            self.address,
            self.address,
            physical_address(self.address),
            physical_address(self.address),
            self.width,
            self.value,
            self.value,
            self.input.json()
        )
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ZnBoardInputReadStats {
    ports: [ZnBoardInputPortReadStats; ZN_BOARD_INPUT_READ_PORTS.len()],
}

impl ZnBoardInputReadStats {
    fn record(&mut self, event: ZnBoardInputReadEvent) {
        let Some(index) = zn_input_port_index(event.address) else {
            return;
        };
        self.ports[index].record(event);
    }

    fn json(&self) -> String {
        let ports = self
            .ports
            .iter()
            .enumerate()
            .map(|(index, stats)| stats.json(ZN_BOARD_INPUT_READ_PORTS[index]))
            .collect::<Vec<_>>()
            .join(",");
        format!("{{\"ports\":[{}]}}", ports)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ZnBoardInputPortReadStats {
    reads: u64,
    active_reads: u64,
    start_active_reads: u64,
    coin_active_reads: u64,
    service_active_reads: u64,
    up_active_reads: u64,
    down_active_reads: u64,
    left_active_reads: u64,
    right_active_reads: u64,
    punch_active_reads: u64,
    kick_active_reads: u64,
    beast_active_reads: u64,
    guard_active_reads: u64,
    first_active_vblank: Option<u64>,
    last_read_vblank: Option<u64>,
    last_active_vblank: Option<u64>,
    last_start_active_vblank: Option<u64>,
    last_coin_active_vblank: Option<u64>,
    last_service_active_vblank: Option<u64>,
    last_width: u8,
    last_value: u32,
    last_active_value: Option<u32>,
    last_start_active_value: Option<u32>,
    last_coin_active_value: Option<u32>,
    last_service_active_value: Option<u32>,
    last_pc: Option<u32>,
}

impl ZnBoardInputPortReadStats {
    fn record(&mut self, event: ZnBoardInputReadEvent) {
        self.reads = self.reads.saturating_add(1);
        self.last_read_vblank = Some(event.vblank);
        self.last_width = event.width;
        self.last_value = event.value;
        self.last_pc = event.pc;

        if action_buttons_have_any_input(event.input) {
            self.active_reads = self.active_reads.saturating_add(1);
            self.first_active_vblank.get_or_insert(event.vblank);
            self.last_active_vblank = Some(event.vblank);
            self.last_active_value = Some(event.value);
        }

        self.record_button(event.input.start, &mut |stats| {
            stats.start_active_reads = stats.start_active_reads.saturating_add(1);
            stats.last_start_active_vblank = Some(event.vblank);
            stats.last_start_active_value = Some(event.value);
        });
        self.record_button(event.input.coin, &mut |stats| {
            stats.coin_active_reads = stats.coin_active_reads.saturating_add(1);
            stats.last_coin_active_vblank = Some(event.vblank);
            stats.last_coin_active_value = Some(event.value);
        });
        self.record_button(event.input.service, &mut |stats| {
            stats.service_active_reads = stats.service_active_reads.saturating_add(1);
            stats.last_service_active_vblank = Some(event.vblank);
            stats.last_service_active_value = Some(event.value);
        });
        self.up_active_reads = self
            .up_active_reads
            .saturating_add(u64_from_bool(event.input.up));
        self.down_active_reads = self
            .down_active_reads
            .saturating_add(u64_from_bool(event.input.down));
        self.left_active_reads = self
            .left_active_reads
            .saturating_add(u64_from_bool(event.input.left));
        self.right_active_reads = self
            .right_active_reads
            .saturating_add(u64_from_bool(event.input.right));
        self.punch_active_reads = self
            .punch_active_reads
            .saturating_add(u64_from_bool(event.input.punch));
        self.kick_active_reads = self
            .kick_active_reads
            .saturating_add(u64_from_bool(event.input.kick));
        self.beast_active_reads = self
            .beast_active_reads
            .saturating_add(u64_from_bool(event.input.beast));
        self.guard_active_reads = self
            .guard_active_reads
            .saturating_add(u64_from_bool(event.input.guard));
    }

    fn record_button<F>(&mut self, active: bool, update: &mut F)
    where
        F: FnMut(&mut Self),
    {
        if active {
            update(self);
        }
    }

    fn json(&self, address: u32) -> String {
        format!(
            "{{\"address\":{},\"address_hex\":\"0x{:08x}\",\"label\":\"{}\",\"reads\":{},\"active_reads\":{},\"start_active_reads\":{},\"coin_active_reads\":{},\"service_active_reads\":{},\"up_active_reads\":{},\"down_active_reads\":{},\"left_active_reads\":{},\"right_active_reads\":{},\"punch_active_reads\":{},\"kick_active_reads\":{},\"beast_active_reads\":{},\"guard_active_reads\":{},\"first_active_vblank\":{},\"last_read_vblank\":{},\"last_active_vblank\":{},\"last_start_active_vblank\":{},\"last_coin_active_vblank\":{},\"last_service_active_vblank\":{},\"last_width\":{},\"last_value\":{},\"last_value_hex\":\"0x{:08x}\",\"last_active_value\":{},\"last_active_value_hex\":{},\"last_start_active_value\":{},\"last_start_active_value_hex\":{},\"last_coin_active_value\":{},\"last_coin_active_value_hex\":{},\"last_service_active_value\":{},\"last_service_active_value_hex\":{},\"last_pc\":{},\"last_pc_hex\":{}}}",
            address,
            address,
            zn_input_port_label(address),
            self.reads,
            self.active_reads,
            self.start_active_reads,
            self.coin_active_reads,
            self.service_active_reads,
            self.up_active_reads,
            self.down_active_reads,
            self.left_active_reads,
            self.right_active_reads,
            self.punch_active_reads,
            self.kick_active_reads,
            self.beast_active_reads,
            self.guard_active_reads,
            optional_u64_json(self.first_active_vblank),
            optional_u64_json(self.last_read_vblank),
            optional_u64_json(self.last_active_vblank),
            optional_u64_json(self.last_start_active_vblank),
            optional_u64_json(self.last_coin_active_vblank),
            optional_u64_json(self.last_service_active_vblank),
            self.last_width,
            self.last_value,
            self.last_value,
            optional_u32_json(self.last_active_value),
            optional_u32_hex_json(self.last_active_value),
            optional_u32_json(self.last_start_active_value),
            optional_u32_hex_json(self.last_start_active_value),
            optional_u32_json(self.last_coin_active_value),
            optional_u32_hex_json(self.last_coin_active_value),
            optional_u32_json(self.last_service_active_value),
            optional_u32_hex_json(self.last_service_active_value),
            optional_u32_json(self.last_pc),
            optional_u32_hex_json(self.last_pc)
        )
    }
}

fn push_recent_zn_input_event(
    events: &RefCell<Vec<ZnBoardInputReadEvent>>,
    event: ZnBoardInputReadEvent,
    limit: usize,
) {
    if limit == 0 {
        return;
    }

    let mut events = events.borrow_mut();
    events.push(event);
    if events.len() > limit {
        let overflow = events.len() - limit;
        events.drain(0..overflow);
    }
}

fn zn_input_events_json(events: &[ZnBoardInputReadEvent]) -> String {
    events
        .iter()
        .map(ZnBoardInputReadEvent::json)
        .collect::<Vec<_>>()
        .join(",")
}

fn zn_input_tail_events_json(events: &[ZnBoardInputReadEvent], limit: usize) -> String {
    let start = events.len().saturating_sub(limit);
    zn_input_events_json(&events[start..])
}

fn is_zn_input_read_address(address: u32) -> bool {
    zn_input_port_index(address).is_some()
}

fn zn_input_port_index(address: u32) -> Option<usize> {
    let base = physical_address(address) & !0x03;
    ZN_BOARD_INPUT_READ_PORTS
        .iter()
        .position(|candidate| *candidate == base)
}

fn zn_input_port_label(address: u32) -> &'static str {
    match physical_address(address) & !0x03 {
        0x1fa0_0000 => "p1",
        0x1fa0_0200 => "service",
        0x1fa0_0300 => "system",
        0x1fa1_0000 => "p3",
        0x1fa2_0000 => "coin_register",
        _ => "unknown",
    }
}

fn action_buttons_have_any_input(buttons: ActionButtons) -> bool {
    buttons.start
        || buttons.coin
        || buttons.service
        || buttons.up
        || buttons.down
        || buttons.left
        || buttons.right
        || buttons.punch
        || buttons.kick
        || buttons.beast
        || buttons.guard
}

fn u64_from_bool(value: bool) -> u64 {
    if value { 1 } else { 0 }
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

fn dma_transfer_end_address(start: u32, words: u32, control: u32) -> Option<u32> {
    if words == 0 {
        return None;
    }

    let byte_delta = words.saturating_sub(1).saturating_mul(4);
    Some(if control & DMA_STEP_DECREMENT != 0 {
        start.wrapping_sub(byte_delta) & 0x00ff_fffc
    } else {
        start.wrapping_add(byte_delta) & 0x00ff_fffc
    })
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

fn looks_like_textured_primitive_opcode(opcode: u8) -> bool {
    matches!(opcode, 0x24..=0x27 | 0x2c..=0x2f | 0x34..=0x37 | 0x3c..=0x3f | 0x64..=0x67 | 0x74..=0x77 | 0x7c..=0x7f)
}

fn gp0_command_has_playfield_draw_bounds(words: &[u32]) -> bool {
    let Some(bounds) = gp0_command_draw_bounds(words) else {
        return false;
    };

    bounds.has_visible_x && bounds.max_y >= 96 && bounds.min_y <= 430 && bounds.min_y < 420
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Gp0ReplayDrawRejectReason {
    NoBounds,
    NonPlayfieldBounds,
    UnsafeBounds,
    ZeroTextureOversize,
    CommandTextureContamination,
    PrimitivePointerContamination,
    LinkedListArtifact,
    TitleOverlayAtlas,
}

fn gp0_command_has_replay_safe_draw_bounds(words: &[u32]) -> bool {
    gp0_command_replay_draw_reject_reason(words).is_none()
}

fn gp0_command_replay_draw_reject_reason(words: &[u32]) -> Option<Gp0ReplayDrawRejectReason> {
    let Some(command) = words.first() else {
        return Some(Gp0ReplayDrawRejectReason::NoBounds);
    };
    if !looks_like_draw_primitive_opcode((command >> 24) as u8) {
        return None;
    }
    let opcode = (words[0] >> 24) as u8;
    let Some(bounds) = gp0_command_draw_bounds(words) else {
        return Some(Gp0ReplayDrawRejectReason::NoBounds);
    };

    if !gp0_command_has_playfield_draw_bounds(words) {
        return Some(Gp0ReplayDrawRejectReason::NonPlayfieldBounds);
    }

    // Unlinked replay scans normal RAM for command-looking packets. Keep this
    // stricter than linked-list DMA so random command-shaped data cannot paint
    // giant off-screen quads over the real frame.
    if bounds.min_x < -256 || bounds.max_x > 896 || bounds.min_y < -96 || bounds.max_y > 640 {
        return Some(Gp0ReplayDrawRejectReason::UnsafeBounds);
    }
    if bounds.width() > 768 || bounds.height() > 576 || bounds.area() > 360_000 {
        return Some(Gp0ReplayDrawRejectReason::UnsafeBounds);
    }
    if matches!(opcode, 0x64..=0x67)
        && (bounds.width() > 256 || bounds.height() > 256 || bounds.area() > 65_536)
    {
        return Some(Gp0ReplayDrawRejectReason::UnsafeBounds);
    }
    if bounds.zero_vertices > 0 && bounds.area() > 40_000 {
        return Some(Gp0ReplayDrawRejectReason::UnsafeBounds);
    }
    if looks_like_textured_primitive_opcode(opcode)
        && bounds.zero_texture_words > 0
        && bounds.min_y <= 0
        && bounds.min_x <= 0
        && (bounds.width() > 320 || bounds.height() > 240 || bounds.area() > 40_000)
    {
        return Some(Gp0ReplayDrawRejectReason::ZeroTextureOversize);
    }
    if bounds.command_like_texture_words > 0 && bounds.zero_vertices > 0 && bounds.area() > 32_000 {
        return Some(Gp0ReplayDrawRejectReason::CommandTextureContamination);
    }
    if looks_like_textured_primitive_opcode(opcode) && bounds.primitive_pointer_words > 0 {
        return Some(Gp0ReplayDrawRejectReason::PrimitivePointerContamination);
    }
    if gp0_command_is_linked_list_artifact_draw(words) {
        return Some(Gp0ReplayDrawRejectReason::LinkedListArtifact);
    }
    if gp0_command_is_title_overlay_atlas_replay_artifact(words, bounds) {
        return Some(Gp0ReplayDrawRejectReason::TitleOverlayAtlas);
    }

    None
}

fn record_replay_draw_rejects(words: &[u32], diagnostics: &mut UnlinkedPrimitiveReplayDiagnostics) {
    let mut offset = 0;
    while offset < words.len() {
        let Some(command_words) = gp0_command_word_count(&words[offset..]) else {
            break;
        };
        if command_words == 0 || offset + command_words > words.len() {
            break;
        }
        let end = offset + command_words;
        if let Some(command) = words.get(offset)
            && looks_like_draw_primitive_opcode((command >> 24) as u8)
            && let Some(reason) = gp0_command_replay_draw_reject_reason(&words[offset..end])
        {
            diagnostics.record_reject_reason(reason);
        }
        offset = end;
    }
}

fn gp0_command_is_replay_safe_draw(words: &[u32]) -> bool {
    let Some(command) = words.first() else {
        return false;
    };
    looks_like_draw_primitive_opcode((command >> 24) as u8)
        && gp0_command_has_replay_safe_draw_bounds(words)
}

fn gp0_command_is_linked_list_artifact_draw(words: &[u32]) -> bool {
    let Some(command) = words.first() else {
        return false;
    };
    let opcode = (command >> 24) as u8;
    let Some(bounds) = gp0_command_draw_bounds(words) else {
        return false;
    };

    let left_edge_tall = bounds.width() <= 160
        && bounds.min_x <= 0
        && bounds.min_y <= 0
        && bounds.max_x <= 128
        && bounds.max_y >= 420
        && bounds.height() > 300;

    left_edge_tall
        && ((!looks_like_textured_primitive_opcode(opcode) && bounds.zero_vertices > 0)
            || (looks_like_textured_primitive_opcode(opcode)
                && (bounds.zero_texture_words > 0 || bounds.command_like_texture_words > 0)))
        || gp0_command_is_high_page_title_stripe_artifact(words, bounds)
}

fn gp0_command_is_high_page_title_stripe_artifact(words: &[u32], bounds: Gp0DrawBounds) -> bool {
    let Some(command) = words.first() else {
        return false;
    };
    let opcode = (command >> 24) as u8;
    if !looks_like_textured_primitive_opcode(opcode) || bounds.command_like_texture_words == 0 {
        return false;
    }

    let Some((texture_page, clut)) = gp0_command_texture_descriptor(words) else {
        return false;
    };
    let zn_high_8bpp_page = texture_page & 0x2000 != 0 && ((texture_page >> 7) & 0x03) == 1;
    let title_clut_band = (0x7800..=0x7fff).contains(&clut);
    if !zn_high_8bpp_page || !title_clut_band {
        return false;
    }

    let has_left_edge_anchor = gp0_command_vertex_words(words)
        .unwrap_or_default()
        .into_iter()
        .any(|word| {
            let (x, y) = gp0_signed_xy(word);
            x == 0 && (224..=360).contains(&y)
        });
    let texture_words = gp0_command_texture_words(words).unwrap_or_default();
    let has_captured_stream_contamination = words.contains(&0x0900_0000)
        && texture_words
            .iter()
            .any(|word| (*word & 0xffff_0000) == 0x2e80_0000);

    has_left_edge_anchor
        && has_captured_stream_contamination
        && bounds.min_x <= 16
        && bounds.max_x >= 384
        && (72..=260).contains(&bounds.min_y)
        && (224..=360).contains(&bounds.max_y)
        && bounds.width() >= 384
        && bounds.height() >= 60
        && bounds.area() > 25_000
}

fn gp0_command_is_title_overlay_atlas_replay_artifact(
    words: &[u32],
    bounds: Gp0DrawBounds,
) -> bool {
    let Some((texture_page, clut)) = gp0_command_texture_descriptor(words) else {
        return false;
    };

    let high_texture_page = matches!(texture_page & 0x003f, 0x0039 | 0x003f);
    let title_overlay_clut = (0x7800..=0x7fff).contains(&clut);
    let centered_overlay_region = (180..=330).contains(&bounds.min_x)
        && (180..=340).contains(&bounds.max_x)
        && (200..=340).contains(&bounds.min_y)
        && (220..=360).contains(&bounds.max_y);
    let compact_overlay = bounds.area() <= 12_000;
    let upper_right_sparkle_region = (340..=430).contains(&bounds.min_x)
        && (348..=448).contains(&bounds.max_x)
        && (96..=180).contains(&bounds.min_y)
        && (104..=188).contains(&bounds.max_y);
    let tiny_sparkle = bounds.width() <= 16 && bounds.height() <= 16 && bounds.area() <= 256;

    (high_texture_page && title_overlay_clut && centered_overlay_region && compact_overlay)
        || ((texture_page & 0x003f) == 0x003f
            && clut == 0x7818
            && upper_right_sparkle_region
            && tiny_sparkle)
}

fn gp0_replay_safe_draw_command_ranges(words: &[u32]) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut offset = 0;
    let mut pending_state_start = None;
    while offset < words.len() {
        let Some(command_words) = gp0_command_word_count(&words[offset..]) else {
            break;
        };
        if command_words == 0 || offset + command_words > words.len() {
            break;
        }
        let end = offset + command_words;

        if gp0_command_is_replay_safe_state(&words[offset..end]) {
            pending_state_start.get_or_insert(offset);
            offset = end;
            continue;
        }

        if gp0_command_is_replay_safe_draw(&words[offset..end]) {
            let start = pending_state_start.take().unwrap_or(offset);
            ranges.push(start..end);
        } else {
            pending_state_start = None;
        }
        offset = end;
    }
    ranges
}

fn gp0_replay_safe_state_command_ranges(words: &[u32]) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut offset = 0;
    while offset < words.len() {
        let Some(command_words) = gp0_command_word_count(&words[offset..]) else {
            break;
        };
        if command_words == 0 || offset + command_words > words.len() {
            break;
        }
        let end = offset + command_words;
        if gp0_command_is_replay_safe_state(&words[offset..end]) {
            ranges.push(offset..end);
        }
        offset = end;
    }
    ranges
}

fn gp0_command_is_replay_safe_state(words: &[u32]) -> bool {
    if words.len() != 1 {
        return false;
    }

    let command = words[0];
    match command >> 24 {
        0xe1 | 0xe2 | 0xe3 | 0xe4 | 0xe6 => true,
        0xe5 => {
            let (x, y) = gp0_drawing_offset_xy(command);
            (-512..=512).contains(&x) && (-512..=512).contains(&y)
        }
        _ => false,
    }
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

fn gp0_command_texture_words(words: &[u32]) -> Option<Vec<u32>> {
    let opcode = (*words.first()? >> 24) as u8;
    let texture_words = match opcode {
        0x24..=0x27 if words.len() >= 7 => vec![words[2], words[4], words[6]],
        0x2c..=0x2f if words.len() >= 9 => vec![words[2], words[4], words[6], words[8]],
        0x34..=0x37 if words.len() >= 9 => vec![words[2], words[5], words[8]],
        0x3c..=0x3f if words.len() >= 12 => {
            vec![words[2], words[5], words[8], words[11]]
        }
        _ => Vec::new(),
    };
    Some(texture_words)
}

fn gp0_command_texture_descriptor(words: &[u32]) -> Option<(u16, u16)> {
    let texture_words = gp0_command_texture_words(words)?;
    if texture_words.len() < 2 {
        return None;
    }

    let clut = (texture_words[0] >> 16) as u16;
    let texture_page = (texture_words[1] >> 16) as u16;
    Some((texture_page, clut))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Gp0DrawBounds {
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    has_visible_x: bool,
    zero_vertices: usize,
    zero_texture_words: usize,
    command_like_texture_words: usize,
    primitive_pointer_words: usize,
}

impl Gp0DrawBounds {
    fn width(self) -> i32 {
        self.max_x.saturating_sub(self.min_x).saturating_add(1)
    }

    fn height(self) -> i32 {
        self.max_y.saturating_sub(self.min_y).saturating_add(1)
    }

    fn area(self) -> i32 {
        self.width().saturating_mul(self.height())
    }
}

fn gp0_command_draw_bounds(words: &[u32]) -> Option<Gp0DrawBounds> {
    let opcode = (*words.first()? >> 24) as u8;
    let points = gp0_command_vertex_words(words)?;
    if points.is_empty() {
        return None;
    }

    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    let mut has_visible_x = false;
    let mut zero_vertices = 0usize;
    let mut record_point = |word| {
        let (x, y) = gp0_signed_xy(word);
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
        has_visible_x |= (-64..=576).contains(&x);
        if x == 0 && y == 0 {
            zero_vertices = zero_vertices.saturating_add(1);
        }
    };

    if (0x60..=0x7f).contains(&opcode) {
        let (x, y) = gp0_signed_xy(points[0]);
        let (width, height) = gp0_command_rect_dimensions(opcode, words)?;
        let max_rect_x = x.saturating_add(width.saturating_sub(1));
        let max_rect_y = y.saturating_add(height.saturating_sub(1));
        record_point(points[0]);
        min_x = min_x.min(max_rect_x);
        max_x = max_x.max(max_rect_x);
        min_y = min_y.min(max_rect_y);
        max_y = max_y.max(max_rect_y);
        has_visible_x |= x <= 576 && max_rect_x >= -64;
    } else {
        for word in points {
            record_point(word);
        }
    }
    let texture_words = gp0_command_texture_words(words)?;
    let zero_texture_words = texture_words.iter().filter(|word| **word == 0).count();
    let command_like_texture_words = texture_words
        .iter()
        .filter(|word| looks_like_gp0_command_opcode(((**word) >> 24) as u8))
        .count();
    let primitive_pointer_words = words
        .iter()
        .skip(1)
        .filter(|word| looks_like_primitive_ram_pointer_word(**word))
        .count();

    Some(Gp0DrawBounds {
        min_x,
        max_x,
        min_y,
        max_y,
        has_visible_x,
        zero_vertices,
        zero_texture_words,
        command_like_texture_words,
        primitive_pointer_words,
    })
}

fn gp0_command_rect_dimensions(opcode: u8, words: &[u32]) -> Option<(i32, i32)> {
    match opcode {
        0x60..=0x63 if words.len() >= 3 => gp0_variable_rect_dimensions(words[2]),
        0x64..=0x67 if words.len() >= 4 => gp0_variable_rect_dimensions(words[3]),
        0x68..=0x6f => Some((1, 1)),
        0x70..=0x77 => Some((8, 8)),
        0x78..=0x7f => Some((16, 16)),
        _ => None,
    }
}

fn gp0_variable_rect_dimensions(word: u32) -> Option<(i32, i32)> {
    let width = (word & 0xffff) as i32;
    let height = (word >> 16) as i32;
    (width > 0 && height > 0).then_some((width, height))
}

fn looks_like_primitive_ram_pointer_word(value: u32) -> bool {
    matches!(value & 0xe000_0000, 0x8000_0000 | 0xa000_0000)
        && (BR2_PRIMITIVE_RAM_START..BR2_PRIMITIVE_RAM_END).contains(&physical_address(value))
}

fn gp0_signed_xy(value: u32) -> (i32, i32) {
    (
        sign_extend_11_bits(value & 0x07ff),
        sign_extend_11_bits((value >> 16) & 0x07ff),
    )
}

fn gp0_drawing_offset_xy(value: u32) -> (i32, i32) {
    (
        sign_extend_11_bits(value & 0x07ff),
        sign_extend_11_bits((value >> 11) & 0x07ff),
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
    diagnostics: UnlinkedPrimitiveReplayDiagnostics,
}

impl UnlinkedPrimitiveReplayDecision {
    fn enabled(reason: &'static str, candidate_headers: usize) -> Self {
        Self {
            enabled: true,
            reason,
            candidate_headers,
            diagnostics: UnlinkedPrimitiveReplayDiagnostics::default(),
        }
    }

    fn disabled(reason: &'static str, candidate_headers: usize) -> Self {
        Self {
            enabled: false,
            reason,
            candidate_headers,
            diagnostics: UnlinkedPrimitiveReplayDiagnostics::default(),
        }
    }

    fn with_diagnostics(mut self, diagnostics: UnlinkedPrimitiveReplayDiagnostics) -> Self {
        self.diagnostics = diagnostics;
        self
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

fn br2_stale_unlinked_primitive_scan_allowed() -> bool {
    std::env::var_os("BR2_NATIVE_ENABLE_STALE_UNLINKED_PRIMITIVE_SCAN").is_some()
        && std::env::var_os("BR2_NATIVE_DISABLE_STALE_UNLINKED_PRIMITIVE_REPLAY").is_none()
}

fn native_unlinked_primitive_replay_interval_override() -> Option<Option<u64>> {
    let value = std::env::var(BR2_UNLINKED_PRIMITIVE_REPLAY_INTERVAL_ENV).ok()?;
    parse_native_unlinked_primitive_replay_interval_override(&value)
}

fn native_replay_validation_debug_min_vblank() -> Option<u64> {
    std::env::var("BR2_NATIVE_REPLAY_VALIDATION_DEBUG_MIN_VBLANK")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
}

fn parse_native_unlinked_primitive_replay_interval_override(value: &str) -> Option<Option<u64>> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("off")
        || value.eq_ignore_ascii_case("false")
        || value.eq_ignore_ascii_case("disable")
        || value.eq_ignore_ascii_case("disabled")
        || value.eq_ignore_ascii_case("none")
        || value == "0"
    {
        return Some(None);
    }

    value
        .parse::<u64>()
        .ok()
        .map(|interval| if interval == 0 { None } else { Some(interval) })
}

fn cache_isolated_primitive_ram_write_passthrough_allowed() -> bool {
    std::env::var_os("BR2_NATIVE_CACHE_ISOLATED_PRIMITIVE_RAM_WRITETHROUGH").is_some()
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
            | 0x1fa2_0000..=0x1fa2_0003
            | 0x1fa3_0000..=0x1fa3_0003
            | 0x1fa4_0000..=0x1fa4_0003
            | 0x1fa6_0000..=0x1fa6_0001
            | 0x1faf_0000..=0x1faf_07ff
            | 0x1fb0_0004
            | 0x1fb2_0000..=0x1fb2_0007
    )
}

#[derive(Clone, Debug)]
struct ZnBoard {
    rom_bank: u8,
    znsecsel: u8,
    znsecsel_writes: u64,
    coin: u8,
    coin_register_writes: u64,
    recent_coin_register_writes: [CoinRegisterWriteEvent; ZN_BOARD_RECENT_COIN_REGISTER_WRITES],
    recent_coin_register_write_count: usize,
    recent_coin_register_write_cursor: usize,
    coin_input_latched: bool,
    coin_insert_edges: u64,
    coin_counter_0: bool,
    coin_counter_1: bool,
    coin_counter_0_edges: u64,
    coin_counter_1_edges: u64,
    coin_lockout_0: bool,
    coin_lockout_1: bool,
    legacy_coin_read_latch: Cell<u8>,
    legacy_system_coin_latch_reads: Cell<u8>,
    legacy_system_start_latch_reads: Cell<u8>,
    legacy_system_coin_latch_edges: u64,
    legacy_system_start_latch_edges: u64,
    native_credit_adapter_pending_writes: Cell<u8>,
    native_credit_adapter_edge_projection_writes: Cell<u8>,
    native_credit_adapter_active: bool,
    native_credit_adapter_writes: u64,
    native_credit_adapter_edges: u64,
    native_credit_adapter_last_raw_value: u32,
    native_credit_adapter_last_value: u32,
    native_credit_adapter_last_pc: Option<u32>,
    native_credit_adapter_last_vblank: u64,
    native_credit_adapter_last_cycles: u64,
    native_credit_adapter_input_bit: u32,
    native_credit_projection_rules: Vec<NativeCreditProjectionRule>,
    sound_irq_latch: u8,
    at28c16: [u8; 2048],
    at28c16_reads: Cell<u64>,
    at28c16_writes: u64,
    last_at28c16_read_offset: Cell<Option<u16>>,
    last_at28c16_read_value: Cell<u8>,
    last_at28c16_write_offset: Option<u16>,
    last_at28c16_write_value: u8,
    zn2_spu_hack: Cell<u16>,
    zn2_spu_hack_reads: Cell<u64>,
    legacy_zinc_input_compat: bool,
    coin_input_mapping: NativeCoinInputMapping,
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
    system_service_active_reads: Cell<u64>,
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
        Self::with_at28c16_and_compat(default_at28c16, false)
    }

    fn with_board_assets(assets: &NativeBoardAssets) -> Self {
        Self::with_at28c16_and_compat(assets.at28c16.clone(), assets.legacy_zinc_input_compat)
    }

    fn with_at28c16_and_compat(
        default_at28c16: Option<Vec<u8>>,
        legacy_zinc_input_compat: bool,
    ) -> Self {
        let mut at28c16 = [0xff; 2048];
        if let Some(bytes) = default_at28c16 {
            let len = bytes.len().min(at28c16.len());
            at28c16[..len].copy_from_slice(&bytes[..len]);
        }
        Self {
            rom_bank: 0,
            znsecsel: 0,
            znsecsel_writes: 0,
            coin: 0,
            coin_register_writes: 0,
            recent_coin_register_writes: [CoinRegisterWriteEvent::default();
                ZN_BOARD_RECENT_COIN_REGISTER_WRITES],
            recent_coin_register_write_count: 0,
            recent_coin_register_write_cursor: 0,
            coin_input_latched: false,
            coin_insert_edges: 0,
            coin_counter_0: false,
            coin_counter_1: false,
            coin_counter_0_edges: 0,
            coin_counter_1_edges: 0,
            coin_lockout_0: false,
            coin_lockout_1: false,
            legacy_coin_read_latch: Cell::new(0),
            legacy_system_coin_latch_reads: Cell::new(0),
            legacy_system_start_latch_reads: Cell::new(0),
            legacy_system_coin_latch_edges: 0,
            legacy_system_start_latch_edges: 0,
            native_credit_adapter_pending_writes: Cell::new(0),
            native_credit_adapter_edge_projection_writes: Cell::new(0),
            native_credit_adapter_active: false,
            native_credit_adapter_writes: 0,
            native_credit_adapter_edges: 0,
            native_credit_adapter_last_raw_value: 0,
            native_credit_adapter_last_value: 0,
            native_credit_adapter_last_pc: None,
            native_credit_adapter_last_vblank: 0,
            native_credit_adapter_last_cycles: 0,
            native_credit_adapter_input_bit: native_credit_adapter_input_bit_from_env(),
            native_credit_projection_rules: native_credit_projection_rules_from_env(),
            sound_irq_latch: 0,
            at28c16,
            at28c16_reads: Cell::new(0),
            at28c16_writes: 0,
            last_at28c16_read_offset: Cell::new(None),
            last_at28c16_read_value: Cell::new(0),
            last_at28c16_write_offset: None,
            last_at28c16_write_value: 0,
            zn2_spu_hack: Cell::new(0),
            zn2_spu_hack_reads: Cell::new(0),
            legacy_zinc_input_compat,
            coin_input_mapping: NativeCoinInputMapping::from_env(legacy_zinc_input_compat),
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
            system_service_active_reads: Cell::new(0),
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
        let base = board_register_base(address);
        board_read_lane(self.read_base_u32(base), base, address, access_len)
    }

    fn write(
        &mut self,
        address: u32,
        value: u32,
        access_len: usize,
        pc: Option<u32>,
        vblank: u64,
        cycles: u64,
    ) {
        let base = board_register_base(address);
        let merged = board_write_lane(
            self.read_base_u32_for_write_merge(base),
            base,
            address,
            value,
            access_len,
        );
        if physical_address(base) == 0x1fa2_0000 {
            self.record_coin_register_write(address, access_len, value, merged, pc, vblank, cycles);
        }
        self.write_base_u32(base, merged);
    }

    fn read_base_u32(&self, address: u32) -> u32 {
        let physical = physical_address(address);
        match physical {
            0x1fa0_0000 => self.read_player1_input(),
            0x1fa0_0100 => mirrored_input_port(active_low_player2_input()),
            0x1fa0_0200 => self.read_service_input(),
            0x1fa0_0300 => self.read_system_input(),
            0x1fa1_0000 => self.read_player3_input(),
            0x1fa1_0100 => mirrored_input_port(active_low_player4_input()),
            0x1fa1_0200 => 0x0000_0069,
            0x1fa1_0300 => self.znsecsel as u32,
            0x1fa2_0000 => self.read_coin_register(),
            0x1fa3_0000 | 0x1fa4_0000 => 0,
            0x1fa6_0000 => self.read_zn2_spu_hack() as u32,
            0x1faf_0000..=0x1faf_07ff => {
                let offset = (physical - 0x1faf_0000) as usize;
                self.at28c16_reads
                    .set(self.at28c16_reads.get().saturating_add(1));
                self.last_at28c16_read_offset.set(Some(offset as u16));
                self.last_at28c16_read_value.set(self.at28c16[offset]);
                self.at28c16[offset] as u32
            }
            0x1fb0_0004 => self.sound_irq_latch as u32,
            0x1fb2_0000..=0x1fb2_0007 => 0xffff,
            _ => 0,
        }
    }

    fn read_base_u32_for_write_merge(&self, address: u32) -> u32 {
        let physical = physical_address(address);
        match physical {
            0x1fa0_0000 => mirrored_input_port(active_low_player1_input(
                self.input,
                self.legacy_input_compat_active(),
            )),
            0x1fa0_0100 => mirrored_input_port(active_low_player2_input()),
            0x1fa0_0200 => mirrored_input_port(active_low_service_input(
                self.input,
                self.coin_input_mapping,
            )),
            0x1fa0_0300 => mirrored_input_port(active_low_system_input(
                self.input,
                self.legacy_input_compat_active(),
                self.coin_input_mapping,
            )),
            0x1fa1_0000 => mirrored_input_port(active_low_player3_input(self.input)),
            0x1fa1_0100 => mirrored_input_port(active_low_player4_input()),
            0x1fa1_0200 => 0x0000_0069,
            0x1fa1_0300 => self.znsecsel as u32,
            0x1fa2_0000 => self.coin as u32,
            0x1fa3_0000 | 0x1fa4_0000 => 0,
            0x1fa6_0000 => self.zn2_spu_hack.get() as u32,
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
                self.znsecsel_writes = self.znsecsel_writes.saturating_add(1);
                self.rom_bank = self.znsecsel & 0x03;
            }
            0x1fa2_0000 => self.write_coin_register(value as u8),
            0x1faf_0000..=0x1faf_07ff => {
                let offset = (physical - 0x1faf_0000) as usize;
                let byte = value as u8;
                self.at28c16[offset] = byte;
                self.at28c16_writes = self.at28c16_writes.saturating_add(1);
                self.last_at28c16_write_offset = Some(offset as u16);
                self.last_at28c16_write_value = byte;
            }
            0x1fb0_0004 => self.sound_irq_latch = value as u8,
            _ => {}
        }
    }

    fn write_coin_register(&mut self, data: u8) {
        self.coin = data;
        self.coin_register_writes = self.coin_register_writes.saturating_add(1);

        let counter_0 = data & 0x01 != 0;
        let counter_1 = data & 0x10 != 0;
        if counter_0 && !self.coin_counter_0 {
            self.coin_counter_0_edges = self.coin_counter_0_edges.saturating_add(1);
        }
        if counter_1 && !self.coin_counter_1 {
            self.coin_counter_1_edges = self.coin_counter_1_edges.saturating_add(1);
        }
        self.coin_counter_0 = counter_0;
        self.coin_counter_1 = counter_1;
        self.coin_lockout_0 = data & 0x02 != 0;
        self.coin_lockout_1 = data & 0x20 != 0;
    }

    fn record_coin_register_write(
        &mut self,
        address: u32,
        access_len: usize,
        raw_value: u32,
        merged_value: u32,
        pc: Option<u32>,
        vblank: u64,
        cycles: u64,
    ) {
        let event = CoinRegisterWriteEvent {
            vblank,
            cycles,
            pc,
            address,
            access_len,
            raw_value,
            merged_value,
            data: merged_value as u8,
        };
        self.recent_coin_register_writes[self.recent_coin_register_write_cursor] = event;
        self.recent_coin_register_write_cursor =
            (self.recent_coin_register_write_cursor + 1) % ZN_BOARD_RECENT_COIN_REGISTER_WRITES;
        self.recent_coin_register_write_count = self
            .recent_coin_register_write_count
            .saturating_add(1)
            .min(ZN_BOARD_RECENT_COIN_REGISTER_WRITES);
    }

    fn recent_coin_register_writes_json(&self) -> String {
        let count = self.recent_coin_register_write_count;
        let start = if count == ZN_BOARD_RECENT_COIN_REGISTER_WRITES {
            self.recent_coin_register_write_cursor
        } else {
            0
        };
        (0..count)
            .map(|offset| {
                let index = (start + offset) % ZN_BOARD_RECENT_COIN_REGISTER_WRITES;
                self.recent_coin_register_writes[index].json()
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    fn last_coin_register_write_json(&self) -> String {
        if self.recent_coin_register_write_count == 0 {
            return "null".to_string();
        }
        let index = self
            .recent_coin_register_write_cursor
            .checked_sub(1)
            .unwrap_or(ZN_BOARD_RECENT_COIN_REGISTER_WRITES - 1);
        self.recent_coin_register_writes[index].json()
    }

    fn native_credit_projection_rules_json(&self) -> String {
        self.native_credit_projection_rules
            .iter()
            .copied()
            .map(NativeCreditProjectionRule::json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn board_diagnostics_json(&self) -> String {
        format!(
            "{{\"coin_input_mapping\":\"{}\",\"znsecsel_writes\":{},\"coin_register_writes\":{},\"last_coin_register_write\":{},\"recent_coin_register_writes\":[{}],\"coin_counter_0\":{},\"coin_counter_1\":{},\"coin_counter_0_edges\":{},\"coin_counter_1_edges\":{},\"coin_lockout_0\":{},\"coin_lockout_1\":{},\"legacy_system_coin_latch_reads\":{},\"legacy_system_start_latch_reads\":{},\"legacy_system_coin_latch_edges\":{},\"legacy_system_start_latch_edges\":{},\"native_credit_adapter_pending_writes\":{},\"native_credit_adapter_edge_projection_writes\":{},\"native_credit_adapter_active\":{},\"native_credit_adapter_input_bit\":{},\"native_credit_adapter_input_bit_hex\":\"0x{:08x}\",\"native_credit_projection_rules\":[{}],\"native_credit_adapter_writes\":{},\"native_credit_adapter_edges\":{},\"native_credit_adapter_last_raw_value\":{},\"native_credit_adapter_last_raw_value_hex\":\"0x{:08x}\",\"native_credit_adapter_last_value\":{},\"native_credit_adapter_last_value_hex\":\"0x{:08x}\",\"native_credit_adapter_last_pc\":{},\"native_credit_adapter_last_pc_hex\":{},\"native_credit_adapter_last_vblank\":{},\"native_credit_adapter_last_cycles\":{},\"at28c16_reads\":{},\"at28c16_writes\":{},\"last_at28c16_read_offset\":{},\"last_at28c16_read_value\":{},\"last_at28c16_read_value_hex\":\"0x{:02x}\",\"last_at28c16_write_offset\":{},\"last_at28c16_write_value\":{},\"last_at28c16_write_value_hex\":\"0x{:02x}\",\"zn2_spu_hack_reads\":{},\"zn2_spu_hack_value\":{},\"zn2_spu_hack_value_hex\":\"0x{:04x}\"}}",
            self.coin_input_mapping.name(),
            self.znsecsel_writes,
            self.coin_register_writes,
            self.last_coin_register_write_json(),
            self.recent_coin_register_writes_json(),
            self.coin_counter_0,
            self.coin_counter_1,
            self.coin_counter_0_edges,
            self.coin_counter_1_edges,
            self.coin_lockout_0,
            self.coin_lockout_1,
            self.legacy_system_coin_latch_reads.get(),
            self.legacy_system_start_latch_reads.get(),
            self.legacy_system_coin_latch_edges,
            self.legacy_system_start_latch_edges,
            self.native_credit_adapter_pending_writes.get(),
            self.native_credit_adapter_edge_projection_writes.get(),
            self.native_credit_adapter_active,
            self.native_credit_adapter_input_bit,
            self.native_credit_adapter_input_bit,
            self.native_credit_projection_rules_json(),
            self.native_credit_adapter_writes,
            self.native_credit_adapter_edges,
            self.native_credit_adapter_last_raw_value,
            self.native_credit_adapter_last_raw_value,
            self.native_credit_adapter_last_value,
            self.native_credit_adapter_last_value,
            optional_u32_json(self.native_credit_adapter_last_pc),
            optional_u32_hex_json(self.native_credit_adapter_last_pc),
            self.native_credit_adapter_last_vblank,
            self.native_credit_adapter_last_cycles,
            self.at28c16_reads.get(),
            self.at28c16_writes,
            optional_u32_json(self.last_at28c16_read_offset.get().map(u32::from)),
            self.last_at28c16_read_value.get(),
            self.last_at28c16_read_value.get(),
            optional_u32_json(self.last_at28c16_write_offset.map(u32::from)),
            self.last_at28c16_write_value,
            self.last_at28c16_write_value,
            self.zn2_spu_hack_reads.get(),
            self.zn2_spu_hack.get(),
            self.zn2_spu_hack.get()
        )
    }

    fn json(&self) -> String {
        format!(
            "{{\"rom_bank\":{},\"znsecsel\":{},\"coin_input_mapping\":\"{}\",\"coin\":{},\"coin_insert_edges\":{},\"legacy_system_coin_latch_reads\":{},\"legacy_system_start_latch_reads\":{},\"legacy_system_coin_latch_edges\":{},\"legacy_system_start_latch_edges\":{},\"native_credit_adapter_writes\":{},\"native_credit_adapter_edges\":{},\"sound_irq_latch\":{},\"zn2_spu_hack_reads\":{},\"zn2_spu_hack_value\":{},\"zn2_spu_hack_value_hex\":\"0x{:04x}\",\"p1_input_reads\":{},\"p1_up_active_reads\":{},\"p1_down_active_reads\":{},\"p1_left_active_reads\":{},\"p1_right_active_reads\":{},\"p1_start_active_reads\":{},\"p1_punch_active_reads\":{},\"p1_kick_active_reads\":{},\"p1_beast_active_reads\":{},\"p3_input_reads\":{},\"p3_guard_active_reads\":{},\"system_input_reads\":{},\"system_coin_active_reads\":{},\"system_service_active_reads\":{},\"system_start_active_reads\":{},\"coin_register_reads\":{},\"coin_register_active_reads\":{},\"last_p1_input\":{},\"last_p1_input_hex\":\"0x{:08x}\",\"last_p3_input\":{},\"last_p3_input_hex\":\"0x{:08x}\",\"last_system_input\":{},\"last_system_input_hex\":\"0x{:08x}\",\"last_coin_register\":{},\"last_coin_register_hex\":\"0x{:08x}\"}}",
            self.rom_bank,
            self.znsecsel,
            self.coin_input_mapping.name(),
            self.coin,
            self.coin_insert_edges,
            self.legacy_system_coin_latch_reads.get(),
            self.legacy_system_start_latch_reads.get(),
            self.legacy_system_coin_latch_edges,
            self.legacy_system_start_latch_edges,
            self.native_credit_adapter_writes,
            self.native_credit_adapter_edges,
            self.sound_irq_latch,
            self.zn2_spu_hack_reads.get(),
            self.zn2_spu_hack.get(),
            self.zn2_spu_hack.get(),
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
            self.system_service_active_reads.get(),
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
            "{{\"rom_bank\":{},\"znsecsel\":{},\"znsecsel_hex\":\"0x{:02x}\",\"coin_input_mapping\":\"{}\",\"cat702_1_select_line\":{},\"cat702_1_selected\":{},\"cat702_2_select_line\":{},\"cat702_2_selected\":{},\"zn_mcu_analog_read\":{},\"zn_mcu_trackball_read\":{},\"zn_mcu_selected\":{},\"coin\":{},\"coin_hex\":\"0x{:02x}\",\"coin_insert_edges\":{},\"legacy_coin_read_latch\":{},\"legacy_coin_read_latch_hex\":\"0x{:02x}\",\"legacy_system_coin_latch_reads\":{},\"legacy_system_start_latch_reads\":{},\"legacy_system_coin_latch_edges\":{},\"legacy_system_start_latch_edges\":{},\"native_credit_adapter_writes\":{},\"native_credit_adapter_edges\":{},\"zn2_spu_hack_reads\":{},\"zn2_spu_hack_value\":{},\"zn2_spu_hack_value_hex\":\"0x{:04x}\",\"p1_input_reads\":{},\"p1_start_active_reads\":{},\"p1_punch_active_reads\":{},\"p1_kick_active_reads\":{},\"p1_beast_active_reads\":{},\"p3_input_reads\":{},\"p3_guard_active_reads\":{},\"system_input_reads\":{},\"system_coin_active_reads\":{},\"system_service_active_reads\":{},\"system_start_active_reads\":{},\"coin_register_reads\":{},\"coin_register_active_reads\":{},\"last_p1_input_hex\":\"0x{:08x}\",\"last_p3_input_hex\":\"0x{:08x}\",\"last_system_input_hex\":\"0x{:08x}\",\"last_coin_register_hex\":\"0x{:08x}\",\"diagnostics\":{}}}",
            self.rom_bank,
            self.znsecsel,
            self.znsecsel,
            self.coin_input_mapping.name(),
            self.cat702_1_select(),
            !self.cat702_1_select(),
            self.cat702_2_select(),
            !self.cat702_2_select(),
            self.zn_mcu_analog_read(),
            self.zn_mcu_trackball_read(),
            self.zn_mcu_selected(),
            self.coin,
            self.coin,
            self.coin_insert_edges,
            self.legacy_coin_read_latch.get(),
            self.legacy_coin_read_latch.get(),
            self.legacy_system_coin_latch_reads.get(),
            self.legacy_system_start_latch_reads.get(),
            self.legacy_system_coin_latch_edges,
            self.legacy_system_start_latch_edges,
            self.native_credit_adapter_writes,
            self.native_credit_adapter_edges,
            self.zn2_spu_hack_reads.get(),
            self.zn2_spu_hack.get(),
            self.zn2_spu_hack.get(),
            self.p1_input_reads.get(),
            self.p1_start_active_reads.get(),
            self.p1_punch_active_reads.get(),
            self.p1_kick_active_reads.get(),
            self.p1_beast_active_reads.get(),
            self.p3_input_reads.get(),
            self.p3_guard_active_reads.get(),
            self.system_input_reads.get(),
            self.system_coin_active_reads.get(),
            self.system_service_active_reads.get(),
            self.system_start_active_reads.get(),
            self.coin_register_reads.get(),
            self.coin_register_active_reads.get(),
            self.last_p1_input.get(),
            self.last_p3_input.get(),
            self.last_system_input.get(),
            self.last_coin_register.get(),
            self.board_diagnostics_json()
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
        if self.znsecsel_writes == 0 {
            return false;
        }
        // MAME routes `(data & 0x8c) != 0x8c` to the MCU select line itself.
        // The MCU device treats that line as active-low, so the device is
        // selected only when those bits produce a low line.
        self.znsecsel & 0x8c == 0x8c
    }

    fn set_input(&mut self, input: ActionButtons) {
        if input.coin && !self.coin_input_latched {
            self.coin_insert_edges = self.coin_insert_edges.saturating_add(1);
            if self.legacy_input_compat_active() {
                self.legacy_coin_read_latch
                    .set(self.legacy_coin_read_latch.get() | 0x01);
                self.legacy_system_coin_latch_reads
                    .set(LEGACY_ZINC_SYSTEM_EDGE_LATCH_READS);
                self.legacy_system_coin_latch_edges =
                    self.legacy_system_coin_latch_edges.saturating_add(1);
            }
        }
        if input.start && !self.input.start && self.legacy_input_compat_active() {
            self.legacy_system_start_latch_reads
                .set(LEGACY_ZINC_SYSTEM_EDGE_LATCH_READS);
            self.legacy_system_start_latch_edges =
                self.legacy_system_start_latch_edges.saturating_add(1);
        }
        self.coin_input_latched = input.coin;
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
            system_service_active_reads: self.system_service_active_reads.get(),
            system_start_active_reads: self.system_start_active_reads.get(),
            coin_register_reads: self.coin_register_reads.get(),
            coin_register_active_reads: self.coin_register_active_reads.get(),
            coin_register_writes: self.coin_register_writes,
            coin_insert_edges: self.coin_insert_edges,
            coin_counter_0_edges: self.coin_counter_0_edges,
            coin_counter_1_edges: self.coin_counter_1_edges,
            legacy_system_coin_latch_edges: self.legacy_system_coin_latch_edges,
            legacy_system_start_latch_edges: self.legacy_system_start_latch_edges,
            native_credit_adapter_writes: self.native_credit_adapter_writes,
            native_credit_adapter_edges: self.native_credit_adapter_edges,
            last_system_input: self.last_system_input.get(),
            last_coin_register: self.last_coin_register.get(),
        }
    }

    fn set_coin_input_mapping_name(&mut self, value: &str) -> bool {
        let Some(mapping) = NativeCoinInputMapping::parse(value) else {
            return false;
        };
        self.coin_input_mapping = mapping;
        true
    }

    fn set_native_credit_adapter_input_bit(&mut self, value: u32) -> bool {
        if value == 0 {
            return false;
        }
        self.native_credit_adapter_input_bit = value;
        true
    }

    fn set_native_credit_projection_name(&mut self, value: &str) -> bool {
        let Some(rules) = parse_native_credit_projection_rules(value) else {
            return false;
        };
        self.native_credit_projection_rules = rules;
        true
    }

    fn read_player1_input(&self) -> u32 {
        let input = self.effective_legacy_system_input();
        let value = active_low_player1_input(input, self.legacy_input_compat_active());
        self.p1_input_reads
            .set(self.p1_input_reads.get().saturating_add(1));
        self.count_active_player1_inputs(input);
        self.last_p1_input.set(value);
        self.decay_legacy_start_latch_after_read();
        mirrored_input_port(value)
    }

    fn count_active_player1_inputs(&self, input: ActionButtons) {
        increment_if(&self.p1_up_active_reads, input.up);
        increment_if(&self.p1_down_active_reads, input.down);
        increment_if(&self.p1_left_active_reads, input.left);
        increment_if(&self.p1_right_active_reads, input.right);
        increment_if(&self.p1_start_active_reads, input.start);
        increment_if(&self.p1_punch_active_reads, input.punch);
        increment_if(&self.p1_kick_active_reads, input.kick);
        increment_if(&self.p1_beast_active_reads, input.beast);
    }

    fn read_player3_input(&self) -> u32 {
        let value = active_low_player3_input(self.input);
        self.p3_input_reads
            .set(self.p3_input_reads.get().saturating_add(1));
        increment_if(&self.p3_guard_active_reads, self.input.guard);
        self.last_p3_input.set(value);
        mirrored_input_port(value)
    }

    fn read_service_input(&self) -> u32 {
        let input = self.effective_legacy_system_input();
        self.decay_legacy_coin_latch_after_read();
        mirrored_input_port(active_low_service_input(input, self.coin_input_mapping))
    }

    fn read_system_input(&self) -> u32 {
        let input = self.effective_legacy_system_input();
        let value = active_low_system_input(
            input,
            self.legacy_input_compat_active(),
            self.coin_input_mapping,
        );
        self.system_input_reads
            .set(self.system_input_reads.get().saturating_add(1));
        if input.coin {
            self.system_coin_active_reads
                .set(self.system_coin_active_reads.get().saturating_add(1));
            self.native_credit_adapter_pending_writes
                .set(BR2_NATIVE_CREDIT_ADAPTER_PENDING_WRITES);
        }
        if input.service {
            self.system_service_active_reads
                .set(self.system_service_active_reads.get().saturating_add(1));
        }
        if input.start {
            self.system_start_active_reads
                .set(self.system_start_active_reads.get().saturating_add(1));
        }
        self.last_system_input.set(value);
        self.decay_legacy_coin_latch_after_read();
        self.decay_legacy_start_latch_after_read();
        mirrored_input_port(value)
    }

    fn native_credit_adapter_scratchpad_write_value(
        &mut self,
        physical_scratchpad_address: u32,
        raw_value: u32,
        access_len: usize,
        pc: Option<u32>,
        vblank: u64,
        cycles: u64,
    ) -> Option<u32> {
        let physical_pc = pc.map(physical_address);
        let rule = self
            .native_credit_projection_rules
            .iter()
            .copied()
            .find(|rule| rule.matches(physical_scratchpad_address, access_len, physical_pc))?;
        let pending = match rule.bucket {
            NativeCreditProjectionBucket::Current => {
                self.native_credit_adapter_pending_writes.get()
            }
            NativeCreditProjectionBucket::Edge => {
                self.native_credit_adapter_edge_projection_writes.get()
            }
        };
        if pending == 0 {
            if matches!(rule.bucket, NativeCreditProjectionBucket::Current) {
                self.native_credit_adapter_active = false;
                self.native_credit_adapter_edge_projection_writes.set(0);
            }
            return None;
        }

        match rule.bucket {
            NativeCreditProjectionBucket::Current => self
                .native_credit_adapter_pending_writes
                .set(pending.saturating_sub(1)),
            NativeCreditProjectionBucket::Edge => self
                .native_credit_adapter_edge_projection_writes
                .set(pending.saturating_sub(1)),
        }
        let input_bit = rule.effective_mask(self.native_credit_adapter_input_bit)
            & native_credit_access_len_mask(access_len);
        let value = raw_value | input_bit;
        self.native_credit_adapter_writes = self.native_credit_adapter_writes.saturating_add(1);
        if !self.native_credit_adapter_active
            && matches!(rule.bucket, NativeCreditProjectionBucket::Current)
        {
            self.native_credit_adapter_edges = self.native_credit_adapter_edges.saturating_add(1);
            self.native_credit_adapter_edge_projection_writes
                .set(BR2_NATIVE_CREDIT_ADAPTER_EDGE_PROJECTION_WRITES);
        }
        self.native_credit_adapter_active = true;
        self.native_credit_adapter_last_raw_value = raw_value;
        self.native_credit_adapter_last_value = value;
        self.native_credit_adapter_last_pc = pc;
        self.native_credit_adapter_last_vblank = vblank;
        self.native_credit_adapter_last_cycles = cycles;
        Some(value)
    }

    fn read_coin_register(&self) -> u32 {
        let mut value = self.coin as u32;
        if self.coin_input_mapping.mirrors_coin_register_bit0() {
            if self.input.coin {
                value |= 0x01;
            }
            value |= u32::from(self.legacy_coin_read_latch.get());
        }
        self.coin_register_reads
            .set(self.coin_register_reads.get().saturating_add(1));
        if value != 0 {
            self.coin_register_active_reads
                .set(self.coin_register_active_reads.get().saturating_add(1));
        }
        self.last_coin_register.set(value);
        if self.coin_input_mapping.mirrors_coin_register_bit0() {
            self.legacy_coin_read_latch.set(0);
            return mirrored_input_port(value);
        }
        value
    }

    fn read_zn2_spu_hack(&self) -> u16 {
        let value = self.zn2_spu_hack.get() ^ 0x0008;
        self.zn2_spu_hack.set(value);
        self.zn2_spu_hack_reads
            .set(self.zn2_spu_hack_reads.get().saturating_add(1));
        value
    }

    fn effective_legacy_system_input(&self) -> ActionButtons {
        if !self.legacy_input_compat_active() {
            return self.input;
        }

        ActionButtons {
            coin: self.input.coin || self.legacy_system_coin_latch_reads.get() > 0,
            start: self.input.start || self.legacy_system_start_latch_reads.get() > 0,
            ..self.input
        }
    }

    fn decay_legacy_coin_latch_after_read(&self) {
        decay_legacy_latch_after_read(&self.legacy_system_coin_latch_reads, self.input.coin);
    }

    fn decay_legacy_start_latch_after_read(&self) {
        decay_legacy_latch_after_read(&self.legacy_system_start_latch_reads, self.input.start);
    }

    fn legacy_input_compat_active(&self) -> bool {
        self.legacy_zinc_input_compat || self.coin_input_mapping.enables_legacy_start_compat()
    }
}

fn native_credit_adapter_input_bit_from_env() -> u32 {
    env::var(BR2_NATIVE_CREDIT_INPUT_BIT_ENV)
        .ok()
        .and_then(|value| parse_native_u32_env_value(&value))
        .filter(|value| *value != 0)
        .unwrap_or(BR2_NATIVE_CREDIT_INPUT_BIT)
}

fn native_credit_access_len_mask(access_len: usize) -> u32 {
    match access_len {
        1 => 0x0000_00ff,
        2 => 0x0000_ffff,
        _ => u32::MAX,
    }
}

fn parse_native_u32_env_value(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u32::from_str_radix(hex, 16).ok();
    }
    trimmed.parse::<u32>().ok()
}

fn mirrored_input_port(value: u32) -> u32 {
    let byte = value & 0xff;
    byte | (byte << 8) | (byte << 16) | (byte << 24)
}

fn active_low_player1_input(input: ActionButtons, legacy_zinc_input_compat: bool) -> u32 {
    let mut value = 0xffff_ffff;
    clear_bit_if(&mut value, 0x0000_0001, input.up);
    clear_bit_if(&mut value, 0x0000_0002, input.down);
    clear_bit_if(&mut value, 0x0000_0004, input.left);
    clear_bit_if(&mut value, 0x0000_0008, input.right);
    clear_bit_if(&mut value, 0x0000_0010, input.punch);
    clear_bit_if(&mut value, 0x0000_0020, input.kick);
    clear_bit_if(&mut value, 0x0000_0040, input.beast);
    clear_bit_if(
        &mut value,
        0x0000_0080,
        input.start && legacy_zinc_input_compat,
    );
    value
}

fn active_low_player2_input() -> u32 {
    0xffff_ffff
}

fn active_low_player3_input(input: ActionButtons) -> u32 {
    let mut value = 0xffff_ffff;
    // Bloody Roar 2 maps P3 bit 0x10 as P1 button 4; bit 0x20 is P2 button 4.
    clear_bit_if(&mut value, 0x0000_0010, input.guard);
    value
}

fn active_low_player4_input() -> u32 {
    0xffff_ffff
}

fn active_low_service_input(input: ActionButtons, coin_mapping: NativeCoinInputMapping) -> u32 {
    let mut value = 0xffff_ffff;
    clear_bit_if(&mut value, 0x0000_0001, input.service);
    clear_bit_if(&mut value, 0x0000_0002, input.service);
    clear_bit_if(
        &mut value,
        0x0000_0001,
        input.coin && coin_mapping.clears_service_01(),
    );
    clear_bit_if(
        &mut value,
        0x0000_0002,
        input.coin && coin_mapping.clears_service_02(),
    );
    clear_bit_if(
        &mut value,
        0x0000_0010,
        input.coin && coin_mapping.clears_service_10(),
    );
    value
}

fn active_low_system_input(
    input: ActionButtons,
    legacy_zinc_input_compat: bool,
    coin_mapping: NativeCoinInputMapping,
) -> u32 {
    let mut value = 0xffff_ffff;
    clear_bit_if(&mut value, 0x0000_0001, input.start);
    clear_bit_if(
        &mut value,
        0x0000_0002,
        input.start && legacy_zinc_input_compat,
    );
    clear_bit_if(
        &mut value,
        0x0000_0010,
        input.coin && coin_mapping.clears_system_10(),
    );
    clear_bit_if(
        &mut value,
        0x0000_0020,
        input.coin && coin_mapping.clears_system_20(),
    );
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

fn decay_legacy_latch_after_read(latch: &Cell<u8>, raw_button_active: bool) {
    if raw_button_active {
        return;
    }
    let remaining = latch.get();
    if remaining > 0 {
        latch.set(remaining - 1);
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
    let physical_base = physical_address(base);
    let physical_address = physical_address(address);
    let shifted = value >> ((physical_address - physical_base) * 8);
    match access_len {
        1 => shifted & 0xff,
        2 => shifted & 0xffff,
        _ => shifted,
    }
}

fn board_write_lane(current: u32, base: u32, address: u32, value: u32, access_len: usize) -> u32 {
    let physical_base = physical_address(base);
    let physical_address = physical_address(address);
    let shift = (physical_address - physical_base) * 8;
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

fn cache_isolated_write_suppression_disabled() -> bool {
    std::env::var_os("BR2_NATIVE_DISABLE_CACHE_ISOLATED_WRITE_SUPPRESSION").is_some()
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

fn dma_activity_range_bounds(sample: &DmaActivitySample) -> Option<(u32, u32)> {
    let start = sample.start?;
    let end = sample.end?;
    Some((start.min(end), start.max(end)))
}

fn distance_to_range(address: u32, low: u32, high: u32) -> u32 {
    if (low..=high).contains(&address) {
        0
    } else {
        address.abs_diff(low).min(address.abs_diff(high))
    }
}

fn optional_u32_json(value: Option<u32>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn optional_u64_json(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn ranges_json(ranges: &[std::ops::Range<usize>]) -> String {
    ranges
        .iter()
        .map(|range| format!("{{\"start\":{},\"end\":{}}}", range.start, range.end))
        .collect::<Vec<_>>()
        .join(",")
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
        BR2_BOOT_WORD_COPY_LOOP_PHYSICAL, BR2_CODE_PATCH_SNAPSHOT_LEN,
        BR2_CREDIT_PLAYER_MODE_OFFSET, BR2_CREDIT_REQUIRED_P1_OFFSET,
        BR2_CREDIT_REQUIRED_P2_OFFSET, BR2_CREDIT_SHARED_SLOT_OFFSET, BR2_CREDIT_STATE_BASE,
        BR2_DRAW_SYNC_FLAG_VIRTUAL, BR2_RUNTIME_CODE_SNAPSHOT_START,
        BR2_UNLINKED_PRIMITIVE_REPLAY_FULL_VALIDATION_COOLDOWN_VBLANKS,
        BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_DRAW_PACKETS,
        BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_RECENT_HEADERS,
        BR2_UNLINKED_PRIMITIVE_REPLAY_PACKET_LIMIT, BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW,
        Br2NativeCreditHleCheck, Bus, DMA_ACTIVITY_RECENT_LIMIT, DMA_GPU_COMPLETION_DELAY_CYCLES,
        DMA_MDEC_COMPLETION_DELAY_CYCLES, DMA_STEP_DECREMENT, GPU_LINKED_LIST_NODE_LIMIT,
        GPU_VBLANK_PRESENTATION_CAPTURE_INTERVAL, GpuLinkedListDmaRunStats, NativeBoardAssets,
        NativeInputActivity, PRIMITIVE_RAM_RECENT_LIMIT, UnlinkedPrimitiveReplayDiagnostics,
        draw_primitive_count, gp0_command_has_playfield_draw_bounds,
        gp0_command_is_linked_list_artifact_draw, gp0_command_is_replay_safe_draw,
        gp0_replay_safe_draw_command_ranges, gpu_linked_list_command_ranges,
        parse_native_unlinked_primitive_replay_interval_override,
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
    fn bus_vblank_presentation_capture_interval_is_configurable() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        assert_eq!(
            bus.vblank_presentation_capture_interval(),
            Some(GPU_VBLANK_PRESENTATION_CAPTURE_INTERVAL)
        );
        bus.vblank_count = 1;
        assert!(bus.should_capture_vblank_presented_frame());
        bus.vblank_count = GPU_VBLANK_PRESENTATION_CAPTURE_INTERVAL - 1;
        assert!(!bus.should_capture_vblank_presented_frame());
        bus.vblank_count = GPU_VBLANK_PRESENTATION_CAPTURE_INTERVAL;
        assert!(bus.should_capture_vblank_presented_frame());

        bus.set_vblank_presentation_capture_interval(Some(2));
        bus.vblank_count = 2;
        assert!(bus.should_capture_vblank_presented_frame());
        bus.vblank_count = 3;
        assert!(!bus.should_capture_vblank_presented_frame());

        bus.set_vblank_presentation_capture_interval(None);
        bus.vblank_count = 1;
        assert!(bus.should_capture_vblank_presented_frame());
        bus.vblank_count = 2;
        assert!(!bus.should_capture_vblank_presented_frame());

        bus.set_vblank_presentation_capture_interval(Some(0));
        assert_eq!(bus.vblank_presentation_capture_interval(), None);
    }

    #[test]
    fn bus_unlinked_primitive_replay_interval_is_configurable() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        assert_eq!(bus.unlinked_primitive_replay_interval(), None);
        bus.vblank_count = 17;
        assert!(bus.should_attempt_unlinked_primitive_replay());

        bus.set_unlinked_primitive_replay_interval(Some(4));
        bus.vblank_count = 8;
        assert!(bus.should_attempt_unlinked_primitive_replay());
        bus.vblank_count = 10;
        assert!(!bus.should_attempt_unlinked_primitive_replay());

        bus.set_unlinked_primitive_replay_interval(None);
        assert!(bus.should_attempt_unlinked_primitive_replay());
        bus.set_unlinked_primitive_replay_interval(Some(0));
        assert_eq!(bus.unlinked_primitive_replay_interval(), None);
        assert!(bus.should_attempt_unlinked_primitive_replay());
    }

    #[test]
    fn bus_unlinked_primitive_replay_reject_throttles_same_vblank_validation() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.set_unlinked_primitive_replay_interval(Some(1));
        bus.vblank_count = 24;
        let stats = GpuLinkedListDmaRunStats::started(0, 0);
        bus.unlinked_primitive_replay.record_skip(
            bus.vblank_count,
            "replay_rejected_after_validation",
            0,
            &stats,
            UnlinkedPrimitiveReplayDiagnostics::default(),
        );

        let decision = bus.unlinked_primitive_replay_decision(&stats);

        assert!(!decision.enabled);
        assert_eq!(decision.reason, "already_attempted_this_vblank");
    }

    #[test]
    fn bus_unlinked_primitive_replay_reject_throttles_nearby_validation() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.set_unlinked_primitive_replay_interval(Some(1));
        let stats = GpuLinkedListDmaRunStats::started(0, 0);
        bus.unlinked_primitive_replay.record_skip(
            20,
            "replay_rejected_after_validation",
            0,
            &stats,
            UnlinkedPrimitiveReplayDiagnostics::default(),
        );
        bus.vblank_count = 24;

        let decision = bus.unlinked_primitive_replay_decision(&stats);

        assert!(!decision.enabled);
        assert_eq!(decision.reason, "recent_validation_reject_cooldown");
    }

    #[test]
    fn bus_unlinked_primitive_replay_full_validation_has_cooldown() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.vblank_count = 24;
        assert!(bus.should_run_full_unlinked_primitive_replay_validation());

        bus.unlinked_primitive_replay.record_full_validation(20);
        bus.vblank_count = 24;
        assert!(!bus.should_run_full_unlinked_primitive_replay_validation());

        bus.vblank_count = 20 + BR2_UNLINKED_PRIMITIVE_REPLAY_FULL_VALIDATION_COOLDOWN_VBLANKS;
        assert!(bus.should_run_full_unlinked_primitive_replay_validation());
    }

    #[test]
    fn bus_unlinked_primitive_replay_full_validation_throttle_blocks_same_vblank_retry() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.set_unlinked_primitive_replay_interval(Some(1));
        bus.vblank_count = 24;
        let stats = GpuLinkedListDmaRunStats::started(0, 0);
        bus.unlinked_primitive_replay.record_skip(
            bus.vblank_count,
            "replay_full_validation_throttled",
            0,
            &stats,
            UnlinkedPrimitiveReplayDiagnostics::default(),
        );

        let decision = bus.unlinked_primitive_replay_decision(&stats);

        assert!(!decision.enabled);
        assert_eq!(decision.reason, "already_attempted_this_vblank");
    }

    #[test]
    fn bus_unlinked_primitive_replay_interval_env_values_parse() {
        assert_eq!(
            parse_native_unlinked_primitive_replay_interval_override("1"),
            Some(Some(1))
        );
        assert_eq!(
            parse_native_unlinked_primitive_replay_interval_override(" 4 "),
            Some(Some(4))
        );
        assert_eq!(
            parse_native_unlinked_primitive_replay_interval_override("off"),
            Some(None)
        );
        assert_eq!(
            parse_native_unlinked_primitive_replay_interval_override("0"),
            Some(None)
        );
        assert_eq!(
            parse_native_unlinked_primitive_replay_interval_override("invalid"),
            None
        );
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
        assert_eq!(p1 & 0x0011, 0);
        assert_eq!(p1 & 0x0080, 0x0080);
        assert_eq!((p1 >> 8) & 0x00ff, p1 & 0x00ff);
        let service = bus.read_u8(0x1fa0_0200);
        assert_eq!(service & 0x02, 0x02);
        assert_eq!(service & 0x10, 0x10);
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x01, 0);
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x10, 0);
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x20, 0x20);
        let p3 = bus.read_u8(0x1fa1_0000);
        assert_eq!(p3 & 0x10, 0);
        assert_eq!(p3 & 0x20, 0x20);
        assert_eq!(bus.read_u8(0x1fa2_0000) & 0x01, 0);
        let board_json = bus.zn_board_json();
        assert!(board_json.contains("\"p1_up_active_reads\":1"));
        assert!(board_json.contains("\"p1_start_active_reads\":1"));
        assert!(board_json.contains("\"system_start_active_reads\":3"));
        assert!(board_json.contains("\"system_coin_active_reads\":3"));
        assert!(board_json.contains("\"p1_punch_active_reads\":1"));
        assert!(board_json.contains("\"p3_guard_active_reads\":1"));
    }

    #[test]
    fn bus_initializes_controller_security_selects_from_board_lines() {
        let bus = Bus::with_board_assets(
            Vec::new(),
            Vec::new(),
            2 * 1024 * 1024,
            NativeBoardAssets {
                cat702_1: Some([0xff; 8]),
                cat702_2: Some([0xfe; 8]),
                ..NativeBoardAssets::default()
            },
        );

        let probe_json = bus.input_probe_json();
        assert!(probe_json.contains("\"cat702_1_selected\":true"));
        assert!(probe_json.contains("\"cat702_2_selected\":true"));
        assert!(probe_json.contains("\"zn_mcu_selected\":false"));
        assert!(probe_json.contains(
            "\"cat702\":[{\"index\":0,\"loaded\":true,\"select_line\":false,\"selected\":true"
        ));
        assert!(probe_json.contains("\"select_transitions\":1"));
    }

    #[test]
    fn zn_mcu_select_is_active_low_like_mame() {
        let mut bus = Bus::with_board_assets(
            Vec::new(),
            Vec::new(),
            2 * 1024 * 1024,
            NativeBoardAssets {
                cat702_1: Some([0xff; 8]),
                cat702_2: Some([0xfe; 8]),
                ..NativeBoardAssets::default()
            },
        );

        assert!(bus.input_probe_json().contains("\"zn_mcu_selected\":false"));
        bus.write_u8(0x1fa1_0300, 0x8c);

        let probe_json = bus.input_probe_json();
        assert!(probe_json.contains("\"zn_mcu_selected\":true"));
        assert!(probe_json.contains("\"zn_mcu\":{\"selected\":true"));

        bus.write_u8(0x1fa1_0300, 0x00);
        let probe_json = bus.input_probe_json();
        assert!(probe_json.contains("\"zn_mcu_selected\":false"));
        assert!(probe_json.contains("\"zn_mcu\":{\"selected\":false"));
    }

    #[test]
    fn znt2p_default_input_mapping_uses_mame_coin_start_bits_only() {
        let input = ActionButtons {
            start: true,
            coin: true,
            service: true,
            ..ActionButtons::default()
        };

        let p1 = super::active_low_player1_input(input, false);
        let service = super::active_low_service_input(input, super::NativeCoinInputMapping::Mame);
        let system =
            super::active_low_system_input(input, false, super::NativeCoinInputMapping::Mame);

        assert_eq!(p1 & 0x80, 0x80, "start is not mirrored onto P1 button 5");
        assert_eq!(service & 0x01, 0, "service clears test/service-mode bit");
        assert_eq!(service & 0x02, 0, "service clears service-button bit");
        assert_eq!(
            service & 0x10,
            0x10,
            "coin is not mirrored onto service coin bit"
        );
        assert_eq!(system & 0x01, 0, "start clears SYSTEM start1");
        assert_eq!(system & 0x02, 0x02, "start2 stays released");
        assert_eq!(system & 0x10, 0, "coin clears SYSTEM coin1");
        assert_eq!(system & 0x20, 0x20, "coin2 stays released");
    }

    #[test]
    fn znt2p_coin_mapping_modes_cover_diagnostic_candidates() {
        let input = ActionButtons {
            coin: true,
            ..ActionButtons::default()
        };

        let legacy_service =
            super::active_low_service_input(input, super::NativeCoinInputMapping::LegacyZinc);
        let legacy_system =
            super::active_low_system_input(input, true, super::NativeCoinInputMapping::LegacyZinc);
        assert_eq!(legacy_service & 0x02, 0);
        assert_eq!(legacy_service & 0x10, 0);
        assert_eq!(legacy_system & 0x10, 0);
        assert_eq!(legacy_system & 0x20, 0);

        let service01 =
            super::active_low_service_input(input, super::NativeCoinInputMapping::Service01);
        assert_eq!(service01 & 0x01, 0);
        assert_eq!(service01 & 0x02, 0x02);
        assert_eq!(service01 & 0x10, 0x10);

        let system20 =
            super::active_low_system_input(input, false, super::NativeCoinInputMapping::System20);
        assert_eq!(system20 & 0x10, 0x10);
        assert_eq!(system20 & 0x20, 0);

        assert_eq!(
            super::NativeCoinInputMapping::parse("coin-register-bit0"),
            Some(super::NativeCoinInputMapping::CoinRegisterBit0)
        );
        assert_eq!(
            super::NativeCoinInputMapping::parse("legacy_zinc"),
            Some(super::NativeCoinInputMapping::LegacyZinc)
        );
    }

    #[test]
    fn bus_zinc_legacy_assets_enable_compat_input_bits() {
        let mut bus = Bus::with_board_assets(
            Vec::new(),
            Vec::new(),
            2 * 1024 * 1024,
            NativeBoardAssets {
                legacy_zinc_input_compat: true,
                ..NativeBoardAssets::default()
            },
        );

        bus.set_input(ActionButtons {
            start: true,
            coin: true,
            ..ActionButtons::default()
        });

        let p1 = bus.read_u16(0x1fa0_0000);
        assert_eq!(p1 & 0x0080, 0);
        assert_eq!((p1 >> 8) & 0x00ff, p1 & 0x00ff);
        assert_eq!(bus.read_u8(0x1fa0_0200) & 0x02, 0);
        assert_eq!(bus.read_u8(0x1fa0_0200) & 0x10, 0);
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x01, 0);
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x02, 0);
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x10, 0);
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x20, 0);

        let board_json = bus.zn_board_json();
        assert!(board_json.contains("\"p1_start_active_reads\":1"));
        assert!(board_json.contains("\"legacy_zinc_input_compat\":true"));
    }

    #[test]
    fn explicit_legacy_zinc_mapping_enables_start_mirror_without_asset_compat() {
        let mut bus = Bus::with_board_assets(
            Vec::new(),
            Vec::new(),
            2 * 1024 * 1024,
            NativeBoardAssets::default(),
        );
        assert!(bus.set_coin_input_mapping_name("legacy-zinc"));

        bus.set_input(ActionButtons {
            start: true,
            coin: true,
            ..ActionButtons::default()
        });

        let p1 = bus.read_u8(0x1fa0_0000);
        let system = bus.read_u8(0x1fa0_0300);
        let board_json = bus.zn_board_json();

        assert_eq!(p1 & 0x80, 0, "explicit legacy mapping mirrors start to P1");
        assert_eq!(system & 0x02, 0, "explicit legacy mapping mirrors start2");
        assert!(board_json.contains("\"legacy_zinc_input_compat\":false"));
        assert!(board_json.contains("\"legacy_system_coin_latch_edges\":1"));
        assert!(board_json.contains("\"legacy_system_start_latch_edges\":1"));
    }

    #[test]
    fn bus_injects_br2_credit_slot_from_coin_edge_once() {
        let mut bus = Bus::with_board_assets(
            Vec::new(),
            Vec::new(),
            4 * 1024 * 1024,
            NativeBoardAssets::default(),
        );
        bus.write_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_PLAYER_MODE_OFFSET, 0);
        bus.write_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_REQUIRED_P1_OFFSET, 1);
        bus.write_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_REQUIRED_P2_OFFSET, 1);

        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });
        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });

        assert_eq!(
            bus.read_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_SHARED_SLOT_OFFSET),
            1
        );
        assert_eq!(bus.consume_br2_native_credit_hle_coin_edges(), 0);
        assert!(bus.br2_native_credit_hle_accepted_seen());
    }

    #[test]
    fn bus_injects_br2_credit_after_required_zero_hle_confirms_state() {
        let mut bus = Bus::with_board_assets(
            Vec::new(),
            Vec::new(),
            4 * 1024 * 1024,
            NativeBoardAssets::default(),
        );
        bus.write_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_PLAYER_MODE_OFFSET, 0);
        bus.write_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_REQUIRED_P1_OFFSET, 0);
        bus.write_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_REQUIRED_P2_OFFSET, 0);

        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });
        bus.set_input(ActionButtons::default());
        assert_eq!(
            bus.read_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_SHARED_SLOT_OFFSET),
            0,
            "required=0 alone is still treated as uninitialized"
        );

        bus.record_br2_native_credit_hle_check(Br2NativeCreditHleCheck {
            player: 0,
            freeplay: false,
            required: 0,
            credit_slot: BR2_CREDIT_STATE_BASE + BR2_CREDIT_SHARED_SLOT_OFFSET,
            credit_before: 0,
            credit_after: 0,
            pending_coin_edges: 0,
            result: 0,
        });
        bus.set_input(ActionButtons::default());

        assert_eq!(
            bus.read_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_SHARED_SLOT_OFFSET),
            1
        );
        assert_eq!(bus.consume_br2_native_credit_hle_coin_edges(), 0);
        assert!(bus.br2_native_credit_hle_accepted_seen());
    }

    #[test]
    fn bus_injects_br2_credit_after_required_zero_adapter_confirms_input_loop() {
        let mut bus = Bus::with_board_assets(
            Vec::new(),
            Vec::new(),
            4 * 1024 * 1024,
            NativeBoardAssets::default(),
        );
        bus.write_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_PLAYER_MODE_OFFSET, 0);
        bus.write_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_REQUIRED_P1_OFFSET, 0);
        bus.write_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_REQUIRED_P2_OFFSET, 0);
        bus.zn_board.native_credit_adapter_writes = 1;

        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });
        bus.set_input(ActionButtons::default());

        assert_eq!(
            bus.read_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_SHARED_SLOT_OFFSET),
            1
        );
        assert_eq!(bus.consume_br2_native_credit_hle_coin_edges(), 0);
        assert!(bus.br2_native_credit_hle_accepted_seen());
    }

    #[test]
    fn bus_does_not_inject_br2_credit_before_price_state_is_initialized() {
        let mut bus = Bus::with_board_assets(
            Vec::new(),
            Vec::new(),
            4 * 1024 * 1024,
            NativeBoardAssets::default(),
        );

        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });
        bus.set_input(ActionButtons::default());

        assert_eq!(
            bus.read_u8(BR2_CREDIT_STATE_BASE + BR2_CREDIT_SHARED_SLOT_OFFSET),
            0
        );
        assert_eq!(bus.consume_br2_native_credit_hle_coin_edges(), 1);
        assert!(!bus.br2_native_credit_hle_accepted_seen());
    }

    #[test]
    fn board_input_reads_use_register_base_before_lane_selection() {
        let mut bus = Bus::with_board_assets(
            Vec::new(),
            Vec::new(),
            2 * 1024 * 1024,
            NativeBoardAssets {
                legacy_zinc_input_compat: true,
                ..NativeBoardAssets::default()
            },
        );

        bus.set_input(ActionButtons {
            start: true,
            coin: true,
            punch: true,
            ..ActionButtons::default()
        });

        assert_eq!(bus.read_u16(0x1fa0_0000) & 0x0010, 0);
        assert_eq!(bus.read_u8(0x1fa0_0001) & 0x10, 0);
        let system = bus.read_u16(0x1fa0_0300);
        assert_eq!(system & 0x0011, 0);
        assert_eq!(system & 0x0022, 0);
        let system_mirror = bus.read_u8(0x1fa0_0301);
        assert_eq!(system_mirror & 0x11, 0);
        assert_eq!(system_mirror & 0x22, 0);
        assert_eq!(bus.read_u8(0x9fa0_0301) & 0x11, 0);
        assert_eq!(bus.read_u8(0x9fa0_0301) & 0x22, 0);
        assert_eq!(bus.read_u8(0xbfa0_0301) & 0x11, 0);
        assert_eq!(bus.read_u8(0xbfa0_0301) & 0x22, 0);

        let board_json = bus.zn_board_json();
        assert!(board_json.contains("\"p1_input_reads\":2"), "{board_json}");
        assert!(
            board_json.contains("\"system_input_reads\":6"),
            "{board_json}"
        );
    }

    #[test]
    fn zn2_spu_hack_register_toggles_on_halfword_reads() {
        let bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        assert_eq!(bus.read_u16(0x1fa6_0000), 0x0008);
        assert_eq!(bus.read_u16(0x1fa6_0000), 0x0000);
        assert_eq!(bus.read_u16(0x1fa6_0000), 0x0008);

        let board_json = bus.zn_board_json();
        assert!(board_json.contains("\"zn2_spu_hack_reads\":3"));
        assert!(board_json.contains("\"zn2_spu_hack_value_hex\":\"0x0008\""));
    }

    #[test]
    fn zn2_spu_hack_register_supports_byte_lanes_without_write_side_effects() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        assert_eq!(bus.read_u8(0x1fa6_0000), 0x08);
        assert_eq!(bus.read_u8(0x1fa6_0001), 0x00);
        bus.write_u16(0x1fa6_0000, 0xffff);

        assert_eq!(bus.read_u16(0x1fa6_0000), 0x0008);
        let probe_json = bus.runtime_probe_json();
        assert!(probe_json.contains("\"zn2_spu_hack_reads\":3"));
        assert!(probe_json.contains("\"zn2_spu_hack_value_hex\":\"0x0008\""));
    }

    #[test]
    fn board_input_read_summary_tracks_active_buttons_by_port() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.set_input(ActionButtons {
            start: true,
            coin: true,
            ..ActionButtons::default()
        });

        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x11, 0);
        assert_eq!(bus.read_u8(0x1fa2_0000), 0);

        let probe_json = bus.input_probe_json();
        assert!(probe_json.contains("\"summary\""));
        assert!(probe_json.contains(
            "\"label\":\"system\",\"reads\":1,\"active_reads\":1,\"start_active_reads\":1,\"coin_active_reads\":1"
        ));
        assert!(probe_json.contains(
            "\"label\":\"coin_register\",\"reads\":1,\"active_reads\":1,\"start_active_reads\":1,\"coin_active_reads\":1"
        ));
        assert!(probe_json.contains("\"last_coin_active_value_hex\":\"0x00000000\""));
        assert!(probe_json.contains("\"recent_active\""));
    }

    #[test]
    fn compact_input_probe_keeps_port_summary_without_recent_event_arrays() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.set_input(ActionButtons {
            start: true,
            coin: true,
            ..ActionButtons::default()
        });

        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x11, 0);

        let probe_json = bus.input_compact_probe_json();
        assert!(probe_json.contains("\"zn_input_read_summary\""));
        assert!(probe_json.contains("\"recent_active_zn_input_reads\""));
        assert!(probe_json.contains(
            "\"label\":\"system\",\"reads\":1,\"active_reads\":1,\"start_active_reads\":1,\"coin_active_reads\":1"
        ));
        assert!(probe_json.contains("\"last_start_active_value_hex\":\"0x000000ee\""));
        assert!(probe_json.contains("\"address_hex\":\"0x1fa00300\""));
        assert!(!probe_json.contains("\"recent\""));
        assert!(!probe_json.contains("\"recent_active\""));
        assert!(!probe_json.contains("\"recent_security_transfers\""));
    }

    #[test]
    fn bus_input_activity_includes_serial_controller_polls() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.set_input(ActionButtons {
            start: true,
            punch: true,
            kick: true,
            beast: true,
            guard: true,
            ..ActionButtons::default()
        });

        bus.write_u8(SIO_DATA, 0x01);
        bus.write_u8(SIO_DATA, 0x42);
        bus.write_u8(SIO_DATA, 0x00);
        bus.write_u8(SIO_DATA, 0x00);

        let activity = bus.input_activity();
        assert_eq!(activity.p1_input_reads, 1);
        assert_eq!(activity.p1_start_active_reads, 1);
        assert_eq!(activity.p1_punch_active_reads, 1);
        assert_eq!(activity.p1_kick_active_reads, 1);
        assert_eq!(activity.p1_beast_active_reads, 1);
        assert_eq!(activity.p3_guard_active_reads, 1);
    }

    #[test]
    fn board_coin_input_updates_system_port_and_coin_register_edge_counter() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        assert_eq!(bus.read_u8(0x1fa2_0000), 0);

        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });
        assert_eq!(bus.read_u8(0x1fa0_0200) & 0x02, 0x02);
        assert_eq!(bus.read_u8(0x1fa0_0200) & 0x10, 0x10);
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x10, 0);
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x20, 0x20);
        assert_eq!(bus.read_u8(0x1fa2_0000) & 0x01, 0);

        bus.write_u8(0x1fa2_0000, 0x22);
        assert_eq!(bus.read_u8(0x1fa2_0000), 0x22);

        bus.set_input(ActionButtons::default());
        assert_eq!(bus.read_u8(0x1fa2_0000), 0x22);
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x10, 0x10);

        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x10, 0);
        assert_eq!(bus.read_u8(0x1fa2_0000), 0x22);

        let board_json = bus.zn_board_json();
        assert!(board_json.contains("\"coin_insert_edges\":2"));
        assert_eq!(bus.input_activity().coin_register_active_reads, 3);
    }

    #[test]
    fn native_credit_adapter_projects_system_coin_to_br2_input_word() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x10, 0);

        bus.set_trace_context(0x802c_ff84, 1234);
        bus.write_u32(0x1f80_006c, 0);
        bus.clear_trace_context();

        assert_eq!(
            bus.read_u32(0x1f80_006c),
            super::BR2_NATIVE_CREDIT_INPUT_BIT
        );
        let activity = bus.input_activity();
        assert_eq!(activity.native_credit_adapter_writes, 1);
        assert_eq!(activity.native_credit_adapter_edges, 1);
        assert!(activity.has_native_credit_adapter_activity());
        let probe_json = bus.input_probe_json();
        assert!(
            probe_json.contains("\"native_credit_adapter_last_value_hex\":\"0x00000008\""),
            "{probe_json}"
        );
        assert!(
            probe_json.contains("\"native_credit_adapter_last_pc_hex\":\"0x802cff84\""),
            "{probe_json}"
        );
    }

    #[test]
    fn native_credit_adapter_input_bit_can_be_set_per_bus() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        assert!(!bus.set_native_credit_adapter_input_bit(0));
        assert!(bus.set_native_credit_adapter_input_bit(0x400));
        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x10, 0);

        bus.set_trace_context(0x802c_ff84, 1234);
        bus.write_u32(0x1f80_006c, 0x80);
        bus.clear_trace_context();

        assert_eq!(bus.read_u32(0x1f80_006c), 0x480);
        let probe_json = bus.input_probe_json();
        assert!(
            probe_json.contains("\"native_credit_adapter_input_bit_hex\":\"0x00000400\""),
            "{probe_json}"
        );
    }

    #[test]
    fn native_credit_projection_rules_parse_wide_and_custom_targets() {
        let rules =
            super::parse_native_credit_projection_rules("wide 0x1f800080/4=0x20 0x1f80007e/2=0x40")
                .expect("projection rules should parse");

        assert!(
            rules
                .iter()
                .any(|rule| rule.address == 0x1f80_0070 && rule.access_len == 4)
        );
        assert!(rules.iter().any(|rule| {
            rule.address == 0x1f80_0080 && rule.access_len == 4 && rule.mask == Some(0x20)
        }));
        assert!(rules.iter().any(|rule| {
            rule.address == 0x1f80_007e && rule.access_len == 2 && rule.mask == Some(0x40)
        }));
    }

    #[test]
    fn native_credit_projection_projects_configured_copy_halfword() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        assert!(bus.set_native_credit_projection_name("copies"));
        assert!(bus.set_native_credit_adapter_input_bit(0x0040));
        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x10, 0);

        bus.write_u16(0x1f80_007e, 0x0001);

        assert_eq!(bus.read_u16(0x1f80_007e), 0x0041);
        let probe_json = bus.input_probe_json();
        assert!(
            probe_json.contains("\"address_hex\":\"0x1f80007e\""),
            "{probe_json}"
        );
        assert_eq!(bus.input_activity().native_credit_adapter_writes, 1);
    }

    #[test]
    fn native_credit_adapter_projects_first_coin_edge_words() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.set_input(ActionButtons {
            coin: true,
            start: true,
            ..ActionButtons::default()
        });
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x10, 0);

        bus.set_trace_context(0x802c_ff84, 1);
        bus.write_u32(0x1f80_006c, 0x800);
        bus.set_trace_context(0x802c_ff94, 2);
        bus.write_u32(0x1f80_0068, 0x800);
        bus.set_trace_context(0x802c_ff98, 3);
        bus.write_u32(0x1f80_0074, 0x800);

        assert_eq!(bus.read_u32(0x1f80_006c), 0x808);
        assert_eq!(bus.read_u32(0x1f80_0068), 0x808);
        assert_eq!(bus.read_u32(0x1f80_0074), 0x808);

        bus.set_trace_context(0x802c_ff84, 4);
        bus.write_u32(0x1f80_006c, 0x800);
        bus.set_trace_context(0x802c_ff94, 5);
        bus.write_u32(0x1f80_0068, 0);
        bus.set_trace_context(0x802c_ff98, 6);
        bus.write_u32(0x1f80_0074, 0);
        bus.clear_trace_context();

        assert_eq!(bus.read_u32(0x1f80_006c), 0x808);
        assert_eq!(bus.read_u32(0x1f80_0068), 0);
        assert_eq!(bus.read_u32(0x1f80_0074), 0);
        assert_eq!(bus.input_activity().native_credit_adapter_edges, 1);
    }

    #[test]
    fn native_credit_adapter_requires_coin_poll_and_br2_input_write_site() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });
        bus.set_trace_context(0x802c_ff84, 10);
        bus.write_u32(0x1f80_006c, 0);
        bus.clear_trace_context();
        assert_eq!(bus.read_u32(0x1f80_006c), 0);

        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x10, 0);
        bus.set_trace_context(0x802c_ff80, 20);
        bus.write_u32(0x1f80_006c, 0x80);
        bus.clear_trace_context();
        assert_eq!(bus.read_u32(0x1f80_006c), 0x80);

        bus.set_trace_context(0x802c_ff84, 30);
        bus.write_u32(0x1f80_0070, 0);
        bus.clear_trace_context();
        assert_eq!(bus.read_u32(0x1f80_0070), 0);
        assert_eq!(bus.input_activity().native_credit_adapter_writes, 0);
    }

    #[test]
    fn board_coin_register_write_tracks_counter_and_lockout_outputs() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.set_trace_context(0x8020_0000, 123);
        bus.write_u8(0x1fa2_0000, 0x00);
        assert_eq!(bus.read_u8(0x1fa2_0000), 0x00);
        bus.write_u8(0x1fa2_0000, 0x33);
        bus.write_u8(0x1fa2_0000, 0x33);
        let active_probe_json = bus.input_probe_json();
        assert!(active_probe_json.contains("\"coin_lockout_0\":true"));
        assert!(active_probe_json.contains("\"coin_lockout_1\":true"));
        bus.write_u8(0x1fa2_0000, 0x00);

        let probe_json = bus.input_probe_json();

        assert!(
            probe_json.contains("\"coin_register_writes\":4"),
            "{probe_json}"
        );
        assert!(probe_json.contains("\"coin_counter_0_edges\":1"));
        assert!(probe_json.contains("\"coin_counter_1_edges\":1"));
        assert!(probe_json.contains("\"coin_lockout_0\":false"));
        assert!(probe_json.contains("\"coin_lockout_1\":false"));
        assert!(
            probe_json.contains("\"recent_coin_register_writes\""),
            "{probe_json}"
        );
        assert!(
            probe_json.contains("\"address_hex\":\"0x1fa20000\""),
            "{probe_json}"
        );
        assert!(
            probe_json.contains("\"pc_hex\":\"0x80200000\""),
            "{probe_json}"
        );
        assert!(probe_json.contains("\"cycles\":123"), "{probe_json}");
        assert!(probe_json.contains("\"data_hex\":\"0x33\""), "{probe_json}");
    }

    #[test]
    fn zinc_legacy_coin_compat_mirrors_coin_to_coin_register_bit0() {
        let mut bus = Bus::with_board_assets(
            Vec::new(),
            Vec::new(),
            2 * 1024 * 1024,
            NativeBoardAssets {
                legacy_zinc_input_compat: true,
                ..NativeBoardAssets::default()
            },
        );

        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });
        assert_eq!(bus.read_u8(0x1fa2_0000) & 0x01, 0x01);
        assert_eq!(bus.read_u8(0x1fa2_0001) & 0x01, 0x01);
        assert_eq!(bus.read_u8(0x1fa2_0002) & 0x01, 0x01);
        assert_eq!(bus.read_u8(0x1fa2_0003) & 0x01, 0x01);

        bus.set_input(ActionButtons::default());
        assert_eq!(bus.read_u8(0x1fa2_0000) & 0x01, 0);
    }

    #[test]
    fn zinc_legacy_coin_compat_latches_coin_edge_until_coin_register_read() {
        let mut bus = Bus::with_board_assets(
            Vec::new(),
            Vec::new(),
            2 * 1024 * 1024,
            NativeBoardAssets {
                legacy_zinc_input_compat: true,
                ..NativeBoardAssets::default()
            },
        );

        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });
        bus.set_input(ActionButtons::default());

        assert_eq!(bus.read_u8(0x1fa2_0001) & 0x01, 0x01);
        assert_eq!(bus.read_u8(0x1fa2_0000) & 0x01, 0);
        assert_eq!(bus.input_activity().coin_register_active_reads, 1);
    }

    #[test]
    fn zinc_legacy_coin_latch_is_not_consumed_by_coin_register_write_merge() {
        let mut bus = Bus::with_board_assets(
            Vec::new(),
            Vec::new(),
            2 * 1024 * 1024,
            NativeBoardAssets {
                legacy_zinc_input_compat: true,
                ..NativeBoardAssets::default()
            },
        );

        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });
        bus.set_input(ActionButtons::default());

        bus.write_u8(0x1fa2_0000, 0);

        assert_eq!(bus.input_activity().coin_register_reads, 0);
        assert_eq!(bus.read_u8(0x1fa2_0000) & 0x01, 0x01);
        assert_eq!(bus.read_u8(0x1fa2_0000) & 0x01, 0);
    }

    #[test]
    fn zinc_legacy_system_coin_latch_survives_brief_release_until_system_read() {
        let mut bus = Bus::with_board_assets(
            Vec::new(),
            Vec::new(),
            2 * 1024 * 1024,
            NativeBoardAssets {
                legacy_zinc_input_compat: true,
                ..NativeBoardAssets::default()
            },
        );

        bus.set_input(ActionButtons {
            coin: true,
            ..ActionButtons::default()
        });
        bus.set_input(ActionButtons::default());

        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x10, 0);
        assert_eq!(bus.input_activity().system_coin_active_reads, 1);

        for _ in 1..super::LEGACY_ZINC_SYSTEM_EDGE_LATCH_READS {
            assert_eq!(bus.read_u8(0x1fa0_0300) & 0x10, 0);
        }
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x10, 0x10);

        let probe_json = bus.input_probe_json();
        assert!(
            probe_json.contains("\"legacy_system_coin_latch_edges\":1"),
            "{probe_json}"
        );
    }

    #[test]
    fn zinc_legacy_start_latch_reaches_p1_and_system_ports_after_release() {
        let mut bus = Bus::with_board_assets(
            Vec::new(),
            Vec::new(),
            2 * 1024 * 1024,
            NativeBoardAssets {
                legacy_zinc_input_compat: true,
                ..NativeBoardAssets::default()
            },
        );

        bus.set_input(ActionButtons {
            start: true,
            ..ActionButtons::default()
        });
        bus.set_input(ActionButtons::default());

        assert_eq!(bus.read_u8(0x1fa0_0000) & 0x80, 0);
        assert_eq!(bus.read_u8(0x1fa0_0300) & 0x01, 0);
        assert!(bus.input_activity().p1_start_active_reads > 0);
        assert!(bus.input_activity().system_start_active_reads > 0);

        let probe_json = bus.input_probe_json();
        assert!(
            probe_json.contains("\"legacy_system_start_latch_edges\":1"),
            "{probe_json}"
        );
    }

    #[test]
    fn input_activity_reports_direction_and_full_control_status() {
        let no_activity = NativeInputActivity::default();
        assert!(!no_activity.has_direction_activity());
        assert!(!no_activity.has_play_control_activity());
        assert!(!no_activity.has_full_control_activity());
        assert!(!no_activity.has_any_control_activity());

        let partial_activity = NativeInputActivity {
            p1_input_reads: 2,
            p1_right_active_reads: 1,
            p1_punch_active_reads: 1,
            ..NativeInputActivity::default()
        };
        assert!(!partial_activity.has_direction_activity());
        assert!(!partial_activity.has_play_control_activity());
        assert!(!partial_activity.has_full_control_activity());
        assert!(partial_activity.has_any_direction_activity());
        assert!(partial_activity.has_any_attack_activity());
        assert!(partial_activity.has_any_play_control_activity());
        assert!(partial_activity.has_any_control_activity());

        let service_activity = NativeInputActivity {
            system_service_active_reads: 1,
            ..NativeInputActivity::default()
        };
        assert!(service_activity.has_service_activity());
        assert!(service_activity.has_any_control_activity());
        assert!(!service_activity.has_any_play_control_activity());

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
            system_service_active_reads: 1,
            system_start_active_reads: 1,
            coin_register_reads: 1,
            coin_register_active_reads: 1,
            coin_insert_edges: 1,
            legacy_system_coin_latch_edges: 1,
            legacy_system_start_latch_edges: 1,
            native_credit_adapter_writes: 1,
            native_credit_adapter_edges: 1,
            ..NativeInputActivity::default()
        };

        assert!(full_activity.has_direction_activity());
        assert!(full_activity.has_play_control_activity());
        assert!(full_activity.has_full_control_activity());
        assert!(full_activity.has_any_control_activity());
        assert!(full_activity.has_credit_probe_activity());
        assert!(full_activity.has_coin_register_active_activity());
        assert!(full_activity.has_native_credit_adapter_activity());

        let mame_start_activity = NativeInputActivity {
            system_start_active_reads: 1,
            ..NativeInputActivity::default()
        };
        assert!(mame_start_activity.has_start_edge_activity());
        assert!(mame_start_activity.has_start_probe_activity());

        let json = full_activity.json();
        assert!(json.contains("\"has_direction_activity\":true"));
        assert!(json.contains("\"has_play_control_activity\":true"));
        assert!(json.contains("\"has_full_control_activity\":true"));
        assert!(json.contains("\"has_any_direction_activity\":true"));
        assert!(json.contains("\"has_any_attack_activity\":true"));
        assert!(json.contains("\"has_any_play_control_activity\":true"));
        assert!(json.contains("\"has_service_activity\":true"));
        assert!(json.contains("\"has_any_control_activity\":true"));
        assert!(json.contains("\"has_credit_probe_activity\":true"));
        assert!(json.contains("\"has_coin_register_active_activity\":true"));
        assert!(json.contains("\"has_native_credit_adapter_activity\":true"));
    }

    #[test]
    fn input_activity_merges_and_diffs_branch_reads_safely() {
        let baseline = NativeInputActivity {
            p1_input_reads: 8,
            p1_up_active_reads: 2,
            p1_punch_active_reads: 1,
            system_coin_active_reads: 1,
            native_credit_adapter_writes: 1,
            ..NativeInputActivity::default()
        };
        let branch = NativeInputActivity {
            p1_input_reads: 13,
            p1_up_active_reads: 2,
            p1_down_active_reads: 4,
            p1_punch_active_reads: 3,
            system_coin_active_reads: 1,
            p3_guard_active_reads: 5,
            native_credit_adapter_writes: 4,
            native_credit_adapter_edges: 1,
            ..NativeInputActivity::default()
        };

        let delta = branch.saturating_subtracted(baseline);
        assert_eq!(delta.p1_input_reads, 5);
        assert_eq!(delta.p1_up_active_reads, 0);
        assert_eq!(delta.p1_down_active_reads, 4);
        assert_eq!(delta.p1_punch_active_reads, 2);
        assert_eq!(delta.system_coin_active_reads, 0);
        assert_eq!(delta.p3_guard_active_reads, 5);
        assert_eq!(delta.native_credit_adapter_writes, 3);
        assert_eq!(delta.native_credit_adapter_edges, 1);

        let merged = baseline.saturating_added(delta);
        assert_eq!(merged.p1_input_reads, 13);
        assert_eq!(merged.p1_down_active_reads, 4);
        assert_eq!(merged.p1_punch_active_reads, 3);
        assert_eq!(merged.p3_guard_active_reads, 5);
        assert_eq!(merged.native_credit_adapter_writes, 4);
        assert_eq!(merged.native_credit_adapter_edges, 1);
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
    fn bus_freezes_br2_code_patch_snapshot_after_boot_copy() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let mut boot_copy = vec![0u8; BR2_CODE_PATCH_SNAPSHOT_LEN];
        for (index, byte) in boot_copy.iter_mut().enumerate() {
            *byte = (index as u8).wrapping_mul(17).wrapping_add(3);
        }

        bus.write_bytes(0x802c_c100, &boot_copy);
        assert_eq!(bus.read_u32(0x802c_c100), 0x3625_1403);
        assert_eq!(bus.read_u32(0x802c_c104), 0x7a69_5847);

        bus.write_u32(0x802c_c100, 0xa7be_ffe8);
        bus.write_u32(0x802c_c104, 0x0bc0_51f8);

        assert_eq!(bus.read_u32(0x802c_c100), 0x3625_1403);
        assert_eq!(bus.read_u32(0x802c_c104), 0x7a69_5847);
    }

    #[test]
    fn bus_serves_boot_runtime_code_snapshot_after_table_corruption() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let address = 0x8000_0000 | (BR2_RUNTIME_CODE_SNAPSHOT_START + 0x8374);
        bus.set_trace_context(0x8001_011c, 100);
        bus.write_u32(address, 0x0c0c_209e);
        bus.clear_trace_context();

        bus.set_trace_context(0x8035_6f0c, 200);
        bus.write_u32(address, 0x8c0d_209e);
        bus.clear_trace_context();

        assert_eq!(
            bus.read_ram_u32_physical(BR2_RUNTIME_CODE_SNAPSHOT_START + 0x8374),
            Some(0x8c0d_209e)
        );
        assert_eq!(bus.read_u32(address), 0x8c0d_209e);
        bus.set_trace_context(address, 300);
        assert_eq!(bus.read_u32(address), 0x0c0c_209e);
        bus.set_trace_context(address | 0x2000_0000, 400);
        assert_eq!(bus.read_u32(address | 0x2000_0000), 0x0c0c_209e);
        bus.clear_trace_context();
    }

    #[test]
    fn bus_serves_low_boot_runtime_code_snapshot_after_pattern_corruption() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let address = 0x802d_a6b8;
        bus.set_trace_context(0x8001_011c, 100);
        bus.write_u32(address, 0x2442_0001);
        bus.clear_trace_context();

        bus.write_u32(address, 0x5555_5555);

        assert_eq!(bus.read_ram_u32_physical(0x002d_a6b8), Some(0x5555_5555));
        assert_eq!(bus.read_u32(address), 0x5555_5555);
        bus.set_trace_context(address, 300);
        assert_eq!(bus.read_u32(address), 0x2442_0001);
        bus.clear_trace_context();
    }

    #[test]
    fn bus_restores_br2_boot_dispatch_pointer_when_ram_is_zeroed() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let address = 0x8036_647c;

        bus.set_trace_context(BR2_BOOT_WORD_COPY_LOOP_PHYSICAL, 100);
        bus.write_u32(address, 0x8036_643c);
        bus.clear_trace_context();

        bus.write_u32(address, 0);

        assert_eq!(bus.read_u32(address), 0x8036_643c);
        assert_eq!(bus.read_u16(address), 0x643c);
        assert_eq!(bus.read_u8(address), 0x3c);
    }

    #[test]
    fn bus_prefers_live_br2_boot_dispatch_pointer_when_ram_is_nonzero() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let address = 0x8036_647c;

        bus.set_trace_context(BR2_BOOT_WORD_COPY_LOOP_PHYSICAL, 100);
        bus.write_u32(address, 0x8036_643c);
        bus.clear_trace_context();

        bus.write_u32(address, 0x8123_4567);

        assert_eq!(bus.read_u32(address), 0x8123_4567);
    }

    #[test]
    fn bus_restores_br2_boot_gpu_status_pointer_when_ram_is_zeroed() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let address = 0x8036_658c;

        bus.set_trace_context(BR2_BOOT_WORD_COPY_LOOP_PHYSICAL, 100);
        bus.write_u32(address, 0x1f80_1814);
        bus.clear_trace_context();

        bus.write_u32(address, 0);

        assert_eq!(bus.read_u32(address), 0x1f80_1814);
        assert_eq!(bus.read_u16(address), 0x1814);
        assert_eq!(bus.read_u8(address), 0x14);
    }

    #[test]
    fn bus_prefers_live_br2_boot_gpu_status_pointer_when_ram_is_nonzero() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let address = 0x8036_658c;

        bus.set_trace_context(BR2_BOOT_WORD_COPY_LOOP_PHYSICAL, 100);
        bus.write_u32(address, 0x1f80_1814);
        bus.clear_trace_context();

        bus.write_u32(address, 0x1f80_1810);

        assert_eq!(bus.read_u32(address), 0x1f80_1810);
    }

    #[test]
    fn bus_does_not_snapshot_runtime_code_from_non_boot_writes() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let address = 0x8000_0000 | (BR2_RUNTIME_CODE_SNAPSHOT_START + 0x10);

        bus.set_trace_context(0x8035_6f0c, 200);
        bus.write_u32(address, 0x8c0d_209e);
        bus.clear_trace_context();

        assert_eq!(bus.read_u32(address), 0x8c0d_209e);
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
    fn bus_byte_copy_uses_forward_cpu_order_for_overlapping_backrefs() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.write_u8(0x0000_1000, b'A');

        let copied = bus
            .try_copy_bytes(0x0000_1000, 0x0000_1001, 4)
            .expect("overlapping byte copy");

        assert_eq!(copied, b"AAAA");
        assert_eq!(bus.read_bytes(0x0000_1000, 5), b"AAAAA");
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
    fn native_sync_compact_json_exposes_gpu_dma_registers_and_recent_activity() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(0x0000_3000, 0x0100_3008);
        bus.write_u32(0x0000_3004, 0xe100_0400);
        bus.write_u32(0x0000_3008, 0x00ff_ffff);

        bus.write_u32(DMA_GPU_MADR, 0x8000_3000);
        bus.write_u32(DMA_GPU_BCR, 0);
        bus.write_u32(DMA_GPU_CHCR, 0x0100_0401);
        bus.vblank_count = 7;

        let sync_json = bus.native_sync_compact_json();
        assert!(sync_json.contains("\"gpu_dma_channel\""));
        assert!(sync_json.contains("\"madr_hex\":\"0x00003000\""));
        assert!(sync_json.contains("\"chcr_hex\":\"0x01000401\""));
        assert!(sync_json.contains("\"gpu_dma_stale_vblanks\":7"));
        assert!(sync_json.contains("\"last_gpu_dma_register_write\""));
        assert!(sync_json.contains("\"recent_gpu_dma_register_writes\""));
        assert!(sync_json.contains("\"register\":\"GPU_CHCR\""));
        assert!(sync_json.contains("\"recent_dma_activity_counts\""));
        assert!(sync_json.contains("\"dma_lifetime_activity\""));
        assert!(sync_json.contains("\"channel\":2"));
        assert!(sync_json.contains("\"gpu_linked_list\":1"));
        assert!(sync_json.contains("\"recent_dma_activity\""));
        assert!(sync_json.contains("\"recent_otc_ranges\""));
        assert!(sync_json.contains("\"recent_primitive_header_relations\""));
        assert!(sync_json.contains("\"last_vblank\""));
        assert!(sync_json.contains("\"last_cycles\""));
        assert!(sync_json.contains("\"kind\":\"register_write\""));
        assert!(sync_json.contains("\"kind\":\"gpu_linked_list\""));
        assert!(sync_json.contains("\"start_hex\":\"0x00003000\""));
    }

    #[test]
    fn recent_primitive_header_relations_report_otc_distance() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(DMA_OTC_MADR, 0x0039_ffec);
        bus.write_u32(DMA_OTC_BCR, 4);
        bus.write_u32(DMA_OTC_CHCR, 0x1100_0002);
        bus.write_u32(0x0039_ffe0, 0x01ff_ffff);
        bus.write_u32(0x0039_ffe4, 0xe100_020a);

        let sync_json = bus.native_sync_compact_json();
        assert!(sync_json.contains("\"recent_otc_ranges\""));
        assert!(sync_json.contains("\"recent_primitive_header_relations\""));
        assert!(sync_json.contains("\"address_hex\":\"0x0039ffe0\""));
        assert!(sync_json.contains("\"inside_latest_otc\":true"));
        assert!(sync_json.contains("\"distance_to_latest_otc\":0"));
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
        assert!(sync_json.contains("\"gpu_dma_stale_vblanks\":0"));
        assert!(sync_json.contains("\"last_gpu_dma_register_write\""));
        assert!(sync_json.contains("\"recent_gpu_dma_register_writes\""));
        assert!(sync_json.contains("\"recent_dma_activity_counts\""));
        assert!(sync_json.contains("\"dma_lifetime_activity\""));
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
    fn dma_lifetime_activity_survives_recent_ring_eviction() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);
        bus.write_u32(0x0000_1000, 0x01ff_ffff);
        bus.write_u32(0x0000_1004, 0xe100_0400);

        bus.write_u32(DMA_GPU_MADR, 0x0000_1000);
        bus.write_u32(DMA_GPU_CHCR, 0x0100_0401);

        for index in 0..(DMA_ACTIVITY_RECENT_LIMIT + 4) {
            let head = 0x0000_2000 + (index as u32 * 0x20);
            bus.write_u32(DMA_OTC_MADR, head);
            bus.write_u32(DMA_OTC_BCR, 3);
            bus.write_u32(DMA_OTC_CHCR, 0x1100_0002);
        }

        assert!(
            !bus.dma_activity
                .iter()
                .any(|sample| sample.kind == "gpu_linked_list")
        );
        let sync_json = bus.native_sync_compact_json();
        assert!(sync_json.contains("\"dma_lifetime_activity\""));
        assert!(sync_json.contains("\"channel\":2"));
        assert!(sync_json.contains("\"gpu_linked_list\":1"));
        assert!(sync_json.contains("\"last_transfer\":{\"kind\":\"gpu_linked_list\""));
        assert!(sync_json.contains("\"last_register_write\":{\"kind\":\"register_write\""));
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
        let sync_json = bus.native_sync_compact_json();
        assert!(sync_json.contains("\"kind\":\"gpu_linked_list\""));
        assert!(sync_json.contains("\"gpu_linked_list\":1"));
        assert!(sync_json.contains("\"last_transfer\":{\"kind\":\"gpu_linked_list\""));
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
    fn gpu_linked_list_dma_skips_range_starting_inside_gp0_payload() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let command_start = 0x003b_4ff0;
        let command_words = [
            0x2c80_8080,
            0x0181_0148,
            0x7adf_e000,
            0x0181_01c8,
            0x000f_e080,
            0x01bf_0148,
            0x0000_ff00,
            0x01bf_01c8,
            0x0000_ff80,
        ];
        for (index, word) in command_words.into_iter().enumerate() {
            bus.write_u32(command_start + index as u32 * 4, word);
        }

        let payload_address = command_start + 8;
        let commands = [(payload_address, 0x7adf_e000)];

        assert!(bus.gpu_linked_list_command_start_is_embedded_payload(payload_address));
        bus.write_gpu_dma_linked_list_command_range(&commands, 0..1);

        assert_eq!(bus.io.gpu.commands_seen, 0);
        assert_ne!(bus.io.gpu.gp0_read, 0x7adf_e000);
    }

    #[test]
    fn gpu_linked_list_dma_skips_payload_word_starting_new_gp0_command() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let command_start = 0x003b_4f38;
        let command_words = [
            0x2c80_8080,
            0x0181_0148,
            0x7adf_e000,
            0x0181_01c8,
            0x000f_e080,
            0x01bf_0148,
            0x0000_ff00,
            0x01bf_01c8,
            0x0000_ff80,
        ];
        for (index, word) in command_words.into_iter().enumerate() {
            bus.write_u32(command_start + index as u32 * 4, word);
        }

        let payload_address = command_start + 8;
        bus.write_gpu_dma_linked_list_word(payload_address, 0x7adf_e000);

        assert_eq!(bus.io.gpu.commands_seen, 0);
        assert_eq!(bus.io.gpu.gp0_pending_words(), 0);
        assert!(
            bus.native_sync_json()
                .contains("\"embedded_payload_skips\":1")
        );
    }

    #[test]
    fn gpu_linked_list_dma_allows_payload_word_inside_pending_gp0_command() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let command_start = 0x003b_4f38;
        let command_words = [
            0x2c80_8080,
            0x0181_0148,
            0x7adf_e000,
            0x0181_01c8,
            0x000f_e080,
            0x01bf_0148,
            0x0000_ff00,
            0x01bf_01c8,
            0x0000_ff80,
        ];
        for (index, word) in command_words.into_iter().enumerate() {
            let address = command_start + index as u32 * 4;
            bus.write_u32(address, word);
            bus.write_gpu_dma_linked_list_word(address, word);
        }

        assert_eq!(bus.io.gpu.gp0_pending_words(), 0);
        let io_json = bus.io_json();
        assert!(io_json.contains("\"opcode\":44"));
        assert!(!io_json.contains("\"opcode\":122"));
        assert!(
            bus.native_sync_json()
                .contains("\"embedded_payload_skips\":0")
        );
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
    fn gpu_linked_list_dma_skips_unlinked_br2_primitive_packets_by_default() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(0x003a_1000, 0x01ff_ffff);
        bus.write_u32(0x003a_1004, 0xe100_0400);

        for index in 0..BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_RECENT_HEADERS {
            let base = 0x0038_1000 + (index as u32) * 0x20;
            bus.write_u32(base, 0x05ff_ffff);
            bus.write_u32(base + 4, 0x2800_ff00);
            bus.write_u32(base + 8, 0x0050_0000);
            bus.write_u32(base + 12, 0x0050_0008);
            bus.write_u32(base + 16, 0x0058_0000);
            bus.write_u32(base + 20, 0x0058_0008);
        }

        bus.write_u32(DMA_GPU_MADR, 0x003a_1000);
        bus.write_u32(DMA_GPU_CHCR, 0x0100_0401);

        assert_eq!(bus.io.gpu.commands_seen, 1);
        let sync_json = bus.native_sync_json();
        assert!(sync_json.contains("\"conditional_replays\":0"));
        assert!(sync_json.contains("\"last_reason\":\"disabled_by_default\""));
    }

    #[test]
    fn gpu_short_linked_unlinked_replay_rolls_back_when_validation_fails() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.set_unlinked_primitive_replay_interval(Some(1));
        bus.vblank_count = 12;

        let linked_list_node = 0x003a_1000;
        bus.write_u32(linked_list_node, 0x01ff_ffff);
        bus.write_u32(linked_list_node + 4, 0xe100_0400);

        for index in 0..BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_RECENT_HEADERS {
            let base = 0x0038_2000 + (index as u32) * 0x20;
            bus.write_u32(base, 0x03ff_ffff);
            bus.write_u32(base + 4, 0x6000_ff00);
            bus.write_u32(base + 8, 0x0064_0064);
            bus.write_u32(base + 12, 0x0010_0010);
        }

        bus.write_u32(DMA_GPU_MADR, linked_list_node);
        bus.write_u32(DMA_GPU_CHCR, 0x0100_0401);

        let sync_json = bus.native_sync_json();
        let (width, _, frame) = bus.io.display_rgb_frame();
        let pixel = frame[100 * width + 100];

        assert_eq!(bus.io.gpu.commands_seen, 1, "{sync_json}");
        assert_eq!(
            pixel, 0,
            "failed replay validation must restore the pre-replay framebuffer: {sync_json}"
        );
        assert!(
            sync_json.contains("\"conditional_replays\":0"),
            "{sync_json}"
        );
        assert!(sync_json.contains("\"skipped\":1"), "{sync_json}");
        assert!(
            sync_json.contains("\"last_candidate_headers\":33"),
            "{sync_json}"
        );
        assert!(
            sync_json.contains("\"last_reason\":\"replay_rejected_after_validation\""),
            "{sync_json}"
        );
    }

    #[test]
    fn gpu_unlinked_replay_tracks_headers_beyond_recent_ring_without_default_replay() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let linked_list_node = 0x0010_0000;
        bus.vblank_count = 20;
        bus.write_u32(linked_list_node, 0x01ff_ffff);
        bus.write_u32(linked_list_node + 4, 0xe100_0400);

        let packet_count = PRIMITIVE_RAM_RECENT_LIMIT + 8;
        for index in 0..packet_count {
            let base = 0x0038_4000 + (index as u32) * 0x20;
            let next = if index == 0 {
                0x00ff_ffff
            } else {
                (base - 0x20) & 0x00ff_ffff
            };
            bus.write_u32(base, 0x0500_0000 | next);
            bus.write_u32(base + 4, 0x2800_ff00);
            bus.write_u32(base + 8, 0x0070_0050);
            bus.write_u32(base + 12, 0x0078_0050);
            bus.write_u32(base + 16, 0x0070_0058);
            bus.write_u32(base + 20, 0x0078_0058);
        }

        bus.write_u32(DMA_GPU_MADR, linked_list_node);
        bus.write_u32(DMA_GPU_CHCR, 0x0100_0401);

        assert!(bus.unlinked_primitive_replay.last_candidate_headers >= packet_count);
        assert_eq!(bus.unlinked_primitive_replay.last_packets, 0);
        assert_eq!(bus.io.gpu.commands_seen, 1);

        let linked_nodes = [linked_list_node]
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let (packets, words) = bus.replay_recent_unlinked_primitive_packets(&linked_nodes);

        assert_eq!(packets, BR2_UNLINKED_PRIMITIVE_REPLAY_PACKET_LIMIT);
        assert!(words > packets);
    }

    #[test]
    fn gpu_unlinked_replay_restores_state_packet_pointing_to_draw_packet() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.vblank_count = 20;
        let draw_packet = 0x0038_2000;
        let state_packet = 0x0038_2020;
        bus.write_u32(draw_packet, 0x04ff_ffff);
        bus.write_u32(draw_packet + 4, 0x6480_8080);
        bus.write_u32(draw_packet + 8, 0x0000_0000);
        bus.write_u32(draw_packet + 12, 0x0000_0000);
        bus.write_u32(draw_packet + 16, 0x0010_0010);
        bus.write_u32(state_packet, 0x0100_0000 | draw_packet);
        bus.write_u32(state_packet + 4, 0xe100_0400);

        let linked_nodes = std::collections::HashSet::new();
        let state_packets = bus.collect_unlinked_state_replay_candidates_by_next(&linked_nodes, 18);
        assert!(state_packets.contains_key(&draw_packet));
        let mut replayed_command_addresses = std::collections::HashSet::new();

        let words = bus.replay_unlinked_state_packet_chain_for_target(
            draw_packet,
            &linked_nodes,
            &state_packets,
            &mut replayed_command_addresses,
        );

        assert_eq!(words, 1);
        assert!(bus.io_json().contains("\"gpu_texture_page\":1024"));
        assert_eq!(bus.io.gpu.commands_seen, 1);
    }

    #[test]
    fn stale_unlinked_replay_candidate_cache_invalidates_on_new_header_write() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let linked_nodes = std::collections::HashSet::new();

        bus.write_u32(0x0038_4000, 0x05ff_ffff);
        bus.write_u32(0x0038_4004, 0x2800_ff00);
        bus.write_u32(0x0038_4008, 0x0070_0050);
        bus.write_u32(0x0038_400c, 0x0078_0050);
        bus.write_u32(0x0038_4010, 0x0070_0058);
        bus.write_u32(0x0038_4014, 0x0078_0058);

        let first = bus.collect_stale_unlinked_primitive_replay_candidates_cached(&linked_nodes);
        assert_eq!(first.len(), 1);
        let cached_generation = bus.primitive_header_generation;
        let cached_key = bus.stale_unlinked_primitive_replay_candidates.key;

        bus.write_u32(0x0038_4080, 0x05ff_ffff);
        bus.write_u32(0x0038_4084, 0x2800_00ff);
        bus.write_u32(0x0038_4088, 0x0080_0060);
        bus.write_u32(0x0038_408c, 0x0088_0060);
        bus.write_u32(0x0038_4090, 0x0080_0068);
        bus.write_u32(0x0038_4094, 0x0088_0068);

        let second = bus.collect_stale_unlinked_primitive_replay_candidates_cached(&linked_nodes);
        assert_eq!(second.len(), 2);
        assert!(bus.primitive_header_generation > cached_generation);
        assert_ne!(
            bus.stale_unlinked_primitive_replay_candidates.key,
            cached_key
        );
    }

    #[test]
    fn gpu_vblank_skips_recent_unlinked_textured_primitives_by_default() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.write_u32(0x003a_1000, 0x01ff_ffff);
        bus.write_u32(0x003a_1004, 0xe100_0400);
        bus.write_u32(DMA_GPU_MADR, 0x003a_1000);
        bus.write_u32(DMA_GPU_CHCR, 0x0100_0401);
        let commands_before = bus.io.gpu.commands_seen;

        for index in 0..BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_DRAW_PACKETS {
            let base = 0x0038_2000 + index * 0x40;
            bus.write_u32(base, 0x09ff_ffff);
            bus.write_u32(base + 4, 0x2d40_4040);
            bus.write_u32(base + 8, 0x0050_0000);
            bus.write_u32(base + 12, 0x0000_0000);
            bus.write_u32(base + 16, 0x0050_0008);
            bus.write_u32(base + 20, 0x0001_0000);
            bus.write_u32(base + 24, 0x0058_0000);
            bus.write_u32(base + 28, 0x0000_0100);
            bus.write_u32(base + 32, 0x0058_0008);
            bus.write_u32(base + 36, 0x0000_0101);
        }

        bus.tick(566_000);

        assert_eq!(bus.io.gpu.commands_seen, commands_before);
        assert!(
            bus.io_json()
                .contains("\"gpu_textured_triangle_commands\":0")
        );
        let sync_json = bus.native_sync_json();
        assert!(sync_json.contains("\"conditional_replays\":0"));
        assert!(sync_json.contains("\"last_reason\":\"disabled_by_default\""));
    }

    #[test]
    fn gpu_unlinked_replay_follows_candidate_next_links_from_head_to_tail() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let tail = 0x0038_1000;
        let head = 0x0038_1020;
        let linked_nodes = std::collections::HashSet::new();

        bus.write_u32(tail, 0x03ff_ffff);
        bus.write_u32(tail + 4, 0x6000_ff00);
        bus.write_u32(tail + 8, 0x0064_0064);
        bus.write_u32(tail + 12, 0x0001_0001);
        bus.write_u32(head, 0x0338_1000);
        bus.write_u32(head + 4, 0x6000_00ff);
        bus.write_u32(head + 8, 0x0064_0064);
        bus.write_u32(head + 12, 0x0001_0001);

        let (packets, words) = bus.replay_recent_unlinked_primitive_packets(&linked_nodes);
        let (width, _, frame) = bus.io.display_rgb_frame();
        let pixel = frame[100 * width + 100];

        assert_eq!((packets, words), (2, 6));
        assert_eq!(pixel, 0x0000_ff00);
    }

    #[test]
    fn gpu_unlinked_replay_uses_recent_command_body_writes_when_header_is_stale() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let packet = 0x0038_3000;
        let linked_nodes = std::collections::HashSet::new();

        bus.vblank_count = 1;
        bus.write_u32(packet, 0x03ff_ffff);
        bus.write_u32(packet + 4, 0x6000_00ff);
        bus.write_u32(packet + 8, 0x0064_0064);
        bus.write_u32(packet + 12, 0x0010_0010);

        bus.vblank_count = BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW + 8;
        bus.write_u32(packet + 4, 0x6000_ff00);

        let candidates = bus.collect_unlinked_primitive_replay_candidates(
            &linked_nodes,
            Some(bus.vblank_count - BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW),
        );
        assert!(
            candidates.contains_key(&packet),
            "recent command body write should recover stale-header packet"
        );

        let (packets, words) = bus.replay_recent_unlinked_primitive_packets(&linked_nodes);
        let (width, _, frame) = bus.io.display_rgb_frame();
        let pixel = frame[100 * width + 100];

        assert_eq!((packets, words), (1, 3));
        assert_eq!(pixel, 0x0000_ff00);
    }

    #[test]
    fn gpu_short_linked_list_replay_skips_old_stale_command_ring() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        bus.set_unlinked_primitive_replay_interval(Some(1));
        bus.vblank_count = 1;

        for index in 0..BR2_UNLINKED_PRIMITIVE_REPLAY_MIN_DRAW_PACKETS {
            let base = 0x0038_7000 + index * 0x40;
            bus.write_u32(base, 0x09ff_ffff);
            bus.write_u32(base + 4, 0x2d40_4040);
            bus.write_u32(base + 8, 0x0050_0050);
            bus.write_u32(base + 12, 0x0000_0000);
            bus.write_u32(base + 16, 0x0050_0090);
            bus.write_u32(base + 20, 0x0001_0000);
            bus.write_u32(base + 24, 0x0090_0050);
            bus.write_u32(base + 28, 0x0000_0001);
            bus.write_u32(base + 32, 0x0090_0090);
            bus.write_u32(base + 36, 0x0001_0001);
        }

        bus.primitive_ram_writes.advance_vblank();
        bus.primitive_ram_writes.advance_vblank();
        bus.vblank_count = BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW + 32;
        let linked_node = 0x003a_1000;
        let mut stats = super::GpuLinkedListDmaRunStats::started(linked_node, linked_node);
        stats.last_nodes = 1;
        stats.last_words = 1;
        stats.last_nonempty_nodes = 1;
        stats.visited_nodes.push(linked_node);

        let linked_nodes = [linked_node]
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let min_vblank = bus
            .vblank_count
            .saturating_sub(BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW);
        let recent_stale_candidates =
            bus.recent_stale_unlinked_draw_packet_candidates(&linked_nodes, min_vblank);
        let decision = bus.unlinked_primitive_replay_decision(&stats);

        assert_eq!(recent_stale_candidates, 0);
        assert!(!decision.enabled, "{decision:?}");
        assert_eq!(decision.reason, "linked_list_too_short");
        assert_eq!(decision.diagnostics.recent_draw_candidates, 0);
        assert_eq!(decision.diagnostics.recent_stale_draw_candidates, 0);
    }

    #[test]
    fn gpu_unlinked_replay_rejects_recent_header_with_stale_command_body() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let packet = 0x0038_a11c;
        let linked_nodes = std::collections::HashSet::new();

        bus.vblank_count = 18;
        bus.write_u32(packet + 4, 0x2e7f_7f7f);
        bus.write_u32(packet + 8, 0x0000_308d);
        bus.write_u32(packet + 12, 0x0000_0000);
        bus.write_u32(packet + 16, 0x00c1_0000);
        bus.write_u32(packet + 20, 0x0308_ff6d);
        bus.write_u32(packet + 24, 0xa000_e1f4);
        bus.write_u32(packet + 28, 0x0000_0300);
        bus.write_u32(packet + 32, 0x0000_0000);
        bus.write_u32(packet + 36, 0x0000_0000);

        bus.vblank_count = BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW + 24;
        let min_vblank = bus
            .vblank_count
            .saturating_sub(BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW);
        bus.write_u32(packet, 0x09ff_ffff);

        let candidates =
            bus.collect_unlinked_primitive_replay_candidates(&linked_nodes, Some(min_vblank));

        assert!(
            !candidates.contains_key(&packet),
            "recent header must not revive stale command-shaped primitive bodies"
        );

        let (packets, words) = bus.replay_recent_unlinked_primitive_packets(&linked_nodes);
        assert_eq!((packets, words), (0, 0));
    }

    #[test]
    fn gpu_unlinked_replay_rejects_playfield_packet_when_command_body_is_stale() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let packet = 0x0038_b000;
        let linked_nodes = std::collections::HashSet::new();

        bus.vblank_count = 1;
        bus.write_u32(packet + 4, 0x2c80_8080);
        bus.write_u32(packet + 8, 0x0064_0064);
        bus.write_u32(packet + 12, 0x0001_0001);
        bus.write_u32(packet + 16, 0x0064_00c8);
        bus.write_u32(packet + 20, 0x0002_0002);
        bus.write_u32(packet + 24, 0x00c8_0064);
        bus.write_u32(packet + 28, 0x0003_0003);
        bus.write_u32(packet + 32, 0x00c8_00c8);
        bus.write_u32(packet + 36, 0x0004_0004);

        bus.vblank_count = BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW + 16;
        let min_vblank = bus
            .vblank_count
            .saturating_sub(BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW);
        bus.write_u32(packet, 0x09ff_ffff);

        let candidates =
            bus.collect_unlinked_primitive_replay_candidates(&linked_nodes, Some(min_vblank));

        assert!(
            !candidates.contains_key(&packet),
            "fresh headers must not replay stale but plausible draw bodies"
        );
    }

    #[test]
    fn gpu_linked_list_dma_rejects_recent_header_with_stale_draw_body() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let packet = 0x0039_05f8;
        let stale_words = [
            0x2c7f_7f7f,
            0x014a_0100,
            0x7d18_3840,
            0x014a_0202,
            0x000c_3880,
            0x016c_0100,
            0x0000_3f40,
            0x016c_0202,
            0x0000_3f80,
        ];

        bus.vblank_count = 18;
        for (index, word) in stale_words.iter().copied().enumerate() {
            bus.write_u32(packet + 4 + index as u32 * 4, word);
        }

        bus.vblank_count = BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW + 64;
        bus.write_u32(packet, 0x09ff_ffff);
        let commands_before = bus.io.gpu.commands_seen;
        bus.write_u32(DMA_GPU_MADR, packet);
        bus.write_u32(DMA_GPU_CHCR, 0x0100_0401);

        assert_eq!(
            bus.io.gpu.commands_seen, commands_before,
            "recent linked-list headers must not revive stale playfield draw bodies"
        );
    }

    #[test]
    fn gpu_linked_list_dma_allows_recent_header_with_recent_draw_body() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let packet = 0x0039_15f8;
        let words = [
            0x2c7f_7f7f,
            0x014a_0100,
            0x7d18_3840,
            0x014a_0202,
            0x000c_3880,
            0x016c_0100,
            0x0000_3f40,
            0x016c_0202,
            0x0000_3f80,
        ];

        bus.vblank_count = BR2_UNLINKED_PRIMITIVE_REPLAY_VBLANK_WINDOW + 64;
        bus.write_u32(packet, 0x09ff_ffff);
        for (index, word) in words.iter().copied().enumerate() {
            bus.write_u32(packet + 4 + index as u32 * 4, word);
        }
        let commands_before = bus.io.gpu.commands_seen;
        bus.write_u32(DMA_GPU_MADR, packet);
        bus.write_u32(DMA_GPU_CHCR, 0x0100_0401);

        assert!(
            bus.io.gpu.commands_seen > commands_before,
            "fresh linked-list draw bodies must still be submitted"
        );
    }

    #[test]
    fn gpu_unlinked_replay_uses_recent_headerless_draw_command_streams() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let command = 0x0038_5000;
        let linked_nodes = std::collections::HashSet::new();

        bus.vblank_count = 24;
        bus.write_u32(command, 0x6000_00ff);
        bus.write_u32(command + 4, 0x0064_0064);
        bus.write_u32(command + 8, 0x0010_0010);

        let (packets, words) = bus.replay_recent_unlinked_primitive_packets(&linked_nodes);
        let (width, _, frame) = bus.io.display_rgb_frame();
        let pixel = frame[100 * width + 100];

        assert_eq!((packets, words), (1, 3));
        assert_eq!(pixel, 0x00ff_0000);
    }

    #[test]
    fn gpu_unlinked_replay_skips_top_warning_glyph_quads() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let packet = 0x003a_2370;
        let linked_nodes = std::collections::HashSet::new();

        bus.vblank_count = 18;
        bus.write_u32(packet, 0x09ff_ffff);
        bus.write_u32(packet + 4, 0x2d7f_7f7f);
        bus.write_u32(packet + 8, 0x0048_0061);
        bus.write_u32(packet + 12, 0x78df_1078);
        bus.write_u32(packet + 16, 0x0048_0067);
        bus.write_u32(packet + 20, 0x002f_107e);
        bus.write_u32(packet + 24, 0x0050_0061);
        bus.write_u32(packet + 28, 0x0000_1878);
        bus.write_u32(packet + 32, 0x0050_0067);
        bus.write_u32(packet + 36, 0x0000_187e);

        bus.vblank_count = 432;
        bus.write_u32(packet, 0x09ff_ffff);

        let candidates = bus.collect_unlinked_primitive_replay_candidates(&linked_nodes, None);
        assert!(
            !candidates.contains_key(&packet),
            "top warning glyphs must not be replayed as playfield primitives"
        );

        let (packets, words) = bus.replay_recent_unlinked_primitive_packets(&linked_nodes);
        assert_eq!((packets, words), (0, 0));
    }

    #[test]
    fn gpu_unlinked_replay_skips_state_commands_that_pollute_draw_offset() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let packet = 0x0038_6000;
        let linked_nodes = std::collections::HashSet::new();

        bus.write_u32(packet, 0x04ff_ffff);
        bus.write_u32(packet + 4, 0x6000_ff00);
        bus.write_u32(packet + 8, 0x0064_0064);
        bus.write_u32(packet + 12, 0x0010_0010);
        bus.write_u32(packet + 16, 0xe530_0600);

        let (packets, words) = bus.replay_recent_unlinked_primitive_packets(&linked_nodes);
        let (width, _, frame) = bus.io.display_rgb_frame();
        let pixel = frame[100 * width + 100];
        let io_json = bus.io_json();

        assert_eq!((packets, words), (1, 3));
        assert_eq!(pixel, 0x0000_ff00);
        assert!(io_json.contains("\"gpu_drawing_offset\":0"));
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
    fn gp0_replay_safe_draw_rejects_oversized_shaded_textured_quad() {
        let corrupt = [
            0x3da0_fc20,
            0x02aa_f700,
            0x0000_0000,
            0x00e9_0000,
            0x000c_fe97,
            0x0000_0000,
            0x0000_0000,
            0x38a0_03c0,
            0xff1e_1ea0,
            0x0000_0000,
            0x00f9_0000,
            0xfffa_fee0,
        ];

        assert!(gp0_command_has_playfield_draw_bounds(&corrupt));
        assert!(!gp0_command_is_replay_safe_draw(&corrupt));
        assert!(gp0_replay_safe_draw_command_ranges(&corrupt).is_empty());
    }

    #[test]
    fn gp0_replay_safe_draw_rejects_title_overlay_textured_triangles() {
        let corrupt_triangles = [
            [
                0x2634_0082,
                0x0000_308d,
                0x0000_0000,
                0x00c1_0000,
                0x0308_ff6d,
                0xa000_e1f4,
                0x0000_0300,
            ],
            [
                0x2435_cf8e,
                0x00eb_f0eb,
                0x0000_0000,
                0x00f7_0000,
                0x02ed_feba,
                0x0000_0000,
                0x0000_0000,
            ],
            [
                0x26fb_00b9,
                0x0000_2ffd,
                0x0000_0000,
                0x00bc_0000,
                0x0304_ff6f,
                0xa000_e1f4,
                0x0000_0300,
            ],
            [
                0x2720_0000,
                0x0000_30a0,
                0x0000_0000,
                0x0139_0000,
                0x0368_ff72,
                0x0000_0000,
                0x3c08_0000,
            ],
            [
                0x248e_cff4,
                0x015f_f072,
                0x0000_0000,
                0x0111_0000,
                0x02ef_fedd,
                0x0000_0000,
                0x0000_0000,
            ],
        ];

        for corrupt in corrupt_triangles {
            assert!(gp0_command_has_playfield_draw_bounds(&corrupt));
            assert!(
                !gp0_command_is_replay_safe_draw(&corrupt),
                "corrupt title overlay primitive should be rejected: {corrupt:?}"
            );
            assert!(gp0_replay_safe_draw_command_ranges(&corrupt).is_empty());
        }
    }

    #[test]
    fn gp0_replay_safe_draw_rejects_title_overlay_shaded_textured_quad() {
        let corrupt = [
            0x3c08_0000,
            0x2a2e_1887,
            0xfeac_d98e,
            0x0000_0000,
            0x0197_0000,
            0x0313_ffd6,
            0x0000_0000,
            0x0000_0000,
            0x21b4_24cc,
            0x02aa_e05a,
            0x0000_0000,
            0x0128_0000,
        ];

        assert!(gp0_command_has_playfield_draw_bounds(&corrupt));
        assert!(!gp0_command_is_replay_safe_draw(&corrupt));
        assert!(gp0_replay_safe_draw_command_ranges(&corrupt).is_empty());
    }

    #[test]
    fn gp0_replay_safe_draw_rejects_high_atlas_title_overlay_quad() {
        let corrupt = [
            0x2e7f_7f7f,
            0x00fb_00e6,
            0x7958_e000,
            0x00fb_0122,
            0x003f_e01f,
            0x0137_00e6,
            0x0000_ff00,
            0x0137_0122,
            0x0000_ff1f,
        ];

        assert!(gp0_command_has_playfield_draw_bounds(&corrupt));
        assert!(!gp0_command_is_linked_list_artifact_draw(&corrupt));
        assert!(!gp0_command_is_replay_safe_draw(&corrupt));
        assert!(gp0_replay_safe_draw_command_ranges(&corrupt).is_empty());
    }

    #[test]
    fn gp0_replay_safe_draw_rejects_high_atlas_title_sparkle_quad() {
        let corrupt = [
            0x2e7f_7f7f,
            0x009c_016d,
            0x7818_8040,
            0x009c_0175,
            0x003f_8048,
            0x00a4_016d,
            0x0000_8840,
            0x00a4_0175,
            0x0000_8848,
        ];

        assert!(gp0_command_has_playfield_draw_bounds(&corrupt));
        assert!(!gp0_command_is_linked_list_artifact_draw(&corrupt));
        assert!(!gp0_command_is_replay_safe_draw(&corrupt));
        assert!(gp0_replay_safe_draw_command_ranges(&corrupt).is_empty());
    }

    #[test]
    fn gp0_replay_safe_draw_rejects_top_warning_glyph_quad() {
        let corrupt = [
            0x2d7f_7f7f,
            0x0048_0061,
            0x78df_1078,
            0x0048_0067,
            0x002f_107e,
            0x0050_0061,
            0x0000_1878,
            0x0050_0067,
            0x0000_187e,
        ];

        assert!(!gp0_command_has_playfield_draw_bounds(&corrupt));
        assert!(!gp0_command_is_replay_safe_draw(&corrupt));
        assert!(gp0_replay_safe_draw_command_ranges(&corrupt).is_empty());
    }

    #[test]
    fn gp0_replay_safe_draw_uses_variable_textured_rect_size_for_bounds() {
        let small_playfield_sprite = [0x6480_8080, 0x0090_0050, 0x0000_0000, 0x0020_0020];
        let oversized_playfield_sprite = [0x6480_8080, 0x0060_0000, 0x0000_0000, 0x01e0_0280];

        assert!(gp0_command_has_playfield_draw_bounds(
            &small_playfield_sprite
        ));
        assert!(gp0_command_is_replay_safe_draw(&small_playfield_sprite));
        assert_eq!(
            gp0_replay_safe_draw_command_ranges(&small_playfield_sprite),
            vec![0..4]
        );

        assert!(gp0_command_has_playfield_draw_bounds(
            &oversized_playfield_sprite
        ));
        assert!(!gp0_command_is_replay_safe_draw(
            &oversized_playfield_sprite
        ));
        assert!(gp0_replay_safe_draw_command_ranges(&oversized_playfield_sprite).is_empty());
    }

    #[test]
    fn gp0_replay_safe_draw_rejects_primitive_pointer_contaminated_quad() {
        let corrupt = [
            0x2e01_a0df,
            0x00b5_018b,
            0x795a_a0c0,
            0x0900_0000,
            0x2e80_8080,
            0x0106_0189,
            0x8038_a4d0,
            0x8038_a808,
            0x0000_0084,
        ];

        assert!(gp0_command_has_playfield_draw_bounds(&corrupt));
        assert!(!gp0_command_is_replay_safe_draw(&corrupt));
        assert!(gp0_replay_safe_draw_command_ranges(&corrupt).is_empty());
    }

    #[test]
    fn gp0_linked_list_artifact_rejects_high_page_title_stripe_quad() {
        let corrupt = [
            0x2e01_a0df,
            0x00be_01f8,
            0x795a_a0c0,
            0x0900_0000,
            0x2e80_8080,
            0x0072_01d3,
            0x78da_afda,
            0x009f_01d5,
            0x0039_afc4,
        ];

        assert!(gp0_command_has_playfield_draw_bounds(&corrupt));
        assert!(gp0_command_is_linked_list_artifact_draw(&corrupt));
        assert!(!gp0_command_is_replay_safe_draw(&corrupt));
        assert!(gp0_replay_safe_draw_command_ranges(&corrupt).is_empty());
    }

    #[test]
    fn gp0_linked_list_artifact_rejects_captured_high_page_title_stripe_quads() {
        let corrupt_quads = [
            [
                0x2e01_a0df,
                0x0077_01a6,
                0x795a_a0c0,
                0x0900_0000,
                0x2e80_8080,
                0x00b5_018b,
                0x78da_afda,
                0x00c5_01a5,
                0x0039_afc4,
            ],
            [
                0x2e10_a0df,
                0x00b6_0191,
                0x795a_a0c0,
                0x0900_0000,
                0x2e80_8080,
                0x0101_018f,
                0x78da_afda,
                0x00f7_01a8,
                0x0039_afc4,
            ],
            [
                0x2e10_a0df,
                0x0101_018f,
                0x795a_a0c0,
                0x0900_0000,
                0x2e80_8080,
                0x013f_01a5,
                0x78da_afda,
                0x011f_01b7,
                0x0039_afc4,
            ],
        ];

        for corrupt in corrupt_quads {
            assert!(gp0_command_has_playfield_draw_bounds(&corrupt));
            assert!(gp0_command_is_linked_list_artifact_draw(&corrupt));
            assert!(!gp0_command_is_replay_safe_draw(&corrupt));
            assert!(gp0_replay_safe_draw_command_ranges(&corrupt).is_empty());
        }
    }

    #[test]
    fn gp0_replay_safe_draw_rejects_left_edge_title_shaded_triangle() {
        let corrupt = [
            0x32a6_2408,
            0xfa2e_0039,
            0x0000_0000,
            0x0150_0000,
            0x031d_fe9f,
            0x0000_0000,
        ];

        assert!(gp0_command_has_playfield_draw_bounds(&corrupt));
        assert!(gp0_command_is_linked_list_artifact_draw(&corrupt));
        assert!(!gp0_command_is_replay_safe_draw(&corrupt));
        assert!(gp0_replay_safe_draw_command_ranges(&corrupt).is_empty());
    }

    #[test]
    fn gp0_replay_safe_draw_rejects_left_edge_title_flat_triangle() {
        let corrupt = [0x2200_eec0, 0x01d8_3040, 0x0000_0000, 0x0139_0000];

        assert!(gp0_command_has_playfield_draw_bounds(&corrupt));
        assert!(gp0_command_is_linked_list_artifact_draw(&corrupt));
        assert!(!gp0_command_is_replay_safe_draw(&corrupt));
        assert!(gp0_replay_safe_draw_command_ranges(&corrupt).is_empty());
    }

    #[test]
    fn gp0_replay_safe_draw_rejects_left_edge_title_textured_sliver() {
        let corrupt = [
            0x3c08_0000,
            0x293b_17ee,
            0xfeac_d7db,
            0x0000_0000,
            0x01ad_0000,
            0x030b_ffdb,
            0x0000_0000,
            0x0000_0000,
            0x1e67_24d9,
            0x02aa_d9f4,
            0x0000_0000,
            0x0137_0000,
        ];

        assert!(gp0_command_has_playfield_draw_bounds(&corrupt));
        assert!(gp0_command_is_linked_list_artifact_draw(&corrupt));
        assert!(!gp0_command_is_replay_safe_draw(&corrupt));
        assert!(gp0_replay_safe_draw_command_ranges(&corrupt).is_empty());
    }

    #[test]
    fn gp0_replay_safe_draw_accepts_bloody_roar_status_quad() {
        let status_quad = [
            0x2c80_8080,
            0x0181_0148,
            0x7adf_e000,
            0x0181_01c8,
            0x000f_e080,
            0x01bf_0148,
            0x0000_ff00,
            0x01bf_01c8,
            0x0000_ff80,
        ];

        assert!(gp0_command_has_playfield_draw_bounds(&status_quad));
        assert!(!gp0_command_is_linked_list_artifact_draw(&status_quad));
        assert!(gp0_command_is_replay_safe_draw(&status_quad));
        assert_eq!(
            gp0_replay_safe_draw_command_ranges(&status_quad),
            vec![0..9]
        );
    }

    #[test]
    fn gpu_unlinked_replay_skips_oversized_shaded_textured_quad_candidate() {
        let mut bus = Bus::new(Vec::new(), 4 * 1024 * 1024);
        let packet = 0x0038_7000;
        let linked_nodes = std::collections::HashSet::new();
        let corrupt = [
            0x3da0_fc20,
            0x02aa_f700,
            0x0000_0000,
            0x00e9_0000,
            0x000c_fe97,
            0x0000_0000,
            0x0000_0000,
            0x38a0_03c0,
            0xff1e_1ea0,
            0x0000_0000,
            0x00f9_0000,
            0xfffa_fee0,
        ];

        bus.write_u32(packet, 0x0cff_ffff);
        for (index, word) in corrupt.into_iter().enumerate() {
            bus.write_u32(packet + 4 + index as u32 * 4, word);
        }

        let candidates = bus.collect_unlinked_primitive_replay_candidates(&linked_nodes, None);
        assert!(!candidates.contains_key(&packet));

        let (packets, words) = bus.replay_recent_unlinked_primitive_packets(&linked_nodes);
        assert_eq!((packets, words), (0, 0));
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

        assert_eq!(bus.read_u8(SIO_DATA), 0x1f);
        assert!(bus.runtime_probe_json().contains("\"selected\":true"));

        bus.write_u8(0x1fa1_0300, 0x00);
        bus.write_u8(SIO_DATA, 0);

        assert_eq!(bus.read_u8(SIO_DATA), 0xff);
        assert!(bus.runtime_probe_json().contains("\"selected\":false"));

        bus.write_u8(0x1fa1_0300, 0x10);
        bus.write_u8(SIO_DATA, 0);

        assert_eq!(bus.read_u8(SIO_DATA), 0xff);
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
