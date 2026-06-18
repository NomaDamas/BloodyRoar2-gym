use std::cell::{Cell, RefCell};

use crate::action::ActionButtons;
use crate::native::io::{
    DMA_GPU_CHCR, DMA_OTC_CHCR, IO_REGION_END, IO_REGION_START, Io, io_access_for,
};
use crate::native::platform::{NativePlatformOps, PreferredNativePlatform};

const DMA_CHANNEL_COUNT: usize = 7;
const DMA_GPU_CHANNEL: usize = 2;
const DMA_OTC_CHANNEL: usize = 6;
const DMA_GPU_COMPLETION_DELAY_CYCLES: u64 = 4_096;
const DMA_OTC_COMPLETION_DELAY_CYCLES: u64 = 512;
const BR2_DRAW_SYNC_FLAG_VIRTUAL: u32 = 0x803a_2210;
const BR2_DRAW_SYNC_FLAG_PHYSICAL: u32 = 0x003a_2210;

#[derive(Clone, Debug)]
pub struct Bus {
    ram: Vec<u8>,
    scratchpad: Vec<u8>,
    rom: Vec<u8>,
    banked_roms: Vec<u8>,
    zn_board: ZnBoard,
    cache_control: u32,
    cache_isolated: bool,
    pending_dma_completion_cycles: [u64; DMA_CHANNEL_COUNT],
    vblank_cycle_accumulator: u64,
    vblank_draw_sync_clears: u64,
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
            pending_dma_completion_cycles: [0; DMA_CHANNEL_COUNT],
            vblank_cycle_accumulator: 0,
            vblank_draw_sync_clears: 0,
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
            self.sync_dma_irq();
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
            self.sync_dma_irq();
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
            self.io.write_u32(io_address, value);
            self.process_dma_transfer(io_address, value);
            self.sync_dma_irq();
            self.record_access_trace("write", "io", address, 4, value);
            return;
        }

        let bytes = PreferredNativePlatform::write_le_u32(value);
        self.write_bytes(address, &bytes);
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

    pub fn set_cache_isolated(&mut self, isolated: bool) {
        self.cache_isolated = isolated;
    }

    pub fn cache_isolated(&self) -> bool {
        self.cache_isolated
    }

    pub fn tick(&mut self, cycles: u64) {
        self.io.tick(cycles);
        self.tick_pending_dma(cycles);
        self.vblank_cycle_accumulator = self.vblank_cycle_accumulator.saturating_add(cycles);
        while self.vblank_cycle_accumulator >= 566_000 {
            self.vblank_cycle_accumulator -= 566_000;
            self.io.irq.status |= 1;
            self.complete_draw_sync_on_vblank();
        }
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
            "{{\"br2_draw_sync_flag\":{},\"vblank_draw_sync_clears\":{}}}",
            self.read_ram_u32_physical(BR2_DRAW_SYNC_FLAG_PHYSICAL)
                .unwrap_or(0),
            self.vblank_draw_sync_clears
        )
    }

    pub fn io_json(&self) -> String {
        self.io.json()
    }

    pub fn set_input(&mut self, buttons: ActionButtons) {
        self.io.set_input(buttons);
        self.zn_board.set_input(buttons);
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
        );
    }

    fn sync_dma_irq(&mut self) {
        if self.io.dma.irq_pending() {
            self.io.irq.status |= 1 << 3;
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
            DMA_GPU_CHCR => self.process_gpu_dma(control),
            DMA_OTC_CHCR => self.process_otc_dma(),
            _ => {}
        }
    }

    fn process_gpu_dma(&mut self, control: u32) {
        let Some(channel) = self.io.dma.channel_state(2) else {
            return;
        };

        if control & 0x0400 != 0 {
            self.process_gpu_linked_list_dma(channel.madr);
        } else {
            self.process_gpu_block_dma(channel.madr, channel.bcr);
        }
        self.schedule_dma_completion(DMA_GPU_CHANNEL, DMA_GPU_COMPLETION_DELAY_CYCLES);
    }

    fn process_gpu_linked_list_dma(&mut self, start_address: u32) {
        let mut address = start_address & 0x00ff_fffc;
        for _ in 0..4096 {
            let header = self.read_u32(address);
            let words = (header >> 24).min(1024);
            for index in 0..words {
                let command = self.read_u32(address.wrapping_add(4 + index * 4));
                self.io.gpu.write_gp0(command);
            }

            let next = header & 0x00ff_ffff;
            if next == 0x00ff_ffff {
                break;
            }
            address = next & 0x00ff_fffc;
        }
    }

    fn process_gpu_block_dma(&mut self, start_address: u32, bcr: u32) {
        let words = dma_word_count(bcr).min(4096);
        let mut address = start_address & 0x00ff_fffc;
        for _ in 0..words {
            let command = self.read_u32(address);
            self.io.gpu.write_gp0(command);
            address = address.wrapping_add(4);
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
            self.write_u32(address, next);
            address = address.wrapping_sub(4);
        }
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
            return self.banked_roms[offset..offset + len].to_vec();
        }

        self.record_access_trace("read", "unmapped", address, len as u8, 0);
        vec![0; len]
    }

    fn read_ram_u32_physical(&self, physical: u32) -> Option<u32> {
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

    fn write_bytes(&mut self, address: u32, bytes: &[u8]) {
        if self.cache_isolated && cacheable_address(address) {
            self.record_access_trace(
                "write",
                "cache_isolated",
                address,
                bytes.len() as u8,
                bytes_to_le_u32(bytes),
            );
            return;
        }

        if let Some(offset) = ram_offset(address, self.ram.len(), bytes.len()) {
            self.ram[offset..offset + bytes.len()].copy_from_slice(bytes);
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
            0x1fa0_0000 => active_low_player_input(self.input),
            0x1fa0_0100 => 0xffff_ffff,
            0x1fa0_0200 => active_low_system_input(self.input),
            0x1fa0_0300 | 0x1fa1_0000 | 0x1fa1_0100 => 0xffff_ffff,
            0x1fa1_0200 => 0x0000_0069,
            0x1fa1_0300 => self.znsecsel as u32,
            0x1fa2_0000 => self.coin as u32,
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
            "{{\"rom_bank\":{},\"znsecsel\":{},\"coin\":{},\"sound_irq_latch\":{}}}",
            self.rom_bank, self.znsecsel, self.coin, self.sound_irq_latch
        )
    }

    fn cat702_1_select(&self) -> bool {
        self.znsecsel & 0x04 != 0
    }

    fn cat702_2_select(&self) -> bool {
        self.znsecsel & 0x08 != 0
    }

    fn set_input(&mut self, input: ActionButtons) {
        self.input = input;
    }
}

fn active_low_player_input(input: ActionButtons) -> u32 {
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

fn active_low_system_input(input: ActionButtons) -> u32 {
    let mut value = 0xffff_ffff;
    clear_bit_if(&mut value, 0x0000_0001, input.coin);
    clear_bit_if(&mut value, 0x0000_0008, input.start);
    value
}

fn clear_bit_if(value: &mut u32, bit: u32, clear: bool) {
    if clear {
        *value &= !bit;
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

fn cache_control_address(address: u32) -> bool {
    address == 0xfffe_0130 || physical_address(address) == 0x1ffe_0130
}

fn cacheable_address(address: u32) -> bool {
    address < 0xa000_0000
}

fn physical_address(address: u32) -> u32 {
    address & 0x1fff_ffff
}

fn optional_u32_json(value: Option<u32>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn optional_u32_hex_json(value: Option<u32>) -> String {
    value.map_or_else(|| "null".to_string(), |value| format!("\"0x{value:08x}\""))
}

#[cfg(test)]
mod tests {
    use super::{BR2_DRAW_SYNC_FLAG_VIRTUAL, Bus, DMA_GPU_COMPLETION_DELAY_CYCLES};
    use crate::action::ActionButtons;
    use crate::native::io::{
        DMA_GPU_CHCR, DMA_GPU_MADR, DMA_INTERRUPT, DMA_OTC_BCR, DMA_OTC_CHCR, DMA_OTC_MADR,
        DMA_SPU_CHCR, GPU_GP0, IRQ_MASK, IRQ_STATUS, SIO_DATA, SPU_REGION_START,
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
            ..ActionButtons::default()
        });

        assert_eq!(bus.io.controller.p1_state & 0x0008, 0);
        assert_eq!(bus.io.controller.p1_state & 0x0010, 0);
        assert_eq!(bus.io.controller.p1_state & 0x4000, 0);
        assert_eq!(bus.read_u8(0x1fa0_0000) & 0x11, 0);
        assert_eq!(bus.read_u8(0x1fa0_0200) & 0x09, 0);
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
    fn bus_raises_dma_irq_when_enabled_channel_completes() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.write_u32(DMA_INTERRUPT, (1 << 23) | (1 << 20));
        bus.write_u32(DMA_SPU_CHCR, 1 << 24);

        assert_eq!(bus.io.irq.status & (1 << 3), 1 << 3);
        assert!(bus.io.dma.irq_pending());
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
