#![allow(clippy::collapsible_if, clippy::too_many_arguments)]

use std::cell::Cell;
use std::sync::OnceLock;

use crate::action::ActionButtons;
use crate::native::framebuffer::{
    ClipRect, DEFAULT_DISPLAY_HEIGHT, DEFAULT_DISPLAY_WIDTH, FrameBufferBounds, FrameBufferStats,
    FrameBufferWindow, NativeFrameBuffer, PSX_VRAM_HEIGHT, PixelWriteOptions, Point,
    TextureCoordinate, TextureDrawOptions, TextureWindow, TexturedDrawStats, TexturedPoint,
    VRAM_HEIGHT, VRAM_WIDTH, bytes_base64, png_from_rgb888_pixels,
};

pub const IO_REGION_START: u32 = 0x1f80_1000;
pub const IO_REGION_END: u32 = 0x1f80_1fff;

pub const MEMCTRL_EXP1_BASE: u32 = 0x1f80_1000;
pub const MEMCTRL_EXP2_BASE: u32 = 0x1f80_1004;
pub const MEMCTRL_EXP1_DELAY_SIZE: u32 = 0x1f80_1008;
pub const MEMCTRL_EXP3_DELAY_SIZE: u32 = 0x1f80_100c;
pub const MEMCTRL_BIOS_DELAY_SIZE: u32 = 0x1f80_1010;
pub const MEMCTRL_SPU_DELAY_SIZE: u32 = 0x1f80_1014;
pub const MEMCTRL_CDROM_DELAY_SIZE: u32 = 0x1f80_1018;
pub const MEMCTRL_EXP2_DELAY_SIZE: u32 = 0x1f80_101c;
pub const MEMCTRL_COMMON_DELAY: u32 = 0x1f80_1020;

pub const SIO_DATA: u32 = 0x1f80_1040;
pub const SIO_STATUS: u32 = 0x1f80_1044;
pub const SIO_MODE: u32 = 0x1f80_1048;
pub const SIO_CONTROL: u32 = 0x1f80_104a;
pub const SIO_BAUD: u32 = 0x1f80_104e;

pub const RAM_SIZE_CONTROL: u32 = 0x1f80_1060;
pub const IRQ_STATUS: u32 = 0x1f80_1070;
pub const IRQ_MASK: u32 = 0x1f80_1074;

pub const DMA_REGION_START: u32 = 0x1f80_1080;
pub const DMA_REGION_END: u32 = 0x1f80_10ff;
pub const DMA_CHANNEL_STRIDE: u32 = 0x10;
pub const DMA_MDEC_IN_MADR: u32 = 0x1f80_1080;
pub const DMA_MDEC_IN_BCR: u32 = 0x1f80_1084;
pub const DMA_MDEC_IN_CHCR: u32 = 0x1f80_1088;
pub const DMA_MDEC_OUT_MADR: u32 = 0x1f80_1090;
pub const DMA_MDEC_OUT_BCR: u32 = 0x1f80_1094;
pub const DMA_MDEC_OUT_CHCR: u32 = 0x1f80_1098;
pub const DMA_GPU_MADR: u32 = 0x1f80_10a0;
pub const DMA_GPU_BCR: u32 = 0x1f80_10a4;
pub const DMA_GPU_CHCR: u32 = 0x1f80_10a8;
pub const DMA_CDROM_MADR: u32 = 0x1f80_10b0;
pub const DMA_CDROM_BCR: u32 = 0x1f80_10b4;
pub const DMA_CDROM_CHCR: u32 = 0x1f80_10b8;
pub const DMA_SPU_MADR: u32 = 0x1f80_10c0;
pub const DMA_SPU_BCR: u32 = 0x1f80_10c4;
pub const DMA_SPU_CHCR: u32 = 0x1f80_10c8;
pub const DMA_PIO_MADR: u32 = 0x1f80_10d0;
pub const DMA_PIO_BCR: u32 = 0x1f80_10d4;
pub const DMA_PIO_CHCR: u32 = 0x1f80_10d8;
pub const DMA_OTC_MADR: u32 = 0x1f80_10e0;
pub const DMA_OTC_BCR: u32 = 0x1f80_10e4;
pub const DMA_OTC_CHCR: u32 = 0x1f80_10e8;
pub const DMA_CONTROL: u32 = 0x1f80_10f0;
pub const DMA_INTERRUPT: u32 = 0x1f80_10f4;

pub const TIMER0_COUNTER: u32 = 0x1f80_1100;
pub const TIMER0_MODE: u32 = 0x1f80_1104;
pub const TIMER0_TARGET: u32 = 0x1f80_1108;
pub const TIMER1_COUNTER: u32 = 0x1f80_1110;
pub const TIMER1_MODE: u32 = 0x1f80_1114;
pub const TIMER1_TARGET: u32 = 0x1f80_1118;
pub const TIMER2_COUNTER: u32 = 0x1f80_1120;
pub const TIMER2_MODE: u32 = 0x1f80_1124;
pub const TIMER2_TARGET: u32 = 0x1f80_1128;

pub const CDROM_INDEX_STATUS: u32 = 0x1f80_1800;
pub const CDROM_RESPONSE: u32 = 0x1f80_1801;
pub const CDROM_DATA: u32 = 0x1f80_1802;
pub const CDROM_INTERRUPT_ENABLE: u32 = 0x1f80_1803;
pub const GPU_GP0: u32 = 0x1f80_1810;
pub const GPU_GP1: u32 = 0x1f80_1814;
pub const MDEC_COMMAND: u32 = 0x1f80_1820;
pub const MDEC_STATUS: u32 = 0x1f80_1824;
pub const SPU_REGION_START: u32 = 0x1f80_1c00;
pub const SPU_REGION_END: u32 = 0x1f80_1dff;

const IRQ_BITS: u32 = 0x07ff;
const IRQ_CONTROLLER: u32 = 1 << 7;
const DMA_CHANNEL_COUNT: usize = 7;
const DMA_INTERRUPT_IRQ_ENABLE_MASK: u32 = 0x007f_0000;
const DMA_INTERRUPT_FLAG_MASK: u32 = 0x7f00_0000;
const SIO_STATUS_IRQ_REQUEST: u16 = 1 << 9;
const GP0_RECENT_COMMAND_LIMIT: usize = 16;
const GP0_RECENT_COMMAND_WORD_LIMIT: usize = 12;
const GP1_RECENT_COMMAND_LIMIT: usize = 16;
const GPU_RECENT_TRANSFER_LIMIT: usize = 16;
const GPU_RECENT_DRAW_LIMIT: usize = 128;
const GPU_TOP_DRAW_LIMIT: usize = 12;
const GPU_FOCUS_DRAW_LIMIT: usize = 64;
const GPU_OVERLAP_DRAW_LIMIT: usize = 96;
const GPU_DRAW_CAPTURE_LIMIT: usize = 64;
const GPU_DISPLAY_AREA_HISTORY_LIMIT: usize = 32;
const GPU_IMAGE_UPLOAD_RECT_LIMIT: usize = 128;
const GP0_BEST_OBSERVATION_EAGER_COMMANDS: u64 = 64;
const GP0_BEST_OBSERVATION_COMMAND_INTERVAL: u64 = 32_768;
const GP0_BEST_OBSERVATION_DRAW_INTERVAL: u64 = 128;
const STALE_PRESENTATION_CAPTURE_GRACE: u64 = 8;
const DISPLAY_RESOLVE_MIN_IMAGE_UPLOAD_COMMANDS: u64 = 16;
const DISPLAY_RESOLVE_MIN_TEXTURED_TRIANGLE_COMMANDS: u64 = 512;
const DISPLAY_RESOLVE_MIN_TEXTURED_RECT_COMMANDS: u64 = 64;
const DISPLAY_RESOLVE_MIN_TEXTURED_WRITTEN_PIXELS: u64 = 8_192;
const DISPLAY_SCENE_MAX_CHANNEL_DOMINANCE: u64 = 2;
const VRAM_X_MASK: u32 = 0x03ff;
const VRAM_Y_MASK: u32 = 0x03ff;

pub const ACCESS_WIDTH_8: u8 = 1 << 0;
pub const ACCESS_WIDTH_16: u8 = 1 << 1;
pub const ACCESS_WIDTH_32: u8 = 1 << 2;
pub const ACCESS_WIDTH_ANY: u8 = ACCESS_WIDTH_8 | ACCESS_WIDTH_16 | ACCESS_WIDTH_32;
const IO_REGISTER_BYTES: usize = (IO_REGION_END - IO_REGION_START + 1) as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IoAccess {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

impl IoAccess {
    pub const fn readable(self) -> bool {
        matches!(self, Self::ReadOnly | Self::ReadWrite)
    }

    pub const fn writable(self) -> bool {
        matches!(self, Self::WriteOnly | Self::ReadWrite)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IoDevice {
    MemoryControl,
    SerialController,
    InterruptController,
    Dma,
    Timer,
    Cdrom,
    Gpu,
    Mdec,
    Spu,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IoRegister {
    pub address: u32,
    pub name: &'static str,
    pub device: IoDevice,
    pub access: IoAccess,
    pub access_widths: u8,
    pub description: &'static str,
}

impl IoRegister {
    pub const fn supports_width(self, width: u8) -> bool {
        self.access_widths & width != 0
    }
}

pub const IO_REGISTER_MAP: &[IoRegister] = &[
    register(
        MEMCTRL_EXP1_BASE,
        "MEMCTRL_EXP1_BASE",
        IoDevice::MemoryControl,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_32,
        "Expansion region 1 base address control",
    ),
    register(
        MEMCTRL_EXP2_BASE,
        "MEMCTRL_EXP2_BASE",
        IoDevice::MemoryControl,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_32,
        "Expansion region 2 base address control",
    ),
    register(
        MEMCTRL_EXP1_DELAY_SIZE,
        "MEMCTRL_EXP1_DELAY_SIZE",
        IoDevice::MemoryControl,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_32,
        "Expansion region 1 access timing and size",
    ),
    register(
        MEMCTRL_EXP3_DELAY_SIZE,
        "MEMCTRL_EXP3_DELAY_SIZE",
        IoDevice::MemoryControl,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_32,
        "Expansion region 3 access timing and size",
    ),
    register(
        MEMCTRL_BIOS_DELAY_SIZE,
        "MEMCTRL_BIOS_DELAY_SIZE",
        IoDevice::MemoryControl,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_32,
        "BIOS ROM access timing and size",
    ),
    register(
        MEMCTRL_SPU_DELAY_SIZE,
        "MEMCTRL_SPU_DELAY_SIZE",
        IoDevice::MemoryControl,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_32,
        "SPU region access timing and size",
    ),
    register(
        MEMCTRL_CDROM_DELAY_SIZE,
        "MEMCTRL_CDROM_DELAY_SIZE",
        IoDevice::MemoryControl,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_32,
        "CD-ROM region access timing and size",
    ),
    register(
        MEMCTRL_EXP2_DELAY_SIZE,
        "MEMCTRL_EXP2_DELAY_SIZE",
        IoDevice::MemoryControl,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_32,
        "Expansion region 2 access timing and size",
    ),
    register(
        MEMCTRL_COMMON_DELAY,
        "MEMCTRL_COMMON_DELAY",
        IoDevice::MemoryControl,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_32,
        "Common memory control delay register",
    ),
    register(
        SIO_DATA,
        "SIO_DATA",
        IoDevice::SerialController,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_8 | ACCESS_WIDTH_16 | ACCESS_WIDTH_32,
        "Controller and memory-card serial data port",
    ),
    register(
        SIO_STATUS,
        "SIO_STATUS",
        IoDevice::SerialController,
        IoAccess::ReadOnly,
        ACCESS_WIDTH_16,
        "Controller and memory-card serial status",
    ),
    register(
        SIO_MODE,
        "SIO_MODE",
        IoDevice::SerialController,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_16,
        "Controller and memory-card serial mode",
    ),
    register(
        SIO_CONTROL,
        "SIO_CONTROL",
        IoDevice::SerialController,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_16,
        "Controller and memory-card serial control",
    ),
    register(
        SIO_BAUD,
        "SIO_BAUD",
        IoDevice::SerialController,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_16,
        "Controller and memory-card serial baud rate",
    ),
    register(
        RAM_SIZE_CONTROL,
        "RAM_SIZE_CONTROL",
        IoDevice::MemoryControl,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_32,
        "Main RAM size control",
    ),
    register(
        IRQ_STATUS,
        "IRQ_STATUS",
        IoDevice::InterruptController,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_16 | ACCESS_WIDTH_32,
        "Interrupt request status and acknowledge bits",
    ),
    register(
        IRQ_MASK,
        "IRQ_MASK",
        IoDevice::InterruptController,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_16 | ACCESS_WIDTH_32,
        "Interrupt enable mask",
    ),
    dma_register(
        DMA_MDEC_IN_MADR,
        "DMA_MDEC_IN_MADR",
        "DMA channel 0 base address",
    ),
    dma_register(
        DMA_MDEC_IN_BCR,
        "DMA_MDEC_IN_BCR",
        "DMA channel 0 block control",
    ),
    dma_register(
        DMA_MDEC_IN_CHCR,
        "DMA_MDEC_IN_CHCR",
        "DMA channel 0 channel control",
    ),
    dma_register(
        DMA_MDEC_OUT_MADR,
        "DMA_MDEC_OUT_MADR",
        "DMA channel 1 base address",
    ),
    dma_register(
        DMA_MDEC_OUT_BCR,
        "DMA_MDEC_OUT_BCR",
        "DMA channel 1 block control",
    ),
    dma_register(
        DMA_MDEC_OUT_CHCR,
        "DMA_MDEC_OUT_CHCR",
        "DMA channel 1 channel control",
    ),
    dma_register(DMA_GPU_MADR, "DMA_GPU_MADR", "DMA channel 2 base address"),
    dma_register(DMA_GPU_BCR, "DMA_GPU_BCR", "DMA channel 2 block control"),
    dma_register(
        DMA_GPU_CHCR,
        "DMA_GPU_CHCR",
        "DMA channel 2 channel control",
    ),
    dma_register(
        DMA_CDROM_MADR,
        "DMA_CDROM_MADR",
        "DMA channel 3 base address",
    ),
    dma_register(
        DMA_CDROM_BCR,
        "DMA_CDROM_BCR",
        "DMA channel 3 block control",
    ),
    dma_register(
        DMA_CDROM_CHCR,
        "DMA_CDROM_CHCR",
        "DMA channel 3 channel control",
    ),
    dma_register(DMA_SPU_MADR, "DMA_SPU_MADR", "DMA channel 4 base address"),
    dma_register(DMA_SPU_BCR, "DMA_SPU_BCR", "DMA channel 4 block control"),
    dma_register(
        DMA_SPU_CHCR,
        "DMA_SPU_CHCR",
        "DMA channel 4 channel control",
    ),
    dma_register(DMA_PIO_MADR, "DMA_PIO_MADR", "DMA channel 5 base address"),
    dma_register(DMA_PIO_BCR, "DMA_PIO_BCR", "DMA channel 5 block control"),
    dma_register(
        DMA_PIO_CHCR,
        "DMA_PIO_CHCR",
        "DMA channel 5 channel control",
    ),
    dma_register(DMA_OTC_MADR, "DMA_OTC_MADR", "DMA channel 6 base address"),
    dma_register(DMA_OTC_BCR, "DMA_OTC_BCR", "DMA channel 6 block control"),
    dma_register(
        DMA_OTC_CHCR,
        "DMA_OTC_CHCR",
        "DMA channel 6 channel control",
    ),
    dma_register(DMA_CONTROL, "DMA_CONTROL", "DMA global priority/control"),
    dma_register(
        DMA_INTERRUPT,
        "DMA_INTERRUPT",
        "DMA interrupt control/status",
    ),
    timer_register(
        TIMER0_COUNTER,
        "TIMER0_COUNTER",
        "Root counter 0 current value",
    ),
    timer_register(TIMER0_MODE, "TIMER0_MODE", "Root counter 0 mode"),
    timer_register(TIMER0_TARGET, "TIMER0_TARGET", "Root counter 0 target"),
    timer_register(
        TIMER1_COUNTER,
        "TIMER1_COUNTER",
        "Root counter 1 current value",
    ),
    timer_register(TIMER1_MODE, "TIMER1_MODE", "Root counter 1 mode"),
    timer_register(TIMER1_TARGET, "TIMER1_TARGET", "Root counter 1 target"),
    timer_register(
        TIMER2_COUNTER,
        "TIMER2_COUNTER",
        "Root counter 2 current value",
    ),
    timer_register(TIMER2_MODE, "TIMER2_MODE", "Root counter 2 mode"),
    timer_register(TIMER2_TARGET, "TIMER2_TARGET", "Root counter 2 target"),
    register(
        CDROM_INDEX_STATUS,
        "CDROM_INDEX_STATUS",
        IoDevice::Cdrom,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_8,
        "CD-ROM command index and status register",
    ),
    register(
        CDROM_RESPONSE,
        "CDROM_RESPONSE",
        IoDevice::Cdrom,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_8,
        "CD-ROM response FIFO and command parameter port",
    ),
    register(
        CDROM_DATA,
        "CDROM_DATA",
        IoDevice::Cdrom,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_8,
        "CD-ROM data FIFO and interrupt request port",
    ),
    register(
        CDROM_INTERRUPT_ENABLE,
        "CDROM_INTERRUPT_ENABLE",
        IoDevice::Cdrom,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_8,
        "CD-ROM interrupt enable and control port",
    ),
    register(
        GPU_GP0,
        "GPU_GP0",
        IoDevice::Gpu,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_32,
        "GPU command/data port",
    ),
    register(
        GPU_GP1,
        "GPU_GP1",
        IoDevice::Gpu,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_32,
        "GPU status/control port",
    ),
    register(
        MDEC_COMMAND,
        "MDEC_COMMAND",
        IoDevice::Mdec,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_32,
        "MDEC command and data port",
    ),
    register(
        MDEC_STATUS,
        "MDEC_STATUS",
        IoDevice::Mdec,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_32,
        "MDEC status and control port",
    ),
];

pub const IO_REGISTER_RANGES: &[IoRegisterRange] = &[
    IoRegisterRange {
        start: DMA_REGION_START,
        end: DMA_REGION_END,
        device: IoDevice::Dma,
        access: IoAccess::ReadWrite,
        access_widths: ACCESS_WIDTH_32,
        name: "DMA_CHANNEL_WINDOW",
        description: "DMA channel register window, including reserved channel padding",
    },
    IoRegisterRange {
        start: SPU_REGION_START,
        end: SPU_REGION_END,
        device: IoDevice::Spu,
        access: IoAccess::ReadWrite,
        access_widths: ACCESS_WIDTH_16,
        name: "SPU_REGISTER_WINDOW",
        description: "SPU voice, mixer, transfer, and reverb register window",
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IoRegisterRange {
    pub start: u32,
    pub end: u32,
    pub device: IoDevice,
    pub access: IoAccess,
    pub access_widths: u8,
    pub name: &'static str,
    pub description: &'static str,
}

pub const fn is_io_register_address(address: u32) -> bool {
    address >= IO_REGION_START && address <= IO_REGION_END
}

pub fn io_register(address: u32) -> Option<IoRegister> {
    IO_REGISTER_MAP
        .iter()
        .copied()
        .find(|register| register.address == address)
}

pub fn io_register_range(address: u32) -> Option<IoRegisterRange> {
    IO_REGISTER_RANGES
        .iter()
        .copied()
        .find(|range| address >= range.start && address <= range.end)
}

pub fn io_access_for(address: u32, access_len: usize) -> Option<IoAccess> {
    let width = access_width_flag(access_len)?;
    io_register_for_access(address, access_len)
        .map(|register| register.access)
        .or_else(|| {
            io_register_range_for_access(address, access_len, width).map(|range| range.access)
        })
}

fn io_register_for_access(address: u32, access_len: usize) -> Option<IoRegister> {
    let width = access_width_flag(access_len)?;
    IO_REGISTER_MAP.iter().copied().find(|register| {
        let offset = address.saturating_sub(register.address);
        let span = register_span(register.access_widths);
        address >= register.address
            && offset as usize + access_len <= span
            && (offset as usize).is_multiple_of(access_len)
            && register.supports_width(width)
    })
}

fn io_register_range_for_access(
    address: u32,
    access_len: usize,
    width: u8,
) -> Option<IoRegisterRange> {
    let access_end = address.checked_add(access_len as u32 - 1)?;
    IO_REGISTER_RANGES.iter().copied().find(|range| {
        address >= range.start
            && access_end <= range.end
            && ((address - range.start) as usize).is_multiple_of(access_len)
            && range.access_widths & width != 0
    })
}

fn access_width_flag(access_len: usize) -> Option<u8> {
    match access_len {
        1 => Some(ACCESS_WIDTH_8),
        2 => Some(ACCESS_WIDTH_16),
        4 => Some(ACCESS_WIDTH_32),
        _ => None,
    }
}

fn register_span(access_widths: u8) -> usize {
    if access_widths & ACCESS_WIDTH_32 != 0 {
        4
    } else if access_widths & ACCESS_WIDTH_16 != 0 {
        2
    } else {
        1
    }
}

const fn register(
    address: u32,
    name: &'static str,
    device: IoDevice,
    access: IoAccess,
    access_widths: u8,
    description: &'static str,
) -> IoRegister {
    IoRegister {
        address,
        name,
        device,
        access,
        access_widths,
        description,
    }
}

const fn dma_register(address: u32, name: &'static str, description: &'static str) -> IoRegister {
    register(
        address,
        name,
        IoDevice::Dma,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_32,
        description,
    )
}

const fn timer_register(address: u32, name: &'static str, description: &'static str) -> IoRegister {
    register(
        address,
        name,
        IoDevice::Timer,
        IoAccess::ReadWrite,
        ACCESS_WIDTH_16 | ACCESS_WIDTH_32,
        description,
    )
}

#[derive(Clone, Debug, Default)]
pub struct Io {
    pub memory: MemoryControl,
    pub irq: InterruptController,
    pub gpu: Gpu,
    pub dma: Dma,
    pub timers: Timers,
    pub controller: Controller,
    pub cdrom: Cdrom,
    pub mdec: Mdec,
    pub spu: Spu,
    register_file: IoRegisterFile,
}

impl Io {
    pub fn read_u8(&self, address: u32) -> u8 {
        self.read(address, 1) as u8
    }

    pub fn read_u16(&self, address: u32) -> u16 {
        self.read(address, 2) as u16
    }

    pub fn read_u32(&self, address: u32) -> u32 {
        self.read(address, 4)
    }

    pub fn write_u8(&mut self, address: u32, value: u8) {
        self.write(address, value as u32, 1);
    }

    pub fn write_u16(&mut self, address: u32, value: u16) {
        self.write(address, value as u32, 2);
    }

    pub fn write_u32(&mut self, address: u32, value: u32) {
        self.write(address, value, 4);
    }

    pub fn set_input(&mut self, buttons: ActionButtons) {
        self.controller.set_buttons(buttons);
    }

    pub fn tick(&mut self, cycles: u64) -> u32 {
        self.timers.tick(cycles)
    }

    pub fn json(&self) -> String {
        let framebuffer_stats = self.gpu.framebuffer_stats();
        let vram_stats = self.gpu.vram_stats();
        let vram_bounds = self.gpu.vram_nonzero_bounds_json();
        let screenshot_window = self.gpu.screenshot_window();
        let (display_width, display_height) = self.gpu.display_dimensions();
        format!(
            "{{\"irq_status\":{},\"irq_mask\":{},\"dma_control\":{},\"dma_interrupt\":{},\"gpu_status\":{},\"gpu_commands_seen\":{},\"gpu_gp0_pending_words\":{},\"gpu_gp0_pending_head\":{},\"gpu_gp0_expected_words\":{},\"gpu_frame_nonzero_pixels\":{},\"gpu_frame_checksum\":{},\"gpu_vram_nonzero_pixels\":{},\"gpu_vram_checksum\":{},\"gpu_vram_nonzero_bounds\":{},\"gpu_screenshot_x\":{},\"gpu_screenshot_y\":{},\"gpu_screenshot_nonzero_pixels\":{},\"gpu_screenshot_checksum\":{},\"gpu_display_width\":{},\"gpu_display_height\":{},\"gpu_window_diagnostics\":{},\"gpu_resolved_display\":{},\"gpu_best_observation_window\":{},\"gpu_presented_frame_window\":{},\"gpu_presentation_captures\":{},\"gpu_display_area_start\":{},\"gpu_display_area_history\":[{}],\"gpu_horizontal_range\":{},\"gpu_vertical_range\":{},\"gpu_drawing_area_top_left\":{},\"gpu_drawing_area_bottom_right\":{},\"gpu_drawing_offset\":{},\"gpu_texture_page\":{},\"gpu_fill_rect_commands\":{},\"gpu_flat_triangle_commands\":{},\"gpu_textured_triangle_commands\":{},\"gpu_textured_rect_commands\":{},\"gpu_flat_line_commands\":{},\"gpu_textured_sampled_pixels\":{},\"gpu_textured_drawn_pixels\":{},\"gpu_textured_written_pixels\":{},\"gpu_textured_clipped_pixels\":{},\"gpu_textured_transparent_pixels\":{},\"gpu_image_upload_commands\":{},\"gpu_vram_copy_commands\":{},\"gpu_invalid_fill_rect_commands\":{},\"gpu_invalid_image_upload_commands\":{},\"gpu_invalid_vram_copy_commands\":{},\"gpu_gp0_command_counts\":[{}],\"gpu_recent_gp0_commands\":[{}],\"gpu_recent_gp1_commands\":[{}],\"gpu_recent_transfer_commands\":[{}],\"gpu_recent_draw_commands\":[{}],\"gpu_largest_draw_command\":{},\"gpu_top_draw_commands\":[{}],\"gpu_focus_draw_commands\":[{}],\"gpu_overlap_draw_commands\":[{}],\"mdec\":{},\"timer0_counter\":{},\"timer1_counter\":{},\"timer2_counter\":{},\"sio_status\":{},\"p1_state\":{},\"zn_mcu\":{}}}",
            self.irq.status,
            self.irq.mask,
            self.dma.control,
            self.dma.interrupt,
            self.gpu.status,
            self.gpu.commands_seen,
            self.gpu.gp0_pending_words(),
            self.gpu.gp0_pending_head(),
            optional_usize_json(self.gpu.gp0_pending_expected_words()),
            framebuffer_stats.nonzero_pixels,
            framebuffer_stats.checksum,
            vram_stats.nonzero_pixels,
            vram_stats.checksum,
            vram_bounds,
            screenshot_window.x,
            screenshot_window.y,
            screenshot_window.stats.nonzero_pixels,
            screenshot_window.stats.checksum,
            display_width,
            display_height,
            self.gpu.window_diagnostics_json(),
            self.gpu.resolved_display_json(),
            self.gpu.best_observation_window_json(),
            self.gpu.presented_frame_window_json(),
            self.gpu.presentation_captures,
            self.gpu.display_area_start,
            self.gpu.display_area_history_json(),
            self.gpu.horizontal_range,
            self.gpu.vertical_range,
            self.gpu.drawing_area_top_left,
            self.gpu.drawing_area_bottom_right,
            self.gpu.drawing_offset,
            self.gpu.texture_page,
            self.gpu.fill_rect_commands,
            self.gpu.flat_triangle_commands,
            self.gpu.textured_triangle_commands,
            self.gpu.textured_rect_commands,
            self.gpu.flat_line_commands,
            self.gpu.textured_draw_stats.sampled_pixels,
            self.gpu.textured_draw_stats.drawn_pixels,
            self.gpu.textured_draw_stats.written_pixels,
            self.gpu.textured_draw_stats.clipped_pixels,
            self.gpu.textured_draw_stats.transparent_pixels,
            self.gpu.image_upload_commands,
            self.gpu.vram_copy_commands,
            self.gpu.invalid_fill_rect_commands,
            self.gpu.invalid_image_upload_commands,
            self.gpu.invalid_vram_copy_commands,
            self.gpu.gp0_command_counts_json(),
            self.gpu.recent_gp0_commands_json(),
            self.gpu.recent_gp1_commands_json(),
            self.gpu.recent_transfer_commands_json(),
            self.gpu.recent_draw_commands_json(),
            self.gpu.largest_draw_command_json(),
            self.gpu.top_draw_commands_json(),
            self.gpu.focus_draw_commands_json(),
            self.gpu.overlap_draw_commands_json(),
            self.mdec.diagnostic_json(),
            self.timers.0[0].counter,
            self.timers.0[1].counter,
            self.timers.0[2].counter,
            self.controller.status,
            self.controller.p1_state,
            self.controller.zn_mcu_diagnostic_json()
        )
    }

    pub fn compact_json(&self) -> String {
        let framebuffer_stats = self.gpu.framebuffer_stats();
        let vram_stats = self.gpu.vram_stats();
        let screenshot_window = self.gpu.screenshot_window();
        let (display_width, display_height) = self.gpu.display_dimensions();
        format!(
            "{{\"irq_status\":{},\"irq_mask\":{},\"dma_control\":{},\"dma_interrupt\":{},\"gpu_status\":{},\"gpu_commands_seen\":{},\"gpu_gp0_pending_words\":{},\"gpu_frame_nonzero_pixels\":{},\"gpu_frame_checksum\":{},\"gpu_vram_nonzero_pixels\":{},\"gpu_vram_checksum\":{},\"gpu_vram_nonzero_bounds\":{},\"gpu_screenshot_x\":{},\"gpu_screenshot_y\":{},\"gpu_screenshot_nonzero_pixels\":{},\"gpu_screenshot_checksum\":{},\"gpu_display_width\":{},\"gpu_display_height\":{},\"gpu_window_diagnostics\":{},\"gpu_resolved_display\":{},\"gpu_best_observation_window\":{},\"gpu_presented_frame_window\":{},\"gpu_presentation_captures\":{},\"gpu_display_area_start\":{},\"gpu_display_area_history\":[{}],\"gpu_horizontal_range\":{},\"gpu_vertical_range\":{},\"gpu_drawing_area_top_left\":{},\"gpu_drawing_area_bottom_right\":{},\"gpu_drawing_offset\":{},\"gpu_texture_page\":{},\"gpu_fill_rect_commands\":{},\"gpu_flat_triangle_commands\":{},\"gpu_textured_triangle_commands\":{},\"gpu_textured_rect_commands\":{},\"gpu_flat_line_commands\":{},\"gpu_textured_sampled_pixels\":{},\"gpu_textured_drawn_pixels\":{},\"gpu_textured_written_pixels\":{},\"gpu_textured_clipped_pixels\":{},\"gpu_textured_transparent_pixels\":{},\"gpu_image_upload_commands\":{},\"gpu_vram_copy_commands\":{},\"gpu_invalid_fill_rect_commands\":{},\"gpu_invalid_image_upload_commands\":{},\"gpu_invalid_vram_copy_commands\":{},\"gpu_gp0_command_counts\":[{}],\"gpu_largest_draw_command\":{},\"gpu_top_draw_commands\":[{}],\"gpu_recent_draw_commands\":[{}],\"gpu_recent_focus_draw_commands\":[{}],\"gpu_recent_overlap_draw_commands\":[{}],\"mdec\":{},\"timer0_counter\":{},\"timer1_counter\":{},\"timer2_counter\":{},\"sio_status\":{},\"p1_state\":{},\"zn_mcu\":{}}}",
            self.irq.status,
            self.irq.mask,
            self.dma.control,
            self.dma.interrupt,
            self.gpu.status,
            self.gpu.commands_seen,
            self.gpu.gp0_pending_words(),
            framebuffer_stats.nonzero_pixels,
            framebuffer_stats.checksum,
            vram_stats.nonzero_pixels,
            vram_stats.checksum,
            self.gpu.vram_nonzero_bounds_json(),
            screenshot_window.x,
            screenshot_window.y,
            screenshot_window.stats.nonzero_pixels,
            screenshot_window.stats.checksum,
            display_width,
            display_height,
            self.gpu.window_diagnostics_json(),
            self.gpu.resolved_display_json(),
            self.gpu.best_observation_window_json(),
            self.gpu.presented_frame_window_json(),
            self.gpu.presentation_captures,
            self.gpu.display_area_start,
            self.gpu.display_area_history_json(),
            self.gpu.horizontal_range,
            self.gpu.vertical_range,
            self.gpu.drawing_area_top_left,
            self.gpu.drawing_area_bottom_right,
            self.gpu.drawing_offset,
            self.gpu.texture_page,
            self.gpu.fill_rect_commands,
            self.gpu.flat_triangle_commands,
            self.gpu.textured_triangle_commands,
            self.gpu.textured_rect_commands,
            self.gpu.flat_line_commands,
            self.gpu.textured_draw_stats.sampled_pixels,
            self.gpu.textured_draw_stats.drawn_pixels,
            self.gpu.textured_draw_stats.written_pixels,
            self.gpu.textured_draw_stats.clipped_pixels,
            self.gpu.textured_draw_stats.transparent_pixels,
            self.gpu.image_upload_commands,
            self.gpu.vram_copy_commands,
            self.gpu.invalid_fill_rect_commands,
            self.gpu.invalid_image_upload_commands,
            self.gpu.invalid_vram_copy_commands,
            self.gpu.gp0_command_counts_json(),
            self.gpu.largest_draw_command_compact_json(),
            self.gpu.top_draw_commands_compact_json(8),
            self.gpu.recent_draw_commands_compact_json(128),
            self.gpu.focus_draw_commands_compact_json(8),
            self.gpu.overlap_draw_commands_compact_json(8),
            self.mdec.diagnostic_json(),
            self.timers.0[0].counter,
            self.timers.0[1].counter,
            self.timers.0[2].counter,
            self.controller.status,
            self.controller.p1_state,
            self.controller.zn_mcu_diagnostic_json()
        )
    }

    pub fn runtime_probe_json(&self) -> String {
        let framebuffer_stats = self.gpu.framebuffer_stats();
        let vram_stats = self.gpu.vram_stats();
        let screenshot_window = self.gpu.screenshot_window();
        let (display_width, display_height) = self.gpu.display_dimensions();
        format!(
            "{{\"irq_status\":{},\"irq_mask\":{},\"gpu_commands_seen\":{},\"gpu_draw_sequence\":{},\"gpu_frame_nonzero_pixels\":{},\"gpu_frame_checksum\":{},\"gpu_vram_nonzero_pixels\":{},\"gpu_vram_checksum\":{},\"gpu_screenshot_x\":{},\"gpu_screenshot_y\":{},\"gpu_screenshot_nonzero_pixels\":{},\"gpu_screenshot_checksum\":{},\"gpu_display_width\":{},\"gpu_display_height\":{},\"gpu_window_diagnostics\":{},\"gpu_resolved_display\":{},\"gpu_best_observation_window\":{},\"gpu_presented_frame_window\":{},\"gpu_presentation_captures\":{},\"gpu_display_area_start\":{},\"gpu_display_area_history\":[{}],\"gpu_recent_gp0_commands\":[{}],\"gpu_recent_gp1_commands\":[{}],\"gpu_recent_transfer_commands\":[{}],\"gpu_fill_rect_commands\":{},\"gpu_textured_triangle_commands\":{},\"gpu_image_upload_commands\":{},\"gpu_vram_copy_commands\":{},\"mdec\":{},\"timer0_counter\":{},\"timer1_counter\":{},\"timer2_counter\":{},\"sio_status\":{},\"p1_state\":{},\"zn_mcu\":{}}}",
            self.irq.status,
            self.irq.mask,
            self.gpu.commands_seen,
            self.gpu.draw_sequence,
            framebuffer_stats.nonzero_pixels,
            framebuffer_stats.checksum,
            vram_stats.nonzero_pixels,
            vram_stats.checksum,
            screenshot_window.x,
            screenshot_window.y,
            screenshot_window.stats.nonzero_pixels,
            screenshot_window.stats.checksum,
            display_width,
            display_height,
            self.gpu.window_diagnostics_json(),
            self.gpu.resolved_display_json(),
            self.gpu.best_observation_window_json(),
            self.gpu.presented_frame_window_json(),
            self.gpu.presentation_captures,
            self.gpu.display_area_start,
            self.gpu.display_area_history_json(),
            self.gpu.recent_gp0_commands_json(),
            self.gpu.recent_gp1_commands_json(),
            self.gpu.recent_transfer_commands_json(),
            self.gpu.fill_rect_commands,
            self.gpu.textured_triangle_commands,
            self.gpu.image_upload_commands,
            self.gpu.vram_copy_commands,
            self.mdec.diagnostic_json(),
            self.timers.0[0].counter,
            self.timers.0[1].counter,
            self.timers.0[2].counter,
            self.controller.status,
            self.controller.p1_state,
            self.controller.zn_mcu_diagnostic_json()
        )
    }

    pub fn runtime_compact_probe_json(&self) -> String {
        format!(
            "{{\"irq_status\":{},\"irq_status_hex\":\"0x{:04x}\",\"irq_mask\":{},\"irq_mask_hex\":\"0x{:04x}\",\"dma_interrupt\":{},\"dma_interrupt_hex\":\"0x{:08x}\",\"gpu_commands_seen\":{},\"gpu_draw_sequence\":{},\"playability\":{},\"timer0_counter\":{},\"timer1_counter\":{},\"timer2_counter\":{},\"sio_status\":{},\"sio_status_hex\":\"0x{:04x}\",\"p1_state\":{},\"p1_state_hex\":\"0x{:04x}\",\"zn_mcu\":{}}}",
            self.irq.status,
            self.irq.status,
            self.irq.mask,
            self.irq.mask,
            self.dma.interrupt,
            self.dma.interrupt,
            self.gpu.commands_seen,
            self.gpu.draw_sequence,
            self.gpu.native_playability_compact_json(),
            self.timers.0[0].counter,
            self.timers.0[1].counter,
            self.timers.0[2].counter,
            self.controller.status,
            self.controller.status,
            self.controller.p1_state,
            self.controller.p1_state,
            self.controller.zn_mcu_diagnostic_json()
        )
    }

    pub fn native_playability_json(&self) -> String {
        self.gpu.native_playability_json()
    }

    pub fn native_playability_compact_json(&self) -> String {
        self.gpu.native_playability_compact_json()
    }

    pub fn native_playable_candidate(&self) -> bool {
        self.gpu.native_playable_candidate()
    }

    pub fn display_rgb_frame(&self) -> (usize, usize, Vec<u32>) {
        self.gpu.display_rgb_frame()
    }

    pub fn stable_display_rgb_frame(&self) -> (usize, usize, Vec<u32>) {
        self.gpu.stable_display_rgb_frame()
    }

    fn read(&self, address: u32, access_len: usize) -> u32 {
        if !io_access_for(address, access_len).is_some_and(IoAccess::readable) {
            return 0;
        }

        if let Some(register) = io_register_for_access(address, access_len) {
            return read_lane(
                self.read_modeled_u32(register.address),
                register.address,
                address,
                access_len,
            );
        }

        self.register_file.read(address, access_len)
    }

    fn write(&mut self, address: u32, value: u32, access_len: usize) {
        if !io_access_for(address, access_len).is_some_and(IoAccess::writable) {
            return;
        }

        if let Some(register) = io_register_for_access(address, access_len) {
            if register.address == SIO_DATA {
                let masked = match access_len {
                    1 => value & 0xff,
                    2 => value & 0xffff,
                    _ => value,
                };
                self.write_modeled_u32(register.address, masked);
                return;
            }

            let merged = write_lane(
                self.read_modeled_u32(register.address),
                register.address,
                address,
                value,
                access_len,
            );
            self.write_modeled_u32(register.address, merged);
            return;
        }

        self.register_file.write(address, value, access_len);
    }

    fn read_modeled_u32(&self, address: u32) -> u32 {
        match address {
            MEMCTRL_EXP1_BASE
            | MEMCTRL_EXP2_BASE
            | MEMCTRL_EXP1_DELAY_SIZE
            | MEMCTRL_EXP3_DELAY_SIZE
            | MEMCTRL_BIOS_DELAY_SIZE
            | MEMCTRL_SPU_DELAY_SIZE
            | MEMCTRL_CDROM_DELAY_SIZE
            | MEMCTRL_EXP2_DELAY_SIZE
            | MEMCTRL_COMMON_DELAY
            | RAM_SIZE_CONTROL => self.memory.read_u32(address),
            SIO_DATA | SIO_STATUS | SIO_MODE | SIO_CONTROL | SIO_BAUD => {
                self.controller.read_u32(address)
            }
            IRQ_STATUS | IRQ_MASK => self.irq.read_u32(address),
            GPU_GP0 => self.gpu.gp0_read,
            GPU_GP1 => self.gpu.read_status(),
            DMA_REGION_START..=DMA_REGION_END => self.dma.read_u32(address),
            TIMER0_COUNTER | TIMER0_MODE | TIMER0_TARGET | TIMER1_COUNTER | TIMER1_MODE
            | TIMER1_TARGET | TIMER2_COUNTER | TIMER2_MODE | TIMER2_TARGET => {
                self.timers.read_u32(address)
            }
            CDROM_INDEX_STATUS | CDROM_RESPONSE | CDROM_DATA | CDROM_INTERRUPT_ENABLE => {
                self.cdrom.read_u32(address)
            }
            MDEC_COMMAND | MDEC_STATUS => self.mdec.read_u32(address),
            SPU_REGION_START..=SPU_REGION_END => self.spu.read_u32(address),
            _ => self
                .register_file
                .read(address, register_span_for_address(address)),
        }
    }

    fn write_modeled_u32(&mut self, address: u32, value: u32) {
        match address {
            MEMCTRL_EXP1_BASE
            | MEMCTRL_EXP2_BASE
            | MEMCTRL_EXP1_DELAY_SIZE
            | MEMCTRL_EXP3_DELAY_SIZE
            | MEMCTRL_BIOS_DELAY_SIZE
            | MEMCTRL_SPU_DELAY_SIZE
            | MEMCTRL_CDROM_DELAY_SIZE
            | MEMCTRL_EXP2_DELAY_SIZE
            | MEMCTRL_COMMON_DELAY
            | RAM_SIZE_CONTROL => self.memory.write_u32(address, value),
            SIO_DATA | SIO_STATUS | SIO_MODE | SIO_CONTROL | SIO_BAUD => {
                self.controller.write_u32(address, value);
                if address == SIO_DATA && self.controller.irq_requested() {
                    self.irq.status |= IRQ_CONTROLLER;
                } else if address == SIO_CONTROL && !self.controller.irq_requested() {
                    self.irq.status &= !IRQ_CONTROLLER;
                }
            }
            IRQ_STATUS | IRQ_MASK => self.irq.write_u32(address, value),
            GPU_GP0 => self.gpu.write_gp0(value),
            GPU_GP1 => self.gpu.write_gp1(value),
            DMA_REGION_START..=DMA_REGION_END => self.dma.write_u32(address, value),
            TIMER0_COUNTER | TIMER0_MODE | TIMER0_TARGET | TIMER1_COUNTER | TIMER1_MODE
            | TIMER1_TARGET | TIMER2_COUNTER | TIMER2_MODE | TIMER2_TARGET => {
                self.timers.write_u32(address, value)
            }
            CDROM_INDEX_STATUS | CDROM_RESPONSE | CDROM_DATA | CDROM_INTERRUPT_ENABLE => {
                self.cdrom.write_u32(address, value)
            }
            MDEC_COMMAND | MDEC_STATUS => self.mdec.write_u32(address, value),
            SPU_REGION_START..=SPU_REGION_END => self.spu.write_u32(address, value),
            _ => self
                .register_file
                .write(address, value, register_span_for_address(address)),
        }
    }
}

#[derive(Clone, Debug)]
struct IoRegisterFile {
    bytes: [u8; IO_REGISTER_BYTES],
}

impl Default for IoRegisterFile {
    fn default() -> Self {
        Self {
            bytes: [0; IO_REGISTER_BYTES],
        }
    }
}

impl IoRegisterFile {
    fn read(&self, address: u32, access_len: usize) -> u32 {
        let Some(offset) = io_offset(address, access_len) else {
            return 0;
        };

        self.bytes[offset..offset + access_len]
            .iter()
            .enumerate()
            .fold(0, |value, (index, byte)| {
                value | ((*byte as u32) << (index * 8))
            })
    }

    fn write(&mut self, address: u32, value: u32, access_len: usize) {
        let Some(offset) = io_offset(address, access_len) else {
            return;
        };

        for index in 0..access_len {
            self.bytes[offset + index] = (value >> (index * 8)) as u8;
        }
    }
}

fn io_offset(address: u32, access_len: usize) -> Option<usize> {
    let offset = address.checked_sub(IO_REGION_START)? as usize;
    (offset + access_len <= IO_REGISTER_BYTES).then_some(offset)
}

fn read_lane(value: u32, base: u32, address: u32, access_len: usize) -> u32 {
    let shifted = value >> ((address - base) * 8);
    match access_len {
        1 => shifted & 0xff,
        2 => shifted & 0xffff,
        _ => shifted,
    }
}

fn write_lane(current: u32, base: u32, address: u32, value: u32, access_len: usize) -> u32 {
    let shift = (address - base) * 8;
    let mask = match access_len {
        1 => 0xff,
        2 => 0xffff,
        _ => u32::MAX,
    } << shift;
    (current & !mask) | ((value << shift) & mask)
}

fn register_span_for_address(address: u32) -> usize {
    io_register(address)
        .map(|register| register_span(register.access_widths))
        .unwrap_or(4)
}

#[derive(Clone, Debug)]
pub struct MemoryControl {
    exp1_base: u32,
    exp2_base: u32,
    exp1_delay_size: u32,
    exp3_delay_size: u32,
    bios_delay_size: u32,
    spu_delay_size: u32,
    cdrom_delay_size: u32,
    exp2_delay_size: u32,
    common_delay: u32,
    ram_size_control: u32,
}

impl Default for MemoryControl {
    fn default() -> Self {
        Self {
            exp1_base: 0x1f00_0000,
            exp2_base: 0x1f80_2000,
            exp1_delay_size: 0x0013_243f,
            exp3_delay_size: 0x0000_3022,
            bios_delay_size: 0x0013_243f,
            spu_delay_size: 0x2009_31e1,
            cdrom_delay_size: 0x0002_0843,
            exp2_delay_size: 0x0007_0777,
            common_delay: 0x0003_1125,
            ram_size_control: 0x0000_0b88,
        }
    }
}

impl MemoryControl {
    fn read_u32(&self, address: u32) -> u32 {
        match address {
            MEMCTRL_EXP1_BASE => self.exp1_base,
            MEMCTRL_EXP2_BASE => self.exp2_base,
            MEMCTRL_EXP1_DELAY_SIZE => self.exp1_delay_size,
            MEMCTRL_EXP3_DELAY_SIZE => self.exp3_delay_size,
            MEMCTRL_BIOS_DELAY_SIZE => self.bios_delay_size,
            MEMCTRL_SPU_DELAY_SIZE => self.spu_delay_size,
            MEMCTRL_CDROM_DELAY_SIZE => self.cdrom_delay_size,
            MEMCTRL_EXP2_DELAY_SIZE => self.exp2_delay_size,
            MEMCTRL_COMMON_DELAY => self.common_delay,
            RAM_SIZE_CONTROL => self.ram_size_control,
            _ => 0,
        }
    }

    fn write_u32(&mut self, address: u32, value: u32) {
        match address {
            MEMCTRL_EXP1_BASE => self.exp1_base = value,
            MEMCTRL_EXP2_BASE => self.exp2_base = value,
            MEMCTRL_EXP1_DELAY_SIZE => self.exp1_delay_size = value,
            MEMCTRL_EXP3_DELAY_SIZE => self.exp3_delay_size = value,
            MEMCTRL_BIOS_DELAY_SIZE => self.bios_delay_size = value,
            MEMCTRL_SPU_DELAY_SIZE => self.spu_delay_size = value,
            MEMCTRL_CDROM_DELAY_SIZE => self.cdrom_delay_size = value,
            MEMCTRL_EXP2_DELAY_SIZE => self.exp2_delay_size = value,
            MEMCTRL_COMMON_DELAY => self.common_delay = value,
            RAM_SIZE_CONTROL => self.ram_size_control = value,
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct InterruptController {
    pub status: u32,
    pub mask: u32,
}

impl InterruptController {
    fn read_u32(&self, address: u32) -> u32 {
        match address {
            IRQ_STATUS => self.status & IRQ_BITS,
            IRQ_MASK => self.mask & IRQ_BITS,
            _ => 0,
        }
    }

    fn write_u32(&mut self, address: u32, value: u32) {
        match address {
            IRQ_STATUS => self.status &= value & IRQ_BITS,
            IRQ_MASK => self.mask = value & IRQ_BITS,
            _ => {}
        }
    }
}

#[derive(Clone, Debug)]
pub struct Gpu {
    pub gp0_read: u32,
    pub status: u32,
    pub commands_seen: u64,
    pub display_area_start: u32,
    pub horizontal_range: u32,
    pub vertical_range: u32,
    status_reads: Cell<u32>,
    framebuffer: NativeFrameBuffer,
    gp0_fifo: Vec<u32>,
    gp0_fifo_sources: Vec<Option<GpuCommandSource>>,
    drawing_area_top_left: u32,
    drawing_area_bottom_right: u32,
    drawing_offset: u32,
    texture_page: u16,
    texture_window: TextureWindow,
    set_mask_bit: bool,
    check_mask_bit: bool,
    gp0_command_counts: [u64; 256],
    fill_rect_commands: u64,
    flat_triangle_commands: u64,
    textured_triangle_commands: u64,
    textured_rect_commands: u64,
    flat_line_commands: u64,
    textured_draw_stats: TexturedDrawStats,
    image_upload_commands: u64,
    invalid_fill_rect_commands: u64,
    invalid_image_upload_commands: u64,
    vram_copy_commands: u64,
    invalid_vram_copy_commands: u64,
    recent_gp0_commands: Vec<Gp0CommandTrace>,
    recent_gp1_commands: Vec<Gp1CommandTrace>,
    recent_transfer_commands: Vec<GpuTransferTrace>,
    image_upload_rects: Vec<DrawBounds>,
    recent_draw_commands: Vec<GpuDrawTrace>,
    display_area_history: Vec<DisplayAreaTrace>,
    largest_draw_command: Option<GpuDrawTrace>,
    top_draw_commands: Vec<GpuDrawTrace>,
    focus_draw_commands: Vec<GpuDrawTrace>,
    overlap_draw_commands: Vec<GpuDrawTrace>,
    draw_sequence: u64,
    draw_capture_range: Option<(u64, u64)>,
    draw_captures: Vec<NativeGpuDrawCapture>,
    best_observation_png: Option<Vec<u8>>,
    best_observation_rgb: Option<Vec<u32>>,
    best_observation_window: Option<FrameBufferWindow>,
    best_observation_width: usize,
    best_observation_height: usize,
    best_observation_last_probe_command: u64,
    best_observation_last_probe_draw_sequence: u64,
    presented_frame_png: Option<Vec<u8>>,
    presented_frame_rgb: Option<Vec<u32>>,
    presented_frame_window: Option<FrameBufferWindow>,
    presented_frame_width: usize,
    presented_frame_height: usize,
    presented_frame_capture_index: u64,
    presentation_captures: u64,
    field_composed_display_png: Option<Vec<u8>>,
    field_composed_display_rgb: Option<Vec<u32>>,
    field_composed_display_window: Option<FrameBufferWindow>,
    field_composed_display_width: usize,
    field_composed_display_height: usize,
    field_composed_display_capture_index: u64,
}

impl Default for Gpu {
    fn default() -> Self {
        Self {
            gp0_read: 0,
            status: 0x1480_2000,
            commands_seen: 0,
            display_area_start: 0,
            horizontal_range: 0,
            vertical_range: 0,
            status_reads: Cell::new(0),
            framebuffer: NativeFrameBuffer::default(),
            gp0_fifo: Vec::new(),
            gp0_fifo_sources: Vec::new(),
            drawing_area_top_left: 0,
            drawing_area_bottom_right: 0,
            drawing_offset: 0,
            texture_page: 0,
            texture_window: TextureWindow::default(),
            set_mask_bit: false,
            check_mask_bit: false,
            gp0_command_counts: [0; 256],
            fill_rect_commands: 0,
            flat_triangle_commands: 0,
            textured_triangle_commands: 0,
            textured_rect_commands: 0,
            flat_line_commands: 0,
            textured_draw_stats: TexturedDrawStats::default(),
            image_upload_commands: 0,
            invalid_fill_rect_commands: 0,
            invalid_image_upload_commands: 0,
            vram_copy_commands: 0,
            invalid_vram_copy_commands: 0,
            recent_gp0_commands: Vec::new(),
            recent_gp1_commands: Vec::new(),
            recent_transfer_commands: Vec::new(),
            image_upload_rects: Vec::new(),
            recent_draw_commands: Vec::new(),
            display_area_history: Vec::new(),
            largest_draw_command: None,
            top_draw_commands: Vec::new(),
            focus_draw_commands: Vec::new(),
            overlap_draw_commands: Vec::new(),
            draw_sequence: 0,
            draw_capture_range: None,
            draw_captures: Vec::new(),
            best_observation_png: None,
            best_observation_rgb: None,
            best_observation_window: None,
            best_observation_width: 0,
            best_observation_height: 0,
            best_observation_last_probe_command: 0,
            best_observation_last_probe_draw_sequence: 0,
            presented_frame_png: None,
            presented_frame_rgb: None,
            presented_frame_window: None,
            presented_frame_width: 0,
            presented_frame_height: 0,
            presented_frame_capture_index: 0,
            presentation_captures: 0,
            field_composed_display_png: None,
            field_composed_display_rgb: None,
            field_composed_display_window: None,
            field_composed_display_width: 0,
            field_composed_display_height: 0,
            field_composed_display_capture_index: 0,
        }
    }
}

impl Gpu {
    pub fn read_status(&self) -> u32 {
        let reads = self.status_reads.get().wrapping_add(1);
        self.status_reads.set(reads);
        if reads & 1 == 0 {
            self.status & !0x8000_0000
        } else {
            self.status | 0x8000_0000
        }
    }

    pub fn write_gp0(&mut self, value: u32) {
        self.write_gp0_from(value, None);
    }

    pub fn write_gp0_with_source(&mut self, value: u32, source: GpuCommandSource) {
        self.write_gp0_from(value, Some(source));
    }

    fn write_gp0_from(&mut self, value: u32, source: Option<GpuCommandSource>) {
        self.gp0_read = value;
        self.commands_seen += 1;
        self.gp0_fifo.push(value);
        self.gp0_fifo_sources.push(source);
        self.drain_gp0_fifo();
    }

    pub fn write_gp1(&mut self, value: u32) {
        let command = value >> 24;
        self.push_recent_gp1_command(value);
        match command {
            0x00 => {
                let draw_capture_range = self.draw_capture_range;
                let draw_captures = std::mem::take(&mut self.draw_captures);
                *self = Self::default();
                self.draw_capture_range = draw_capture_range;
                self.draw_captures = draw_captures;
            }
            0x01 => {
                self.gp0_read = 0;
                self.gp0_fifo.clear();
                self.gp0_fifo_sources.clear();
            }
            0x02 => {
                self.status &= !(1 << 24);
            }
            0x03 => {
                self.status = (self.status & !(1 << 23)) | ((value & 1) << 23);
            }
            0x04 => {
                self.status = (self.status & !(0x3 << 29)) | ((value & 0x3) << 29);
            }
            0x05 => {
                self.display_area_start = value & 0x00ff_ffff;
                self.push_display_area_trace(value);
                self.capture_presented_frame_after_display_area_change();
            }
            0x06 => self.horizontal_range = value & 0x00ff_ffff,
            0x07 => self.vertical_range = value & 0x00ff_ffff,
            0x08 => {
                self.status = (self.status & !0x007f_0000) | ((value & 0x3f) << 17);
            }
            _ => {}
        }
        self.commands_seen += 1;
    }

    pub fn screenshot_png_base64(&self) -> String {
        let output = self.current_display_output_window();
        if self.should_use_field_composed_output(output) {
            return bytes_base64(&self.display_output_png(output));
        }
        let resolved = self.display_resolve();
        if resolved.promoted && self.should_present_resolved_display(resolved) {
            return bytes_base64(&self.resolved_display_png(resolved));
        }
        if self.should_show_current_display_over_stale_candidates() {
            let window = self.current_display_window();
            let (width, height) = self.display_dimensions();
            return self
                .framebuffer
                .psx_display_png_base64(window.x, window.y, width, height);
        }
        if self.should_prefer_current_display() {
            let window = self.current_display_window();
            let (width, height) = self.display_dimensions();
            return self
                .framebuffer
                .psx_display_png_base64(window.x, window.y, width, height);
        }
        if self.should_prefer_presented_frame()
            && let Some(png) = self.presented_frame_png()
        {
            return bytes_base64(&png);
        }
        if self.should_prefer_best_observation()
            && let Some(png) = self.best_observation_png()
        {
            return bytes_base64(&png);
        }
        if let Some(presented) = self.presented_frame_window {
            if !self.display_window_is_texture_atlas(presented)
                && let Some(png) = self.presented_frame_png()
            {
                return bytes_base64(&png);
            }
        }
        if let Some(best) = self.best_observation_window {
            if !self.display_window_is_texture_atlas(best)
                && let Some(png) = self.best_observation_png()
            {
                return bytes_base64(&png);
            }
        }
        let (width, height) = self.display_dimensions();
        let mut window = self.screenshot_window();
        if self.display_window_is_texture_atlas_with_dimensions(window, width, height) {
            window = self.current_display_window();
        }
        self.framebuffer
            .psx_display_png_base64(window.x, window.y, width, height)
    }

    pub fn screenshot_png(&self) -> Vec<u8> {
        let output = self.current_display_output_window();
        if self.should_use_field_composed_output(output) {
            return self.display_output_png(output);
        }
        let resolved = self.display_resolve();
        if resolved.promoted && self.should_present_resolved_display(resolved) {
            return self.resolved_display_png(resolved);
        }
        if self.should_show_current_display_over_stale_candidates() {
            let window = self.current_display_window();
            let (width, height) = self.display_dimensions();
            return self
                .framebuffer
                .psx_display_png(window.x, window.y, width, height);
        }
        if self.should_prefer_current_display() {
            let window = self.current_display_window();
            let (width, height) = self.display_dimensions();
            return self
                .framebuffer
                .psx_display_png(window.x, window.y, width, height);
        }
        if self.should_prefer_presented_frame()
            && let Some(png) = self.presented_frame_png()
        {
            return png;
        }
        if self.should_prefer_best_observation()
            && let Some(png) = self.best_observation_png()
        {
            return png;
        }
        if let Some(presented) = self.presented_frame_window {
            if !self.display_window_is_texture_atlas(presented)
                && let Some(png) = self.presented_frame_png()
            {
                return png;
            }
        }
        if let Some(best) = self.best_observation_window {
            if !self.display_window_is_texture_atlas(best)
                && let Some(png) = self.best_observation_png()
            {
                return png;
            }
        }
        let (width, height) = self.display_dimensions();
        let mut window = self.screenshot_window();
        if self.display_window_is_texture_atlas_with_dimensions(window, width, height) {
            window = self.current_display_window();
        }
        self.framebuffer
            .psx_display_png(window.x, window.y, width, height)
    }

    pub fn display_png(&self) -> Vec<u8> {
        let output = self.current_display_output_window();
        if self.should_use_field_composed_output(output) {
            return self.display_output_png(output);
        }
        let resolved = self.display_resolve();
        if resolved.promoted && self.should_present_resolved_display(resolved) {
            return self.resolved_display_png(resolved);
        }
        if self.should_show_current_display_over_stale_candidates() {
            let window = self.current_display_window();
            let (width, height) = self.display_dimensions();
            return self
                .framebuffer
                .psx_display_png(window.x, window.y, width, height);
        }
        if self.should_prefer_current_display() {
            let window = self.current_display_window();
            let (width, height) = self.display_dimensions();
            return self
                .framebuffer
                .psx_display_png(window.x, window.y, width, height);
        }
        if self.should_prefer_presented_frame()
            && let Some(png) = self.presented_frame_png()
        {
            return png;
        }
        if self.should_prefer_best_observation()
            && let Some(png) = self.best_observation_png()
        {
            return png;
        }
        if let Some(presented) = self.presented_frame_window {
            if !self.display_window_is_texture_atlas(presented)
                && let Some(png) = self.presented_frame_png()
            {
                return png;
            }
        }
        let (start_x, start_y) = display_area_start_xy(self.display_area_start);
        let (width, height) = self.display_dimensions();
        self.framebuffer
            .psx_display_png(start_x, start_y, width, height)
    }

    pub fn display_rgb_frame(&self) -> (usize, usize, Vec<u32>) {
        let output = self.current_display_output_window();
        if self.should_use_field_composed_output(output) {
            return (output.width, output.height, self.display_output_rgb(output));
        }

        self.stable_display_rgb_frame()
    }

    pub fn stable_display_rgb_frame(&self) -> (usize, usize, Vec<u32>) {
        let resolved = self.display_resolve();
        let (width, height) = self.display_dimensions();
        if resolved.promoted && self.should_present_resolved_display(resolved) {
            return self.resolved_display_rgb_frame(resolved, width, height);
        }
        if self.should_show_current_display_over_stale_candidates() {
            let window = self.current_display_window();
            return (
                width,
                height,
                self.framebuffer
                    .psx_display_rgb_window(window.x, window.y, width, height),
            );
        }
        if self.should_prefer_current_display() {
            let window = self.current_display_window();
            return (
                width,
                height,
                self.framebuffer
                    .psx_display_rgb_window(window.x, window.y, width, height),
            );
        }
        if let Some(presented) = self.presented_frame_window
            && self.should_prefer_presented_frame()
        {
            return self.cached_display_rgb_frame_or_capture(
                self.presented_frame_rgb.as_deref(),
                self.presented_frame_width,
                self.presented_frame_height,
                presented,
                width,
                height,
                true,
            );
        }
        if self.should_prefer_best_observation()
            && let Some(best) = self.best_observation_window
        {
            return self.cached_display_rgb_frame_or_capture(
                self.best_observation_rgb.as_deref(),
                self.best_observation_width,
                self.best_observation_height,
                best,
                width,
                height,
                false,
            );
        }
        if let Some(presented) = self.presented_frame_window {
            if !self.display_window_is_texture_atlas(presented) {
                return self.cached_display_rgb_frame_or_capture(
                    self.presented_frame_rgb.as_deref(),
                    self.presented_frame_width,
                    self.presented_frame_height,
                    presented,
                    width,
                    height,
                    true,
                );
            }
        }
        if let Some(best) = self.best_observation_window {
            if !self.display_window_is_texture_atlas(best) {
                return self.cached_display_rgb_frame_or_capture(
                    self.best_observation_rgb.as_deref(),
                    self.best_observation_width,
                    self.best_observation_height,
                    best,
                    width,
                    height,
                    false,
                );
            }
        }
        let (start_x, start_y) = display_area_start_xy(self.display_area_start);
        (
            width,
            height,
            self.framebuffer
                .psx_display_rgb_window(start_x, start_y, width, height),
        )
    }

    fn current_display_window(&self) -> FrameBufferWindow {
        let (start_x, start_y) = display_area_start_xy(self.display_area_start);
        let (width, height) = self.display_dimensions();
        FrameBufferWindow {
            x: start_x,
            y: start_y,
            stats: self
                .framebuffer
                .psx_display_stats(start_x, start_y, width, height),
        }
    }

    fn current_display_output_window(&self) -> DisplayOutputWindow {
        let (width, height) = self.display_dimensions();
        if let Some(output) = self.interlaced_field_pair_output_window(width, height) {
            if self.should_use_field_composed_output(output) {
                return output;
            }
            if let Some(cached) = self.cached_interlaced_field_pair_output_window(width, height)
                && self.should_use_cached_field_composed_output(cached)
            {
                return cached;
            }
        } else if let Some(output) = self.cached_interlaced_field_pair_output_window(width, height)
            && self.should_use_cached_field_composed_output(output)
        {
            return output;
        }

        if let Some(output) = self.cached_field_composed_display_output_window()
            && self.should_use_cached_field_composed_output(output)
        {
            return output;
        }

        DisplayOutputWindow {
            source: "gp1_display_area",
            field_composed: false,
            cached: false,
            width,
            height,
            window: self.current_display_window(),
        }
    }

    fn current_raw_display_output_window(&self) -> DisplayOutputWindow {
        let (width, height) = self.display_dimensions();
        if let Some(output) = self.interlaced_field_pair_output_window(width, height) {
            return output;
        }
        if let Some(output) = self.cached_interlaced_field_pair_output_window(width, height) {
            return output;
        }

        DisplayOutputWindow {
            source: "gp1_display_area",
            field_composed: false,
            cached: false,
            width,
            height,
            window: self.current_display_window(),
        }
    }

    fn interlaced_field_pair_output_window(
        &self,
        width: usize,
        field_height: usize,
    ) -> Option<DisplayOutputWindow> {
        if field_height == 0 || field_height.saturating_mul(2) > PSX_VRAM_HEIGHT {
            return None;
        }
        if field_height < DEFAULT_DISPLAY_HEIGHT {
            return None;
        }

        let (current_x, current_y) = display_area_start_xy(self.display_area_start);
        let other_y = if current_y >= field_height {
            current_y - field_height
        } else {
            current_y.checked_add(field_height)?
        };
        if other_y >= PSX_VRAM_HEIGHT {
            return None;
        }
        if !self.recent_display_area_history_has_field_pair(current_x, current_y, other_y) {
            return None;
        }

        let base_y = current_y.min(other_y);
        let output_height = field_height * 2;
        let top_stats = self
            .framebuffer
            .psx_display_stats(current_x, base_y, width, field_height);
        let bottom_stats = self.framebuffer.psx_display_stats(
            current_x,
            base_y + field_height,
            width,
            field_height,
        );
        if !screen_observation_worth_saving(top_stats)
            || !screen_observation_worth_saving(bottom_stats)
        {
            return None;
        }

        Some(DisplayOutputWindow {
            source: "gp1_display_area_fields",
            field_composed: true,
            cached: false,
            width,
            height: output_height,
            window: FrameBufferWindow {
                x: current_x,
                y: base_y,
                stats: self
                    .framebuffer
                    .psx_display_stats(current_x, base_y, width, output_height),
            },
        })
    }

    fn cached_interlaced_field_pair_output_window(
        &self,
        width: usize,
        field_height: usize,
    ) -> Option<DisplayOutputWindow> {
        let cached = self.cached_field_composed_display_output_window()?;
        let output_height = field_height.checked_mul(2)?;
        if cached.width != width || cached.height != output_height {
            return None;
        }

        Some(cached)
    }

    fn cached_field_composed_display_output_window(&self) -> Option<DisplayOutputWindow> {
        let cached = self.field_composed_display_window?;
        if self.field_composed_display_width == 0
            || self.field_composed_display_height == 0
            || cached.stats.pixel_count
                != self
                    .field_composed_display_width
                    .saturating_mul(self.field_composed_display_height) as u64
        {
            return None;
        }

        Some(DisplayOutputWindow {
            source: "cached_gp1_display_area_fields",
            field_composed: true,
            cached: true,
            width: self.field_composed_display_width,
            height: self.field_composed_display_height,
            window: cached,
        })
    }

    fn display_output_png(&self, output: DisplayOutputWindow) -> Vec<u8> {
        if output.cached
            && let Some(png) = self.field_composed_display_png()
        {
            return png;
        }
        self.framebuffer.psx_display_png(
            output.window.x,
            output.window.y,
            output.width,
            output.height,
        )
    }

    fn display_output_rgb(&self, output: DisplayOutputWindow) -> Vec<u32> {
        if output.cached
            && let Some(rgb) = &self.field_composed_display_rgb
        {
            return rgb.clone();
        }
        self.framebuffer.psx_display_rgb_window(
            output.window.x,
            output.window.y,
            output.width,
            output.height,
        )
    }

    fn best_observation_png(&self) -> Option<Vec<u8>> {
        self.cached_display_png(
            self.best_observation_png.as_ref(),
            self.best_observation_rgb.as_ref(),
            self.best_observation_width,
            self.best_observation_height,
        )
    }

    fn presented_frame_png(&self) -> Option<Vec<u8>> {
        self.cached_display_png(
            self.presented_frame_png.as_ref(),
            self.presented_frame_rgb.as_ref(),
            self.presented_frame_width,
            self.presented_frame_height,
        )
    }

    fn field_composed_display_png(&self) -> Option<Vec<u8>> {
        self.cached_display_png(
            self.field_composed_display_png.as_ref(),
            self.field_composed_display_rgb.as_ref(),
            self.field_composed_display_width,
            self.field_composed_display_height,
        )
    }

    fn cached_display_png(
        &self,
        cached_png: Option<&Vec<u8>>,
        cached_rgb: Option<&Vec<u32>>,
        width: usize,
        height: usize,
    ) -> Option<Vec<u8>> {
        if let Some(rgb) = cached_rgb {
            let expected_len = width.checked_mul(height)?;
            if width > 0 && height > 0 && rgb.len() == expected_len {
                return Some(png_from_rgb888_pixels(width, height, rgb));
            }
        }

        cached_png.cloned()
    }

    fn recent_display_area_history_has_field_pair(
        &self,
        x: usize,
        current_y: usize,
        other_y: usize,
    ) -> bool {
        let mut saw_current = false;
        let mut saw_other = false;
        for trace in self.display_area_history.iter().rev().take(8) {
            if trace.x != x {
                continue;
            }
            saw_current |= trace.y == current_y;
            saw_other |= trace.y == other_y;
            if saw_current && saw_other {
                return true;
            }
        }
        false
    }

    fn recent_display_area_history_contains(&self, x: usize, y: usize) -> bool {
        self.display_area_history
            .iter()
            .rev()
            .take(8)
            .any(|trace| trace.x == x && trace.y == y)
    }

    fn should_prefer_current_display(&self) -> bool {
        let current = self.current_display_window();
        if self.display_window_is_texture_atlas(current) {
            return false;
        }
        if self.should_show_current_display_over_stale_candidates_with(current) {
            return true;
        }
        if !screen_observation_worth_saving(current.stats) {
            return false;
        }
        if let Some(best) = self.best_observation_window
            && !self.display_window_is_texture_atlas(best)
        {
            let current_score = screen_observation_score(current.stats);
            let best_score = screen_observation_score(best.stats);
            if is_sparse_display(current.stats)
                && is_detailed_observation(best.stats)
                && best.stats.nonzero_pixels > current.stats.nonzero_pixels.saturating_mul(4)
            {
                return false;
            }
            if is_sparse_display(current.stats)
                && is_detailed_observation(best.stats)
                && best_score > current_score.saturating_mul(2)
            {
                return false;
            }
            if should_defer_low_detail_current(current.stats, best.stats) {
                return false;
            }
        }
        if let Some(presented) = self.presented_frame_window {
            if self.display_window_is_texture_atlas(presented) {
                return false;
            }
            let current_score = screen_observation_score(current.stats);
            let presented_score = screen_observation_score(presented.stats);
            if is_sparse_display(current.stats)
                && is_detailed_observation(presented.stats)
                && presented.stats.nonzero_pixels > current.stats.nonzero_pixels.saturating_mul(4)
            {
                return false;
            }
            if is_sparse_display(current.stats)
                && is_detailed_observation(presented.stats)
                && presented_score > current_score.saturating_mul(2)
            {
                return false;
            }
            if should_defer_low_detail_current(current.stats, presented.stats) {
                return false;
            }
        }
        true
    }

    fn visible_display_window(&self) -> FrameBufferWindow {
        let current = self.current_display_window();
        let resolved = self.display_resolve();
        if resolved.promoted {
            return resolved.window;
        }
        if self.should_show_current_display_over_stale_candidates_with(current) {
            return current;
        }
        if self.should_prefer_current_display() {
            return current;
        }
        if self.should_prefer_presented_frame() {
            if let Some(presented) = self.presented_frame_window {
                return presented;
            }
        }
        if self.should_prefer_best_observation() {
            if let Some(best) = self.best_observation_window {
                return best;
            }
        }
        if let Some(presented) = self.presented_frame_window {
            if !self.display_window_is_texture_atlas(presented) {
                return presented;
            }
        }
        if let Some(best) = self.best_observation_window {
            if !self.display_window_is_texture_atlas(best) {
                return best;
            }
        }
        current
    }

    fn has_visible_presentation(&self) -> bool {
        let visible = self.visible_display_window();
        let current = self.current_display_window();
        self.presentation_captures > 0
            || visible.stats.checksum == current.stats.checksum
            || screen_observation_score(visible.stats)
                >= screen_observation_score(current.stats)
                    .saturating_sub(screen_observation_score(current.stats) / 8)
    }

    fn should_show_current_display_over_stale_candidates(&self) -> bool {
        self.should_show_current_display_over_stale_candidates_with(self.current_display_window())
    }

    fn should_show_current_display_over_stale_candidates_with(
        &self,
        current: FrameBufferWindow,
    ) -> bool {
        if self.presented_frame_window.is_none() {
            return false;
        }
        if self.presented_frame_is_fresh() {
            return false;
        }
        if !screen_observation_worth_saving(current.stats) {
            return false;
        }
        if self.has_better_cached_observation_than_current(current) {
            return false;
        }

        let display_pixels = (self.display_dimensions().0 * self.display_dimensions().1) as u64;
        current.stats.nonzero_pixels >= display_pixels / 512
    }

    fn has_better_cached_observation_than_current(&self, current: FrameBufferWindow) -> bool {
        let current_score = screen_observation_score(current.stats);
        let presented = if self.presented_frame_is_fresh() {
            self.presented_frame_window
        } else {
            None
        };

        presented
            .into_iter()
            .chain(self.best_observation_window)
            .filter(|candidate| !self.display_window_is_texture_atlas(*candidate))
            .any(|candidate| {
                if should_defer_low_detail_current(current.stats, candidate.stats) {
                    return true;
                }
                if is_sparse_display(current.stats)
                    && is_detailed_observation(candidate.stats)
                    && candidate.stats.nonzero_pixels
                        > current.stats.nonzero_pixels.saturating_mul(4)
                {
                    return true;
                }
                !is_detailed_observation(current.stats)
                    && has_native_full_scene_detail(candidate.stats)
                    && screen_observation_score(candidate.stats) > current_score.saturating_mul(2)
                    && candidate.stats.detail_edges
                        > current.stats.detail_edges.saturating_mul(4).max(512)
            })
    }

    pub fn actual_display_png(&self) -> Vec<u8> {
        let output = self.current_display_output_window();
        if self.should_use_field_composed_output(output) {
            return self.display_output_png(output);
        }
        let resolved = self.display_resolve();
        if !resolved.promoted || !self.should_present_resolved_display(resolved) {
            return self.display_png();
        }
        self.resolved_display_png(resolved)
    }

    pub fn actual_display_rgb_frame(&self) -> (usize, usize, Vec<u32>) {
        let output = self.current_display_output_window();
        if self.should_use_field_composed_output(output) {
            return (output.width, output.height, self.display_output_rgb(output));
        }
        let resolved = self.display_resolve();
        let (width, height) = self.display_dimensions();
        if !resolved.promoted || !self.should_present_resolved_display(resolved) {
            return self.display_rgb_frame();
        }
        self.resolved_display_rgb_frame(resolved, width, height)
    }

    fn resolved_display_png(&self, resolved: DisplayResolve) -> Vec<u8> {
        if resolved.source == "best_observation" {
            if let Some(png) = self.best_observation_png() {
                return png;
            }
        }
        if resolved.source == "presented_frame" {
            if let Some(png) = self.presented_frame_png() {
                return png;
            }
        }
        let (width, height) = self.display_dimensions();
        if resolved.source == "gp1_display_area" {
            self.framebuffer
                .psx_display_png(resolved.window.x, resolved.window.y, width, height)
        } else {
            self.framebuffer
                .png(resolved.window.x, resolved.window.y, width, height)
        }
    }

    fn resolved_display_rgb(&self, resolved: DisplayResolve) -> Vec<u32> {
        let (width, height) = self.display_dimensions();
        if resolved.source == "best_observation" {
            return self.cached_display_rgb_or_capture(
                self.best_observation_rgb.as_deref(),
                resolved.window,
                width,
                height,
                false,
            );
        }
        if resolved.source == "presented_frame" {
            return self.cached_display_rgb_or_capture(
                self.presented_frame_rgb.as_deref(),
                resolved.window,
                width,
                height,
                true,
            );
        }
        if resolved.source == "gp1_display_area" {
            self.framebuffer.psx_display_rgb_window(
                resolved.window.x,
                resolved.window.y,
                width,
                height,
            )
        } else {
            self.framebuffer
                .rgb_window(resolved.window.x, resolved.window.y, width, height)
        }
    }

    fn resolved_display_rgb_frame(
        &self,
        resolved: DisplayResolve,
        fallback_width: usize,
        fallback_height: usize,
    ) -> (usize, usize, Vec<u32>) {
        if resolved.source == "best_observation" {
            return self.cached_display_rgb_frame_or_capture(
                self.best_observation_rgb.as_deref(),
                self.best_observation_width,
                self.best_observation_height,
                resolved.window,
                fallback_width,
                fallback_height,
                false,
            );
        }
        if resolved.source == "presented_frame" {
            return self.cached_display_rgb_frame_or_capture(
                self.presented_frame_rgb.as_deref(),
                self.presented_frame_width,
                self.presented_frame_height,
                resolved.window,
                fallback_width,
                fallback_height,
                true,
            );
        }
        (
            fallback_width,
            fallback_height,
            self.resolved_display_rgb(resolved),
        )
    }

    fn cached_display_rgb_or_capture(
        &self,
        cached: Option<&[u32]>,
        window: FrameBufferWindow,
        width: usize,
        height: usize,
        use_psx_wrap: bool,
    ) -> Vec<u32> {
        let expected_len = width.saturating_mul(height);
        if let Some(rgb) = cached
            && rgb.len() == expected_len
        {
            return rgb.to_vec();
        }

        if use_psx_wrap {
            self.framebuffer
                .psx_display_rgb_window(window.x, window.y, width, height)
        } else {
            self.framebuffer
                .rgb_window(window.x, window.y, width, height)
        }
    }

    fn cached_display_rgb_frame_or_capture(
        &self,
        cached: Option<&[u32]>,
        cached_width: usize,
        cached_height: usize,
        window: FrameBufferWindow,
        fallback_width: usize,
        fallback_height: usize,
        use_psx_wrap: bool,
    ) -> (usize, usize, Vec<u32>) {
        if let Some(rgb) = cached {
            let cached_len = cached_width.saturating_mul(cached_height);
            if cached_width > 0 && cached_height > 0 && rgb.len() == cached_len {
                return (cached_width, cached_height, rgb.to_vec());
            }

            let fallback_len = fallback_width.saturating_mul(fallback_height);
            if rgb.len() == fallback_len {
                return (fallback_width, fallback_height, rgb.to_vec());
            }
        }

        (
            fallback_width,
            fallback_height,
            self.cached_display_rgb_or_capture(
                None,
                window,
                fallback_width,
                fallback_height,
                use_psx_wrap,
            ),
        )
    }

    pub fn raw_actual_display_rgb_frame(&self) -> (usize, usize, Vec<u32>) {
        let output = self.current_raw_display_output_window();
        (output.width, output.height, self.display_output_rgb(output))
    }

    pub fn raw_actual_display_png(&self) -> Vec<u8> {
        let output = self.current_raw_display_output_window();
        self.display_output_png(output)
    }

    fn should_use_field_composed_output(&self, output: DisplayOutputWindow) -> bool {
        output.field_composed
            && has_native_full_scene_detail(output.window.stats)
            && (output.cached
                || !self.display_window_is_texture_atlas_with_dimensions(
                    output.window,
                    output.width,
                    output.height,
                ))
    }

    fn should_use_cached_field_composed_output(&self, output: DisplayOutputWindow) -> bool {
        if !output.cached || !self.should_use_field_composed_output(output) {
            return false;
        }
        if self.field_composed_display_is_fresh() {
            return true;
        }

        let current = self.current_display_window();
        if self.display_window_is_texture_atlas(current)
            || !screen_observation_worth_saving(current.stats)
        {
            return true;
        }
        if self.current_display_is_unusable_for_cached_field_handoff(current) {
            return true;
        }

        let cached_score = screen_observation_score(output.window.stats);
        if current.stats.checksum != output.window.stats.checksum
            && is_detailed_observation(current.stats)
        {
            return false;
        }

        if let Some(presented) = self.presented_frame_window
            && self.presented_frame_is_fresh()
            && !self.display_window_is_texture_atlas(presented)
            && screen_observation_worth_saving(presented.stats)
            && screen_observation_score(presented.stats)
                >= cached_score.saturating_sub(cached_score / 8)
        {
            return false;
        }

        true
    }

    fn current_display_is_unusable_for_cached_field_handoff(
        &self,
        current: FrameBufferWindow,
    ) -> bool {
        let (width, height) = self.display_dimensions();
        if self.display_window_is_texture_atlas_with_dimensions(current, width, height) {
            return true;
        }
        if !screen_observation_worth_saving(current.stats) || is_sparse_display(current.stats) {
            return true;
        }

        self.display_window_color_stats(current, width, height, true)
            .has_intro_caption_band()
    }

    fn field_composed_display_is_fresh(&self) -> bool {
        self.presentation_captures
            .saturating_sub(self.field_composed_display_capture_index)
            < STALE_PRESENTATION_CAPTURE_GRACE
    }

    pub fn capture_vblank_presented_frame(&mut self) {
        self.capture_current_presented_frame();
    }

    fn display_resolve(&self) -> DisplayResolve {
        let current = self.current_display_window();
        if !self.should_resolve_display_from_candidates(current) {
            return DisplayResolve {
                source: "gp1_display_area",
                promoted: false,
                window: current,
            };
        }

        let Some((source, candidate)) = self.best_resolved_display_candidate(current) else {
            return DisplayResolve {
                source: "gp1_display_area",
                promoted: false,
                window: current,
            };
        };

        DisplayResolve {
            source,
            promoted: true,
            window: candidate,
        }
    }

    fn should_resolve_display_from_candidates(&self, current: FrameBufferWindow) -> bool {
        if has_native_playfield_density(current.stats) && is_detailed_observation(current.stats) {
            return false;
        }
        if self.image_upload_commands < DISPLAY_RESOLVE_MIN_IMAGE_UPLOAD_COMMANDS
            || !self.has_textured_scene_draws()
        {
            return false;
        }
        if self.textured_draw_stats.written_pixels < DISPLAY_RESOLVE_MIN_TEXTURED_WRITTEN_PIXELS {
            return false;
        }
        if !self.has_playfield_draws() {
            return false;
        }
        if self.should_show_current_display_over_stale_candidates_with(current)
            && !self.has_resolvable_cached_display_candidate(current)
            && !self.has_resolvable_framebuffer_candidate(current)
        {
            return false;
        }
        if self.display_window_is_texture_atlas(current)
            && !self.has_resolvable_cached_display_candidate(current)
            && !self.has_resolvable_framebuffer_candidate(current)
        {
            return false;
        }
        true
    }

    fn has_textured_scene_draws(&self) -> bool {
        self.textured_triangle_commands >= DISPLAY_RESOLVE_MIN_TEXTURED_TRIANGLE_COMMANDS
            || self.textured_rect_commands >= DISPLAY_RESOLVE_MIN_TEXTURED_RECT_COMMANDS
            || (self.textured_draw_stats.written_pixels
                >= DISPLAY_RESOLVE_MIN_TEXTURED_WRITTEN_PIXELS.saturating_mul(8)
                && self.textured_draw_stats.color_changes >= 64)
    }

    fn has_resolvable_cached_display_candidate(&self, current: FrameBufferWindow) -> bool {
        let current_score = screen_observation_score(current.stats);
        self.presented_frame_window
            .into_iter()
            .chain(self.best_observation_window)
            .any(|candidate| {
                self.resolved_display_candidate_is_valid(current, current_score, candidate)
            })
    }

    fn has_resolvable_framebuffer_candidate(&self, current: FrameBufferWindow) -> bool {
        let (width, height) = self.display_dimensions();
        let current_score = screen_observation_score(current.stats);
        self.display_candidate_windows(width, height)
            .into_iter()
            .filter(|(_, candidate)| candidate.x != current.x || candidate.y != current.y)
            .any(|(_, candidate)| {
                self.resolved_display_candidate_is_valid(current, current_score, candidate)
            })
    }

    fn best_resolved_display_candidate(
        &self,
        current: FrameBufferWindow,
    ) -> Option<(&'static str, FrameBufferWindow)> {
        let (width, height) = self.display_dimensions();
        let current_score = screen_observation_score(current.stats);

        if let Some(presented) = self.presented_frame_window
            && self.presented_frame_is_fresh()
            && self.resolved_display_candidate_is_valid(current, current_score, presented)
        {
            return Some(("presented_frame", presented));
        }

        if self.should_prefer_best_observation()
            && let Some(best) = self.best_observation_window
            && self.resolved_display_candidate_is_valid(current, current_score, best)
        {
            return Some(("best_observation", best));
        }

        self.display_candidate_windows(width, height)
            .into_iter()
            .filter(|(_, candidate)| candidate.x != current.x || candidate.y != current.y)
            .filter(|(_, candidate)| {
                self.resolved_display_candidate_is_valid(current, current_score, *candidate)
            })
            .max_by(|(_, left), (_, right)| {
                screen_observation_score(left.stats).cmp(&screen_observation_score(right.stats))
            })
    }

    fn resolved_display_candidate_is_valid(
        &self,
        current: FrameBufferWindow,
        current_score: u64,
        candidate: FrameBufferWindow,
    ) -> bool {
        self.display_candidate_resolution_reason(current, current_score, candidate) == "valid"
    }

    fn display_resolve_gate_json(&self, current: FrameBufferWindow) -> String {
        format!(
            "{{\"can_attempt_candidate_resolve\":{},\"current_is_texture_atlas\":{},\"current_has_scene_density\":{},\"current_has_scene_detail\":{},\"image_upload_commands\":{},\"minimum_image_upload_commands\":{},\"image_upload_gate_passed\":{},\"textured_triangle_commands\":{},\"minimum_textured_triangle_commands\":{},\"textured_triangle_gate_passed\":{},\"textured_rect_commands\":{},\"minimum_textured_rect_commands\":{},\"textured_rect_gate_passed\":{},\"textured_scene_draw_gate_passed\":{},\"textured_written_pixels\":{},\"minimum_textured_written_pixels\":{},\"textured_written_gate_passed\":{},\"has_playfield_draws\":{},\"stale_current_without_candidates\":{}}}",
            self.should_resolve_display_from_candidates(current),
            self.display_window_is_texture_atlas(current),
            has_native_playfield_density(current.stats),
            is_detailed_observation(current.stats),
            self.image_upload_commands,
            DISPLAY_RESOLVE_MIN_IMAGE_UPLOAD_COMMANDS,
            self.image_upload_commands >= DISPLAY_RESOLVE_MIN_IMAGE_UPLOAD_COMMANDS,
            self.textured_triangle_commands,
            DISPLAY_RESOLVE_MIN_TEXTURED_TRIANGLE_COMMANDS,
            self.textured_triangle_commands >= DISPLAY_RESOLVE_MIN_TEXTURED_TRIANGLE_COMMANDS,
            self.textured_rect_commands,
            DISPLAY_RESOLVE_MIN_TEXTURED_RECT_COMMANDS,
            self.textured_rect_commands >= DISPLAY_RESOLVE_MIN_TEXTURED_RECT_COMMANDS,
            self.has_textured_scene_draws(),
            self.textured_draw_stats.written_pixels,
            DISPLAY_RESOLVE_MIN_TEXTURED_WRITTEN_PIXELS,
            self.textured_draw_stats.written_pixels >= DISPLAY_RESOLVE_MIN_TEXTURED_WRITTEN_PIXELS,
            self.has_playfield_draws(),
            self.should_show_current_display_over_stale_candidates_with(current)
                && !self.has_resolvable_cached_display_candidate(current)
                && !self.has_resolvable_framebuffer_candidate(current)
        )
    }

    fn display_candidate_diagnostics_json(&self, current: FrameBufferWindow) -> String {
        let (width, height) = self.display_dimensions();
        let current_score = screen_observation_score(current.stats);
        self.display_candidate_windows(width, height)
            .into_iter()
            .map(|(label, candidate)| {
                self.display_candidate_diagnostic_json(label, current, current_score, candidate)
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    fn display_candidate_diagnostic_json(
        &self,
        label: &'static str,
        current: FrameBufferWindow,
        current_score: u64,
        candidate: FrameBufferWindow,
    ) -> String {
        let candidate_score = screen_observation_score(candidate.stats);
        let live_draw_overlap_area = self.display_candidate_live_draw_overlap_area(candidate);
        let minimum_live_draw_overlap_area = self.minimum_live_draw_overlap_area();
        let live_draw_overlap = live_draw_overlap_area >= minimum_live_draw_overlap_area;
        let scene_upload_overlap_area = self.display_candidate_scene_upload_overlap_area(candidate);
        let minimum_scene_upload_overlap_area = self.minimum_scene_upload_overlap_area();
        let scene_upload_overlap = scene_upload_overlap_area >= minimum_scene_upload_overlap_area;
        let scene_signal =
            resolved_display_candidate_has_scene_signal(current, current_score, candidate);
        let rejection_reason =
            self.display_candidate_resolution_reason(current, current_score, candidate);
        format!(
            "{{\"label\":\"{}\",\"window\":{},\"score\":{},\"score_delta\":{},\"texture_atlas\":{},\"live_draw_overlap_area\":{},\"minimum_live_draw_overlap_area\":{},\"live_draw_overlap\":{},\"scene_upload_overlap_area\":{},\"minimum_scene_upload_overlap_area\":{},\"scene_upload_overlap\":{},\"scene_signal\":{},\"valid_for_display_resolve\":{},\"rejection_reason\":\"{}\"}}",
            label,
            framebuffer_window_json(candidate),
            candidate_score,
            candidate_score as i128 - current_score as i128,
            self.display_window_is_texture_atlas(candidate),
            live_draw_overlap_area,
            minimum_live_draw_overlap_area,
            live_draw_overlap,
            scene_upload_overlap_area,
            minimum_scene_upload_overlap_area,
            scene_upload_overlap,
            scene_signal,
            rejection_reason == "valid",
            rejection_reason
        )
    }

    fn display_candidate_resolution_reason(
        &self,
        current: FrameBufferWindow,
        current_score: u64,
        candidate: FrameBufferWindow,
    ) -> &'static str {
        if candidate.x == current.x
            && candidate.y == current.y
            && candidate.stats.checksum == current.stats.checksum
        {
            return "current_display";
        }
        if self.display_window_is_texture_atlas(candidate) {
            return "texture_atlas";
        }
        if self.resolved_display_candidate_has_sparse_live_display_signal(
            current,
            current_score,
            candidate,
        ) {
            return "valid";
        }
        if !has_native_playfield_density(candidate.stats) {
            return "low_density";
        }
        if !is_detailed_observation(candidate.stats) {
            return "low_detail";
        }

        let candidate_score = screen_observation_score(candidate.stats);
        if candidate_score <= current_score.saturating_add(current_score / 2) {
            return "score_not_high_enough";
        }
        if candidate.stats.detail_edges <= current.stats.detail_edges.saturating_mul(4).max(512) {
            return "detail_not_high_enough";
        }
        if !self.display_candidate_has_live_draw_overlap(candidate)
            && !self.display_candidate_has_scene_upload_overlap(candidate)
        {
            return "no_live_draw_or_scene_upload_overlap";
        }
        "valid"
    }

    fn has_playfield_draws(&self) -> bool {
        self.playfield_draw_traces()
            .any(|trace| trace.score() > 0 && trace.bounds.area() >= 512)
    }

    fn playfield_draw_traces(&self) -> impl Iterator<Item = &GpuDrawTrace> {
        self.overlap_draw_commands
            .iter()
            .chain(self.top_draw_commands.iter())
            .chain(self.focus_draw_commands.iter())
    }

    fn display_candidate_has_live_draw_overlap(&self, candidate: FrameBufferWindow) -> bool {
        self.display_candidate_live_draw_overlap_area(candidate)
            >= self.minimum_live_draw_overlap_area()
    }

    fn display_candidate_has_scene_upload_overlap(&self, candidate: FrameBufferWindow) -> bool {
        if !has_native_playfield_density(candidate.stats)
            || !is_detailed_observation(candidate.stats)
        {
            return false;
        }
        self.display_candidate_scene_upload_overlap_area(candidate)
            >= self.minimum_scene_upload_overlap_area()
    }

    fn minimum_live_draw_overlap_area(&self) -> i64 {
        let (width, height) = self.display_dimensions();
        ((width as i64).saturating_mul(height as i64) / 64).max(512)
    }

    fn minimum_scene_upload_overlap_area(&self) -> i64 {
        let (width, height) = self.display_dimensions();
        ((width as i64).saturating_mul(height as i64) / 16).max(1024)
    }

    fn display_candidate_live_draw_overlap_area(&self, candidate: FrameBufferWindow) -> i64 {
        let (width, height) = self.display_dimensions();
        let Some(right) = candidate
            .x
            .checked_add(width)
            .and_then(|value| value.checked_sub(1))
        else {
            return 0;
        };
        let Some(bottom) = candidate
            .y
            .checked_add(height)
            .and_then(|value| value.checked_sub(1))
        else {
            return 0;
        };
        let candidate_playfield = DrawBounds {
            left: candidate.x as i32,
            top: candidate.y.saturating_add(64) as i32,
            right: right as i32,
            bottom: bottom as i32,
        };
        self.playfield_draw_traces()
            .filter(|trace| {
                (trace.kind == "textured_quad" || trace.kind == "textured_triangle")
                    && trace.stats.written_pixels > 0
                    && trace.score() > 0
            })
            .map(|trace| trace.bounds.intersection_area(candidate_playfield))
            .sum::<i64>()
    }

    fn display_candidate_scene_upload_overlap_area(&self, candidate: FrameBufferWindow) -> i64 {
        let (width, height) = self.display_dimensions();
        self.texture_upload_overlap_area(candidate, width, height)
    }

    fn resolved_display_candidate_has_sparse_live_display_signal(
        &self,
        current: FrameBufferWindow,
        current_score: u64,
        candidate: FrameBufferWindow,
    ) -> bool {
        if candidate.x == current.x
            && candidate.y == current.y
            && candidate.stats.checksum == current.stats.checksum
        {
            return false;
        }
        if self.display_window_is_texture_atlas(candidate) {
            return false;
        }
        if !is_sparse_display(current.stats) {
            return false;
        }
        if !self.display_candidate_has_live_draw_overlap(candidate) {
            return false;
        }
        if !screen_observation_worth_saving(candidate.stats) {
            return false;
        }

        let stats = candidate.stats;
        let sparse_detail_cutoff = (stats.pixel_count / 128).max(512);
        let candidate_score = screen_observation_score(stats);
        stats.max_luma >= 128
            && stats.bright_pixels >= 384
            && stats.nonzero_pixels >= 512
            && stats.detail_edges >= sparse_detail_cutoff
            && candidate_score > current_score.saturating_add(current_score / 2)
    }

    fn should_prefer_best_observation(&self) -> bool {
        let Some(best) = self.best_observation_window else {
            return false;
        };
        if self.display_window_is_texture_atlas(best) {
            return false;
        }
        let Some(presented) = self.presented_frame_window else {
            return true;
        };
        if self.display_window_is_texture_atlas(presented) {
            return true;
        }
        if self.should_prefer_presented_frame() {
            return false;
        }
        if is_sparse_display(presented.stats)
            && best.stats.nonzero_pixels > presented.stats.nonzero_pixels.saturating_mul(4)
        {
            return true;
        }
        if should_defer_low_detail_current(presented.stats, best.stats) {
            return true;
        }
        if (best.x != presented.x || best.y != presented.y)
            && is_detailed_observation(presented.stats)
        {
            return false;
        }
        screen_observation_score(best.stats) > screen_observation_score(presented.stats)
    }

    fn should_prefer_presented_frame(&self) -> bool {
        let Some(presented) = self.presented_frame_window else {
            return false;
        };
        if self.display_window_is_texture_atlas(presented) {
            return false;
        }
        if self.should_show_current_display_over_stale_candidates() {
            return false;
        }
        if !screen_observation_worth_saving(presented.stats) {
            return false;
        }
        let Some(best) = self.best_observation_window else {
            return true;
        };
        if self.display_window_is_texture_atlas(best) {
            return true;
        }
        if !screen_observation_worth_saving(best.stats) {
            return true;
        }
        if is_sparse_display(presented.stats)
            && best.stats.nonzero_pixels > presented.stats.nonzero_pixels.saturating_mul(4)
        {
            return false;
        }
        screen_observation_score(presented.stats) >= screen_observation_score(best.stats)
    }

    fn presented_frame_is_fresh(&self) -> bool {
        self.presentation_captures
            .saturating_sub(self.presented_frame_capture_index)
            < STALE_PRESENTATION_CAPTURE_GRACE
    }

    pub fn screenshot_window(&self) -> FrameBufferWindow {
        let (start_x, start_y) = display_area_start_xy(self.display_area_start);
        let (width, height) = self.display_dimensions();
        let stats = self
            .framebuffer
            .psx_display_stats(start_x, start_y, width, height);
        let display_window = FrameBufferWindow {
            x: start_x,
            y: start_y,
            stats,
        };
        let (drawing_x, drawing_y) = drawing_area_xy(self.drawing_area_top_left);
        let drawing_x = drawing_x.max(0) as usize;
        let drawing_y = drawing_y.max(0) as usize;
        let drawing_window = FrameBufferWindow {
            x: drawing_x,
            y: drawing_y,
            stats: self
                .framebuffer
                .display_stats(drawing_x, drawing_y, width, height),
        };

        let mut best_window = display_window;
        if should_use_observation_fallback(best_window.stats, drawing_window.stats) {
            best_window = drawing_window;
        }

        if let Some(densest_window) = self.framebuffer.densest_window(width, height, 8)
            && should_use_observation_fallback(best_window.stats, densest_window.stats)
        {
            best_window = densest_window;
        }

        if let Some(brightest_window) = self.framebuffer.brightest_window(width, height, 8)
            && should_use_observation_fallback(best_window.stats, brightest_window.stats)
        {
            best_window = brightest_window;
        }
        best_window
    }

    pub fn vram_png(&self) -> Vec<u8> {
        self.framebuffer.png(0, 0, VRAM_WIDTH, VRAM_HEIGHT)
    }

    pub fn set_draw_capture_range(&mut self, start: u64, end: u64) {
        self.draw_capture_range = Some((start.min(end), start.max(end)));
        self.draw_captures.clear();
    }

    pub fn draw_captures(&self) -> &[NativeGpuDrawCapture] {
        &self.draw_captures
    }

    pub fn framebuffer_stats(&self) -> FrameBufferStats {
        let (start_x, start_y) = display_area_start_xy(self.display_area_start);
        let (width, height) = self.display_dimensions();
        self.framebuffer
            .psx_display_stats(start_x, start_y, width, height)
    }

    pub fn vram_stats(&self) -> FrameBufferStats {
        self.framebuffer.stats()
    }

    pub fn vram_nonzero_bounds(&self) -> Option<FrameBufferBounds> {
        self.framebuffer.nonzero_bounds()
    }

    pub fn vram_nonzero_bounds_json(&self) -> String {
        self.vram_nonzero_bounds()
            .map_or_else(|| "null".to_string(), FrameBufferBounds::json)
    }

    pub fn gp0_pending_words(&self) -> usize {
        self.gp0_fifo.len()
    }

    pub fn gp0_pending_head(&self) -> u32 {
        self.gp0_fifo.first().copied().unwrap_or(0)
    }

    pub fn gp0_pending_expected_words(&self) -> Option<usize> {
        gp0_expected_words(&self.gp0_fifo)
    }

    pub fn display_dimensions(&self) -> (usize, usize) {
        display_dimensions_from_registers(self.status, self.horizontal_range, self.vertical_range)
    }

    fn drain_gp0_fifo(&mut self) {
        loop {
            let Some(expected_words) = gp0_expected_words(&self.gp0_fifo) else {
                return;
            };
            if self.gp0_fifo.len() < expected_words {
                return;
            }

            let command = self.gp0_fifo[..expected_words].to_vec();
            let command_sources = self.gp0_fifo_sources[..expected_words].to_vec();
            self.gp0_fifo.drain(..expected_words);
            self.gp0_fifo_sources.drain(..expected_words);
            self.execute_gp0_command(&command, &command_sources);
        }
    }

    fn execute_gp0_command(&mut self, words: &[u32], sources: &[Option<GpuCommandSource>]) {
        if words.is_empty() {
            return;
        }

        let source = first_command_source(sources);
        let source_ref = source.as_ref();
        let command = (words[0] >> 24) as u8;
        self.gp0_command_counts[command as usize] =
            self.gp0_command_counts[command as usize].saturating_add(1);
        self.push_recent_gp0_command(words, source_ref);
        match command {
            0x02 if words.len() >= 3 => {
                let (x, y) = xy(words[1]);
                let width = (words[2] & 0xffff) as i32;
                let height = (words[2] >> 16) as i32;
                if !fill_rect_dimensions_valid(width, height) {
                    self.invalid_fill_rect_commands += 1;
                    return;
                }
                self.capture_presented_frame_before_clear(x, y, width, height);
                self.fill_rect_commands += 1;
                self.framebuffer.fill_rect_unclipped_with_options(
                    x,
                    y,
                    width,
                    height,
                    color(words[0]),
                    self.pixel_write_options(),
                );
                self.push_recent_draw_command(GpuDrawTrace::flat(
                    "fill_rect_unclipped",
                    color(words[0]),
                    rect_bounds(Point { x, y }, width, height),
                    words,
                    &[Point { x, y }],
                    source_ref,
                ));
            }
            0x20..=0x23 if words.len() >= 4 => {
                self.draw_flat_triangle(
                    words,
                    source_ref,
                    words[1],
                    words[2],
                    words[3],
                    color(words[0]),
                );
            }
            0x24..=0x27 if words.len() >= 7 => {
                self.draw_textured_triangle(
                    words,
                    source_ref,
                    [
                        (words[1], words[2]),
                        (words[3], words[4]),
                        (words[5], words[6]),
                    ],
                );
            }
            0x28..=0x2b if words.len() >= 5 => {
                self.draw_flat_quad(
                    words,
                    words[1],
                    words[2],
                    words[3],
                    words[4],
                    color(words[0]),
                    source_ref,
                );
            }
            0x2c..=0x2f if words.len() >= 9 => {
                self.draw_textured_quad(
                    words,
                    source_ref,
                    [
                        (words[1], words[2]),
                        (words[3], words[4]),
                        (words[5], words[6]),
                        (words[7], words[8]),
                    ],
                );
            }
            0x30..=0x33 if words.len() >= 6 => {
                self.draw_flat_triangle(
                    words,
                    source_ref,
                    words[1],
                    words[3],
                    words[5],
                    color(words[0]),
                );
            }
            0x34..=0x37 if words.len() >= 9 => {
                self.draw_textured_triangle(
                    words,
                    source_ref,
                    [
                        (words[1], words[2]),
                        (words[4], words[5]),
                        (words[7], words[8]),
                    ],
                );
            }
            0x38..=0x3b if words.len() >= 8 => {
                self.draw_flat_quad(
                    words,
                    words[1],
                    words[3],
                    words[5],
                    words[7],
                    color(words[0]),
                    source_ref,
                );
            }
            0x3c..=0x3f if words.len() >= 12 => {
                self.draw_textured_quad(
                    words,
                    source_ref,
                    [
                        (words[1], words[2]),
                        (words[4], words[5]),
                        (words[7], words[8]),
                        (words[10], words[11]),
                    ],
                );
            }
            0x40..=0x47 if words.len() >= 3 => {
                self.draw_flat_line(words, source_ref, words[1], words[2], color(words[0]));
            }
            0x50..=0x57 if words.len() >= 4 => {
                self.draw_flat_line(words, source_ref, words[1], words[3], color(words[0]));
            }
            0x60..=0x63 if words.len() >= 3 => {
                let (x, y) = xy(words[1]);
                let width = (words[2] & 0xffff) as i32;
                let height = (words[2] >> 16) as i32;
                if !fill_rect_dimensions_valid(width, height) {
                    self.invalid_fill_rect_commands += 1;
                    return;
                }
                self.fill_rect_commands += 1;
                self.framebuffer.fill_rect_with_options(
                    x,
                    y,
                    width,
                    height,
                    color(words[0]),
                    self.flat_draw_options(words[0]),
                );
                self.push_recent_draw_command(GpuDrawTrace::flat(
                    "fill_rect",
                    color(words[0]),
                    rect_bounds(Point { x, y }, width, height),
                    words,
                    &[Point { x, y }],
                    source_ref,
                ));
            }
            0x64..=0x67 if words.len() >= 4 => {
                let width = (words[3] & 0xffff) as i32;
                let height = (words[3] >> 16) as i32;
                self.draw_textured_rect(words, source_ref, words[1], words[2], width, height);
            }
            0x68..=0x6f if words.len() >= 2 => {
                let (x, y) = xy(words[1]);
                self.fill_rect_commands += 1;
                self.framebuffer.fill_rect_with_options(
                    x,
                    y,
                    1,
                    1,
                    color(words[0]),
                    self.flat_draw_options(words[0]),
                );
                self.push_recent_draw_command(GpuDrawTrace::flat(
                    "fill_rect_1x1",
                    color(words[0]),
                    rect_bounds(Point { x, y }, 1, 1),
                    words,
                    &[Point { x, y }],
                    source_ref,
                ));
            }
            0x70..=0x73 if words.len() >= 2 => {
                let (x, y) = xy(words[1]);
                self.fill_rect_commands += 1;
                self.framebuffer.fill_rect_with_options(
                    x,
                    y,
                    8,
                    8,
                    color(words[0]),
                    self.flat_draw_options(words[0]),
                );
                self.push_recent_draw_command(GpuDrawTrace::flat(
                    "fill_rect_8x8",
                    color(words[0]),
                    rect_bounds(Point { x, y }, 8, 8),
                    words,
                    &[Point { x, y }],
                    source_ref,
                ));
            }
            0x74..=0x77 if words.len() >= 3 => {
                self.draw_textured_rect(words, source_ref, words[1], words[2], 8, 8);
            }
            0x78..=0x7b if words.len() >= 2 => {
                let (x, y) = xy(words[1]);
                self.fill_rect_commands += 1;
                self.framebuffer.fill_rect_with_options(
                    x,
                    y,
                    16,
                    16,
                    color(words[0]),
                    self.flat_draw_options(words[0]),
                );
                self.push_recent_draw_command(GpuDrawTrace::flat(
                    "fill_rect_16x16",
                    color(words[0]),
                    rect_bounds(Point { x, y }, 16, 16),
                    words,
                    &[Point { x, y }],
                    source_ref,
                ));
            }
            0x7c..=0x7f if words.len() >= 3 => {
                self.draw_textured_rect(words, source_ref, words[1], words[2], 16, 16);
            }
            0x80 if words.len() >= 4 => {
                let (source_x, source_y) = unsigned_xy(words[1]);
                let (dest_x, dest_y) = unsigned_xy(words[2]);
                if let Some((width, height)) = vram_copy_dimensions(words[3]) {
                    let valid =
                        vram_copy_request_valid(source_x, source_y, dest_x, dest_y, width, height);
                    if !valid {
                        self.invalid_vram_copy_commands += 1;
                        self.push_recent_transfer_command(GpuTransferTrace::vram_copy(
                            source_x, source_y, dest_x, dest_y, width, height, false,
                        ));
                        return;
                    }
                    self.vram_copy_commands += 1;
                    self.push_recent_transfer_command(GpuTransferTrace::vram_copy(
                        source_x, source_y, dest_x, dest_y, width, height, true,
                    ));
                    self.framebuffer.copy_rect_with_options(
                        source_x,
                        source_y,
                        dest_x,
                        dest_y,
                        width,
                        height,
                        self.pixel_write_options(),
                    );
                } else {
                    self.invalid_vram_copy_commands += 1;
                    let (width, height) = raw_dimensions(words[3]);
                    self.push_recent_transfer_command(GpuTransferTrace::vram_copy(
                        source_x,
                        source_y,
                        dest_x,
                        dest_y,
                        width as i32,
                        height as i32,
                        false,
                    ));
                }
            }
            0xa0 if words.len() >= 3 => {
                let (x, y) = unsigned_xy(words[1]);
                if let Some((width, height)) = image_transfer_dimensions(words[2]) {
                    self.image_upload_commands += 1;
                    self.push_image_upload_rect(x, y, width as i32, height as i32);
                    self.push_recent_transfer_command(GpuTransferTrace::image_upload(
                        x,
                        y,
                        width as i32,
                        height as i32,
                        words.len().saturating_sub(3),
                        true,
                    ));
                    self.framebuffer.write_rgb555_image_with_options(
                        x,
                        y,
                        width as i32,
                        height as i32,
                        &words[3..],
                        self.pixel_write_options(),
                    );
                } else {
                    self.invalid_image_upload_commands += 1;
                    let (width, height) = raw_dimensions(words[2]);
                    self.push_recent_transfer_command(GpuTransferTrace::image_upload(
                        x,
                        y,
                        width as i32,
                        height as i32,
                        words.len().saturating_sub(3),
                        false,
                    ));
                }
            }
            0xc0 if words.len() >= 3 => {
                self.gp0_read = 0;
            }
            0xe1 => self.texture_page = (words[0] & 0x3fff) as u16,
            0xe2 => self.texture_window = TextureWindow::from_gp0_e2(words[0]),
            0xe3 => {
                self.drawing_area_top_left = words[0] & 0x000f_ffff;
                self.update_drawing_clip();
            }
            0xe4 => {
                self.drawing_area_bottom_right = words[0] & 0x000f_ffff;
                self.update_drawing_clip();
            }
            0xe5 => self.drawing_offset = words[0] & 0x003f_ffff,
            0xe6 => {
                self.set_mask_bit = words[0] & 0x01 != 0;
                self.check_mask_bit = words[0] & 0x02 != 0;
            }
            _ => {}
        }
        self.capture_best_observation_after_gp0(command);
    }

    fn draw_flat_triangle(
        &mut self,
        words: &[u32],
        source: Option<&GpuCommandSource>,
        a: u32,
        b: u32,
        c: u32,
        color: u32,
    ) {
        self.flat_triangle_commands += 1;
        let a = self.offset_point(point(a));
        let b = self.offset_point(point(b));
        let c = self.offset_point(point(c));
        self.framebuffer.draw_triangle_with_options(
            a,
            b,
            c,
            color,
            self.flat_draw_options(words[0]),
        );
        self.push_recent_draw_command(GpuDrawTrace::flat(
            "flat_triangle",
            color,
            points_bounds(&[a, b, c]),
            words,
            &[a, b, c],
            source,
        ));
    }

    fn draw_flat_quad(
        &mut self,
        words: &[u32],
        a: u32,
        b: u32,
        c: u32,
        d: u32,
        color: u32,
        source: Option<&GpuCommandSource>,
    ) {
        self.flat_triangle_commands += 2;
        let a = self.offset_point(point(a));
        let b = self.offset_point(point(b));
        let c = self.offset_point(point(c));
        let d = self.offset_point(point(d));
        self.framebuffer.draw_triangle_with_options(
            a,
            b,
            c,
            color,
            self.flat_draw_options(words[0]),
        );
        self.framebuffer.draw_triangle_with_options(
            b,
            c,
            d,
            color,
            self.flat_draw_options(words[0]),
        );
        self.push_recent_draw_command(GpuDrawTrace::flat(
            "flat_quad",
            color,
            points_bounds(&[a, b, c, d]),
            words,
            &[a, b, c, d],
            source,
        ));
    }

    fn draw_flat_line(
        &mut self,
        words: &[u32],
        source: Option<&GpuCommandSource>,
        a: u32,
        b: u32,
        color: u32,
    ) {
        self.flat_line_commands += 1;
        let a = self.offset_point(point(a));
        let b = self.offset_point(point(b));
        self.framebuffer
            .draw_line_with_options(a, b, color, self.flat_draw_options(words[0]));
        self.push_recent_draw_command(GpuDrawTrace::flat(
            "flat_line",
            color,
            points_bounds(&[a, b]),
            words,
            &[a, b],
            source,
        ));
    }

    fn draw_textured_triangle(
        &mut self,
        words: &[u32],
        source: Option<&GpuCommandSource>,
        vertices: [(u32, u32); 3],
    ) {
        self.textured_triangle_commands += 1;
        let [(a, a_uv), (b, b_uv), (c, c_uv)] = vertices;
        let clut = clut(a_uv);
        let texture_page = texture_page(b_uv);
        self.texture_page = texture_page;
        let options = self.texture_draw_options(words[0], texture_page);
        let a = self.textured_point(a, a_uv);
        let b = self.textured_point(b, b_uv);
        let c = self.textured_point(c, c_uv);
        let stats = self.framebuffer.draw_textured_triangle(
            a,
            b,
            c,
            texture_page,
            clut,
            options,
            self.texture_window,
        );
        self.add_textured_draw_stats(stats);
        self.push_recent_draw_command(GpuDrawTrace::textured(
            "textured_triangle",
            texture_page,
            clut,
            points_bounds(&[a.point, b.point, c.point]),
            stats,
            words,
            &[a.point, b.point, c.point],
            source,
        ));
    }

    fn draw_textured_quad(
        &mut self,
        words: &[u32],
        source: Option<&GpuCommandSource>,
        vertices: [(u32, u32); 4],
    ) {
        self.textured_triangle_commands += 2;
        let [(a, a_uv), (b, b_uv), (c, c_uv), (d, d_uv)] = vertices;
        let clut = clut(a_uv);
        let texture_page = texture_page(b_uv);
        self.texture_page = texture_page;
        let options = self.texture_draw_options(words[0], texture_page);
        let a = self.textured_point(a, a_uv);
        let b = self.textured_point(b, b_uv);
        let c = self.textured_point(c, c_uv);
        let d = self.textured_point(d, d_uv);
        let first = self.framebuffer.draw_textured_triangle(
            a,
            b,
            c,
            texture_page,
            clut,
            options,
            self.texture_window,
        );
        let second = self.framebuffer.draw_textured_triangle(
            b,
            c,
            d,
            texture_page,
            clut,
            options,
            self.texture_window,
        );
        self.add_textured_draw_stats(first);
        self.add_textured_draw_stats(second);
        self.push_recent_draw_command(GpuDrawTrace::textured(
            "textured_quad",
            texture_page,
            clut,
            points_bounds(&[a.point, b.point, c.point, d.point]),
            combine_textured_draw_stats(first, second),
            words,
            &[a.point, b.point, c.point, d.point],
            source,
        ));
    }

    fn draw_textured_rect(
        &mut self,
        words: &[u32],
        source: Option<&GpuCommandSource>,
        xy: u32,
        uv: u32,
        width: i32,
        height: i32,
    ) {
        self.textured_rect_commands += 1;
        let point = self.offset_point(point(xy));
        let stats = self.framebuffer.draw_textured_rect(
            point,
            (width, height),
            self.texture_page,
            clut(uv),
            texture_coordinate(uv),
            self.texture_draw_options(words[0], self.texture_page),
            self.texture_window,
        );
        self.add_textured_draw_stats(stats);
        self.push_recent_draw_command(GpuDrawTrace::textured(
            "textured_rect",
            self.texture_page,
            clut(uv),
            rect_bounds(point, width, height),
            stats,
            words,
            &[point],
            source,
        ));
    }

    fn add_textured_draw_stats(&mut self, stats: TexturedDrawStats) {
        self.textured_draw_stats.sampled_pixels = self
            .textured_draw_stats
            .sampled_pixels
            .saturating_add(stats.sampled_pixels);
        self.textured_draw_stats.drawn_pixels = self
            .textured_draw_stats
            .drawn_pixels
            .saturating_add(stats.drawn_pixels);
        self.textured_draw_stats.written_pixels = self
            .textured_draw_stats
            .written_pixels
            .saturating_add(stats.written_pixels);
        self.textured_draw_stats.clipped_pixels = self
            .textured_draw_stats
            .clipped_pixels
            .saturating_add(stats.clipped_pixels);
        self.textured_draw_stats.transparent_pixels = self
            .textured_draw_stats
            .transparent_pixels
            .saturating_add(stats.transparent_pixels);
        if stats.drawn_pixels != 0 {
            if self.textured_draw_stats.drawn_pixels == stats.drawn_pixels {
                self.textured_draw_stats.first_color = stats.first_color;
            } else if self.textured_draw_stats.last_color != stats.first_color {
                self.textured_draw_stats.color_changes =
                    self.textured_draw_stats.color_changes.saturating_add(1);
            }
            self.textured_draw_stats.last_color = stats.last_color;
            self.textured_draw_stats.color_hash ^= stats.color_hash;
            self.textured_draw_stats.color_hash =
                self.textured_draw_stats.color_hash.wrapping_mul(16_777_619);
            self.textured_draw_stats.color_changes = self
                .textured_draw_stats
                .color_changes
                .saturating_add(stats.color_changes);
        }
    }

    fn textured_point(&self, xy: u32, uv: u32) -> TexturedPoint {
        TexturedPoint {
            point: self.offset_point(point(xy)),
            u: (uv & 0xff) as u8,
            v: ((uv >> 8) & 0xff) as u8,
        }
    }

    fn offset_point(&self, point: Point) -> Point {
        let (x, y) = drawing_offset_xy(self.drawing_offset);
        Point {
            x: point.x + x,
            y: point.y + y,
        }
    }

    fn update_drawing_clip(&mut self) {
        self.framebuffer.set_clip(drawing_area_clip(
            self.drawing_area_top_left,
            self.drawing_area_bottom_right,
        ));
    }

    fn pixel_write_options(&self) -> PixelWriteOptions {
        PixelWriteOptions {
            set_mask_bit: self.set_mask_bit,
            check_mask_bit: self.check_mask_bit,
            semi_transparent: false,
            semi_transparency_mode: 0,
        }
    }

    fn flat_draw_options(&self, command_word: u32) -> PixelWriteOptions {
        PixelWriteOptions {
            set_mask_bit: self.set_mask_bit,
            check_mask_bit: self.check_mask_bit,
            semi_transparent: command_word & 0x0200_0000 != 0,
            semi_transparency_mode: ((self.texture_page >> 5) & 0x03) as u8,
        }
    }

    fn texture_draw_options(&self, command_word: u32, texture_page: u16) -> TextureDrawOptions {
        let mut options = texture_draw_options(command_word, texture_page);
        options.set_mask_bit = self.set_mask_bit;
        options.check_mask_bit = self.check_mask_bit;
        options
    }

    fn gp0_command_counts_json(&self) -> String {
        self.gp0_command_counts
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

    fn push_recent_gp0_command(&mut self, words: &[u32], source: Option<&GpuCommandSource>) {
        self.recent_gp0_commands
            .push(Gp0CommandTrace::new(words, source));
        if self.recent_gp0_commands.len() > GP0_RECENT_COMMAND_LIMIT {
            self.recent_gp0_commands.remove(0);
        }
    }

    fn recent_gp0_commands_json(&self) -> String {
        self.recent_gp0_commands
            .iter()
            .map(Gp0CommandTrace::json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn push_recent_gp1_command(&mut self, value: u32) {
        self.recent_gp1_commands.push(Gp1CommandTrace::new(value));
        if self.recent_gp1_commands.len() > GP1_RECENT_COMMAND_LIMIT {
            self.recent_gp1_commands.remove(0);
        }
    }

    fn recent_gp1_commands_json(&self) -> String {
        self.recent_gp1_commands
            .iter()
            .map(Gp1CommandTrace::json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn push_recent_transfer_command(&mut self, trace: GpuTransferTrace) {
        self.recent_transfer_commands.push(trace);
        if self.recent_transfer_commands.len() > GPU_RECENT_TRANSFER_LIMIT {
            self.recent_transfer_commands.remove(0);
        }
    }

    fn push_image_upload_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        if width <= 0 || height <= 0 {
            return;
        }
        self.image_upload_rects
            .push(rect_bounds(Point { x, y }, width, height));
        if self.image_upload_rects.len() > GPU_IMAGE_UPLOAD_RECT_LIMIT {
            self.image_upload_rects.remove(0);
        }
    }

    fn recent_transfer_commands_json(&self) -> String {
        self.recent_transfer_commands
            .iter()
            .map(GpuTransferTrace::json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn push_recent_draw_command(&mut self, mut trace: GpuDrawTrace) {
        self.draw_sequence = self.draw_sequence.saturating_add(1);
        trace.sequence = self.draw_sequence;
        if trace.score() > 0
            && self
                .largest_draw_command
                .as_ref()
                .is_none_or(|largest| trace.score() > largest.score())
        {
            self.largest_draw_command = Some(trace.clone());
        }
        self.push_top_draw_command(trace.clone());
        self.push_focus_draw_command(trace.clone());
        self.push_overlap_draw_command(trace.clone());
        self.capture_draw_if_requested(&trace);
        self.recent_draw_commands.push(trace);
        if self.recent_draw_commands.len() > GPU_RECENT_DRAW_LIMIT {
            self.recent_draw_commands.remove(0);
        }
    }

    fn capture_draw_if_requested(&mut self, trace: &GpuDrawTrace) {
        let Some((start, end)) = self.draw_capture_range else {
            return;
        };
        if trace.sequence < start || trace.sequence > end {
            return;
        }
        if self.draw_captures.len() >= GPU_DRAW_CAPTURE_LIMIT {
            return;
        }

        let (display_width, display_height) = self.display_dimensions();
        let (display_x, display_y) = display_area_start_xy(self.display_area_start);
        let (bounds_x, bounds_y, bounds_width, bounds_height) =
            trace
                .bounds
                .capture_window(display_width, display_height, 16);
        let texture_png = trace
            .texture_page
            .zip(trace.clut)
            .map(|(texture_page, clut)| self.framebuffer.decoded_texture_png(texture_page, clut));
        let palette_png = trace
            .texture_page
            .zip(trace.clut)
            .map(|(texture_page, clut)| self.framebuffer.texture_palette_png(texture_page, clut));
        self.draw_captures.push(NativeGpuDrawCapture {
            sequence: trace.sequence,
            trace_json: trace.json(),
            display_x,
            display_y,
            display_width,
            display_height,
            display_png: self.framebuffer.psx_display_png(
                display_x,
                display_y,
                display_width,
                display_height,
            ),
            bounds_x,
            bounds_y,
            bounds_width,
            bounds_height,
            bounds_png: self
                .framebuffer
                .png(bounds_x, bounds_y, bounds_width, bounds_height),
            texture_png,
            palette_png,
        });
    }

    fn push_top_draw_command(&mut self, trace: GpuDrawTrace) {
        if trace.score() <= 0 {
            return;
        }
        self.top_draw_commands.push(trace);
        self.top_draw_commands.sort_by(|left, right| {
            right
                .score()
                .cmp(&left.score())
                .then(right.bounds.area().cmp(&left.bounds.area()))
        });
        self.top_draw_commands.truncate(GPU_TOP_DRAW_LIMIT);
    }

    fn push_focus_draw_command(&mut self, trace: GpuDrawTrace) {
        if !trace.is_focus_candidate() {
            return;
        }
        self.focus_draw_commands.push(trace);
        if self.focus_draw_commands.len() > GPU_FOCUS_DRAW_LIMIT {
            self.focus_draw_commands.remove(0);
        }
    }

    fn push_overlap_draw_command(&mut self, trace: GpuDrawTrace) {
        if !trace.overlaps_playfield() {
            return;
        }
        self.overlap_draw_commands.push(trace);
        if self.overlap_draw_commands.len() > GPU_OVERLAP_DRAW_LIMIT {
            self.overlap_draw_commands.remove(0);
        }
    }

    fn recent_draw_commands_json(&self) -> String {
        self.recent_draw_commands
            .iter()
            .map(GpuDrawTrace::json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn recent_draw_commands_compact_json(&self, limit: usize) -> String {
        let skip = self.recent_draw_commands.len().saturating_sub(limit);
        self.recent_draw_commands
            .iter()
            .skip(skip)
            .map(GpuDrawTrace::compact_json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn push_display_area_trace(&mut self, value: u32) {
        let value = value & 0x00ff_ffff;
        let (x, y) = display_area_start_xy(value);
        let trace = DisplayAreaTrace {
            value,
            x,
            y,
            command_index: self.commands_seen,
            draw_sequence: self.draw_sequence,
        };
        if self
            .display_area_history
            .last()
            .is_some_and(|last| last.value == trace.value)
        {
            return;
        }
        self.display_area_history.push(trace);
        if self.display_area_history.len() > GPU_DISPLAY_AREA_HISTORY_LIMIT {
            self.display_area_history.remove(0);
        }
    }

    fn display_area_history_json(&self) -> String {
        self.display_area_history
            .iter()
            .map(DisplayAreaTrace::json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn largest_draw_command_json(&self) -> String {
        self.largest_draw_command
            .as_ref()
            .map_or_else(|| "null".to_string(), GpuDrawTrace::json)
    }

    fn top_draw_commands_json(&self) -> String {
        self.top_draw_commands
            .iter()
            .map(GpuDrawTrace::json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn top_draw_commands_compact_json(&self, limit: usize) -> String {
        self.top_draw_commands
            .iter()
            .take(limit)
            .map(GpuDrawTrace::compact_json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn focus_draw_commands_json(&self) -> String {
        self.focus_draw_commands
            .iter()
            .map(GpuDrawTrace::json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn focus_draw_commands_compact_json(&self, limit: usize) -> String {
        let skip = self.focus_draw_commands.len().saturating_sub(limit);
        self.focus_draw_commands
            .iter()
            .skip(skip)
            .map(GpuDrawTrace::compact_json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn overlap_draw_commands_json(&self) -> String {
        self.overlap_draw_commands
            .iter()
            .map(GpuDrawTrace::json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn overlap_draw_commands_compact_json(&self, limit: usize) -> String {
        let skip = self.overlap_draw_commands.len().saturating_sub(limit);
        self.overlap_draw_commands
            .iter()
            .skip(skip)
            .map(GpuDrawTrace::compact_json)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn largest_draw_command_compact_json(&self) -> String {
        self.largest_draw_command
            .as_ref()
            .map_or_else(|| "null".to_string(), GpuDrawTrace::compact_json)
    }

    fn window_diagnostics_json(&self) -> String {
        let (width, height) = self.display_dimensions();
        let (display_x, display_y) = display_area_start_xy(self.display_area_start);
        let display = FrameBufferWindow {
            x: display_x,
            y: display_y,
            stats: self
                .framebuffer
                .psx_display_stats(display_x, display_y, width, height),
        };

        let (drawing_x, drawing_y) = drawing_area_xy(self.drawing_area_top_left);
        let drawing_x = drawing_x.max(0) as usize;
        let drawing_y = drawing_y.max(0) as usize;
        let drawing = FrameBufferWindow {
            x: drawing_x,
            y: drawing_y,
            stats: self
                .framebuffer
                .display_stats(drawing_x, drawing_y, width, height),
        };

        let densest = self
            .framebuffer
            .densest_window(width, height, 8)
            .map_or_else(|| "null".to_string(), framebuffer_window_json);
        let brightest = self
            .framebuffer
            .brightest_window(width, height, 8)
            .map_or_else(|| "null".to_string(), framebuffer_window_json);

        format!(
            "{{\"width\":{},\"height\":{},\"display\":{},\"drawing_area\":{},\"densest\":{},\"brightest\":{},\"candidates\":[{}]}}",
            width,
            height,
            framebuffer_window_json(display),
            framebuffer_window_json(drawing),
            densest,
            brightest,
            self.display_candidate_windows_json(width, height)
        )
    }

    fn display_candidate_windows_json(&self, width: usize, height: usize) -> String {
        self.display_candidate_windows(width, height)
            .into_iter()
            .map(|(label, window)| {
                format!(
                    "{{\"label\":\"{}\",\"window\":{},\"score\":{}}}",
                    label,
                    framebuffer_window_json(window),
                    screen_observation_score(window.stats)
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    fn display_candidate_windows(
        &self,
        width: usize,
        height: usize,
    ) -> Vec<(&'static str, FrameBufferWindow)> {
        let mut candidates = Vec::new();
        let (display_x, display_y) = display_area_start_xy(self.display_area_start);
        candidates.push(("current_display", display_x, display_y));

        let (drawing_x, drawing_y) = drawing_area_xy(self.drawing_area_top_left);
        if drawing_x >= 0 && drawing_y >= 0 {
            candidates.push(("drawing_area", drawing_x as usize, drawing_y as usize));
        }

        candidates.extend([
            ("page_0_0", 0, 0),
            ("page_0_240", 0, 240),
            ("page_0_480", 0, 480),
            ("page_0_512", 0, PSX_VRAM_HEIGHT),
            ("page_320_0", 320, 0),
            ("page_320_240", 320, 240),
            ("page_512_0", 512, 0),
            ("page_512_240", 512, 240),
            ("page_512_480", 512, 480),
            ("page_512_512", 512, PSX_VRAM_HEIGHT),
            ("page_640_0", 640, 0),
            ("page_640_240", 640, 240),
        ]);

        for trace in &self.display_area_history {
            candidates.push(("display_area_history", trace.x, trace.y));
        }

        let mut unique = Vec::new();
        for (label, x, y) in candidates {
            if x >= VRAM_WIDTH || y >= VRAM_HEIGHT {
                continue;
            }
            if x.saturating_add(width) > VRAM_WIDTH || y.saturating_add(height) > VRAM_HEIGHT {
                continue;
            }
            if unique
                .iter()
                .any(|(_, existing_x, existing_y)| *existing_x == x && *existing_y == y)
            {
                continue;
            }
            unique.push((label, x, y));
        }

        let mut windows = unique
            .into_iter()
            .map(|(label, x, y)| {
                (
                    label,
                    FrameBufferWindow {
                        x,
                        y,
                        stats: self.framebuffer.display_stats(x, y, width, height),
                    },
                )
            })
            .collect::<Vec<_>>();

        for (label, candidate) in [
            ("densest", self.framebuffer.densest_window(width, height, 8)),
            (
                "brightest",
                self.framebuffer.brightest_window(width, height, 8),
            ),
        ] {
            let Some(window) = candidate else {
                continue;
            };
            if windows
                .iter()
                .any(|(_, existing)| existing.x == window.x && existing.y == window.y)
            {
                continue;
            }
            windows.push((label, window));
        }

        windows
    }

    pub fn display_candidate_pngs(&self) -> Vec<NativeGpuDisplayCandidate> {
        let (width, height) = self.display_dimensions();
        let current = self.current_display_window();
        let current_score = screen_observation_score(current.stats);
        self.display_candidate_windows(width, height)
            .into_iter()
            .map(|(label, window)| {
                let rejection_reason =
                    self.display_candidate_resolution_reason(current, current_score, window);
                let live_draw_overlap_area = self.display_candidate_live_draw_overlap_area(window);
                let minimum_live_draw_overlap_area = self.minimum_live_draw_overlap_area();
                let scene_upload_overlap_area =
                    self.display_candidate_scene_upload_overlap_area(window);
                let minimum_scene_upload_overlap_area = self.minimum_scene_upload_overlap_area();
                NativeGpuDisplayCandidate {
                    label,
                    x: window.x,
                    y: window.y,
                    width,
                    height,
                    score: screen_observation_score(window.stats),
                    stats: window.stats,
                    texture_atlas: self.display_window_is_texture_atlas(window),
                    live_draw_overlap_area,
                    minimum_live_draw_overlap_area,
                    live_draw_overlap: live_draw_overlap_area >= minimum_live_draw_overlap_area,
                    scene_upload_overlap_area,
                    minimum_scene_upload_overlap_area,
                    scene_upload_overlap: scene_upload_overlap_area
                        >= minimum_scene_upload_overlap_area,
                    scene_signal: resolved_display_candidate_has_scene_signal(
                        current,
                        current_score,
                        window,
                    ),
                    valid_for_display_resolve: rejection_reason == "valid",
                    rejection_reason,
                    png: self.framebuffer.png(window.x, window.y, width, height),
                }
            })
            .collect()
    }

    fn resolved_display_json(&self) -> String {
        let resolved = self.display_resolve();
        format!(
            "{{\"source\":\"{}\",\"promoted\":{},\"window\":{},\"score\":{}}}",
            resolved.source,
            resolved.promoted,
            framebuffer_window_json(resolved.window),
            screen_observation_score(resolved.window.stats)
        )
    }

    fn best_observation_window_json(&self) -> String {
        self.best_observation_window
            .map_or_else(|| "null".to_string(), framebuffer_window_json)
    }

    fn presented_frame_window_json(&self) -> String {
        self.presented_frame_window
            .map_or_else(|| "null".to_string(), framebuffer_window_json)
    }

    fn capture_best_observation(&mut self) {
        let (width, height) = self.display_dimensions();
        let candidate = self.screenshot_window();
        if self.display_window_is_texture_atlas_with_dimensions(candidate, width, height) {
            return;
        }
        if !screen_observation_worth_saving(candidate.stats) {
            return;
        }

        let best_score = self
            .best_observation_window
            .map_or(0, |window| screen_observation_score(window.stats));
        let candidate_score = screen_observation_score(candidate.stats);
        if candidate_score <= best_score {
            return;
        }

        self.best_observation_png = None;
        self.best_observation_rgb =
            Some(
                self.framebuffer
                    .rgb_window(candidate.x, candidate.y, width, height),
            );
        self.best_observation_window = Some(candidate);
        self.best_observation_width = width;
        self.best_observation_height = height;
    }

    fn capture_best_observation_after_gp0(&mut self, command: u8) {
        if !self.should_probe_best_observation_after_gp0(command) {
            return;
        }

        self.best_observation_last_probe_command = self.commands_seen;
        self.best_observation_last_probe_draw_sequence = self.draw_sequence;
        self.capture_best_observation();
    }

    fn should_probe_best_observation_after_gp0(&self, command: u8) -> bool {
        if !gp0_command_may_update_framebuffer(command) {
            return false;
        }
        if self.commands_seen <= GP0_BEST_OBSERVATION_EAGER_COMMANDS {
            return true;
        }
        if self.best_observation_last_probe_command == 0 {
            return true;
        }
        if self
            .commands_seen
            .saturating_sub(self.best_observation_last_probe_command)
            >= GP0_BEST_OBSERVATION_COMMAND_INTERVAL
        {
            return true;
        }
        self.draw_sequence > self.best_observation_last_probe_draw_sequence
            && self
                .draw_sequence
                .saturating_sub(self.best_observation_last_probe_draw_sequence)
                >= GP0_BEST_OBSERVATION_DRAW_INTERVAL
    }

    fn display_window_is_texture_atlas(&self, window: FrameBufferWindow) -> bool {
        let (width, height) = self.display_dimensions();
        self.display_window_is_texture_atlas_with_dimensions(window, width, height)
    }

    fn display_window_is_texture_atlas_with_dimensions(
        &self,
        window: FrameBufferWindow,
        width: usize,
        height: usize,
    ) -> bool {
        if self.image_upload_rects.is_empty() {
            return false;
        }

        let (upload_overlap, upload_overlap_rects, _) =
            self.texture_upload_overlap_summary(window, width, height);
        if upload_overlap <= 0 {
            return false;
        }

        let upload_overlap = upload_overlap as u64;
        let pixel_count = (width as u64).saturating_mul(height as u64).max(1);
        let nonzero_pixels = window.stats.nonzero_pixels.max(1);
        let mostly_full_bright = window.stats.nonzero_pixels >= pixel_count.saturating_mul(9) / 10
            && window.stats.bright_pixels >= pixel_count.saturating_mul(9) / 10;

        let moderate_upload_overlap =
            upload_overlap.saturating_mul(10) < pixel_count.saturating_mul(3);
        let bounded_multi_upload_overlap =
            upload_overlap_rects >= 2 && upload_overlap.saturating_mul(2) < pixel_count;
        let live_draw_overlap = self.display_candidate_has_live_draw_overlap(window);
        let field_height = self.display_dimensions().1;
        let recently_presented_field_pair = height >= field_height.saturating_mul(2)
            && self.recent_display_area_history_has_field_pair(
                window.x,
                window.y,
                window.y.saturating_add(field_height),
            );
        let likely_streamed_texture_page = upload_overlap_rects >= 8
            && !live_draw_overlap
            && !recently_presented_field_pair
            && is_likely_texture_page_candidate(window.x, window.y, width, height);
        let recently_presented_display = self
            .recent_display_area_history_contains(window.x, window.y)
            || (height > field_height
                && self.recent_display_area_history_contains(
                    window.x,
                    window.y.saturating_add(field_height),
                ))
            || recently_presented_field_pair;
        if recently_presented_display
            && has_native_full_scene_detail(window.stats)
            && !mostly_full_bright
            && !likely_streamed_texture_page
        {
            return false;
        }
        if has_native_playfield_density(window.stats)
            && is_detailed_observation(window.stats)
            && !mostly_full_bright
            && !likely_streamed_texture_page
            && (moderate_upload_overlap || (bounded_multi_upload_overlap && live_draw_overlap))
        {
            return false;
        }

        upload_overlap >= pixel_count / 8 || upload_overlap.saturating_mul(2) >= nonzero_pixels
    }

    fn texture_upload_overlap_area(
        &self,
        window: FrameBufferWindow,
        width: usize,
        height: usize,
    ) -> i64 {
        self.texture_upload_overlap_summary(window, width, height).0
    }

    fn texture_upload_overlap_summary(
        &self,
        window: FrameBufferWindow,
        width: usize,
        height: usize,
    ) -> (i64, usize, i64) {
        let Some(right) = window
            .x
            .checked_add(width)
            .and_then(|value| value.checked_sub(1))
        else {
            return (0, 0, 0);
        };
        let Some(bottom) = window
            .y
            .checked_add(height)
            .and_then(|value| value.checked_sub(1))
        else {
            return (0, 0, 0);
        };
        let bounds = DrawBounds {
            left: window.x as i32,
            top: window.y as i32,
            right: right as i32,
            bottom: bottom as i32,
        };

        self.image_upload_rects
            .iter()
            .map(|upload| upload.intersection_area(bounds))
            .filter(|overlap| *overlap > 0)
            .fold((0_i64, 0_usize, 0_i64), |(area, count, max), overlap| {
                (
                    area.saturating_add(overlap),
                    count.saturating_add(1),
                    max.max(overlap),
                )
            })
    }

    fn capture_presented_frame_before_clear(&mut self, x: i32, y: i32, width: i32, height: i32) {
        if !self.is_display_clear_rect(x, y, width, height) {
            return;
        }

        self.capture_current_presented_frame();
    }

    fn capture_presented_frame_after_display_area_change(&mut self) {
        self.capture_current_presented_frame();
    }

    fn capture_current_presented_frame(&mut self) {
        self.capture_current_field_composed_display();

        let (display_width, display_height) = self.display_dimensions();
        let (start_x, start_y) = display_area_start_xy(self.display_area_start);
        let stats =
            self.framebuffer
                .psx_display_stats(start_x, start_y, display_width, display_height);
        if !screen_observation_worth_saving(stats) {
            return;
        }

        self.presentation_captures = self.presentation_captures.saturating_add(1);
        let candidate = FrameBufferWindow {
            x: start_x,
            y: start_y,
            stats,
        };
        if self.display_window_is_texture_atlas_with_dimensions(
            candidate,
            display_width,
            display_height,
        ) {
            return;
        }
        if self.presented_frame_window.is_some() && !presented_frame_has_live_scene(candidate.stats)
        {
            return;
        }
        if self.presented_frame_window.is_some_and(|current| {
            current.x == candidate.x
                && current.y == candidate.y
                && current.stats.checksum == candidate.stats.checksum
        }) {
            self.presented_frame_capture_index = self.presentation_captures;
            return;
        }

        self.presented_frame_png = None;
        self.presented_frame_rgb = Some(self.framebuffer.psx_display_rgb_window(
            start_x,
            start_y,
            display_width,
            display_height,
        ));
        self.presented_frame_window = Some(candidate);
        self.presented_frame_width = display_width;
        self.presented_frame_height = display_height;
        self.presented_frame_capture_index = self.presentation_captures;
    }

    fn capture_current_field_composed_display(&mut self) {
        let (width, field_height) = self.display_dimensions();
        let Some(output) = self.interlaced_field_pair_output_window(width, field_height) else {
            return;
        };
        if output.cached || !has_native_full_scene_detail(output.window.stats) {
            return;
        }
        if self.display_window_is_texture_atlas_with_dimensions(
            output.window,
            output.width,
            output.height,
        ) {
            return;
        }
        if self.field_composed_display_window.is_some_and(|current| {
            current.x == output.window.x
                && current.y == output.window.y
                && current.stats.checksum == output.window.stats.checksum
        }) {
            self.field_composed_display_capture_index = self.presentation_captures;
            return;
        }

        self.field_composed_display_png = None;
        self.field_composed_display_rgb = Some(self.framebuffer.psx_display_rgb_window(
            output.window.x,
            output.window.y,
            output.width,
            output.height,
        ));
        self.field_composed_display_window = Some(output.window);
        self.field_composed_display_width = output.width;
        self.field_composed_display_height = output.height;
        self.field_composed_display_capture_index = self.presentation_captures;
    }

    fn is_display_clear_rect(&self, x: i32, y: i32, width: i32, height: i32) -> bool {
        let (display_width, display_height) = self.display_dimensions();
        let (start_x, start_y) = display_area_start_xy(self.display_area_start);
        let clear_left = x;
        let clear_top = y;
        let clear_right = x.saturating_add(width);
        let clear_bottom = y.saturating_add(height);
        let display_left = start_x as i32;
        let display_top = start_y as i32;
        let display_right = display_left.saturating_add(display_width as i32);
        let display_bottom = display_top.saturating_add(display_height as i32);
        let overlap_width = clear_right.min(display_right) - clear_left.max(display_left);
        let overlap_height = clear_bottom.min(display_bottom) - clear_top.max(display_top);
        if overlap_width <= 0 || overlap_height <= 0 {
            return false;
        }

        let overlap_area = (overlap_width as i64).saturating_mul(overlap_height as i64);
        let display_area = (display_width as i64).saturating_mul(display_height as i64);
        overlap_area.saturating_mul(100) >= display_area.saturating_mul(80)
    }

    fn native_playability_json(&self) -> String {
        let (register_width, register_height) = self.display_dimensions();
        let resolved = self.display_resolve();
        let output = self.current_display_output_window();
        let field_composed_output_used = self.should_use_field_composed_output(output);
        let use_output =
            field_composed_output_used || (!output.field_composed && !resolved.promoted);
        let actual = if use_output {
            output.window
        } else {
            resolved.window
        };
        let raw_actual = if use_output {
            output.window
        } else {
            self.current_display_window()
        };
        let live_actual = if use_output {
            output.window
        } else {
            self.live_actual_display_window(resolved)
        };
        let visible = if use_output {
            output.window
        } else {
            self.visible_display_window()
        };
        let observation = self.screenshot_window();
        let vram = self.vram_stats();
        let width = if use_output {
            output.width
        } else {
            register_width
        };
        let height = if use_output {
            output.height
        } else {
            register_height
        };
        let actual_score = screen_observation_score(actual.stats);
        let live_actual_score = screen_observation_score(live_actual.stats);
        let visible_score = screen_observation_score(visible.stats);
        let observation_score = screen_observation_score(observation.stats);
        let current_is_texture_atlas =
            self.display_window_is_texture_atlas_with_dimensions(raw_actual, width, height);
        let best_is_texture_atlas = self
            .best_observation_window
            .is_some_and(|best| self.display_window_is_texture_atlas(best));
        let presented_is_texture_atlas = self
            .presented_frame_window
            .is_some_and(|presented| self.display_window_is_texture_atlas(presented));
        let best_has_live_draw_overlap = self
            .best_observation_window
            .is_some_and(|best| self.display_candidate_has_live_draw_overlap(best));
        let presented_has_live_draw_overlap = self
            .presented_frame_window
            .is_some_and(|presented| self.display_candidate_has_live_draw_overlap(presented));
        let resolve_current = self.current_display_window();
        let should_resolve_from_candidates =
            self.should_resolve_display_from_candidates(resolve_current);
        let has_resolvable_cached_display_candidate =
            self.has_resolvable_cached_display_candidate(resolve_current);
        let has_resolvable_framebuffer_candidate =
            self.has_resolvable_framebuffer_candidate(resolve_current);
        let resolved_is_live_actual = self.resolved_display_is_live_actual(resolved);
        let has_actual_scene_density = has_native_playfield_density(live_actual.stats);
        let has_actual_scene_detail = is_detailed_observation(live_actual.stats);
        let has_actual_full_scene_detail = has_native_full_scene_detail(live_actual.stats);
        let has_actual_gameplay_profile = has_native_gameplay_display_profile(live_actual.stats);
        let actual_color_stats =
            self.display_window_color_stats(live_actual, width, height, use_output);
        let has_actual_color_diversity = actual_color_stats.has_scene_color_diversity();
        let has_actual_intro_caption_band = actual_color_stats.has_intro_caption_band();
        let has_actual_playfield = if use_output {
            has_actual_gameplay_profile && !current_is_texture_atlas
        } else {
            self.has_live_actual_playfield(raw_actual, resolved)
        };
        let has_scene_density = has_native_playfield_density(visible.stats);
        let has_scene_detail = is_detailed_observation(visible.stats);
        let has_full_scene_detail = has_native_full_scene_detail(visible.stats);
        let has_scene_gameplay_profile = has_native_gameplay_display_profile(visible.stats);
        let scene_color_stats = self.display_window_color_stats(visible, width, height, use_output);
        let has_scene_color_diversity = scene_color_stats.has_scene_color_diversity();
        let has_scene_intro_caption_band = scene_color_stats.has_intro_caption_band();
        let has_playfield_draws = self.has_playfield_draws();
        let has_textured_content = self.textured_draw_stats.written_pixels
            >= DISPLAY_RESOLVE_MIN_TEXTURED_WRITTEN_PIXELS
            && self.textured_draw_stats.color_changes >= 16;
        let caption_band_ui_allowed = self.caption_band_playfield_ui_allowed()
            && has_actual_playfield
            && has_scene_detail
            && has_full_scene_detail;
        let actual_caption_blocks_playability =
            has_actual_intro_caption_band && !caption_band_ui_allowed;
        let scene_caption_blocks_playability =
            has_scene_intro_caption_band && !caption_band_ui_allowed;
        let has_live_presentation = self.has_visible_presentation();
        let playable_candidate = has_actual_playfield
            && has_scene_density
            && has_scene_detail
            && has_actual_full_scene_detail
            && has_full_scene_detail
            && has_actual_gameplay_profile
            && has_scene_gameplay_profile
            && has_actual_color_diversity
            && has_scene_color_diversity
            && !actual_caption_blocks_playability
            && !scene_caption_blocks_playability
            && has_playfield_draws
            && has_textured_content
            && has_live_presentation;
        let classification = if playable_candidate {
            "native_playable_candidate"
        } else if self.commands_seen == 0 {
            "no_gpu_commands"
        } else if self.image_upload_commands == 0 {
            "waiting_for_texture_uploads"
        } else if self.textured_draw_stats.written_pixels == 0 {
            "no_textured_pixels_written"
        } else if !has_playfield_draws {
            "hud_or_offscreen_draws_only"
        } else if resolved.promoted && !resolved_is_live_actual {
            "candidate_not_live_actual"
        } else if !has_actual_scene_density {
            "actual_display_too_sparse"
        } else if !has_actual_scene_detail {
            "actual_display_low_detail"
        } else if !has_actual_full_scene_detail {
            "actual_display_low_scene_complexity"
        } else if !has_actual_gameplay_profile {
            "actual_display_not_gameplay_profile"
        } else if !has_actual_color_diversity {
            "actual_display_low_color_diversity"
        } else if actual_caption_blocks_playability {
            "actual_display_intro_caption_band"
        } else if !has_scene_density {
            "display_too_sparse"
        } else if !has_scene_detail {
            "display_low_detail"
        } else if !has_full_scene_detail {
            "display_low_scene_complexity"
        } else if !has_scene_gameplay_profile {
            "display_not_gameplay_profile"
        } else if !has_scene_color_diversity {
            "display_low_color_diversity"
        } else if scene_caption_blocks_playability {
            "display_intro_caption_band"
        } else if !has_live_presentation {
            "candidate_not_presented"
        } else {
            "not_playable_candidate"
        };

        format!(
            "{{\"playable_candidate\":{},\"classification\":\"{}\",\"display_width\":{},\"display_height\":{},\"actual_display_source\":\"{}\",\"actual_display_promoted\":{},\"actual_display_is_live\":{},\"actual_display_field_composed\":{},\"actual_display_cached\":{},\"actual_display\":{},\"raw_actual_display\":{},\"live_actual_display\":{},\"resolved_display\":{},\"visible_display\":{},\"observation\":{},\"vram\":{},\"actual_score\":{},\"live_actual_score\":{},\"visible_score\":{},\"observation_score\":{},\"current_is_texture_atlas\":{},\"best_is_texture_atlas\":{},\"presented_is_texture_atlas\":{},\"best_has_live_draw_overlap\":{},\"presented_has_live_draw_overlap\":{},\"should_resolve_from_candidates\":{},\"has_resolvable_cached_display_candidate\":{},\"has_resolvable_framebuffer_candidate\":{},\"display_resolve_gate\":{},\"display_candidate_diagnostics\":[{}],\"presented_frame_capture_index\":{},\"presented_frame_fresh\":{},\"field_composed_display_capture_index\":{},\"has_actual_scene_density\":{},\"has_actual_scene_detail\":{},\"has_actual_full_scene_detail\":{},\"has_actual_gameplay_profile\":{},\"has_actual_color_diversity\":{},\"has_actual_intro_caption_band\":{},\"has_actual_playfield\":{},\"has_scene_density\":{},\"has_scene_detail\":{},\"has_full_scene_detail\":{},\"has_scene_gameplay_profile\":{},\"has_scene_color_diversity\":{},\"has_scene_intro_caption_band\":{},\"caption_band_ui_allowed\":{},\"actual_caption_blocks_playability\":{},\"scene_caption_blocks_playability\":{},\"has_playfield_draws\":{},\"has_textured_content\":{},\"has_live_presentation\":{},\"actual_color\":{},\"visible_color\":{},\"gpu_commands_seen\":{},\"gpu_draw_sequence\":{},\"image_upload_commands\":{},\"vram_copy_commands\":{},\"textured_triangle_commands\":{},\"textured_rect_commands\":{},\"textured_written_pixels\":{},\"textured_color_changes\":{},\"presentation_captures\":{}}}",
            playable_candidate,
            classification,
            width,
            height,
            if use_output {
                output.source
            } else {
                resolved.source
            },
            resolved.promoted && !use_output,
            resolved_is_live_actual || use_output,
            field_composed_output_used,
            use_output && output.cached,
            framebuffer_window_json(actual),
            framebuffer_window_json(raw_actual),
            framebuffer_window_json(live_actual),
            self.resolved_display_json(),
            framebuffer_window_json(visible),
            framebuffer_window_json(observation),
            vram.json(),
            actual_score,
            live_actual_score,
            visible_score,
            observation_score,
            current_is_texture_atlas,
            best_is_texture_atlas,
            presented_is_texture_atlas,
            best_has_live_draw_overlap,
            presented_has_live_draw_overlap,
            should_resolve_from_candidates,
            has_resolvable_cached_display_candidate,
            has_resolvable_framebuffer_candidate,
            self.display_resolve_gate_json(resolve_current),
            self.display_candidate_diagnostics_json(resolve_current),
            self.presented_frame_capture_index,
            self.presented_frame_is_fresh(),
            self.field_composed_display_capture_index,
            has_actual_scene_density,
            has_actual_scene_detail,
            has_actual_full_scene_detail,
            has_actual_gameplay_profile,
            has_actual_color_diversity,
            has_actual_intro_caption_band,
            has_actual_playfield,
            has_scene_density,
            has_scene_detail,
            has_full_scene_detail,
            has_scene_gameplay_profile,
            has_scene_color_diversity,
            has_scene_intro_caption_band,
            caption_band_ui_allowed,
            actual_caption_blocks_playability,
            scene_caption_blocks_playability,
            has_playfield_draws,
            has_textured_content,
            has_live_presentation,
            actual_color_stats.json(),
            scene_color_stats.json(),
            self.commands_seen,
            self.draw_sequence,
            self.image_upload_commands,
            self.vram_copy_commands,
            self.textured_triangle_commands,
            self.textured_rect_commands,
            self.textured_draw_stats.written_pixels,
            self.textured_draw_stats.color_changes,
            self.presentation_captures
        )
    }

    fn native_playability_compact_json(&self) -> String {
        let (register_width, register_height) = self.display_dimensions();
        let resolved = self.display_resolve();
        let output = self.current_display_output_window();
        let field_composed_output_used = self.should_use_field_composed_output(output);
        let use_output =
            field_composed_output_used || (!output.field_composed && !resolved.promoted);
        let actual = if use_output {
            output.window
        } else {
            resolved.window
        };
        let raw_actual = if use_output {
            output.window
        } else {
            self.current_display_window()
        };
        let live_actual = if use_output {
            output.window
        } else {
            self.live_actual_display_window(resolved)
        };
        let visible = if use_output {
            output.window
        } else {
            self.visible_display_window()
        };
        let width = if use_output {
            output.width
        } else {
            register_width
        };
        let height = if use_output {
            output.height
        } else {
            register_height
        };
        let current_is_texture_atlas =
            self.display_window_is_texture_atlas_with_dimensions(raw_actual, width, height);
        let resolved_is_live_actual = self.resolved_display_is_live_actual(resolved);
        let has_actual_scene_density = has_native_playfield_density(live_actual.stats);
        let has_actual_scene_detail = is_detailed_observation(live_actual.stats);
        let has_actual_full_scene_detail = has_native_full_scene_detail(live_actual.stats);
        let has_actual_gameplay_profile = has_native_gameplay_display_profile(live_actual.stats);
        let actual_color_stats =
            self.display_window_color_stats(live_actual, width, height, use_output);
        let has_actual_color_diversity = actual_color_stats.has_scene_color_diversity();
        let has_actual_intro_caption_band = actual_color_stats.has_intro_caption_band();
        let has_actual_playfield = if use_output {
            has_actual_gameplay_profile && !current_is_texture_atlas
        } else {
            self.has_live_actual_playfield(raw_actual, resolved)
        };
        let has_scene_density = has_native_playfield_density(visible.stats);
        let has_scene_detail = is_detailed_observation(visible.stats);
        let has_full_scene_detail = has_native_full_scene_detail(visible.stats);
        let has_scene_gameplay_profile = has_native_gameplay_display_profile(visible.stats);
        let scene_color_stats = self.display_window_color_stats(visible, width, height, use_output);
        let has_scene_color_diversity = scene_color_stats.has_scene_color_diversity();
        let has_scene_intro_caption_band = scene_color_stats.has_intro_caption_band();
        let has_playfield_draws = self.has_playfield_draws();
        let has_textured_content = self.textured_draw_stats.written_pixels
            >= DISPLAY_RESOLVE_MIN_TEXTURED_WRITTEN_PIXELS
            && self.textured_draw_stats.color_changes >= 16;
        let caption_band_ui_allowed = self.caption_band_playfield_ui_allowed()
            && has_actual_playfield
            && has_scene_detail
            && has_full_scene_detail;
        let actual_caption_blocks_playability =
            has_actual_intro_caption_band && !caption_band_ui_allowed;
        let scene_caption_blocks_playability =
            has_scene_intro_caption_band && !caption_band_ui_allowed;
        let has_live_presentation = self.has_visible_presentation();
        let playable_candidate = has_actual_playfield
            && has_scene_density
            && has_scene_detail
            && has_actual_full_scene_detail
            && has_full_scene_detail
            && has_actual_gameplay_profile
            && has_scene_gameplay_profile
            && has_actual_color_diversity
            && has_scene_color_diversity
            && !actual_caption_blocks_playability
            && !scene_caption_blocks_playability
            && has_playfield_draws
            && has_textured_content
            && has_live_presentation;
        let classification = if playable_candidate {
            "native_playable_candidate"
        } else if self.commands_seen == 0 {
            "no_gpu_commands"
        } else if self.image_upload_commands == 0 {
            "waiting_for_texture_uploads"
        } else if self.textured_draw_stats.written_pixels == 0 {
            "no_textured_pixels_written"
        } else if !has_playfield_draws {
            "hud_or_offscreen_draws_only"
        } else if resolved.promoted && !resolved_is_live_actual {
            "candidate_not_live_actual"
        } else if !has_actual_scene_density {
            "actual_display_too_sparse"
        } else if !has_actual_scene_detail {
            "actual_display_low_detail"
        } else if !has_actual_full_scene_detail {
            "actual_display_low_scene_complexity"
        } else if !has_actual_gameplay_profile {
            "actual_display_not_gameplay_profile"
        } else if !has_actual_color_diversity {
            "actual_display_low_color_diversity"
        } else if actual_caption_blocks_playability {
            "actual_display_intro_caption_band"
        } else if !has_scene_density {
            "display_too_sparse"
        } else if !has_scene_detail {
            "display_low_detail"
        } else if !has_full_scene_detail {
            "display_low_scene_complexity"
        } else if !has_scene_gameplay_profile {
            "display_not_gameplay_profile"
        } else if !has_scene_color_diversity {
            "display_low_color_diversity"
        } else if scene_caption_blocks_playability {
            "display_intro_caption_band"
        } else if !has_live_presentation {
            "candidate_not_presented"
        } else {
            "not_playable_candidate"
        };

        format!(
            "{{\"playable_candidate\":{},\"classification\":\"{}\",\"display_width\":{},\"display_height\":{},\"source\":\"{}\",\"field_composed\":{},\"cached\":{},\"promoted\":{},\"actual\":{},\"visible\":{},\"actual_color\":{},\"visible_color\":{},\"actual_score\":{},\"visible_score\":{},\"has_actual_scene_density\":{},\"has_actual_scene_detail\":{},\"has_actual_full_scene_detail\":{},\"has_actual_gameplay_profile\":{},\"has_actual_color_diversity\":{},\"has_actual_intro_caption_band\":{},\"has_actual_playfield\":{},\"has_scene_density\":{},\"has_scene_detail\":{},\"has_full_scene_detail\":{},\"has_scene_gameplay_profile\":{},\"has_scene_color_diversity\":{},\"has_scene_intro_caption_band\":{},\"caption_band_ui_allowed\":{},\"actual_caption_blocks_playability\":{},\"scene_caption_blocks_playability\":{},\"has_playfield_draws\":{},\"has_textured_content\":{},\"has_live_presentation\":{},\"image_upload_commands\":{},\"textured_triangle_commands\":{},\"textured_written_pixels\":{},\"textured_color_changes\":{},\"presentation_captures\":{}}}",
            playable_candidate,
            classification,
            width,
            height,
            if use_output {
                output.source
            } else {
                resolved.source
            },
            field_composed_output_used,
            use_output && output.cached,
            resolved.promoted && !use_output,
            framebuffer_window_json(actual),
            framebuffer_window_json(visible),
            actual_color_stats.json(),
            scene_color_stats.json(),
            screen_observation_score(actual.stats),
            screen_observation_score(visible.stats),
            has_actual_scene_density,
            has_actual_scene_detail,
            has_actual_full_scene_detail,
            has_actual_gameplay_profile,
            has_actual_color_diversity,
            has_actual_intro_caption_band,
            has_actual_playfield,
            has_scene_density,
            has_scene_detail,
            has_full_scene_detail,
            has_scene_gameplay_profile,
            has_scene_color_diversity,
            has_scene_intro_caption_band,
            caption_band_ui_allowed,
            actual_caption_blocks_playability,
            scene_caption_blocks_playability,
            has_playfield_draws,
            has_textured_content,
            has_live_presentation,
            self.image_upload_commands,
            self.textured_triangle_commands,
            self.textured_draw_stats.written_pixels,
            self.textured_draw_stats.color_changes,
            self.presentation_captures
        )
    }

    fn native_playable_candidate(&self) -> bool {
        let resolved = self.display_resolve();
        let output = self.current_display_output_window();
        let use_output = self.should_use_field_composed_output(output)
            || (!output.field_composed && !resolved.promoted);
        let raw_actual = if use_output {
            output.window
        } else {
            self.current_display_window()
        };
        let live_actual = if use_output {
            output.window
        } else {
            self.live_actual_display_window(resolved)
        };
        let visible = if use_output {
            output.window
        } else {
            self.visible_display_window()
        };
        let has_actual_full_scene_detail = has_native_full_scene_detail(live_actual.stats);
        let has_actual_gameplay_profile = has_native_gameplay_display_profile(live_actual.stats);
        let width = if use_output {
            output.width
        } else {
            self.display_dimensions().0
        };
        let height = if use_output {
            output.height
        } else {
            self.display_dimensions().1
        };
        let actual_color_stats =
            self.display_window_color_stats(live_actual, width, height, use_output);
        let has_actual_color_diversity = actual_color_stats.has_scene_color_diversity();
        let has_actual_intro_caption_band = actual_color_stats.has_intro_caption_band();
        let current_is_texture_atlas =
            self.display_window_is_texture_atlas_with_dimensions(raw_actual, width, height);
        let has_actual_playfield = if use_output {
            has_actual_gameplay_profile && !current_is_texture_atlas
        } else {
            self.has_live_actual_playfield(raw_actual, resolved)
        };
        let has_scene_density = has_native_playfield_density(visible.stats);
        let has_scene_detail = is_detailed_observation(visible.stats);
        let has_full_scene_detail = has_native_full_scene_detail(visible.stats);
        let scene_color_stats = self.display_window_color_stats(visible, width, height, use_output);
        let has_scene_color_diversity = scene_color_stats.has_scene_color_diversity();
        let has_scene_intro_caption_band = scene_color_stats.has_intro_caption_band();
        let has_playfield_draws = self.has_playfield_draws();
        let has_textured_content = self.textured_draw_stats.written_pixels
            >= DISPLAY_RESOLVE_MIN_TEXTURED_WRITTEN_PIXELS
            && self.textured_draw_stats.color_changes >= 16;
        let caption_band_ui_allowed = self.caption_band_playfield_ui_allowed()
            && has_actual_playfield
            && has_scene_detail
            && has_full_scene_detail;
        let actual_caption_blocks_playability =
            has_actual_intro_caption_band && !caption_band_ui_allowed;
        let scene_caption_blocks_playability =
            has_scene_intro_caption_band && !caption_band_ui_allowed;
        let has_live_presentation = self.has_visible_presentation();

        has_actual_playfield
            && has_native_playfield_density(live_actual.stats)
            && is_detailed_observation(live_actual.stats)
            && has_actual_full_scene_detail
            && has_actual_gameplay_profile
            && has_actual_color_diversity
            && has_scene_density
            && has_scene_detail
            && has_full_scene_detail
            && has_native_gameplay_display_profile(visible.stats)
            && has_scene_color_diversity
            && !actual_caption_blocks_playability
            && !scene_caption_blocks_playability
            && has_playfield_draws
            && has_textured_content
            && has_live_presentation
    }

    fn caption_band_playfield_ui_allowed(&self) -> bool {
        self.textured_triangle_commands >= DISPLAY_RESOLVE_MIN_TEXTURED_TRIANGLE_COMMANDS * 4
            && self.textured_draw_stats.written_pixels
                >= DISPLAY_RESOLVE_MIN_TEXTURED_WRITTEN_PIXELS * 16
            && self.textured_draw_stats.color_changes >= 16_384
            && self.has_playfield_draws()
            && self.has_visible_presentation()
    }

    fn display_window_color_stats(
        &self,
        window: FrameBufferWindow,
        width: usize,
        height: usize,
        psx_wrap: bool,
    ) -> DisplayColorStats {
        let width = width.max(1);
        let height = height.max(1);
        let x_step = (width / 64).max(1);
        let y_step = (height / 64).max(1);
        let mut stats = DisplayColorStats::default();

        if let Some(cached_rgb) = self.cached_rgb_for_display_color_stats(window, width, height) {
            for y in (0..height).step_by(y_step) {
                let row_start = y.saturating_mul(width);
                let bottom_band = y >= height.saturating_mul(4) / 5;
                for x in (0..width).step_by(x_step) {
                    stats.record(cached_rgb[row_start + x], bottom_band);
                }
            }
            return stats;
        }

        for y in (0..height).step_by(y_step) {
            let bottom_band = y >= height.saturating_mul(4) / 5;
            let source_y = if psx_wrap {
                (window.y + y) % PSX_VRAM_HEIGHT
            } else {
                window.y.saturating_add(y)
            };
            for x in (0..width).step_by(x_step) {
                let source_x = if psx_wrap {
                    (window.x + x) % VRAM_WIDTH
                } else {
                    window.x.saturating_add(x)
                };
                let rgb = self.framebuffer.pixel(source_x as i32, source_y as i32);
                stats.record(rgb, bottom_band);
            }
        }

        stats
    }

    fn cached_rgb_for_display_color_stats(
        &self,
        window: FrameBufferWindow,
        width: usize,
        height: usize,
    ) -> Option<&[u32]> {
        let expected_len = width.checked_mul(height)?;
        let cached_matches = |cached_window: Option<FrameBufferWindow>,
                              cached_width,
                              cached_height,
                              rgb: &Vec<u32>| {
            let Some(cached_window) = cached_window else {
                return false;
            };
            cached_window.x == window.x
                && cached_window.y == window.y
                && cached_window.stats.checksum == window.stats.checksum
                && cached_window.stats.pixel_count == window.stats.pixel_count
                && cached_width == width
                && cached_height == height
                && rgb.len() == expected_len
        };

        if let Some(rgb) = self.field_composed_display_rgb.as_ref()
            && cached_matches(
                self.field_composed_display_window,
                self.field_composed_display_width,
                self.field_composed_display_height,
                rgb,
            )
        {
            return Some(rgb);
        }
        if let Some(rgb) = self.presented_frame_rgb.as_ref()
            && cached_matches(
                self.presented_frame_window,
                self.presented_frame_width,
                self.presented_frame_height,
                rgb,
            )
        {
            return Some(rgb);
        }
        if let Some(rgb) = self.best_observation_rgb.as_ref()
            && cached_matches(
                self.best_observation_window,
                self.best_observation_width,
                self.best_observation_height,
                rgb,
            )
        {
            return Some(rgb);
        }
        None
    }

    fn live_actual_display_window(&self, resolved: DisplayResolve) -> FrameBufferWindow {
        if self.resolved_display_is_live_actual(resolved) {
            resolved.window
        } else {
            self.current_display_window()
        }
    }

    fn resolved_display_is_live_actual(&self, resolved: DisplayResolve) -> bool {
        resolved.source == "gp1_display_area"
            || (resolved.source == "presented_frame" && self.presented_frame_is_fresh())
    }

    fn should_present_resolved_display(&self, resolved: DisplayResolve) -> bool {
        self.resolved_display_is_live_actual(resolved)
            || self.display_candidate_has_live_draw_overlap(resolved.window)
    }

    fn has_live_actual_playfield(
        &self,
        raw_actual: FrameBufferWindow,
        resolved: DisplayResolve,
    ) -> bool {
        let raw_has_playfield = has_native_playfield_density(raw_actual.stats)
            && is_detailed_observation(raw_actual.stats)
            && has_native_gameplay_display_profile(raw_actual.stats);
        if raw_has_playfield && !self.display_window_is_texture_atlas(raw_actual) {
            return true;
        }

        resolved.source == "presented_frame"
            && self.presented_frame_is_fresh()
            && has_native_playfield_density(resolved.window.stats)
            && is_detailed_observation(resolved.window.stats)
            && has_native_gameplay_display_profile(resolved.window.stats)
            && self.display_candidate_has_live_draw_overlap(resolved.window)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuCommandSource {
    kind: &'static str,
    address: Option<u32>,
    pc: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DisplayAreaTrace {
    value: u32,
    x: usize,
    y: usize,
    command_index: u64,
    draw_sequence: u64,
}

impl DisplayAreaTrace {
    fn json(&self) -> String {
        format!(
            "{{\"value\":{},\"value_hex\":\"0x{:06x}\",\"x\":{},\"y\":{},\"command_index\":{},\"draw_sequence\":{}}}",
            self.value, self.value, self.x, self.y, self.command_index, self.draw_sequence
        )
    }
}

impl GpuCommandSource {
    pub fn cpu_io(address: u32, pc: Option<u32>) -> Self {
        Self {
            kind: "cpu_io",
            address: Some(address),
            pc,
        }
    }

    pub fn dma_linked_list(address: u32, pc: Option<u32>) -> Self {
        Self {
            kind: "dma_linked_list",
            address: Some(address),
            pc,
        }
    }

    pub fn dma_block(address: u32, pc: Option<u32>) -> Self {
        Self {
            kind: "dma_block",
            address: Some(address),
            pc,
        }
    }

    fn json(&self) -> String {
        format!(
            "{{\"kind\":\"{}\",\"address\":{},\"address_hex\":{},\"pc\":{},\"pc_hex\":{}}}",
            self.kind,
            optional_u32_json(self.address),
            optional_u32_hex_json(self.address),
            optional_u32_json(self.pc),
            optional_u32_hex_json(self.pc)
        )
    }
}

#[derive(Clone, Debug)]
pub struct NativeGpuDisplayCandidate {
    pub label: &'static str,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub score: u64,
    pub stats: FrameBufferStats,
    pub texture_atlas: bool,
    pub live_draw_overlap_area: i64,
    pub minimum_live_draw_overlap_area: i64,
    pub live_draw_overlap: bool,
    pub scene_upload_overlap_area: i64,
    pub minimum_scene_upload_overlap_area: i64,
    pub scene_upload_overlap: bool,
    pub scene_signal: bool,
    pub valid_for_display_resolve: bool,
    pub rejection_reason: &'static str,
    pub png: Vec<u8>,
}

impl NativeGpuDisplayCandidate {
    pub fn json(&self, output: &str) -> String {
        format!(
            "{{\"label\":\"{}\",\"output\":\"{}\",\"x\":{},\"y\":{},\"width\":{},\"height\":{},\"score\":{},\"stats\":{},\"texture_atlas\":{},\"live_draw_overlap_area\":{},\"minimum_live_draw_overlap_area\":{},\"live_draw_overlap\":{},\"scene_upload_overlap_area\":{},\"minimum_scene_upload_overlap_area\":{},\"scene_upload_overlap\":{},\"scene_signal\":{},\"valid_for_display_resolve\":{},\"rejection_reason\":\"{}\"}}",
            self.label,
            escape_json(output),
            self.x,
            self.y,
            self.width,
            self.height,
            self.score,
            self.stats.json(),
            self.texture_atlas,
            self.live_draw_overlap_area,
            self.minimum_live_draw_overlap_area,
            self.live_draw_overlap,
            self.scene_upload_overlap_area,
            self.minimum_scene_upload_overlap_area,
            self.scene_upload_overlap,
            self.scene_signal,
            self.valid_for_display_resolve,
            self.rejection_reason
        )
    }
}

#[derive(Clone, Debug)]
pub struct NativeGpuDrawCapture {
    pub sequence: u64,
    pub trace_json: String,
    pub display_x: usize,
    pub display_y: usize,
    pub display_width: usize,
    pub display_height: usize,
    pub display_png: Vec<u8>,
    pub bounds_x: usize,
    pub bounds_y: usize,
    pub bounds_width: usize,
    pub bounds_height: usize,
    pub bounds_png: Vec<u8>,
    pub texture_png: Option<Vec<u8>>,
    pub palette_png: Option<Vec<u8>>,
}

impl NativeGpuDrawCapture {
    pub fn json(
        &self,
        display_output: &str,
        bounds_output: &str,
        texture_output: Option<&str>,
        palette_output: Option<&str>,
    ) -> String {
        format!(
            "{{\"sequence\":{},\"display_output\":\"{}\",\"bounds_output\":\"{}\",\"texture_output\":{},\"palette_output\":{},\"display_window\":{{\"x\":{},\"y\":{},\"width\":{},\"height\":{}}},\"bounds_window\":{{\"x\":{},\"y\":{},\"width\":{},\"height\":{}}},\"trace\":{}}}",
            self.sequence,
            escape_json(display_output),
            escape_json(bounds_output),
            optional_str_json(texture_output),
            optional_str_json(palette_output),
            self.display_x,
            self.display_y,
            self.display_width,
            self.display_height,
            self.bounds_x,
            self.bounds_y,
            self.bounds_width,
            self.bounds_height,
            self.trace_json
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DisplayResolve {
    source: &'static str,
    promoted: bool,
    window: FrameBufferWindow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DisplayOutputWindow {
    source: &'static str,
    field_composed: bool,
    cached: bool,
    width: usize,
    height: usize,
    window: FrameBufferWindow,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DisplayColorStats {
    samples: u64,
    bottom_band_samples: u64,
    bottom_caption_samples: u64,
    bottom_dark_samples: u64,
    red_sum: u64,
    green_sum: u64,
    blue_sum: u64,
    red_min: u8,
    red_max: u8,
    green_min: u8,
    green_max: u8,
    blue_min: u8,
    blue_max: u8,
    bucket_count: u16,
    buckets: [u64; 8],
}

impl DisplayColorStats {
    fn record(&mut self, rgb: u32, bottom_band: bool) {
        let red = ((rgb >> 16) & 0xff) as u8;
        let green = ((rgb >> 8) & 0xff) as u8;
        let blue = (rgb & 0xff) as u8;
        if self.samples == 0 {
            self.red_min = red;
            self.red_max = red;
            self.green_min = green;
            self.green_max = green;
            self.blue_min = blue;
            self.blue_max = blue;
        } else {
            self.red_min = self.red_min.min(red);
            self.red_max = self.red_max.max(red);
            self.green_min = self.green_min.min(green);
            self.green_max = self.green_max.max(green);
            self.blue_min = self.blue_min.min(blue);
            self.blue_max = self.blue_max.max(blue);
        }

        self.samples = self.samples.saturating_add(1);
        self.red_sum = self.red_sum.saturating_add(u64::from(red));
        self.green_sum = self.green_sum.saturating_add(u64::from(green));
        self.blue_sum = self.blue_sum.saturating_add(u64::from(blue));
        if bottom_band {
            self.bottom_band_samples = self.bottom_band_samples.saturating_add(1);
            let max_channel = red.max(green).max(blue);
            let min_channel = red.min(green).min(blue);
            let luma = (u32::from(red) * 30 + u32::from(green) * 59 + u32::from(blue) * 11) / 100;
            if luma < 40 {
                self.bottom_dark_samples = self.bottom_dark_samples.saturating_add(1);
            } else if luma > 156 && max_channel.saturating_sub(min_channel) <= 96 {
                self.bottom_caption_samples = self.bottom_caption_samples.saturating_add(1);
            }
        }

        let bucket =
            (((red >> 5) as usize) << 6) | (((green >> 5) as usize) << 3) | ((blue >> 5) as usize);
        let word = bucket / 64;
        let bit = bucket % 64;
        let mask = 1_u64 << bit;
        if self.buckets[word] & mask == 0 {
            self.buckets[word] |= mask;
            self.bucket_count = self.bucket_count.saturating_add(1);
        }
    }

    fn has_scene_color_diversity(self) -> bool {
        if self.samples < 16 || self.bucket_count < 6 {
            return false;
        }

        let red_range = self.red_max.saturating_sub(self.red_min);
        let green_range = self.green_max.saturating_sub(self.green_min);
        let blue_range = self.blue_max.saturating_sub(self.blue_min);
        let varied_channels = [red_range, green_range, blue_range]
            .into_iter()
            .filter(|range| *range >= 32)
            .count();
        if varied_channels < 2 {
            return false;
        }

        let strongest = self.red_sum.max(self.green_sum).max(self.blue_sum);
        let weakest = self.red_sum.min(self.green_sum).min(self.blue_sum);
        let middle = self
            .red_sum
            .saturating_add(self.green_sum)
            .saturating_add(self.blue_sum)
            .saturating_sub(strongest)
            .saturating_sub(weakest);
        strongest
            <= middle
                .saturating_mul(DISPLAY_SCENE_MAX_CHANNEL_DOMINANCE)
                .max(1)
    }

    fn has_intro_caption_band(self) -> bool {
        if self.samples == 0 || self.bottom_band_samples == 0 {
            return false;
        }

        let has_caption_pixels =
            self.bottom_caption_samples.saturating_mul(1_000) >= self.samples.saturating_mul(3);
        let dark_subtitle_band = self.bottom_dark_samples.saturating_mul(100)
            >= self.bottom_band_samples.saturating_mul(30)
            && self.bottom_caption_samples.saturating_mul(100)
                <= self.bottom_band_samples.saturating_mul(35);
        let bright_caption_panel = self.bottom_caption_samples.saturating_mul(100)
            >= self.bottom_band_samples.saturating_mul(70)
            && self.bottom_dark_samples.saturating_mul(100)
                >= self.bottom_band_samples.saturating_mul(4);

        has_caption_pixels && (dark_subtitle_band || bright_caption_panel)
    }

    fn json(self) -> String {
        let red_range = self.red_max.saturating_sub(self.red_min);
        let green_range = self.green_max.saturating_sub(self.green_min);
        let blue_range = self.blue_max.saturating_sub(self.blue_min);
        let strongest = self.red_sum.max(self.green_sum).max(self.blue_sum);
        let weakest = self.red_sum.min(self.green_sum).min(self.blue_sum);
        let middle = self
            .red_sum
            .saturating_add(self.green_sum)
            .saturating_add(self.blue_sum)
            .saturating_sub(strongest)
            .saturating_sub(weakest);
        format!(
            "{{\"samples\":{},\"bottom_band_samples\":{},\"bottom_caption_samples\":{},\"bottom_dark_samples\":{},\"intro_caption_band\":{},\"bucket_count\":{},\"red_range\":{},\"green_range\":{},\"blue_range\":{},\"red_sum\":{},\"green_sum\":{},\"blue_sum\":{},\"strongest_sum\":{},\"middle_sum\":{},\"weakest_sum\":{},\"scene_color_diversity\":{}}}",
            self.samples,
            self.bottom_band_samples,
            self.bottom_caption_samples,
            self.bottom_dark_samples,
            self.has_intro_caption_band(),
            self.bucket_count,
            red_range,
            green_range,
            blue_range,
            self.red_sum,
            self.green_sum,
            self.blue_sum,
            strongest,
            middle,
            weakest,
            self.has_scene_color_diversity()
        )
    }
}

#[derive(Clone, Debug)]
struct GpuDrawTrace {
    sequence: u64,
    kind: &'static str,
    texture_page: Option<u16>,
    clut: Option<u16>,
    color: Option<u32>,
    bounds: DrawBounds,
    stats: TexturedDrawStats,
    words: Vec<u32>,
    points: Vec<Point>,
    source: Option<GpuCommandSource>,
}

impl GpuDrawTrace {
    fn textured(
        kind: &'static str,
        texture_page: u16,
        clut: u16,
        bounds: DrawBounds,
        stats: TexturedDrawStats,
        words: &[u32],
        points: &[Point],
        source: Option<&GpuCommandSource>,
    ) -> Self {
        Self {
            sequence: 0,
            kind,
            texture_page: Some(texture_page),
            clut: Some(clut),
            color: None,
            bounds,
            stats,
            words: words.to_vec(),
            points: points.to_vec(),
            source: source.cloned(),
        }
    }

    fn flat(
        kind: &'static str,
        color: u32,
        bounds: DrawBounds,
        words: &[u32],
        points: &[Point],
        source: Option<&GpuCommandSource>,
    ) -> Self {
        Self {
            sequence: 0,
            kind,
            texture_page: None,
            clut: None,
            color: Some(color & 0x00ff_ffff),
            bounds,
            stats: TexturedDrawStats::default(),
            words: words.to_vec(),
            points: points.to_vec(),
            source: source.cloned(),
        }
    }

    fn score(&self) -> i64 {
        if self.color == Some(0) {
            return 0;
        }
        if self.color.is_some() {
            return self.bounds.area();
        }
        let textured_pixels = self
            .stats
            .written_pixels
            .max(self.stats.drawn_pixels)
            .min(i64::MAX as u64) as i64;
        textured_pixels
            .saturating_mul(16)
            .max(self.bounds.area().min(textured_pixels))
    }

    fn is_focus_candidate(&self) -> bool {
        if self.kind != "textured_quad" && self.kind != "textured_triangle" {
            return false;
        }
        if self.stats.written_pixels == 0 {
            return false;
        }
        if self.bounds.area() > 20_000 {
            return false;
        }
        self.bounds.overlaps(DrawBounds {
            left: 64,
            top: 64,
            right: 448,
            bottom: 430,
        })
    }

    fn overlaps_playfield(&self) -> bool {
        if self.color == Some(0) && self.stats.written_pixels == 0 {
            return false;
        }
        self.bounds.overlaps(DrawBounds {
            left: 0,
            top: 96,
            right: 511,
            bottom: 430,
        }) || self.bounds.overlaps(DrawBounds {
            left: 512,
            top: 96,
            right: 1023,
            bottom: 430,
        })
    }

    fn json(&self) -> String {
        format!(
            "{{\"sequence\":{},\"kind\":\"{}\",\"source\":{},\"texture_page\":{},\"texture_page_hex\":{},\"clut\":{},\"clut_hex\":{},\"color\":{},\"color_hex\":{},\"bounds\":{},\"area\":{},\"score\":{},\"points\":[{}],\"words\":[{}],\"sampled_pixels\":{},\"drawn_pixels\":{},\"written_pixels\":{},\"clipped_pixels\":{},\"transparent_pixels\":{},\"first_color\":{},\"first_color_hex\":\"0x{:04x}\",\"last_color\":{},\"last_color_hex\":\"0x{:04x}\",\"color_hash\":{},\"color_hash_hex\":\"0x{:08x}\",\"color_changes\":{}}}",
            self.sequence,
            self.kind,
            optional_gpu_source_json(self.source.as_ref()),
            optional_u16_json(self.texture_page),
            optional_u16_hex_json(self.texture_page),
            optional_u16_json(self.clut),
            optional_u16_hex_json(self.clut),
            optional_u32_json(self.color),
            optional_u32_hex_json(self.color),
            self.bounds.json(),
            self.bounds.area(),
            self.score(),
            points_json(&self.points),
            words_json(&self.words),
            self.stats.sampled_pixels,
            self.stats.drawn_pixels,
            self.stats.written_pixels,
            self.stats.clipped_pixels,
            self.stats.transparent_pixels,
            self.stats.first_color,
            self.stats.first_color,
            self.stats.last_color,
            self.stats.last_color,
            self.stats.color_hash,
            self.stats.color_hash,
            self.stats.color_changes
        )
    }

    fn compact_json(&self) -> String {
        format!(
            "{{\"sequence\":{},\"kind\":\"{}\",\"source\":{},\"texture_page\":{},\"texture_page_hex\":{},\"clut\":{},\"clut_hex\":{},\"color\":{},\"color_hex\":{},\"bounds\":{},\"area\":{},\"score\":{},\"sampled_pixels\":{},\"drawn_pixels\":{},\"written_pixels\":{},\"clipped_pixels\":{},\"transparent_pixels\":{},\"color_hash\":{},\"color_hash_hex\":\"0x{:08x}\",\"color_changes\":{}}}",
            self.sequence,
            self.kind,
            optional_gpu_source_json(self.source.as_ref()),
            optional_u16_json(self.texture_page),
            optional_u16_hex_json(self.texture_page),
            optional_u16_json(self.clut),
            optional_u16_hex_json(self.clut),
            optional_u32_json(self.color),
            optional_u32_hex_json(self.color),
            self.bounds.json(),
            self.bounds.area(),
            self.score(),
            self.stats.sampled_pixels,
            self.stats.drawn_pixels,
            self.stats.written_pixels,
            self.stats.clipped_pixels,
            self.stats.transparent_pixels,
            self.stats.color_hash,
            self.stats.color_hash,
            self.stats.color_changes
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct DrawBounds {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl DrawBounds {
    fn area(self) -> i64 {
        let width = self
            .right
            .saturating_sub(self.left)
            .saturating_add(1)
            .max(0) as i64;
        let height = self
            .bottom
            .saturating_sub(self.top)
            .saturating_add(1)
            .max(0) as i64;
        width.saturating_mul(height)
    }

    fn overlaps(self, other: DrawBounds) -> bool {
        self.left <= other.right
            && self.right >= other.left
            && self.top <= other.bottom
            && self.bottom >= other.top
    }

    fn intersection_area(self, other: DrawBounds) -> i64 {
        let left = self.left.max(other.left);
        let top = self.top.max(other.top);
        let right = self.right.min(other.right);
        let bottom = self.bottom.min(other.bottom);
        if left > right || top > bottom {
            return 0;
        }
        right.saturating_sub(left).saturating_add(1).max(0) as i64
            * bottom.saturating_sub(top).saturating_add(1).max(0) as i64
    }

    fn json(self) -> String {
        format!(
            "{{\"left\":{},\"top\":{},\"right\":{},\"bottom\":{}}}",
            self.left, self.top, self.right, self.bottom
        )
    }

    fn capture_window(
        self,
        fallback_width: usize,
        fallback_height: usize,
        padding: i32,
    ) -> (usize, usize, usize, usize) {
        if self.left > self.right || self.top > self.bottom {
            return (0, 0, fallback_width.max(1), fallback_height.max(1));
        }

        let left = self
            .left
            .saturating_sub(padding)
            .clamp(0, VRAM_WIDTH as i32 - 1) as usize;
        let top = self
            .top
            .saturating_sub(padding)
            .clamp(0, VRAM_HEIGHT as i32 - 1) as usize;
        let right = self
            .right
            .saturating_add(padding)
            .clamp(0, VRAM_WIDTH as i32 - 1) as usize;
        let bottom = self
            .bottom
            .saturating_add(padding)
            .clamp(0, VRAM_HEIGHT as i32 - 1) as usize;
        let width = right.saturating_sub(left).saturating_add(1).max(1);
        let height = bottom.saturating_sub(top).saturating_add(1).max(1);
        (left, top, width, height)
    }
}

#[derive(Clone, Debug)]
struct Gp0CommandTrace {
    opcode: u8,
    word_count: usize,
    words: Vec<u32>,
    source: Option<GpuCommandSource>,
}

impl Gp0CommandTrace {
    fn new(words: &[u32], source: Option<&GpuCommandSource>) -> Self {
        Self {
            opcode: (words[0] >> 24) as u8,
            word_count: words.len(),
            words: words
                .iter()
                .copied()
                .take(GP0_RECENT_COMMAND_WORD_LIMIT)
                .collect(),
            source: source.cloned(),
        }
    }

    fn json(&self) -> String {
        format!(
            "{{\"opcode\":{},\"opcode_hex\":\"0x{:02x}\",\"word_count\":{},\"source\":{},\"words\":[{}]}}",
            self.opcode,
            self.opcode,
            self.word_count,
            optional_gpu_source_json(self.source.as_ref()),
            words_json(&self.words)
        )
    }
}

#[derive(Clone, Debug)]
struct Gp1CommandTrace {
    command: u32,
    value: u32,
}

impl Gp1CommandTrace {
    fn new(value: u32) -> Self {
        Self {
            command: value >> 24,
            value,
        }
    }

    fn json(&self) -> String {
        format!(
            "{{\"command\":{},\"command_hex\":\"0x{:02x}\",\"value\":{},\"value_hex\":\"0x{:08x}\"}}",
            self.command, self.command, self.value, self.value
        )
    }
}

#[derive(Clone, Debug)]
struct GpuTransferTrace {
    kind: &'static str,
    source_x: Option<i32>,
    source_y: Option<i32>,
    dest_x: i32,
    dest_y: i32,
    width: i32,
    height: i32,
    data_words: usize,
    valid: bool,
}

impl GpuTransferTrace {
    fn image_upload(
        dest_x: i32,
        dest_y: i32,
        width: i32,
        height: i32,
        data_words: usize,
        valid: bool,
    ) -> Self {
        Self {
            kind: "image_upload",
            source_x: None,
            source_y: None,
            dest_x,
            dest_y,
            width,
            height,
            data_words,
            valid,
        }
    }

    fn vram_copy(
        source_x: i32,
        source_y: i32,
        dest_x: i32,
        dest_y: i32,
        width: i32,
        height: i32,
        valid: bool,
    ) -> Self {
        Self {
            kind: "vram_copy",
            source_x: Some(source_x),
            source_y: Some(source_y),
            dest_x,
            dest_y,
            width,
            height,
            data_words: 0,
            valid,
        }
    }

    fn json(&self) -> String {
        format!(
            "{{\"kind\":\"{}\",\"source_x\":{},\"source_y\":{},\"dest_x\":{},\"dest_y\":{},\"width\":{},\"height\":{},\"data_words\":{},\"valid\":{}}}",
            self.kind,
            optional_i32_json(self.source_x),
            optional_i32_json(self.source_y),
            self.dest_x,
            self.dest_y,
            self.width,
            self.height,
            self.data_words,
            self.valid
        )
    }
}

fn gp0_expected_words(fifo: &[u32]) -> Option<usize> {
    let first = *fifo.first()?;
    let command = (first >> 24) as u8;
    let expected = match command {
        0x00 | 0x01 | 0x03..=0x1f | 0xe1..=0xe6 => 1,
        0x02 => 3,
        0x20..=0x23 => 4,
        0x24..=0x27 => 7,
        0x28..=0x2b => 5,
        0x2c..=0x2f => 9,
        0x30..=0x33 => 6,
        0x34..=0x37 => 9,
        0x38..=0x3b => 8,
        0x3c..=0x3f => 12,
        0x40..=0x47 => 3,
        0x48..=0x4f => polyline_words(fifo)?,
        0x50..=0x57 => 4,
        0x58..=0x5f => polyline_words(fifo)?,
        0x60..=0x63 => 3,
        0x64..=0x67 => 4,
        0x68..=0x6f => 2,
        0x70..=0x73 => 2,
        0x74..=0x77 => 3,
        0x78..=0x7b => 2,
        0x7c..=0x7f => 3,
        0x80 => 4,
        0xa0 => image_transfer_words(fifo)?,
        0xc0 => 3,
        _ => 1,
    };
    Some(expected)
}

fn gp0_command_may_update_framebuffer(command: u8) -> bool {
    matches!(command, 0x02 | 0x20..=0x7f | 0x80 | 0xa0)
}

pub(crate) fn gp0_command_word_count(fifo: &[u32]) -> Option<usize> {
    gp0_expected_words(fifo)
}

fn polyline_words(fifo: &[u32]) -> Option<usize> {
    fifo.iter()
        .position(|word| *word == 0x5555_5555)
        .map(|index| index + 1)
        .or_else(|| (fifo.len() > 256).then_some(fifo.len()))
}

fn image_transfer_words(fifo: &[u32]) -> Option<usize> {
    if fifo.len() < 3 {
        return Some(3);
    }

    let Some((width, height)) = image_transfer_dimensions(fifo[2]) else {
        return Some(1);
    };
    let pixels = width.saturating_mul(height);
    Some(3 + pixels.div_ceil(2) as usize)
}

fn image_transfer_dimensions(value: u32) -> Option<(u32, u32)> {
    let width = value & 0xffff;
    let height = (value >> 16) & 0xffff;
    if width == 0 || height == 0 || width > VRAM_WIDTH as u32 || height > VRAM_HEIGHT as u32 {
        return None;
    }
    Some((width, height))
}

fn display_area_start_xy(value: u32) -> (usize, usize) {
    let x = (value & VRAM_X_MASK) as usize;
    let y = ((value >> 10) & VRAM_Y_MASK) as usize;
    (x.min(VRAM_WIDTH - 1), y.min(VRAM_HEIGHT - 1))
}

fn display_dimensions_from_registers(
    status: u32,
    horizontal_range: u32,
    vertical_range: u32,
) -> (usize, usize) {
    let (status_width, status_height) = display_dimensions_from_status(status);
    let width = display_width_from_horizontal_range(status_width, horizontal_range);
    let height = display_height_from_vertical_range(status_height, vertical_range);
    (width, height)
}

fn display_dimensions_from_status(status: u32) -> (usize, usize) {
    let width = match (status >> 17) & 0x03 {
        0 => 256,
        1 => 320,
        2 => 512,
        _ => 640,
    };
    let high_vertical_resolution = status & (1 << 19) != 0 && status & (1 << 22) != 0;
    let height = if high_vertical_resolution { 480 } else { 240 };
    (width.min(VRAM_WIDTH), height.min(VRAM_HEIGHT))
}

fn display_width_from_horizontal_range(status_width: usize, horizontal_range: u32) -> usize {
    let start = horizontal_range & 0x0fff;
    let end = (horizontal_range >> 12) & 0x0fff;
    if end <= start {
        return status_width.min(VRAM_WIDTH);
    }

    let dots = end - start;
    let dot_divider = match status_width {
        256 => 10,
        320 => 8,
        512 => 5,
        640 => 4,
        _ => 1,
    };
    let ranged_width = (dots / dot_divider).max(1) as usize;
    ranged_width.min(status_width).min(VRAM_WIDTH)
}

fn display_height_from_vertical_range(status_height: usize, vertical_range: u32) -> usize {
    let start = vertical_range & 0x03ff;
    let end = (vertical_range >> 10) & 0x03ff;
    if end <= start {
        return status_height.min(VRAM_HEIGHT);
    }

    let ranged_height = (end - start).max(1) as usize;
    ranged_height.min(status_height).min(VRAM_HEIGHT)
}

fn xy(value: u32) -> (i32, i32) {
    (
        sign_extend_11(value & 0x07ff),
        sign_extend_11((value >> 16) & 0x07ff),
    )
}

fn unsigned_xy(value: u32) -> (i32, i32) {
    (
        (value & VRAM_X_MASK).min((VRAM_WIDTH - 1) as u32) as i32,
        ((value >> 16) & VRAM_Y_MASK).min((VRAM_HEIGHT - 1) as u32) as i32,
    )
}

fn points_bounds(points: &[Point]) -> DrawBounds {
    let mut left = i32::MAX;
    let mut top = i32::MAX;
    let mut right = i32::MIN;
    let mut bottom = i32::MIN;
    for point in points {
        left = left.min(point.x);
        top = top.min(point.y);
        right = right.max(point.x);
        bottom = bottom.max(point.y);
    }
    DrawBounds {
        left,
        top,
        right,
        bottom,
    }
}

fn rect_bounds(point: Point, width: i32, height: i32) -> DrawBounds {
    DrawBounds {
        left: point.x,
        top: point.y,
        right: point.x.saturating_add(width).saturating_sub(1),
        bottom: point.y.saturating_add(height).saturating_sub(1),
    }
}

fn combine_textured_draw_stats(
    first: TexturedDrawStats,
    second: TexturedDrawStats,
) -> TexturedDrawStats {
    TexturedDrawStats {
        sampled_pixels: first.sampled_pixels.saturating_add(second.sampled_pixels),
        drawn_pixels: first.drawn_pixels.saturating_add(second.drawn_pixels),
        written_pixels: first.written_pixels.saturating_add(second.written_pixels),
        clipped_pixels: first.clipped_pixels.saturating_add(second.clipped_pixels),
        transparent_pixels: first
            .transparent_pixels
            .saturating_add(second.transparent_pixels),
        first_color: if first.drawn_pixels != 0 {
            first.first_color
        } else {
            second.first_color
        },
        last_color: if second.drawn_pixels != 0 {
            second.last_color
        } else {
            first.last_color
        },
        color_hash: first
            .color_hash
            .wrapping_mul(16_777_619)
            .wrapping_add(second.color_hash),
        color_changes: first
            .color_changes
            .saturating_add(second.color_changes)
            .saturating_add(u64::from(
                first.drawn_pixels != 0
                    && second.drawn_pixels != 0
                    && first.last_color != second.first_color,
            )),
    }
}

fn first_command_source(sources: &[Option<GpuCommandSource>]) -> Option<GpuCommandSource> {
    sources.first().and_then(Clone::clone)
}

fn fill_rect_dimensions_valid(width: i32, height: i32) -> bool {
    width > 0 && height > 0 && width <= VRAM_WIDTH as i32 && height <= VRAM_HEIGHT as i32
}

fn vram_copy_dimensions_valid(width: i32, height: i32) -> bool {
    width > 0 && height > 0 && width <= VRAM_WIDTH as i32 && height <= VRAM_HEIGHT as i32
}

fn vram_copy_dimensions(value: u32) -> Option<(i32, i32)> {
    let (width, height) = raw_dimensions(value);
    if width == 0 || height == 0 || width > VRAM_WIDTH as u32 || height > VRAM_HEIGHT as u32 {
        return None;
    }
    Some((width as i32, height as i32))
}

fn vram_copy_request_valid(
    source_x: i32,
    source_y: i32,
    dest_x: i32,
    dest_y: i32,
    width: i32,
    height: i32,
) -> bool {
    if !vram_copy_dimensions_valid(width, height) {
        return false;
    }

    let area = (width as i64).saturating_mul(height as i64);
    let full_vram_area = (VRAM_WIDTH as i64).saturating_mul(VRAM_HEIGHT as i64);
    let shifted_full_vram_copy =
        area >= full_vram_area && (source_x != dest_x || source_y != dest_y);
    !shifted_full_vram_copy
}

fn raw_dimensions(value: u32) -> (u32, u32) {
    (value & 0xffff, (value >> 16) & 0xffff)
}

fn drawing_offset_xy(value: u32) -> (i32, i32) {
    (
        sign_extend_11(value & 0x07ff),
        sign_extend_11((value >> 11) & 0x07ff),
    )
}

fn drawing_area_xy(value: u32) -> (i32, i32) {
    (
        (value & VRAM_X_MASK).min((VRAM_WIDTH - 1) as u32) as i32,
        ((value >> 10) & VRAM_Y_MASK).min((VRAM_HEIGHT - 1) as u32) as i32,
    )
}

fn drawing_area_clip(top_left: u32, bottom_right: u32) -> Option<ClipRect> {
    let (left, top) = drawing_area_xy(top_left);
    let (right, bottom) = drawing_area_xy(bottom_right);
    ClipRect::new(left, top, right, bottom)
}

fn point(value: u32) -> Point {
    let (x, y) = xy(value);
    Point { x, y }
}

fn sign_extend_11(value: u32) -> i32 {
    let value = value & 0x07ff;
    if value & 0x0400 != 0 {
        (value | !0x07ff) as i32
    } else {
        value as i32
    }
}

fn color(value: u32) -> u32 {
    let r = value & 0xff;
    let g = (value >> 8) & 0xff;
    let b = (value >> 16) & 0xff;
    (r << 16) | (g << 8) | b
}

fn clut(value: u32) -> u16 {
    (value >> 16) as u16
}

fn texture_page(value: u32) -> u16 {
    (value >> 16) as u16
}

fn texture_draw_options(command_word: u32, texture_page: u16) -> TextureDrawOptions {
    let command = (command_word >> 24) as u8;
    TextureDrawOptions {
        primitive_color: color(command_word),
        raw_texture: command & 0x01 != 0,
        semi_transparent: command & 0x02 != 0,
        semi_transparency_mode: ((texture_page >> 5) & 0x03) as u8,
        set_mask_bit: false,
        check_mask_bit: false,
        texture_flip_x: texture_page & 0x1000 != 0,
        texture_flip_y: texture_page & 0x2000 != 0,
    }
}

fn texture_coordinate(value: u32) -> TextureCoordinate {
    TextureCoordinate {
        u: (value & 0xff) as u8,
        v: ((value >> 8) & 0xff) as u8,
    }
}

fn optional_usize_json(value: Option<usize>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn optional_i32_json(value: Option<i32>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn optional_u32_json(value: Option<u32>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn optional_u32_hex_json(value: Option<u32>) -> String {
    value.map_or_else(|| "null".to_string(), |value| format!("\"0x{value:08x}\""))
}

fn optional_u16_json(value: Option<u16>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn optional_u16_hex_json(value: Option<u16>) -> String {
    value.map_or_else(|| "null".to_string(), |value| format!("\"0x{value:04x}\""))
}

fn optional_gpu_source_json(value: Option<&GpuCommandSource>) -> String {
    value.map_or_else(|| "null".to_string(), GpuCommandSource::json)
}

fn optional_str_json(value: Option<&str>) -> String {
    value.map_or_else(
        || "null".to_string(),
        |value| format!("\"{}\"", escape_json(value)),
    )
}

fn escape_json(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            _ => vec![ch],
        })
        .collect()
}

fn words_json(words: &[u32]) -> String {
    words
        .iter()
        .map(|word| format!("{{\"value\":{},\"value_hex\":\"0x{word:08x}\"}}", word))
        .collect::<Vec<_>>()
        .join(",")
}

fn points_json(points: &[Point]) -> String {
    points
        .iter()
        .map(|point| format!("{{\"x\":{},\"y\":{}}}", point.x, point.y))
        .collect::<Vec<_>>()
        .join(",")
}

fn framebuffer_window_json(window: FrameBufferWindow) -> String {
    format!(
        "{{\"x\":{},\"y\":{},\"pixel_count\":{},\"nonzero_pixels\":{},\"bright_pixels\":{},\"avg_luma\":{},\"max_luma\":{},\"detail_edges\":{},\"checksum\":{}}}",
        window.x,
        window.y,
        window.stats.pixel_count,
        window.stats.nonzero_pixels,
        window.stats.bright_pixels,
        average_luma(window.stats),
        window.stats.max_luma,
        window.stats.detail_edges,
        window.stats.checksum
    )
}

fn average_luma(stats: FrameBufferStats) -> u64 {
    stats.luma_sum.checked_div(stats.pixel_count).unwrap_or(0)
}

fn screen_observation_worth_saving(stats: FrameBufferStats) -> bool {
    if is_low_information_observation(stats) {
        return false;
    }
    if stats.bright_pixels == 0 && average_luma(stats) < 8 {
        return false;
    }
    stats.bright_pixels >= 256 || stats.nonzero_pixels >= 2048 || stats.max_luma >= 64
}

fn screen_observation_score(stats: FrameBufferStats) -> u64 {
    let average_luma = average_luma(stats);
    let luma_contrast = u64::from(stats.max_luma).saturating_sub(average_luma);
    let almost_full = stats.nonzero_pixels >= stats.pixel_count.saturating_mul(120) / 128;
    let detail_cutoff = detail_edge_cutoff(stats);
    let low_detail_full_frame =
        almost_full && (luma_contrast < 48 || stats.detail_edges < detail_cutoff);
    let bright_weight = if low_detail_full_frame { 2 } else { 8 };

    stats
        .bright_pixels
        .saturating_mul(bright_weight)
        .saturating_add(stats.nonzero_pixels)
        .saturating_add(average_luma.saturating_mul(64))
        .saturating_add(luma_contrast.saturating_mul(4096))
        .saturating_add(stats.detail_edges.saturating_mul(64))
        .saturating_sub(if low_detail_full_frame {
            stats.pixel_count.saturating_mul(2)
        } else {
            0
        })
}

fn is_detailed_observation(stats: FrameBufferStats) -> bool {
    !is_low_information_observation(stats)
        && stats.detail_edges >= detail_edge_cutoff(stats)
        && stats.max_luma >= 128
}

fn has_native_full_scene_detail(stats: FrameBufferStats) -> bool {
    has_native_playfield_density(stats)
        && is_detailed_observation(stats)
        && stats.detail_edges >= full_scene_detail_cutoff(stats)
}

fn has_native_gameplay_display_profile(stats: FrameBufferStats) -> bool {
    stats.pixel_count > 0
        && has_native_full_scene_detail(stats)
        && stats.nonzero_pixels.saturating_mul(100) >= stats.pixel_count.saturating_mul(65)
        && average_luma(stats) >= 12
}

fn has_native_playfield_density(stats: FrameBufferStats) -> bool {
    stats.nonzero_pixels >= stats.pixel_count.saturating_mul(3) / 10
        && stats.bright_pixels >= stats.pixel_count.saturating_div(64).max(512)
}

fn presented_frame_has_live_scene(stats: FrameBufferStats) -> bool {
    has_native_playfield_density(stats) && is_detailed_observation(stats)
}

fn detail_edge_cutoff(stats: FrameBufferStats) -> u64 {
    (stats.pixel_count / 64).max(256)
}

fn full_scene_detail_cutoff(stats: FrameBufferStats) -> u64 {
    (stats.pixel_count / 32).max(2048)
}

fn is_low_information_observation(stats: FrameBufferStats) -> bool {
    let nearly_all_pixels = stats
        .pixel_count
        .saturating_sub(stats.pixel_count / 128)
        .max(1);
    let average_luma = average_luma(stats);
    let nearly_uniform_luma =
        stats.nonzero_pixels >= nearly_all_pixels && u64::from(stats.max_luma) <= average_luma + 2;
    let nearly_solid_bright =
        stats.nonzero_pixels >= nearly_all_pixels && stats.bright_pixels >= nearly_all_pixels;

    nearly_uniform_luma || (nearly_solid_bright && average_luma >= 248)
}

fn should_use_observation_fallback(
    display_stats: FrameBufferStats,
    candidate_stats: FrameBufferStats,
) -> bool {
    if candidate_stats.bright_pixels == 0 {
        return false;
    }

    let sparse_nonzero = is_sparse_display(display_stats)
        && candidate_stats.nonzero_pixels > display_stats.nonzero_pixels.saturating_mul(4);
    let dark_display = display_stats.bright_pixels < (display_stats.pixel_count / 256).max(1)
        && candidate_stats.bright_pixels > display_stats.bright_pixels.saturating_mul(4).max(64);
    let brighter_candidate = average_luma(candidate_stats) >= average_luma(display_stats) + 8
        && candidate_stats.bright_pixels > display_stats.bright_pixels.saturating_mul(2).max(256);

    sparse_nonzero || dark_display || brighter_candidate
}

fn should_defer_low_detail_current(
    current_stats: FrameBufferStats,
    candidate_stats: FrameBufferStats,
) -> bool {
    if !is_detailed_observation(candidate_stats) {
        return false;
    }

    let current_score = screen_observation_score(current_stats);
    let candidate_score = screen_observation_score(candidate_stats);
    let much_more_detail =
        candidate_stats.detail_edges > current_stats.detail_edges.saturating_mul(4).max(512);
    if !much_more_detail {
        return false;
    }
    if candidate_score > current_score.saturating_mul(2) {
        return true;
    }

    has_native_full_scene_detail(candidate_stats)
        && candidate_score > current_score.saturating_add(current_score / 2)
}

fn resolved_display_candidate_has_scene_signal(
    current: FrameBufferWindow,
    current_score: u64,
    candidate: FrameBufferWindow,
) -> bool {
    if candidate.x == current.x
        && candidate.y == current.y
        && candidate.stats.checksum == current.stats.checksum
    {
        return false;
    }
    if !has_native_playfield_density(candidate.stats) || !is_detailed_observation(candidate.stats) {
        return false;
    }

    let candidate_score = screen_observation_score(candidate.stats);
    candidate_score > current_score.saturating_add(current_score / 2)
        && candidate.stats.detail_edges > current.stats.detail_edges.saturating_mul(4).max(512)
}

fn is_sparse_display(display_stats: FrameBufferStats) -> bool {
    let sparse_display_cutoff = (DEFAULT_DISPLAY_WIDTH * DEFAULT_DISPLAY_HEIGHT / 64) as u64;
    display_stats.nonzero_pixels < sparse_display_cutoff
}

fn is_likely_texture_page_origin(x: usize, y: usize) -> bool {
    matches!(x, 0 | 320 | 512 | 640) && matches!(y, 0 | 240 | 480 | PSX_VRAM_HEIGHT)
}

fn is_likely_texture_page_candidate(x: usize, y: usize, width: usize, height: usize) -> bool {
    if is_likely_texture_page_origin(x, y) {
        return true;
    }

    let right = x.saturating_add(width);
    let bottom = y.saturating_add(height);
    let starts_in_texture_column = matches!(x, 512 | 640);
    let is_large_page = width >= DEFAULT_DISPLAY_WIDTH && height >= DEFAULT_DISPLAY_HEIGHT;
    starts_in_texture_column && is_large_page && right <= VRAM_WIDTH && bottom <= PSX_VRAM_HEIGHT
}

#[derive(Clone, Debug)]
pub struct Dma {
    channels: [DmaChannel; DMA_CHANNEL_COUNT],
    pub control: u32,
    pub interrupt: u32,
    padding: [u32; 2],
}

impl Default for Dma {
    fn default() -> Self {
        Self {
            channels: [DmaChannel::default(); DMA_CHANNEL_COUNT],
            control: 0x0765_4321,
            interrupt: 0,
            padding: [0; 2],
        }
    }
}

impl Dma {
    pub fn channel_state(&self, channel: usize) -> Option<DmaChannelState> {
        self.channels.get(channel).map(|channel| DmaChannelState {
            madr: channel.madr,
            bcr: channel.bcr,
            chcr: channel.chcr,
        })
    }

    pub fn read_u32(&self, address: u32) -> u32 {
        match dma_register_slot(address) {
            DmaRegisterSlot::Channel(channel, DmaChannelRegister::Madr) => {
                self.channels[channel].madr
            }
            DmaRegisterSlot::Channel(channel, DmaChannelRegister::Bcr) => {
                self.channels[channel].bcr
            }
            DmaRegisterSlot::Channel(channel, DmaChannelRegister::Chcr) => {
                self.channels[channel].chcr
            }
            DmaRegisterSlot::Control => self.control,
            DmaRegisterSlot::Interrupt => self.interrupt_with_master_flag(),
            DmaRegisterSlot::Padding(index) => self.padding[index],
        }
    }

    pub fn write_u32(&mut self, address: u32, value: u32) {
        match dma_register_slot(address) {
            DmaRegisterSlot::Channel(channel, DmaChannelRegister::Madr) => {
                self.channels[channel].madr = value & 0x00ff_fffc;
            }
            DmaRegisterSlot::Channel(channel, DmaChannelRegister::Bcr) => {
                self.channels[channel].bcr = value;
            }
            DmaRegisterSlot::Channel(channel, DmaChannelRegister::Chcr) => {
                self.channels[channel].chcr = value;
                if value & (1 << 24) != 0 && !bus_driven_dma_channel(channel) {
                    self.complete_channel(channel);
                }
            }
            DmaRegisterSlot::Control => self.control = value,
            DmaRegisterSlot::Interrupt => self.write_interrupt(value),
            DmaRegisterSlot::Padding(index) => self.padding[index] = value,
        }
    }

    pub fn irq_pending(&self) -> bool {
        self.interrupt_with_master_flag() & (1 << 31) != 0
    }

    pub fn acknowledge_pending_irq_flags(&mut self) {
        let pending_flags = self.interrupt & DMA_INTERRUPT_FLAG_MASK;
        if pending_flags == 0 {
            return;
        }

        let preserved_control = self.interrupt & (DMA_INTERRUPT_IRQ_ENABLE_MASK | (1 << 23));
        self.write_interrupt(pending_flags | preserved_control);
    }

    pub fn complete_channel(&mut self, channel: usize) {
        if channel >= self.channels.len() {
            return;
        }

        self.channels[channel].chcr &= !(1 << 24);
        self.mark_channel_complete(channel);
    }

    fn mark_channel_complete(&mut self, channel: usize) {
        let channel_bit = 1 << (16 + channel);
        if self.interrupt & channel_bit != 0 {
            self.interrupt |= 1 << (24 + channel);
        }
    }

    fn write_interrupt(&mut self, value: u32) {
        let acknowledged = value & DMA_INTERRUPT_FLAG_MASK;
        let writable = value & (DMA_INTERRUPT_IRQ_ENABLE_MASK | (1 << 23));
        self.interrupt = (self.interrupt & !acknowledged) | writable;
    }

    fn interrupt_with_master_flag(&self) -> u32 {
        let master_enabled = self.interrupt & (1 << 23) != 0;
        let enabled_flags = ((self.interrupt & DMA_INTERRUPT_FLAG_MASK) >> 8)
            & self.interrupt
            & DMA_INTERRUPT_IRQ_ENABLE_MASK;
        if master_enabled && enabled_flags != 0 {
            self.interrupt | (1 << 31)
        } else {
            self.interrupt & !(1 << 31)
        }
    }
}

fn bus_driven_dma_channel(channel: usize) -> bool {
    matches!(channel, 0 | 1 | 2 | 6)
}

#[derive(Clone, Copy, Debug, Default)]
struct DmaChannel {
    madr: u32,
    bcr: u32,
    chcr: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DmaChannelState {
    pub madr: u32,
    pub bcr: u32,
    pub chcr: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DmaChannelRegister {
    Madr,
    Bcr,
    Chcr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DmaRegisterSlot {
    Channel(usize, DmaChannelRegister),
    Control,
    Interrupt,
    Padding(usize),
}

fn dma_register_slot(address: u32) -> DmaRegisterSlot {
    match address {
        DMA_CONTROL => DmaRegisterSlot::Control,
        DMA_INTERRUPT => DmaRegisterSlot::Interrupt,
        0x1f80_10f8 => DmaRegisterSlot::Padding(0),
        0x1f80_10fc => DmaRegisterSlot::Padding(1),
        _ => {
            let channel = ((address - DMA_REGION_START) / DMA_CHANNEL_STRIDE) as usize;
            let register = match (address - DMA_REGION_START) % DMA_CHANNEL_STRIDE {
                0x0 => DmaChannelRegister::Madr,
                0x4 => DmaChannelRegister::Bcr,
                _ => DmaChannelRegister::Chcr,
            };
            DmaRegisterSlot::Channel(channel.min(DMA_CHANNEL_COUNT - 1), register)
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Timers(pub [Timer; 3]);

impl Timers {
    pub fn tick(&mut self, cycles: u64) -> u32 {
        let mut irq_status = 0;
        for (index, timer) in self.0.iter_mut().enumerate() {
            if timer.tick(cycles) {
                irq_status |= 1 << (4 + index);
            }
        }
        irq_status
    }

    fn read_u32(&self, address: u32) -> u32 {
        let (timer, register) = timer_register_slot(address);
        self.0[timer].read(register) as u32
    }

    fn write_u32(&mut self, address: u32, value: u32) {
        let (timer, register) = timer_register_slot(address);
        self.0[timer].write(register, value as u16);
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Timer {
    pub counter: u16,
    pub mode: u16,
    pub target: u16,
    cycle_accumulator: u64,
    irq_fired: bool,
}

impl Timer {
    fn tick(&mut self, cycles: u64) -> bool {
        const TIMER_COUNTER_DIVISOR: u64 = 128;
        const MODE_RESET_AT_TARGET: u16 = 1 << 3;
        const MODE_IRQ_AT_TARGET: u16 = 1 << 4;
        const MODE_IRQ_AT_OVERFLOW: u16 = 1 << 5;
        const MODE_IRQ_REPEAT: u16 = 1 << 6;
        const MODE_IRQ_TOGGLE: u16 = 1 << 7;
        const MODE_IRQ_NOT_REQUESTED: u16 = 1 << 10;
        const MODE_REACHED_TARGET: u16 = 1 << 11;
        const MODE_REACHED_OVERFLOW: u16 = 1 << 12;

        self.cycle_accumulator = self.cycle_accumulator.saturating_add(cycles);
        let increments = self.cycle_accumulator / TIMER_COUNTER_DIVISOR;
        self.cycle_accumulator %= TIMER_COUNTER_DIVISOR;
        if increments == 0 {
            return false;
        }

        let start = u64::from(self.counter);
        let target = u64::from(self.target);
        let reset_at_target = self.mode & MODE_RESET_AT_TARGET != 0 && target > 0;
        let reached_target = target > 0 && start.saturating_add(increments) >= target;
        let reached_overflow = start.saturating_add(increments) > u64::from(u16::MAX);

        self.counter = if reset_at_target {
            let period = target.saturating_add(1);
            ((start.saturating_add(increments)) % period) as u16
        } else {
            self.counter.wrapping_add(increments as u16)
        };

        if reached_target {
            self.mode |= MODE_REACHED_TARGET;
        }
        if reached_overflow {
            self.mode |= MODE_REACHED_OVERFLOW;
        }

        let irq_requested = (reached_target && self.mode & MODE_IRQ_AT_TARGET != 0)
            || (reached_overflow && self.mode & MODE_IRQ_AT_OVERFLOW != 0);
        let repeat_irq = self.mode & MODE_IRQ_REPEAT != 0;
        if !irq_requested || (self.irq_fired && !repeat_irq) {
            return false;
        }

        self.irq_fired = true;
        if self.mode & MODE_IRQ_TOGGLE != 0 {
            self.mode ^= MODE_IRQ_NOT_REQUESTED;
        } else {
            self.mode &= !MODE_IRQ_NOT_REQUESTED;
        }
        true
    }

    fn read(self, register: TimerRegister) -> u16 {
        match register {
            TimerRegister::Counter => self.counter,
            TimerRegister::Mode => self.mode,
            TimerRegister::Target => self.target,
        }
    }

    fn write(&mut self, register: TimerRegister, value: u16) {
        match register {
            TimerRegister::Counter => {
                self.counter = value;
                self.cycle_accumulator = 0;
            }
            TimerRegister::Mode => {
                self.mode = (value & 0x03ff) | (1 << 10);
                self.irq_fired = false;
            }
            TimerRegister::Target => self.target = value,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TimerRegister {
    Counter,
    Mode,
    Target,
}

fn timer_register_slot(address: u32) -> (usize, TimerRegister) {
    let timer = ((address - TIMER0_COUNTER) / 0x10) as usize;
    let register = match (address - TIMER0_COUNTER) % 0x10 {
        0x0 => TimerRegister::Counter,
        0x4 => TimerRegister::Mode,
        _ => TimerRegister::Target,
    };
    (timer.min(2), register)
}

#[derive(Clone, Debug)]
pub struct Controller {
    pub p1_state: u16,
    pub last_write: u16,
    pub status: u16,
    pub mode: u16,
    pub control: u16,
    pub baud: u16,
    transfer_index: u8,
    security_transfer_index: usize,
    security_response: Vec<u8>,
    cat702: [Option<Cat702>; 2],
    zn_mcu: ZnMcu,
    response: u8,
    irq_requested: bool,
}

impl Default for Controller {
    fn default() -> Self {
        Self {
            p1_state: 0xffff,
            last_write: 0,
            status: 0x0005,
            mode: 0,
            control: 0,
            baud: 0,
            transfer_index: 0,
            security_transfer_index: 0,
            security_response: Vec::new(),
            cat702: [None, None],
            zn_mcu: ZnMcu::default(),
            response: 0xff,
            irq_requested: false,
        }
    }
}

impl Controller {
    pub fn set_security_response(&mut self, response: Vec<u8>) {
        self.security_response = response;
        self.security_transfer_index = 0;
        self.response = 0xff;
        self.status |= 0x0003;
    }

    pub fn set_cat702_transforms(&mut self, cat702_1: Option<[u8; 8]>, cat702_2: Option<[u8; 8]>) {
        self.cat702 = [cat702_1.map(Cat702::new), cat702_2.map(Cat702::new)];
        self.response = 0xff;
        self.status |= 0x0003;
    }

    pub fn set_security_selects(
        &mut self,
        cat702_1_select: bool,
        cat702_2_select: bool,
        zn_mcu_analog_read: bool,
        zn_mcu_trackball_read: bool,
        zn_mcu_selected: bool,
    ) {
        if let Some(cat702) = &mut self.cat702[0] {
            cat702.write_select(cat702_1_select);
        }
        if let Some(cat702) = &mut self.cat702[1] {
            cat702.write_select(cat702_2_select);
        }
        self.zn_mcu
            .set_lines(zn_mcu_analog_read, zn_mcu_trackball_read, zn_mcu_selected);
    }

    pub fn zn_mcu_diagnostic_json(&self) -> String {
        self.zn_mcu.diagnostic_json()
    }

    fn irq_requested(&self) -> bool {
        self.irq_requested
    }

    fn read_u32(&self, address: u32) -> u32 {
        match address {
            SIO_DATA => self.response as u32,
            SIO_STATUS => self.status as u32,
            SIO_MODE => self.mode as u32,
            SIO_CONTROL => self.control as u32,
            SIO_BAUD => self.baud as u32,
            _ => 0,
        }
    }

    fn write_u32(&mut self, address: u32, value: u32) {
        match address {
            SIO_DATA => self.write_data(value as u16),
            SIO_MODE => self.mode = value as u16,
            SIO_CONTROL => self.write_control(value as u16),
            SIO_BAUD => self.baud = value as u16,
            _ => {}
        }
    }

    fn write_data(&mut self, value: u16) {
        self.last_write = value;
        let value = value as u8;
        if self.cat702_selected() {
            self.response = self.transfer_cat702_byte(value);
        } else if self.zn_mcu.is_selected() {
            self.response = self.zn_mcu.transfer_byte();
        } else if self.security_response.is_empty() || value == 0x01 {
            self.response = match self.transfer_index {
                0 => 0xff,
                1 if value == 0x42 => 0x41,
                1 => 0xff,
                2 => 0x5a,
                3 => self.p1_state as u8,
                4 => (self.p1_state >> 8) as u8,
                _ => 0xff,
            };
            self.transfer_index = if value == 0x01 {
                1
            } else {
                self.transfer_index.saturating_add(1)
            };
        } else {
            self.response = self.security_response_byte();
            self.security_transfer_index = self.security_transfer_index.saturating_add(1);
        }
        self.irq_requested = true;
        self.status |= 0x0003 | SIO_STATUS_IRQ_REQUEST;
    }

    fn write_control(&mut self, value: u16) {
        self.control = value;
        if value & 0x0040 != 0 {
            self.transfer_index = 0;
            self.security_transfer_index = 0;
            self.zn_mcu.reset_transfer();
            self.response = 0xff;
            self.irq_requested = false;
            self.status = 0x0007;
        } else if value & 0x0010 != 0 {
            self.irq_requested = false;
            self.status &= !SIO_STATUS_IRQ_REQUEST;
        } else if value & 0x0003 == 0x0003 {
            self.security_transfer_index = 0;
            self.zn_mcu.reset_transfer();
            self.status |= 0x0003;
        }
    }

    fn security_response_byte(&self) -> u8 {
        if self.security_transfer_index == 0 {
            return 0xff;
        }

        self.security_response
            .get(self.security_transfer_index - 1)
            .copied()
            .unwrap_or(0xff)
    }

    fn cat702_selected(&self) -> bool {
        self.cat702
            .iter()
            .flatten()
            .any(|cat702| cat702.is_selected())
    }

    fn transfer_cat702_byte(&mut self, value: u8) -> u8 {
        let mut response = 0u8;
        for bit in 0..8 {
            let datain = (value >> bit) & 1 != 0;
            let mut dataout = true;
            for cat702 in self.cat702.iter_mut().flatten() {
                dataout &= cat702.transfer_bit(datain);
            }
            if dataout {
                response |= 1 << bit;
            }
        }
        response
    }

    pub fn set_buttons(&mut self, buttons: ActionButtons) {
        let mut state = 0xffff_u16;
        clear_if(&mut state, 0x0010, buttons.up);
        clear_if(&mut state, 0x0040, buttons.down);
        clear_if(&mut state, 0x0080, buttons.left);
        clear_if(&mut state, 0x0020, buttons.right);
        clear_if(&mut state, 0x4000, buttons.punch);
        clear_if(&mut state, 0x2000, buttons.kick);
        clear_if(&mut state, 0x1000, buttons.beast);
        clear_if(&mut state, 0x8000, buttons.guard);
        clear_if(&mut state, 0x0008, buttons.start);
        self.p1_state = state;
    }
}

#[derive(Clone, Debug)]
struct ZnMcu {
    analog_read: bool,
    trackball_read: bool,
    selected: bool,
    transfer_index: usize,
    packet: Vec<u8>,
    select_transitions: u64,
    transferred_bytes: u64,
    last_response: u8,
}

impl Default for ZnMcu {
    fn default() -> Self {
        Self {
            analog_read: false,
            trackball_read: false,
            selected: false,
            transfer_index: 0,
            packet: Vec::new(),
            select_transitions: 0,
            transferred_bytes: 0,
            last_response: 0xff,
        }
    }
}

impl ZnMcu {
    fn set_lines(&mut self, analog_read: bool, trackball_read: bool, selected: bool) {
        let mode_changed = self.analog_read != analog_read || self.trackball_read != trackball_read;
        let became_selected = selected && !self.selected;

        self.analog_read = analog_read;
        self.trackball_read = trackball_read;

        if became_selected {
            self.select_transitions = self.select_transitions.saturating_add(1);
        }

        if became_selected || (selected && mode_changed) {
            self.transfer_index = 0;
            self.rebuild_packet();
        } else if !selected && self.selected {
            self.transfer_index = 0;
            self.packet.clear();
        }

        self.selected = selected;
    }

    fn is_selected(&self) -> bool {
        self.selected
    }

    fn reset_transfer(&mut self) {
        self.transfer_index = 0;
        if self.selected {
            self.rebuild_packet();
        }
    }

    fn transfer_byte(&mut self) -> u8 {
        if self.packet.is_empty() {
            self.rebuild_packet();
        }

        let response = self
            .packet
            .get(self.transfer_index)
            .copied()
            .unwrap_or(0xff);
        self.transfer_index = self.transfer_index.saturating_add(1);
        self.transferred_bytes = self.transferred_bytes.saturating_add(1);
        self.last_response = response;
        response
    }

    fn rebuild_packet(&mut self) {
        let databytes = if self.analog_read {
            8
        } else if self.trackball_read {
            6
        } else {
            1
        };

        self.packet.clear();
        self.packet.push((databytes << 4) | 0x0f);

        if self.analog_read {
            self.packet.extend([0xff; 8]);
        } else if self.trackball_read {
            self.packet.extend([0x00; 6]);
        } else {
            self.packet.push(0x00);
        }
    }

    fn diagnostic_json(&self) -> String {
        format!(
            "{{\"selected\":{},\"analog_read\":{},\"trackball_read\":{},\"transfer_index\":{},\"packet_len\":{},\"select_transitions\":{},\"transferred_bytes\":{},\"last_response\":{},\"last_response_hex\":\"0x{:02x}\"}}",
            self.selected,
            self.analog_read,
            self.trackball_read,
            self.transfer_index,
            self.packet.len(),
            self.select_transitions,
            self.transferred_bytes,
            self.last_response,
            self.last_response
        )
    }
}

#[derive(Clone, Debug)]
struct Cat702 {
    transform: [u8; 8],
    select: bool,
    state: u8,
    bit: u8,
    dataout: bool,
}

impl Cat702 {
    const INITIAL_SBOX: [u8; 8] = [0xff, 0xfe, 0xfc, 0xf8, 0xf0, 0xe0, 0xc0, 0x7f];

    fn new(transform: [u8; 8]) -> Self {
        Self {
            transform,
            select: true,
            state: 0,
            bit: 0,
            dataout: true,
        }
    }

    fn write_select(&mut self, select: bool) {
        if self.select == select {
            return;
        }

        if select {
            self.dataout = true;
        } else {
            self.state = 0xfc;
            self.bit = 0;
        }
        self.select = select;
    }

    fn is_selected(&self) -> bool {
        !self.select
    }

    fn transfer_bit(&mut self, datain: bool) -> bool {
        if self.select {
            self.dataout = true;
            return true;
        }

        if self.bit == 0 {
            self.apply_sbox(&Self::INITIAL_SBOX);
        }

        self.dataout = self.state & (1 << self.bit) != 0;
        if !datain {
            self.apply_bit_sbox(self.bit);
        }
        self.bit = self.bit.wrapping_add(1) & 7;
        self.dataout
    }

    fn apply_sbox(&mut self, sbox: &[u8; 8]) {
        let mut next = 0u8;
        for (bit, coefficient) in sbox.iter().enumerate() {
            if self.state & (1 << bit) != 0 {
                next ^= *coefficient;
            }
        }
        self.state = next;
    }

    fn apply_bit_sbox(&mut self, selector: u8) {
        let mut next = 0u8;
        for bit in 0..8 {
            if self.state & (1 << bit) != 0 {
                next ^= self.sbox_coefficient(selector, bit as u8);
            }
        }
        self.state = next;
    }

    fn sbox_coefficient(&self, selector: u8, bit: u8) -> u8 {
        if selector == 0 {
            return self.transform[bit as usize];
        }

        let previous = self.sbox_coefficient(selector.wrapping_sub(1) & 7, bit.wrapping_sub(1) & 7);
        let shifted = (previous << 1) | (((previous >> 7) ^ (previous >> 6)) & 1);
        if bit == 7 {
            shifted ^ self.sbox_coefficient(selector, 0)
        } else {
            shifted
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Cdrom {
    index: u8,
    status: u8,
    response: u8,
    data: u8,
    interrupt_enable: u8,
    last_command: u8,
}

impl Cdrom {
    fn read_u32(&self, address: u32) -> u32 {
        match address {
            CDROM_INDEX_STATUS => ((self.status | 0x18) & !0x03) as u32 | self.index as u32,
            CDROM_RESPONSE => self.response as u32,
            CDROM_DATA => self.data as u32,
            CDROM_INTERRUPT_ENABLE => self.interrupt_enable as u32,
            _ => 0,
        }
    }

    fn write_u32(&mut self, address: u32, value: u32) {
        let value = value as u8;
        match address {
            CDROM_INDEX_STATUS => self.index = value & 0x03,
            CDROM_RESPONSE => {
                self.last_command = value;
                self.response = 0x00;
            }
            CDROM_DATA => self.data = value,
            CDROM_INTERRUPT_ENABLE => self.interrupt_enable = value,
            _ => {}
        }
    }
}

#[derive(Clone, Debug)]
pub struct Mdec {
    command: u32,
    control: u32,
    status: u32,
    input_words_remaining: u32,
    dma_input_words: u64,
    dma_output_words: u64,
    input_words: Vec<u32>,
    decoded_output_words: Vec<u32>,
    decoded_output_index: usize,
    decoded_output_underflow_reads: u64,
    last_decoded_output_words: usize,
    quant_luma: [u8; 64],
    quant_chroma: [u8; 64],
    scale_table: [i16; 64],
}

impl Default for Mdec {
    fn default() -> Self {
        Self {
            command: 0,
            control: 0,
            status: mdec_ready_status(),
            input_words_remaining: 0,
            dma_input_words: 0,
            dma_output_words: 0,
            input_words: Vec::new(),
            decoded_output_words: Vec::new(),
            decoded_output_index: 0,
            decoded_output_underflow_reads: 0,
            last_decoded_output_words: 0,
            quant_luma: [8; 64],
            quant_chroma: [8; 64],
            scale_table: [0; 64],
        }
    }
}

impl Mdec {
    fn read_u32(&self, address: u32) -> u32 {
        match address {
            MDEC_COMMAND => self.command,
            MDEC_STATUS => self.status,
            _ => 0,
        }
    }

    fn write_u32(&mut self, address: u32, value: u32) {
        match address {
            MDEC_COMMAND => self.write_command_or_data(value),
            MDEC_STATUS => {
                if value & 0x8000_0000 != 0 {
                    *self = Self::default();
                } else {
                    self.control = value & 0x6000_0000;
                    self.status = mdec_status(self.input_words_remaining);
                }
            }
            _ => {}
        }
    }

    pub fn write_dma_input(&mut self, value: u32) {
        self.dma_input_words = self.dma_input_words.saturating_add(1);
        self.write_command_or_data(value);
    }

    fn write_command_or_data(&mut self, value: u32) {
        if self.input_words_remaining == 0 {
            self.start_command(value);
        } else {
            self.input_words.push(value);
            self.input_words_remaining = self.input_words_remaining.saturating_sub(1);
            if self.input_words_remaining == 0 {
                self.finish_input_dma();
            }
        }
        self.status = mdec_status(self.input_words_remaining);
    }

    fn start_command(&mut self, value: u32) {
        self.command = value;
        self.input_words_remaining = mdec_input_words_for_command(value);
        self.input_words.clear();
        self.decoded_output_words.clear();
        self.decoded_output_index = 0;
        self.last_decoded_output_words = 0;
        if self.input_words_remaining == 0 {
            self.finish_input_dma();
        }
    }

    pub fn read_dma_output(&mut self) -> u32 {
        self.dma_output_words = self.dma_output_words.saturating_add(1);
        self.status = mdec_status(self.input_words_remaining);
        if self.decoded_output_index >= self.decoded_output_words.len() {
            self.decoded_output_underflow_reads =
                self.decoded_output_underflow_reads.saturating_add(1);
            return 0;
        }
        let word = self.decoded_output_words[self.decoded_output_index];
        self.decoded_output_index = self.decoded_output_index.saturating_add(1);
        word
    }

    fn finish_input_dma(&mut self) {
        self.decoded_output_words.clear();
        self.decoded_output_index = 0;
        match self.command >> 29 {
            0x1 => {
                self.decoded_output_words = decode_mdec_macroblocks(
                    &self.input_words,
                    &self.quant_luma,
                    &self.quant_chroma,
                );
            }
            0x2 => self.set_quant_tables(),
            0x3 => self.set_scale_table(),
            _ => {}
        }
        self.last_decoded_output_words = self.decoded_output_words.len();
    }

    fn set_quant_tables(&mut self) {
        let bytes = mdec_bytes(&self.input_words);
        for (index, value) in bytes.iter().take(64).copied().enumerate() {
            self.quant_luma[index] = value.max(1);
        }
        if bytes.len() >= 128 {
            for (index, value) in bytes.iter().skip(64).take(64).copied().enumerate() {
                self.quant_chroma[index] = value.max(1);
            }
        } else {
            self.quant_chroma = self.quant_luma;
        }
    }

    fn set_scale_table(&mut self) {
        for (index, value) in mdec_halfwords(&self.input_words)
            .into_iter()
            .take(64)
            .enumerate()
        {
            self.scale_table[index] = value as i16;
        }
    }

    pub fn input_words_remaining(&self) -> u32 {
        self.input_words_remaining
    }

    pub fn dma_input_words(&self) -> u64 {
        self.dma_input_words
    }

    pub fn dma_output_words(&self) -> u64 {
        self.dma_output_words
    }

    pub fn diagnostic_json(&self) -> String {
        format!(
            "{{\"command\":{},\"command_hex\":\"0x{:08x}\",\"control\":{},\"control_hex\":\"0x{:08x}\",\"status\":{},\"status_hex\":\"0x{:08x}\",\"input_words_remaining\":{},\"dma_input_words\":{},\"dma_output_words\":{},\"last_decoded_output_words\":{},\"decoded_output_index\":{},\"decoded_output_underflow_reads\":{}}}",
            self.command,
            self.command,
            self.control,
            self.control,
            self.status,
            self.status,
            self.input_words_remaining,
            self.dma_input_words,
            self.dma_output_words,
            self.last_decoded_output_words,
            self.decoded_output_index,
            self.decoded_output_underflow_reads
        )
    }
}

fn mdec_input_words_for_command(value: u32) -> u32 {
    match value >> 29 {
        0x1 => value & 0xffff,
        0x2 => {
            if value & 1 != 0 {
                32
            } else {
                16
            }
        }
        0x3 => 32,
        _ => 0,
    }
}

fn mdec_status(input_words_remaining: u32) -> u32 {
    let busy = u32::from(input_words_remaining != 0) << 29;
    0x8004_0000 | busy
}

fn mdec_ready_status() -> u32 {
    mdec_status(0)
}

const MDEC_ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

fn decode_mdec_macroblocks(
    words: &[u32],
    quant_luma: &[u8; 64],
    quant_chroma: &[u8; 64],
) -> Vec<u32> {
    let halfwords = mdec_halfwords(words);
    let mut cursor = 0usize;
    let mut pixels = Vec::new();

    while cursor < halfwords.len() {
        let Some(cr) = read_mdec_block(&halfwords, &mut cursor, quant_chroma) else {
            break;
        };
        let Some(cb) = read_mdec_block(&halfwords, &mut cursor, quant_chroma) else {
            break;
        };
        let Some(y0) = read_mdec_block(&halfwords, &mut cursor, quant_luma) else {
            break;
        };
        let Some(y1) = read_mdec_block(&halfwords, &mut cursor, quant_luma) else {
            break;
        };
        let Some(y2) = read_mdec_block(&halfwords, &mut cursor, quant_luma) else {
            break;
        };
        let Some(y3) = read_mdec_block(&halfwords, &mut cursor, quant_luma) else {
            break;
        };

        for row in 0..16 {
            for col in 0..16 {
                let y_block = match (row >= 8, col >= 8) {
                    (false, false) => &y0,
                    (false, true) => &y1,
                    (true, false) => &y2,
                    (true, true) => &y3,
                };
                let y = y_block[(row % 8) * 8 + (col % 8)];
                let chroma_index = (row / 2) * 8 + (col / 2);
                pixels.push(ycbcr_to_rgb555(y, cb[chroma_index], cr[chroma_index]));
            }
        }
    }

    pack_rgb555_pixels(&pixels)
}

fn mdec_bytes(words: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(words.len().saturating_mul(4));
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn mdec_halfwords(words: &[u32]) -> Vec<u16> {
    let mut halfwords = Vec::with_capacity(words.len().saturating_mul(2));
    for word in words {
        halfwords.push((word & 0xffff) as u16);
        halfwords.push((word >> 16) as u16);
    }
    halfwords
}

fn read_mdec_block(halfwords: &[u16], cursor: &mut usize, quant: &[u8; 64]) -> Option<[i16; 64]> {
    let first = *halfwords.get(*cursor)?;
    *cursor += 1;
    let qscale = ((first >> 10) & 0x3f).max(1) as i32;
    let dc = sign_extend_10(first & 0x03ff) as i32;
    let mut coefficients = [0_i32; 64];
    coefficients[0] = dc.saturating_mul(i32::from(quant[0]));
    let mut zigzag_index = 0usize;

    while let Some(word) = halfwords.get(*cursor).copied() {
        *cursor += 1;
        if word == 0xfe00 {
            break;
        }
        let run = ((word >> 10) & 0x3f) as usize;
        zigzag_index = zigzag_index.saturating_add(run).saturating_add(1);
        if zigzag_index >= 64 {
            continue;
        }
        let value = sign_extend_10(word & 0x03ff) as i32;
        let natural_index = MDEC_ZIGZAG[zigzag_index];
        coefficients[natural_index] = value
            .saturating_mul(i32::from(quant[zigzag_index]))
            .saturating_mul(qscale)
            / 8;
    }

    Some(idct_8x8(&coefficients))
}

fn idct_8x8(coefficients: &[i32; 64]) -> [i16; 64] {
    let mut block = [0_i16; 64];
    let basis = mdec_idct_basis();
    for output_index in 0..64 {
        let mut sum = 0.0_f64;
        for coefficient_index in 0..64 {
            sum += coefficients[coefficient_index] as f64 * basis[output_index][coefficient_index];
        }
        block[output_index] = sum.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16;
    }
    block
}

fn mdec_idct_basis() -> &'static [[f64; 64]; 64] {
    static BASIS: OnceLock<[[f64; 64]; 64]> = OnceLock::new();
    BASIS.get_or_init(|| {
        let mut basis = [[0.0_f64; 64]; 64];
        for y in 0..8 {
            for x in 0..8 {
                let output_index = y * 8 + x;
                for v in 0..8 {
                    for u in 0..8 {
                        let cu = if u == 0 {
                            std::f64::consts::FRAC_1_SQRT_2
                        } else {
                            1.0
                        };
                        let cv = if v == 0 {
                            std::f64::consts::FRAC_1_SQRT_2
                        } else {
                            1.0
                        };
                        let basis_x =
                            (((2 * x + 1) * u) as f64 * std::f64::consts::PI / 16.0).cos();
                        let basis_y =
                            (((2 * y + 1) * v) as f64 * std::f64::consts::PI / 16.0).cos();
                        basis[output_index][v * 8 + u] = cu * cv * basis_x * basis_y / 4.0;
                    }
                }
            }
        }
        basis
    })
}

fn sign_extend_10(value: u16) -> i16 {
    let value = value & 0x03ff;
    if value & 0x0200 != 0 {
        (value | 0xfc00) as i16
    } else {
        value as i16
    }
}

fn ycbcr_to_rgb555(y: i16, cb: i16, cr: i16) -> u16 {
    let y = 128 + i32::from(y);
    let cb = i32::from(cb);
    let cr = i32::from(cr);
    let r = y + ((1436 * cr) >> 10);
    let g = y - (((352 * cb) + (731 * cr)) >> 10);
    let b = y + ((1815 * cb) >> 10);
    rgb888_to_rgb555_components(clamp_u8(r), clamp_u8(g), clamp_u8(b))
}

fn clamp_u8(value: i32) -> u8 {
    value.clamp(0, u8::MAX as i32) as u8
}

fn rgb888_to_rgb555_components(r: u8, g: u8, b: u8) -> u16 {
    ((u16::from(r) >> 3) & 0x1f)
        | (((u16::from(g) >> 3) & 0x1f) << 5)
        | (((u16::from(b) >> 3) & 0x1f) << 10)
}

fn pack_rgb555_pixels(pixels: &[u16]) -> Vec<u32> {
    pixels
        .chunks(2)
        .map(|chunk| {
            let low = u32::from(chunk[0]);
            let high = chunk.get(1).copied().map(u32::from).unwrap_or(0);
            low | (high << 16)
        })
        .collect()
}

const SPU_REGISTER_COUNT: usize = ((SPU_REGION_END - SPU_REGION_START).div_ceil(2)) as usize;

#[derive(Clone, Debug)]
pub struct Spu {
    registers: [u16; SPU_REGISTER_COUNT],
}

impl Default for Spu {
    fn default() -> Self {
        Self {
            registers: [0; SPU_REGISTER_COUNT],
        }
    }
}

impl Spu {
    fn read_u32(&self, address: u32) -> u32 {
        let index = spu_index(address);
        self.registers[index] as u32
    }

    fn write_u32(&mut self, address: u32, value: u32) {
        let index = spu_index(address);
        self.registers[index] = value as u16;
    }
}

fn spu_index(address: u32) -> usize {
    (((address - SPU_REGION_START) / 2) as usize).min(255)
}

fn clear_if(state: &mut u16, mask: u16, pressed: bool) {
    if pressed {
        *state &= !mask;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ACCESS_WIDTH_16, ACCESS_WIDTH_32, Controller, DISPLAY_RESOLVE_MIN_IMAGE_UPLOAD_COMMANDS,
        DISPLAY_RESOLVE_MIN_TEXTURED_RECT_COMMANDS, DISPLAY_RESOLVE_MIN_TEXTURED_TRIANGLE_COMMANDS,
        DISPLAY_RESOLVE_MIN_TEXTURED_WRITTEN_PIXELS, DMA_GPU_CHCR, DMA_INTERRUPT, DrawBounds,
        GP0_BEST_OBSERVATION_DRAW_INTERVAL, GP0_BEST_OBSERVATION_EAGER_COMMANDS, GPU_GP0, GPU_GP1,
        GpuDrawTrace, IO_REGISTER_MAP, IRQ_CONTROLLER, IRQ_MASK, IRQ_STATUS, Io, IoAccess,
        IoDevice, MDEC_COMMAND, MDEC_STATUS, SIO_CONTROL, SIO_DATA, SIO_STATUS_IRQ_REQUEST,
        SPU_REGION_START, VRAM_HEIGHT, VRAM_WIDTH, has_native_full_scene_detail, io_register,
        io_register_range, is_detailed_observation, is_io_register_address, is_sparse_display,
        resolved_display_candidate_has_scene_signal, screen_observation_score,
        screen_observation_worth_saving,
    };
    use crate::native::framebuffer::{
        FrameBufferStats, FrameBufferWindow, Point, TexturedDrawStats,
    };

    #[test]
    fn register_metadata_identifies_core_io_devices_and_access() {
        let irq_status = io_register(IRQ_STATUS).expect("IRQ status metadata");
        assert_eq!(irq_status.name, "IRQ_STATUS");
        assert_eq!(irq_status.device, IoDevice::InterruptController);
        assert_eq!(irq_status.access, IoAccess::ReadWrite);
        assert!(irq_status.supports_width(ACCESS_WIDTH_16));
        assert!(irq_status.supports_width(ACCESS_WIDTH_32));

        let gpu_control = io_register(GPU_GP1).expect("GPU GP1 metadata");
        assert_eq!(gpu_control.device, IoDevice::Gpu);
        assert!(gpu_control.access.readable());
        assert!(gpu_control.access.writable());
        assert!(gpu_control.supports_width(ACCESS_WIDTH_32));

        let dma_channel_control = io_register(DMA_GPU_CHCR).expect("GPU DMA CHCR metadata");
        assert_eq!(dma_channel_control.device, IoDevice::Dma);
        assert_eq!(
            dma_channel_control.description,
            "DMA channel 2 channel control"
        );

        assert!(io_register(0x1f80_1fff).is_none());
    }

    fn mark_gpu_as_having_live_textured_playfield(io: &mut Io, width: usize, height: usize) {
        io.gpu.commands_seen = 1;
        io.gpu.image_upload_commands = DISPLAY_RESOLVE_MIN_IMAGE_UPLOAD_COMMANDS;
        io.gpu.textured_triangle_commands = DISPLAY_RESOLVE_MIN_TEXTURED_TRIANGLE_COMMANDS;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: DISPLAY_RESOLVE_MIN_TEXTURED_WRITTEN_PIXELS,
            color_changes: 32,
            ..TexturedDrawStats::default()
        };
        io.gpu.overlap_draw_commands.push(GpuDrawTrace::textured(
            "textured_quad",
            0,
            0,
            DrawBounds {
                left: 0,
                top: 64,
                right: width as i32,
                bottom: height as i32,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 64, y: 128 }],
            None,
        ));
    }

    fn fill_multicolor_scene(
        io: &mut Io,
        x_offset: usize,
        y_offset: usize,
        width: usize,
        height: usize,
    ) {
        for x in 0..width {
            let color = match (x / 4) % 8 {
                0 => 0x00ff_ffff,
                1 => 0x0020_80e0,
                2 => 0x00e0_6040,
                3 => 0x0030_b060,
                4 => 0x0060_60d0,
                5 => 0x00f0_b050,
                6 => 0x0040_d0c0,
                _ => 0x0090_7040,
            };
            io.gpu.framebuffer.fill_rect_unclipped(
                x_offset.saturating_add(x) as i32,
                y_offset as i32,
                1,
                height as i32,
                color,
            );
        }
    }

    fn fill_intro_caption_scene(
        io: &mut Io,
        x_offset: usize,
        y_offset: usize,
        width: usize,
        height: usize,
    ) {
        fill_multicolor_scene(io, x_offset, y_offset, width, height);
        let band_start = y_offset + height * 4 / 5;
        io.gpu.framebuffer.fill_rect_unclipped(
            x_offset as i32,
            band_start as i32,
            width as i32,
            (height - height * 4 / 5) as i32,
            0,
        );
        for y in y_offset + height * 9 / 10..(y_offset + height * 9 / 10 + 8).min(y_offset + height)
        {
            for x in x_offset + width / 5..x_offset + width * 4 / 5 {
                if (x + y) % 6 < 3 {
                    io.gpu
                        .framebuffer
                        .fill_rect_unclipped(x as i32, y as i32, 1, 1, 0x00ef_efef);
                }
            }
        }
    }

    #[test]
    fn register_metadata_is_unique_and_inside_io_region() {
        for (index, register) in IO_REGISTER_MAP.iter().enumerate() {
            assert!(
                is_io_register_address(register.address),
                "{} address {:#010x} is outside IO region",
                register.name,
                register.address
            );

            for previous in &IO_REGISTER_MAP[..index] {
                assert_ne!(
                    register.address, previous.address,
                    "{} duplicates {} at {:#010x}",
                    register.name, previous.name, register.address
                );
            }
        }
    }

    #[test]
    fn range_metadata_covers_dma_padding_and_spu_window() {
        let dma_interrupt = io_register(DMA_INTERRUPT).expect("DMA interrupt metadata");
        assert_eq!(dma_interrupt.device, IoDevice::Dma);

        let dma_padding = io_register_range(0x1f80_10fc).expect("DMA range metadata");
        assert_eq!(dma_padding.name, "DMA_CHANNEL_WINDOW");
        assert_eq!(dma_padding.device, IoDevice::Dma);
        assert!(dma_padding.access.writable());

        let spu = io_register_range(SPU_REGION_START).expect("SPU range metadata");
        assert_eq!(spu.device, IoDevice::Spu);
        assert!(spu.access.readable());
        assert!(spu.access_widths & ACCESS_WIDTH_16 != 0);
    }

    #[test]
    fn io_access_uses_named_register_map_addresses() {
        let mut io = Io::default();
        io.irq.status = 0xffff;

        io.write_u32(IRQ_STATUS, 0x00ff);
        io.write_u32(IRQ_MASK, 0x0101);
        io.write_u32(SIO_DATA, 0x1234);
        io.write_u32(GPU_GP1, 0x0300_0000);

        assert_eq!(io.read_u32(IRQ_STATUS), 0x00ff);
        assert_eq!(io.read_u32(IRQ_MASK), 0x0101);
        assert_eq!(io.controller.last_write, 0x1234);
        assert_eq!(io.gpu.status & (1 << 23), 0);
    }

    #[test]
    fn sio_data_write_raises_controller_irq_until_control_ack() {
        let mut io = Io::default();

        io.write_u8(SIO_DATA, 0x01);

        assert_eq!(io.irq.status & IRQ_CONTROLLER, IRQ_CONTROLLER);
        assert_eq!(
            io.controller.status & SIO_STATUS_IRQ_REQUEST,
            SIO_STATUS_IRQ_REQUEST
        );

        io.write_u16(SIO_CONTROL, 0x0010);

        assert_eq!(io.irq.status & IRQ_CONTROLLER, 0);
        assert_eq!(io.controller.status & SIO_STATUS_IRQ_REQUEST, 0);
    }

    #[test]
    fn controller_returns_zn_mcu_default_packet_when_selected() {
        let mut controller = Controller::default();

        controller.set_security_selects(false, false, false, false, true);
        controller.write_data(0);

        assert_eq!(controller.response, 0x1f);
        controller.write_data(0);
        assert_eq!(controller.response, 0x00);
        assert!(
            controller
                .zn_mcu_diagnostic_json()
                .contains("\"select_transitions\":1")
        );
    }

    #[test]
    fn controller_returns_zn_mcu_analog_and_trackball_headers() {
        let mut controller = Controller::default();

        controller.set_security_selects(false, false, true, false, true);
        controller.write_data(0);
        assert_eq!(controller.response, 0x8f);
        controller.write_data(0);
        assert_eq!(controller.response, 0xff);

        controller.set_security_selects(false, false, false, true, false);
        controller.set_security_selects(false, false, false, true, true);
        controller.write_data(0);
        assert_eq!(controller.response, 0x6f);
        controller.write_data(0);
        assert_eq!(controller.response, 0x00);
    }

    #[test]
    fn gpu_unmodeled_gp1_commands_preserve_ready_status() {
        let mut io = Io::default();

        io.write_u32(GPU_GP1, 0x1000_0000);
        assert_eq!(io.read_u32(GPU_GP1) & 0x0400_0000, 0x0400_0000);
        assert_eq!(io.gpu.status & 0xff00_0000, 0x1400_0000);
    }

    #[test]
    fn gpu_status_read_toggles_scanline_parity_bit() {
        let io = Io::default();

        let first = io.read_u32(GPU_GP1);
        let second = io.read_u32(GPU_GP1);

        assert_ne!(first & 0x8000_0000, second & 0x8000_0000);
        assert_eq!(first & 0x0400_0000, 0x0400_0000);
        assert_eq!(second & 0x0400_0000, 0x0400_0000);
    }

    #[test]
    fn gpu_display_dimensions_require_interlace_for_480_line_mode() {
        let mut io = Io::default();

        io.write_u32(GPU_GP1, 0x0800_0006);
        assert_eq!(io.gpu.display_dimensions(), (512, 240));

        io.write_u32(GPU_GP1, 0x0800_0026);
        assert_eq!(io.gpu.display_dimensions(), (512, 480));

        io.write_u32(GPU_GP1, 0x0704_0010);
        assert_eq!(io.gpu.display_dimensions(), (512, 240));
    }

    #[test]
    fn gpu_display_rgb_frame_keeps_cached_presented_frame_after_width_change() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0001);
        let (cached_width, cached_height) = io.gpu.display_dimensions();
        assert_eq!((cached_width, cached_height), (320, 240));

        let cached_len = cached_width * cached_height;
        io.gpu.presented_frame_rgb = Some(vec![0x00ff_00ff; cached_len]);
        io.gpu.presented_frame_width = cached_width;
        io.gpu.presented_frame_height = cached_height;
        io.gpu.presented_frame_window = Some(FrameBufferWindow {
            x: 0,
            y: 0,
            stats: FrameBufferStats {
                pixel_count: cached_len as u64,
                nonzero_pixels: cached_len as u64 / 2,
                bright_pixels: cached_len as u64 / 4,
                luma_sum: cached_len as u64 * 96,
                max_luma: 255,
                detail_edges: cached_len as u64 / 32,
                checksum: 0x1234_5678,
            },
        });

        io.write_u32(GPU_GP1, 0x0800_0006);
        let (frame_width, frame_height, frame) = io.gpu.display_rgb_frame();

        assert_eq!((frame_width, frame_height), (cached_width, cached_height));
        assert_eq!(frame, vec![0x00ff_00ff; cached_len]);
    }

    #[test]
    fn gpu_display_rgb_frame_preserves_promoted_presented_frame_size_after_width_change() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0001);
        let (cached_width, cached_height) = io.gpu.display_dimensions();
        let cached_len = cached_width * cached_height;
        let cached_rgb = vec![0x0040_80ff; cached_len];

        io.gpu.presented_frame_rgb = Some(cached_rgb.clone());
        io.gpu.presented_frame_width = cached_width;
        io.gpu.presented_frame_height = cached_height;
        io.gpu.presentation_captures = 1;
        io.gpu.presented_frame_capture_index = 1;
        io.gpu.presented_frame_window = Some(FrameBufferWindow {
            x: 0,
            y: 0,
            stats: FrameBufferStats {
                pixel_count: cached_len as u64,
                nonzero_pixels: cached_len as u64,
                bright_pixels: cached_len as u64 / 2,
                luma_sum: cached_len as u64 * 128,
                max_luma: 255,
                detail_edges: cached_len as u64 / 8,
                checksum: 0x1234_abcd,
            },
        });

        io.write_u32(GPU_GP1, 0x0800_0006);
        let (width, height) = io.gpu.display_dimensions();
        io.gpu.image_upload_commands = DISPLAY_RESOLVE_MIN_IMAGE_UPLOAD_COMMANDS;
        io.gpu.textured_rect_commands = DISPLAY_RESOLVE_MIN_TEXTURED_RECT_COMMANDS;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: DISPLAY_RESOLVE_MIN_TEXTURED_WRITTEN_PIXELS,
            color_changes: 64,
            ..TexturedDrawStats::default()
        };
        io.gpu.overlap_draw_commands.push(GpuDrawTrace::textured(
            "textured_quad",
            0,
            0,
            DrawBounds {
                left: 0,
                top: 64,
                right: width as i32,
                bottom: height as i32,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 0, y: 64 }],
            None,
        ));

        let resolved = io.gpu.display_resolve();
        let playability = io.gpu.native_playability_json();
        let (frame_width, frame_height, frame) = io.gpu.display_rgb_frame();
        let (actual_width, actual_height, actual_frame) = io.gpu.actual_display_rgb_frame();

        assert_eq!(resolved.source, "presented_frame", "{playability}");
        assert!(resolved.promoted, "{playability}");
        assert_eq!((frame_width, frame_height), (cached_width, cached_height));
        assert_eq!(frame, cached_rgb);
        assert_eq!((actual_width, actual_height), (cached_width, cached_height));
        assert_eq!(actual_frame, frame);
    }

    #[test]
    fn gpu_gp0_fill_rect_updates_screenshot() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0x0200_00ff);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0008_0008);

        assert!(io.gpu.screenshot_png_base64().starts_with("iVBORw0KGgo"));
    }

    #[test]
    fn gpu_gp0_e6_sets_mask_bit_on_drawn_pixels() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xe600_0001);
        io.write_u32(GPU_GP0, 0x0200_00ff);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0001);

        assert_eq!(io.gpu.framebuffer.raw_pixel(0, 0) & 0x8000, 0x8000);
        assert_eq!(io.gpu.framebuffer.raw_pixel(0, 0) & 0x7fff, 0x001f);
    }

    #[test]
    fn gpu_gp0_e6_check_mask_blocks_masked_pixel_overwrite() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xe600_0001);
        io.write_u32(GPU_GP0, 0x0200_00ff);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0001);

        io.write_u32(GPU_GP0, 0xe600_0002);
        io.write_u32(GPU_GP0, 0x0200_ff00);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0001);

        assert_eq!(io.gpu.framebuffer.raw_pixel(0, 0), 0x801f);
    }

    #[test]
    fn gpu_gp0_e6_check_mask_blocks_image_upload_and_vram_copy_overwrite() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xe600_0001);
        io.write_u32(GPU_GP0, 0x0200_00ff);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0001);

        io.write_u32(GPU_GP0, 0xe600_0000);
        io.write_u32(GPU_GP0, 0xa000_0000);
        io.write_u32(GPU_GP0, 0x0001_0000);
        io.write_u32(GPU_GP0, 0x0001_0001);
        io.write_u32(GPU_GP0, 0x03e0_03e0);

        io.write_u32(GPU_GP0, 0xe600_0002);
        io.write_u32(GPU_GP0, 0xa000_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0001);
        io.write_u32(GPU_GP0, 0x7fff_7fff);
        io.write_u32(GPU_GP0, 0x8000_0000);
        io.write_u32(GPU_GP0, 0x0001_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0001);

        assert_eq!(io.gpu.framebuffer.raw_pixel(0, 0), 0x801f);
    }

    #[test]
    fn gpu_gp0_recent_command_json_records_completed_commands() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xe100_0001);

        let json = io.json();
        assert!(json.contains("\"gpu_recent_gp0_commands\""));
        assert!(json.contains("\"opcode_hex\":\"0xe1\""));
        assert!(json.contains("\"value_hex\":\"0xe1000001\""));
    }

    #[test]
    fn gpu_records_display_area_history_and_candidate_windows() {
        let mut io = Io::default();

        io.write_u32(GPU_GP1, 0x0500_0000);
        io.write_u32(GPU_GP1, 0x0500_0200);

        let history = io.gpu.display_area_history_json();
        let diagnostics = io.gpu.window_diagnostics_json();
        let runtime_probe = io.runtime_probe_json();

        assert!(history.contains("\"value_hex\":\"0x000200\""));
        assert!(history.contains("\"x\":512"));
        assert!(history.contains("\"y\":0"));
        assert!(diagnostics.contains("\"candidates\""));
        assert!(diagnostics.contains("\"label\":\"current_display\""));
        assert!(runtime_probe.contains("\"gpu_window_diagnostics\""));
        assert!(runtime_probe.contains("\"gpu_display_area_history\""));
    }

    #[test]
    fn gpu_captures_presented_frame_on_display_area_flip() {
        let mut io = Io::default();

        io.write_u32(GPU_GP1, 0x0800_0001);
        for x in 0..320 {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(x, 240, 1, 240, color);
        }

        io.write_u32(GPU_GP1, 0x0503_c000);

        let presented = io.gpu.presented_frame_window.expect("presented frame");
        assert_eq!((presented.x, presented.y), (0, 240));
        assert_eq!(io.gpu.presentation_captures, 1);
        assert_eq!(io.gpu.presented_frame_capture_index, 1);
    }

    #[test]
    fn gpu_captures_presented_frame_on_vblank() {
        let mut io = Io::default();

        io.write_u32(GPU_GP1, 0x0800_0001);
        for x in 0..320 {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu.framebuffer.fill_rect_unclipped(x, 0, 1, 240, color);
        }

        io.gpu.capture_vblank_presented_frame();

        let presented = io.gpu.presented_frame_window.expect("presented frame");
        assert_eq!((presented.x, presented.y), (0, 0));
        assert_eq!(io.gpu.presentation_captures, 1);
        assert_eq!(io.gpu.presented_frame_capture_index, 1);
    }

    #[test]
    fn gpu_draw_capture_survives_gp1_reset() {
        let mut io = Io::default();

        io.gpu.set_draw_capture_range(1, 1);
        io.write_u32(GPU_GP1, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0200_00ff);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0008_0008);

        assert_eq!(io.gpu.draw_captures().len(), 1);
        assert_eq!(io.gpu.draw_captures()[0].sequence, 1);
    }

    #[test]
    fn gpu_screenshot_window_prefers_dense_backbuffer_when_display_is_sparse() {
        let mut io = Io::default();

        io.gpu.framebuffer.fill_rect_unclipped(0, 0, 4, 4, 0xff);
        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 240, 320, 240, 0x0000_ff00);
        io.write_u32(GPU_GP0, 0xe303_c000);

        let window = io.gpu.screenshot_window();

        assert_eq!(window.x, 0);
        assert_eq!(window.y, 240);
        assert!(window.stats.nonzero_pixels > 16);
    }

    #[test]
    fn gpu_screenshot_window_does_not_stop_at_sparse_drawing_area() {
        let mut io = Io::default();

        io.gpu.framebuffer.fill_rect_unclipped(0, 0, 4, 4, 0xff);
        io.gpu
            .framebuffer
            .fill_rect_unclipped(400, 240, 320, 240, 0x0000_ff00);

        let window = io.gpu.screenshot_window();

        assert_eq!(window.x, 400);
        assert_eq!(window.y, 240);
        assert!(window.stats.nonzero_pixels > 16);
    }

    #[test]
    fn gpu_screenshot_window_prefers_bright_backbuffer_over_dark_full_display() {
        let mut io = Io::default();

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, 320, 240, 0x0000_0008);
        io.gpu
            .framebuffer
            .fill_rect_unclipped(496, 256, 320, 240, 0x0000_ff00);

        let window = io.gpu.screenshot_window();

        assert_eq!(window.x, 496);
        assert_eq!(window.y, 256);
        assert_eq!(window.stats.bright_pixels, window.stats.pixel_count);
    }

    #[test]
    fn gpu_best_observation_hot_path_ignores_non_framebuffer_gp0_commands() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        io.gpu
            .framebuffer
            .fill_rect_unclipped(512, 16, width as i32, height as i32, 0x00ff_ffff);
        for _ in 0..GP0_BEST_OBSERVATION_EAGER_COMMANDS {
            io.write_u32(GPU_GP0, 0xe100_1000);
        }

        assert_eq!(io.gpu.best_observation_last_probe_command, 0);
        assert!(io.gpu.best_observation_window.is_none());
    }

    #[test]
    fn gpu_best_observation_hot_path_throttles_between_draw_probes() {
        let mut io = Io::default();
        io.gpu.commands_seen = GP0_BEST_OBSERVATION_EAGER_COMMANDS;
        io.gpu.best_observation_last_probe_command = io.gpu.commands_seen;
        io.gpu.best_observation_last_probe_draw_sequence = io.gpu.draw_sequence;

        for x in 0..(GP0_BEST_OBSERVATION_DRAW_INTERVAL - 1) {
            io.write_u32(GPU_GP0, 0x6800_00ff);
            io.write_u32(GPU_GP0, x as u32);
        }

        assert_eq!(
            io.gpu.best_observation_last_probe_command,
            GP0_BEST_OBSERVATION_EAGER_COMMANDS
        );

        io.write_u32(GPU_GP0, 0x6800_00ff);
        io.write_u32(GPU_GP0, GP0_BEST_OBSERVATION_DRAW_INTERVAL as u32);

        assert!(io.gpu.best_observation_last_probe_command > GP0_BEST_OBSERVATION_EAGER_COMMANDS);
    }

    #[test]
    fn gpu_screenshot_preserves_best_screen_observation_after_clear() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0x02ff_ffff);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x00f0_0140);
        io.write_u32(GPU_GP0, 0x0200_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0020_0020);
        let patterned_png = io.gpu.screenshot_png();

        io.write_u32(GPU_GP0, 0x0200_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x00f0_0140);

        let best = io
            .gpu
            .best_observation_window
            .expect("best screen observation");
        assert_eq!(best.x, 0);
        assert_eq!(best.y, 0);
        assert!(best.stats.bright_pixels < best.stats.pixel_count);
        assert_eq!(io.gpu.screenshot_png(), patterned_png);
        assert_eq!(io.gpu.framebuffer_stats().bright_pixels, 0);
    }

    #[test]
    fn gpu_presented_frame_capture_keeps_best_frame_before_later_sparse_clear() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        io.gpu.framebuffer.fill_rect_unclipped(
            0,
            0,
            (width / 2) as i32,
            height as i32,
            0x0000_8000,
        );
        io.gpu.framebuffer.fill_rect_unclipped(
            (width / 2) as i32,
            0,
            (width / 2) as i32,
            height as i32,
            0x0080_0000,
        );
        io.write_u32(GPU_GP0, 0x0200_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, ((height as u32) << 16) | width as u32);
        let best_presented_png = io.gpu.display_png();
        let best_presented = io
            .gpu
            .presented_frame_window
            .expect("initial presented frame");

        io.gpu.framebuffer.fill_rect_unclipped(0, 0, 4, 4, 0xff);
        io.write_u32(GPU_GP0, 0x0200_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, ((height as u32) << 16) | width as u32);

        assert_eq!(io.gpu.display_png(), best_presented_png);
        assert_eq!(io.gpu.presented_frame_window, Some(best_presented));
    }

    #[test]
    fn gpu_display_prefers_best_presented_frame_over_sparse_current_display() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(x as i32, 0, 1, height as i32, color);
        }
        io.write_u32(GPU_GP0, 0x0200_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, ((height as u32) << 16) | width as u32);
        let best_presented_png = io.gpu.display_png();
        let best_presented = io.gpu.presented_frame_window.expect("best presented frame");
        assert!(is_detailed_observation(best_presented.stats));

        io.gpu.framebuffer.fill_rect_unclipped(
            (width - 64) as i32,
            (height - 20) as i32,
            56,
            12,
            0x00ff_ffff,
        );
        assert!(screen_observation_worth_saving(
            io.gpu.current_display_window().stats
        ));
        assert_eq!(io.gpu.display_png(), best_presented_png);
        assert_eq!(
            io.gpu.display_rgb_frame().2,
            io.gpu
                .presented_frame_rgb
                .clone()
                .expect("presented rgb frame")
        );

        io.write_u32(GPU_GP0, 0x0200_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, ((height as u32) << 16) | width as u32);

        assert_eq!(io.gpu.display_png(), best_presented_png);
        assert_eq!(
            io.gpu.display_rgb_frame().2,
            io.gpu
                .presented_frame_rgb
                .clone()
                .expect("presented rgb frame")
        );
        assert_eq!(io.gpu.presented_frame_window, Some(best_presented));
    }

    #[test]
    fn gpu_screenshot_prefers_brighter_best_observation_over_dark_presented_frame() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        io.gpu.framebuffer.fill_rect_unclipped(
            512,
            16,
            (width / 2) as i32,
            height as i32,
            0x0000_7fff,
        );
        io.gpu.framebuffer.fill_rect_unclipped(
            512 + (width / 2) as i32,
            16,
            (width / 2) as i32,
            height as i32,
            0x0000_03e0,
        );
        io.gpu.capture_best_observation();
        let best_png = io.gpu.screenshot_png();
        let best_window = io.gpu.best_observation_window.expect("best observation");

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, height as i32, 0x0000_0841);
        let dark_stats = io.gpu.framebuffer.display_stats(0, 0, width, height);
        io.gpu.presented_frame_png = Some(io.gpu.framebuffer.png(0, 0, width, height));
        io.gpu.presented_frame_window = Some(FrameBufferWindow {
            x: 0,
            y: 0,
            stats: dark_stats,
        });

        assert_eq!(io.gpu.best_observation_window, Some(best_window));
        assert_eq!(io.gpu.screenshot_png(), best_png);
        assert_eq!(io.gpu.display_png(), best_png);
    }

    #[test]
    fn gpu_screenshot_prefers_current_display_over_rich_backbuffer_when_valid() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        io.gpu
            .framebuffer
            .fill_rect_unclipped(512, 8, width as i32, height as i32, 0x0000_7fff);
        io.gpu
            .framebuffer
            .fill_rect_unclipped(640, 96, 96, 64, 0x00ff_7f00);
        io.gpu.capture_best_observation();
        let best_png = io.gpu.screenshot_png();

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, height as i32, 0);
        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(x as i32, 0, 1, height as i32, color);
        }
        let current_stats = io.gpu.framebuffer.display_stats(0, 0, width, height);
        assert!(is_detailed_observation(current_stats));
        io.gpu.presented_frame_png = Some(io.gpu.framebuffer.png(0, 0, width, height));
        io.gpu.presented_frame_window = Some(FrameBufferWindow {
            x: 0,
            y: 0,
            stats: current_stats,
        });

        let current_png = io.gpu.actual_display_png();
        assert!(io.gpu.should_prefer_current_display());
        assert_ne!(current_png, best_png);
        assert_eq!(io.gpu.screenshot_png(), current_png);
        assert_eq!(io.gpu.display_png(), current_png);
    }

    #[test]
    fn gpu_display_prefers_best_observation_over_sparse_current_and_presented() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(512 + x as i32, 16, 1, height as i32, color);
        }
        io.gpu.capture_best_observation();
        let best_png = io.gpu.screenshot_png();
        let best_window = io.gpu.best_observation_window.expect("best observation");
        assert!(is_detailed_observation(best_window.stats));

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, height as i32, 0);
        io.gpu.framebuffer.fill_rect_unclipped(
            (width - 64) as i32,
            (height - 20) as i32,
            56,
            12,
            0x00ff_ffff,
        );
        let sparse_stats = io.gpu.framebuffer.display_stats(0, 0, width, height);
        assert!(is_sparse_display(sparse_stats));
        io.gpu.presented_frame_png = Some(io.gpu.framebuffer.png(0, 0, width, height));
        io.gpu.presented_frame_window = Some(FrameBufferWindow {
            x: 0,
            y: 0,
            stats: sparse_stats,
        });

        assert!(!io.gpu.should_prefer_current_display());
        assert!(io.gpu.should_prefer_best_observation());
        assert_eq!(io.gpu.visible_display_window(), best_window);
        assert_eq!(io.gpu.screenshot_png(), best_png);
        assert_eq!(io.gpu.display_png(), best_png);
        assert_eq!(
            io.gpu.display_rgb_frame().2,
            io.gpu
                .best_observation_rgb
                .clone()
                .expect("best observation rgb frame")
        );
    }

    #[test]
    fn gpu_display_prefers_detailed_observation_over_dense_low_detail_current() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(512 + x as i32, 8, 1, height as i32, color);
        }
        io.gpu.capture_best_observation();
        let best_png = io.gpu.display_png();
        let best_window = io.gpu.best_observation_window.expect("best observation");
        assert!(is_detailed_observation(best_window.stats));

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, height as i32, 0x0010_1010);
        io.gpu.framebuffer.fill_rect_unclipped(
            (width - 80) as i32,
            (height - 24) as i32,
            72,
            16,
            0x00ff_ffff,
        );
        let current = io.gpu.current_display_window();
        assert!(!is_detailed_observation(current.stats));
        assert!(!io.gpu.should_prefer_current_display());
        assert!(io.gpu.should_prefer_best_observation());
        assert_eq!(io.gpu.visible_display_window(), best_window);
        assert_eq!(io.gpu.display_png(), best_png);
        assert_eq!(
            io.gpu.display_rgb_frame().2,
            io.gpu
                .best_observation_rgb
                .clone()
                .expect("best observation rgb frame")
        );
    }

    #[test]
    fn gpu_display_rgb_frame_rejects_low_detail_field_pair_for_best_observation() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        let (width, height) = io.gpu.display_dimensions();
        let best_x = 400usize;

        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu.framebuffer.fill_rect_unclipped(
                best_x as i32 + x as i32,
                0,
                1,
                height as i32,
                color,
            );
        }
        io.write_u32(GPU_GP1, 0x0500_0000 | best_x as u32);
        io.gpu.capture_best_observation();
        let best_rgb = io
            .gpu
            .best_observation_rgb
            .clone()
            .expect("best observation rgb frame");
        let best_window = io.gpu.best_observation_window.expect("best observation");
        assert_eq!(best_window.x, best_x);
        assert!(!io.gpu.display_window_is_texture_atlas(best_window));

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, height as i32, 0x0080_0000);
        io.gpu.framebuffer.fill_rect_unclipped(
            0,
            height as i32,
            width as i32,
            height as i32,
            0x00b0_8070,
        );
        io.gpu
            .framebuffer
            .fill_rect_unclipped(360, height as i32 + 220, 48, 8, 0x00ff_ffff);
        for y in 8..28 {
            for x in (0..width).step_by(8) {
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(x as i32, y, 4, 1, 0x00ff_ffff);
            }
        }
        io.write_u32(GPU_GP1, 0x0500_0000 | ((height as u32) << 10));
        io.write_u32(GPU_GP1, 0x0500_0000);

        let (frame_width, frame_height, frame) = io.gpu.display_rgb_frame();

        assert_eq!((frame_width, frame_height), (width, height));
        assert!(
            frame == best_rgb,
            "display_rgb_frame should keep detailed best observation instead of low-detail current field pair"
        );
        assert!(
            !io.gpu
                .should_use_field_composed_output(io.gpu.current_display_output_window())
        );
    }

    #[test]
    fn native_playability_accepts_multicolor_live_playfield_candidate() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        for y in 0..height {
            for x in 0..width {
                let color = match ((x / 5) + (y / 7)) % 6 {
                    0 => 0x00f8_e0c0,
                    1 => 0x0020_80e0,
                    2 => 0x00f0_f8ff,
                    3 => 0x0040_c060,
                    4 => 0x00d0_3040,
                    _ => 0x0030_2018,
                };
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(x as i32, y as i32, 1, 1, color);
            }
        }
        mark_gpu_as_having_live_textured_playfield(&mut io, width, height);

        let current = io.gpu.current_display_window();
        let playability = io.gpu.native_playability_json();

        assert!(has_native_full_scene_detail(current.stats), "{playability}");
        assert!(io.gpu.native_playable_candidate(), "{playability}");
        assert!(playability.contains("\"has_actual_color_diversity\":true"));
        assert!(playability.contains("\"has_scene_color_diversity\":true"));
    }

    #[test]
    fn native_playability_rejects_intro_caption_band_candidate() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        for y in 0..height {
            for x in 0..width {
                let color = match ((x / 5) + (y / 7)) % 6 {
                    0 => 0x00f8_e0c0,
                    1 => 0x0020_80e0,
                    2 => 0x00f0_f8ff,
                    3 => 0x0040_c060,
                    4 => 0x00d0_3040,
                    _ => 0x0030_2018,
                };
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(x as i32, y as i32, 1, 1, color);
            }
        }
        for y in height * 4 / 5..height {
            for x in 0..width {
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(x as i32, y as i32, 1, 1, 0);
            }
        }
        for y in height * 9 / 10..(height * 9 / 10 + 8).min(height) {
            for x in (width / 5)..(width * 4 / 5) {
                if (x + y) % 6 < 3 {
                    io.gpu
                        .framebuffer
                        .fill_rect_unclipped(x as i32, y as i32, 1, 1, 0x00ef_efef);
                }
            }
        }
        mark_gpu_as_having_live_textured_playfield(&mut io, width, height);

        let playability = io.gpu.native_playability_json();

        assert!(!io.gpu.native_playable_candidate(), "{playability}");
        assert!(playability.contains("\"has_actual_intro_caption_band\":true"));
        assert!(playability.contains("\"has_scene_intro_caption_band\":true"));
        assert!(
            playability.contains("\"classification\":\"actual_display_intro_caption_band\""),
            "{playability}"
        );
    }

    #[test]
    fn native_playability_allows_caption_like_playfield_ui_with_strong_3d_signal() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        for y in 0..height {
            for x in 0..width {
                let color = match ((x / 5) + (y / 7)) % 6 {
                    0 => 0x00f8_e0c0,
                    1 => 0x0020_80e0,
                    2 => 0x00f0_f8ff,
                    3 => 0x0040_c060,
                    4 => 0x00d0_3040,
                    _ => 0x0030_2018,
                };
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(x as i32, y as i32, 1, 1, color);
            }
        }
        for y in height * 4 / 5..height {
            for x in 0..width {
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(x as i32, y as i32, 1, 1, 0);
            }
        }
        for y in height * 9 / 10..(height * 9 / 10 + 8).min(height) {
            for x in (width / 5)..(width * 4 / 5) {
                if (x + y) % 6 < 3 {
                    io.gpu
                        .framebuffer
                        .fill_rect_unclipped(x as i32, y as i32, 1, 1, 0x00ef_efef);
                }
            }
        }
        mark_gpu_as_having_live_textured_playfield(&mut io, width, height);
        io.gpu.textured_triangle_commands = DISPLAY_RESOLVE_MIN_TEXTURED_TRIANGLE_COMMANDS * 8;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: DISPLAY_RESOLVE_MIN_TEXTURED_WRITTEN_PIXELS * 32,
            color_changes: 32_768,
            ..TexturedDrawStats::default()
        };

        let playability = io.gpu.native_playability_json();

        assert!(io.gpu.native_playable_candidate(), "{playability}");
        assert!(playability.contains("\"has_actual_intro_caption_band\":true"));
        assert!(playability.contains("\"has_scene_intro_caption_band\":true"));
        assert!(playability.contains("\"caption_band_ui_allowed\":true"));
        assert!(playability.contains("\"classification\":\"native_playable_candidate\""));
    }

    #[test]
    fn native_playability_rejects_bright_intro_caption_panel_candidate() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        for y in 0..height {
            for x in 0..width {
                let color = match ((x / 5) + (y / 7)) % 6 {
                    0 => 0x00f8_e0c0,
                    1 => 0x0020_80e0,
                    2 => 0x00f0_f8ff,
                    3 => 0x0040_c060,
                    4 => 0x00d0_3040,
                    _ => 0x0030_2018,
                };
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(x as i32, y as i32, 1, 1, color);
            }
        }
        for y in height * 4 / 5..height {
            for x in 0..width {
                let color = if (x + y) % 20 == 0 { 0 } else { 0x00f0_f0e8 };
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(x as i32, y as i32, 1, 1, color);
            }
        }
        mark_gpu_as_having_live_textured_playfield(&mut io, width, height);

        let playability = io.gpu.native_playability_json();

        assert!(!io.gpu.native_playable_candidate(), "{playability}");
        assert!(playability.contains("\"has_actual_intro_caption_band\":true"));
        assert!(playability.contains("\"has_scene_intro_caption_band\":true"));
        assert!(
            playability.contains("\"classification\":\"actual_display_intro_caption_band\""),
            "{playability}"
        );
    }

    #[test]
    fn native_playability_rejects_dark_character_versus_screen_candidate() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        for y in height / 8..height * 7 / 8 {
            for x in width / 16..width * 7 / 16 {
                let color = match (x + y) % 5 {
                    0 => 0x00e8_a860,
                    1 => 0x0088_4028,
                    2 => 0x00f0_d8b0,
                    3 => 0x0058_3030,
                    _ => 0x00c8_7038,
                };
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(x as i32, y as i32, 1, 1, color);
            }
            for x in width * 9 / 16..width * 15 / 16 {
                let color = match (x.saturating_mul(3) + y) % 5 {
                    0 => 0x00d8_e8f0,
                    1 => 0x0048_8098,
                    2 => 0x00a8_b8c8,
                    3 => 0x0030_4058,
                    _ => 0x0088_9078,
                };
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(x as i32, y as i32, 1, 1, color);
            }
        }
        mark_gpu_as_having_live_textured_playfield(&mut io, width, height);

        let playability = io.gpu.native_playability_json();

        assert!(!io.gpu.native_playable_candidate(), "{playability}");
        assert!(playability.contains("\"has_actual_gameplay_profile\":false"));
        assert!(
            playability.contains("\"classification\":\"actual_display_not_gameplay_profile\""),
            "{playability}"
        );
    }

    #[test]
    fn native_playability_rejects_red_transition_as_playfield_candidate() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        for y in 0..height {
            for x in 0..width {
                let color = if ((x / 2) + (y / 2)) % 2 == 0 {
                    0x00ff_6028
                } else {
                    0x00c0_4818
                };
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(x as i32, y as i32, 1, 1, color);
            }
        }
        mark_gpu_as_having_live_textured_playfield(&mut io, width, height);

        let current = io.gpu.current_display_window();
        let playability = io.gpu.native_playability_json();

        assert!(has_native_full_scene_detail(current.stats), "{playability}");
        assert!(!io.gpu.native_playable_candidate(), "{playability}");
        assert!(playability.contains("\"has_actual_color_diversity\":false"));
        assert!(
            playability.contains("\"classification\":\"actual_display_low_color_diversity\""),
            "{playability}"
        );
    }

    #[test]
    fn native_playability_rejects_red_white_black_noise_as_playfield_candidate() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();
        let palette = [
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0010_0000,
            0x0030_0000,
            0x0050_0808,
            0x0070_1010,
            0x0090_1818,
            0x00b0_2020,
            0x00d0_2828,
            0x00f0_3030,
            0x00ff_4040,
            0x0040_4040,
            0x0080_8080,
            0x00c0_c0c0,
            0x00ff_ffff,
        ];

        for y in 0..height {
            for x in 0..width {
                let index = (x.saturating_mul(13) + y.saturating_mul(17)) % palette.len();
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(x as i32, y as i32, 1, 1, palette[index]);
            }
        }
        mark_gpu_as_having_live_textured_playfield(&mut io, width, height);

        let current = io.gpu.current_display_window();
        let playability = io.gpu.native_playability_json();

        assert!(has_native_full_scene_detail(current.stats), "{playability}");
        assert!(!io.gpu.native_playable_candidate(), "{playability}");
        assert!(playability.contains("\"has_actual_color_diversity\":false"));
        assert!(
            playability.contains("\"classification\":\"actual_display_low_color_diversity\""),
            "{playability}"
        );
    }

    #[test]
    fn gpu_display_prefers_current_detailed_display_over_offscreen_best_observation() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(512 + x as i32, 0, 1, height as i32, color);
        }
        io.gpu.capture_best_observation();
        let best_window = io.gpu.best_observation_window.expect("best observation");
        assert_eq!(best_window.x, 512);

        for x in 0..width {
            let color = if x % 8 < 4 { 0x00d0_1010 } else { 0x0010_d0f0 };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(x as i32, 0, 1, height as i32, color);
        }
        let current_png = io.gpu.actual_display_png();
        let current = io.gpu.current_display_window();

        assert!(is_detailed_observation(current.stats));
        assert_ne!(current.x, best_window.x);
        assert_eq!(io.gpu.display_png(), current_png);
        assert_eq!(io.gpu.screenshot_png(), current_png);
    }

    #[test]
    fn gpu_display_prefers_current_full_frame_over_previous_presented_frame() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, height as i32, 0x0018_1820);
        io.gpu.framebuffer.fill_rect_unclipped(
            (width / 4) as i32,
            (height / 4) as i32,
            (width / 2) as i32,
            (height / 2) as i32,
            0x00ff_d040,
        );
        let detailed_png = io.gpu.framebuffer.png(0, 0, width, height);
        let detailed_stats = io.gpu.framebuffer.display_stats(0, 0, width, height);
        io.gpu.presented_frame_png = Some(detailed_png.clone());
        io.gpu.presented_frame_window = Some(FrameBufferWindow {
            x: 0,
            y: 0,
            stats: detailed_stats,
        });

        io.gpu.framebuffer.fill_rect_unclipped(
            0,
            0,
            width as i32,
            (height / 2) as i32,
            0x0060_a0c0,
        );
        io.gpu.framebuffer.fill_rect_unclipped(
            0,
            (height / 2) as i32,
            width as i32,
            (height / 2) as i32,
            0x0060_5850,
        );
        io.gpu.capture_best_observation();

        let current_png = io.gpu.actual_display_png();
        assert!(io.gpu.should_prefer_current_display());
        assert_ne!(current_png, detailed_png);
        assert_eq!(io.gpu.display_png(), current_png);
        assert_eq!(io.gpu.screenshot_png(), current_png);
    }

    #[test]
    fn gpu_resolves_live_candidate_page_when_gp1_display_is_sparse() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, height as i32, 0x0000_0810);
        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, 24, 8, 0x00ff_ffff);
        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(512 + x as i32, 0, 1, height as i32, color);
        }

        io.gpu.commands_seen = 1;
        io.gpu.image_upload_commands = 32;
        io.gpu.textured_triangle_commands = 512;
        io.gpu.presentation_captures = 1;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: 16_384,
            color_changes: 32,
            ..TexturedDrawStats::default()
        };
        io.gpu.push_top_draw_command(GpuDrawTrace::textured(
            "textured_quad",
            0,
            0,
            DrawBounds {
                left: 576,
                top: 128,
                right: 704,
                bottom: 256,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 576, y: 128 }],
            None,
        ));

        let resolved = io.gpu.display_resolve();
        let playability = io.gpu.native_playability_json();

        assert!(resolved.promoted, "{playability}");
        assert_eq!(resolved.source, "page_512_0");
        assert_eq!(resolved.window.x, 512);
        assert!(!io.gpu.native_playable_candidate(), "{playability}");
        assert!(playability.contains("\"actual_display_promoted\":true"));
        assert!(playability.contains("\"actual_display_is_live\":false"));
        assert!(playability.contains("\"classification\":\"candidate_not_live_actual\""));
    }

    #[test]
    fn gpu_resolves_sparse_live_text_candidate_when_gp1_display_is_black() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        let (width, height) = io.gpu.display_dimensions();
        assert_eq!((width, height), (512, 240));

        for column in 0..96 {
            let x = 128 + column * 3;
            let y = 340 + (column % 3) * 10;
            io.gpu
                .framebuffer
                .fill_rect_unclipped(x, y, 2, 8, 0x00ff_ffff);
        }

        io.gpu.commands_seen = 1;
        io.gpu.image_upload_commands = 18;
        io.gpu.textured_triangle_commands = 512;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: 16_384,
            color_changes: 32,
            ..TexturedDrawStats::default()
        };
        io.gpu.push_top_draw_command(GpuDrawTrace::textured(
            "textured_triangle",
            0,
            0,
            DrawBounds {
                left: 128,
                top: 304,
                right: 416,
                bottom: 392,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 128, y: 304 }],
            None,
        ));

        let current = io.gpu.current_display_window();
        let candidate = FrameBufferWindow {
            x: 0,
            y: height,
            stats: io.gpu.framebuffer.display_stats(0, height, width, height),
        };
        let current_score = screen_observation_score(current.stats);
        let reason = io
            .gpu
            .display_candidate_resolution_reason(current, current_score, candidate);
        let resolved = io.gpu.display_resolve();
        let playability = io.gpu.native_playability_json();
        let (_, _, actual_frame) = io.gpu.actual_display_rgb_frame();

        assert_eq!(reason, "valid", "{playability}");
        assert!(io.gpu.display_candidate_has_live_draw_overlap(candidate));
        assert!(resolved.promoted, "{playability}");
        assert_eq!(resolved.source, "page_0_240");
        assert_eq!((resolved.window.x, resolved.window.y), (0, height));
        assert!(actual_frame.iter().any(|pixel| *pixel != 0));
        assert!(playability.contains("\"actual_display_promoted\":true"));
        assert!(playability.contains("\"actual_display_is_live\":false"));
    }

    #[test]
    fn gpu_resolves_observation_candidate_with_partial_upload_history() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, 24, 8, 0x00ff_ffff);
        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(512 + x as i32, 0, 1, height as i32, color);
        }

        io.gpu.commands_seen = 1;
        io.gpu.image_upload_commands = 26;
        io.gpu.textured_triangle_commands = 512;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: 16_384,
            color_changes: 32,
            ..TexturedDrawStats::default()
        };
        io.gpu.overlap_draw_commands.push(GpuDrawTrace::textured(
            "textured_quad",
            0,
            0,
            DrawBounds {
                left: 576,
                top: 128,
                right: 704,
                bottom: 256,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 576, y: 128 }],
            None,
        ));

        let resolved = io.gpu.display_resolve();
        let playability = io.gpu.native_playability_json();

        assert!(resolved.promoted, "{playability}");
        assert_eq!(resolved.source, "page_512_0");
        assert_eq!(resolved.window.x, 512);
        assert!(!io.gpu.native_playable_candidate(), "{playability}");
        assert!(playability.contains("\"minimum_image_upload_commands\":16"));
        assert!(playability.contains("\"image_upload_gate_passed\":true"));
        assert!(playability.contains("\"classification\":\"candidate_not_live_actual\""));
    }

    #[test]
    fn gpu_resolves_uploaded_scene_candidate_without_live_draw_overlap() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, 24, 8, 0x00ff_ffff);
        let filled_height = (height * 2 / 3) as i32;
        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(512 + x as i32, 0, 1, filled_height, color);
        }
        io.gpu
            .push_image_upload_rect(512, 0, (width / 4) as i32, filled_height);

        io.gpu.commands_seen = 1;
        io.gpu.image_upload_commands = 26;
        io.gpu.textured_triangle_commands = 512;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: 16_384,
            color_changes: 32,
            ..TexturedDrawStats::default()
        };
        io.gpu.overlap_draw_commands.push(GpuDrawTrace::textured(
            "textured_quad",
            0,
            0,
            DrawBounds {
                left: 64,
                top: 128,
                right: 192,
                bottom: 256,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 64, y: 128 }],
            None,
        ));

        let resolved = io.gpu.display_resolve();
        let playability = io.gpu.native_playability_json();
        let (frame_width, frame_height, frame) = io.gpu.display_rgb_frame();
        let (actual_frame_width, actual_frame_height, actual_frame) =
            io.gpu.actual_display_rgb_frame();

        assert!(resolved.promoted, "{playability}");
        assert_eq!(resolved.source, "page_512_0");
        assert_eq!(resolved.window.x, 512);
        assert!(!io.gpu.display_window_is_texture_atlas(resolved.window));
        assert!(
            !io.gpu
                .display_candidate_has_live_draw_overlap(resolved.window)
        );
        assert!(
            io.gpu
                .display_candidate_has_scene_upload_overlap(resolved.window)
        );
        assert!(
            !io.gpu.should_present_resolved_display(resolved),
            "{playability}"
        );
        assert!(!io.gpu.native_playable_candidate(), "{playability}");
        assert_eq!((frame_width, frame_height), (width, height));
        assert_eq!(
            frame,
            io.gpu
                .framebuffer
                .psx_display_rgb_window(0, 0, width, height)
        );
        assert_eq!((actual_frame_width, actual_frame_height), (width, height));
        assert_eq!(
            actual_frame,
            io.gpu
                .framebuffer
                .psx_display_rgb_window(0, 0, width, height),
            "GUI-facing actual display should keep the live GP1 output until a resolved candidate is presentable"
        );
        assert_eq!(
            io.gpu.actual_display_png(),
            io.gpu.framebuffer.psx_display_png(0, 0, width, height)
        );
        assert!(playability.contains("\"scene_upload_overlap\":true"));
        assert!(playability.contains("\"classification\":\"candidate_not_live_actual\""));
    }

    #[test]
    fn gpu_actual_display_keeps_warning_text_over_nonpresentable_resolved_candidate() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, height as i32, 0);
        for word in 0..2 {
            let base_y = 32 + word * 42;
            for letter in 0..7 {
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(64 + letter * 28, base_y, 8, 3, 0x00ff_ffff);
            }
        }
        let filled_height = (height * 2 / 3) as i32;
        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(512 + x as i32, 0, 1, filled_height, color);
        }
        io.gpu
            .push_image_upload_rect(512, 0, (width / 4) as i32, filled_height);

        io.gpu.commands_seen = 1;
        io.gpu.image_upload_commands = 26;
        io.gpu.textured_triangle_commands = 512;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: 16_384,
            color_changes: 32,
            ..TexturedDrawStats::default()
        };
        io.gpu.overlap_draw_commands.push(GpuDrawTrace::textured(
            "textured_quad",
            0,
            0,
            DrawBounds {
                left: 64,
                top: 128,
                right: 192,
                bottom: 256,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 64, y: 128 }],
            None,
        ));

        let resolved = io.gpu.display_resolve();
        let playability = io.gpu.native_playability_json();
        let (_, _, actual_frame) = io.gpu.actual_display_rgb_frame();
        let live_warning_frame = io
            .gpu
            .framebuffer
            .psx_display_rgb_window(0, 0, width, height);
        let resolved_candidate_frame = io.gpu.framebuffer.rgb_window(512, 0, width, height);

        assert!(resolved.promoted, "{playability}");
        assert_eq!(resolved.source, "page_512_0");
        assert!(
            !io.gpu.should_present_resolved_display(resolved),
            "{playability}"
        );
        assert_eq!(actual_frame, live_warning_frame);
        assert_ne!(actual_frame, resolved_candidate_frame);
        assert_eq!(
            io.gpu.actual_display_png(),
            io.gpu.framebuffer.psx_display_png(0, 0, width, height)
        );
        assert_eq!(
            io.gpu.display_png(),
            io.gpu.framebuffer.psx_display_png(0, 0, width, height)
        );
    }

    #[test]
    fn gpu_rejects_non_live_texture_atlas_candidate_with_large_upload_overlap() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();
        let scene_x = 440usize;
        let scene_y = 256usize;

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, 24, 8, 0x00ff_ffff);
        for y in 0..height {
            for x in 0..width {
                let color = match ((x / 7) + (y / 5)) % 4 {
                    0 => 0x00e0_d0c0,
                    1 => 0x0020_70d0,
                    2 => 0x00f0_f8ff,
                    _ => 0x0040_3018,
                };
                io.gpu.framebuffer.fill_rect_unclipped(
                    scene_x as i32 + x as i32,
                    scene_y as i32 + y as i32,
                    1,
                    1,
                    color,
                );
            }
        }
        io.gpu
            .push_image_upload_rect(scene_x as i32, scene_y as i32, width as i32, height as i32);

        io.gpu.commands_seen = 1;
        io.gpu.image_upload_commands = 32;
        io.gpu.textured_triangle_commands = 512;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: 16_384,
            color_changes: 32,
            ..TexturedDrawStats::default()
        };
        io.gpu.overlap_draw_commands.push(GpuDrawTrace::textured(
            "textured_quad",
            0,
            0,
            DrawBounds {
                left: 64,
                top: 128,
                right: 192,
                bottom: 256,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 64, y: 128 }],
            None,
        ));

        let current = io.gpu.current_display_window();
        let candidate = FrameBufferWindow {
            x: scene_x,
            y: scene_y,
            stats: io
                .gpu
                .framebuffer
                .display_stats(scene_x, scene_y, width, height),
        };
        let resolved = io.gpu.display_resolve();
        let playability = io.gpu.native_playability_json();

        assert!(io.gpu.display_window_is_texture_atlas(candidate));
        assert!(
            !io.gpu.display_candidate_has_live_draw_overlap(candidate),
            "{playability}"
        );
        assert!(
            io.gpu.display_candidate_has_scene_upload_overlap(candidate),
            "{playability}"
        );
        assert_eq!(
            io.gpu.display_candidate_resolution_reason(
                current,
                screen_observation_score(current.stats),
                candidate
            ),
            "texture_atlas"
        );
        assert!(!resolved.promoted, "{playability}");
        assert!(playability.contains("\"scene_upload_overlap\":true"));
        assert!(playability.contains("\"classification\":\"actual_display_too_sparse\""));
    }

    #[test]
    fn gpu_does_not_resolve_texture_upload_atlas_page_as_display() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        let (width, height) = io.gpu.display_dimensions();
        assert_eq!((width, height), (512, 240));

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, height as i32, 0);
        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, 48, 8, 0x00ff_ffff);
        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(512 + x as i32, 0, 1, height as i32, color);
        }

        io.gpu
            .push_image_upload_rect(512, 0, width as i32, height as i32);
        io.gpu.commands_seen = 1;
        io.gpu.image_upload_commands = 32;
        io.gpu.textured_triangle_commands = 512;
        io.gpu.presentation_captures = 1;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: 16_384,
            color_changes: 32,
            ..TexturedDrawStats::default()
        };
        io.gpu.overlap_draw_commands.push(GpuDrawTrace::textured(
            "textured_quad",
            0,
            0,
            DrawBounds {
                left: 576,
                top: 128,
                right: 704,
                bottom: 256,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 576, y: 128 }],
            None,
        ));

        let resolved = io.gpu.display_resolve();
        let playability = io.gpu.native_playability_json();

        assert!(!resolved.promoted, "{playability}");
        assert_eq!(resolved.source, "gp1_display_area");
        assert!(!io.gpu.native_playable_candidate(), "{playability}");
        assert!(playability.contains("\"playable_candidate\":false"));
    }

    #[test]
    fn gpu_rejects_recent_streamed_upload_page_as_presented_display() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        io.write_u32(GPU_GP1, 0x0500_0000);
        let (width, height) = io.gpu.display_dimensions();
        assert_eq!((width, height), (512, 240));

        for y in 0..height {
            for x in 0..width {
                let color = match ((x / 5) + (y / 3)) % 5 {
                    0 => 0x0000_d0d0,
                    1 => 0x0000_4090,
                    2 => 0x00e0_2010,
                    3 => 0x0008_1820,
                    _ => 0x0000_a0f0,
                };
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(x as i32, y as i32, 1, 1, color);
            }
        }
        for x in (0..width).step_by(16) {
            io.gpu
                .push_image_upload_rect(x as i32, 0, 16, height as i32);
        }
        io.gpu.commands_seen = 1;
        io.gpu.image_upload_commands = 64;
        io.gpu.textured_triangle_commands = 512;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: 64_000,
            color_changes: 8_000,
            ..TexturedDrawStats::default()
        };

        let current = io.gpu.current_display_window();
        assert!(
            io.gpu.display_window_is_texture_atlas(current),
            "{}",
            io.gpu.native_playability_json()
        );

        io.gpu.capture_current_presented_frame();
        let resolved = io.gpu.display_resolve();
        let playability = io.gpu.native_playability_json();

        assert!(io.gpu.presented_frame_window.is_none(), "{playability}");
        assert!(!resolved.promoted, "{playability}");
        assert!(!io.gpu.native_playable_candidate(), "{playability}");
    }

    #[test]
    fn gpu_rejects_offset_texture_page_candidate_even_with_scene_signal() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        let (width, height) = io.gpu.display_dimensions();
        let candidate_x = 512usize;
        let candidate_y = 16usize;

        for y in 0..height {
            for x in 0..width {
                let color = match ((x / 4) + (y / 3)) % 4 {
                    0 => 0x00ff_ffff,
                    1 => 0x00d0_4010,
                    2 => 0x0020_60d0,
                    _ => 0x0008_0808,
                };
                io.gpu.framebuffer.fill_rect_unclipped(
                    candidate_x as i32 + x as i32,
                    candidate_y as i32 + y as i32,
                    1,
                    1,
                    color,
                );
            }
        }
        io.gpu.push_image_upload_rect(
            candidate_x as i32,
            candidate_y as i32,
            width as i32,
            height as i32,
        );
        io.gpu.commands_seen = 1;
        io.gpu.image_upload_commands = 32;
        io.gpu.textured_triangle_commands = 512;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: 16_384,
            color_changes: 32,
            ..TexturedDrawStats::default()
        };

        let current = io.gpu.current_display_window();
        let candidate = FrameBufferWindow {
            x: candidate_x,
            y: candidate_y,
            stats: io
                .gpu
                .framebuffer
                .display_stats(candidate_x, candidate_y, width, height),
        };
        let playability = io.gpu.native_playability_json();

        assert!(io.gpu.display_window_is_texture_atlas(candidate));
        assert!(resolved_display_candidate_has_scene_signal(
            current,
            screen_observation_score(current.stats),
            candidate
        ));
        assert_eq!(
            io.gpu.display_candidate_resolution_reason(
                current,
                screen_observation_score(current.stats),
                candidate
            ),
            "texture_atlas",
            "{playability}"
        );
    }

    #[test]
    fn gpu_rejects_multi_upload_texture_atlas_even_with_live_draw_overlap() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        let (width, height) = io.gpu.display_dimensions();

        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(320 + x as i32, 0, 1, height as i32, color);
        }
        io.gpu
            .push_image_upload_rect(320, 0, (width / 2) as i32, height as i32);
        io.gpu
            .push_image_upload_rect(576, 0, (width / 2) as i32, height as i32);
        io.gpu.commands_seen = 1;
        io.gpu.image_upload_commands = 32;
        io.gpu.textured_triangle_commands = 512;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: 16_384,
            color_changes: 32,
            ..TexturedDrawStats::default()
        };
        io.gpu.overlap_draw_commands.push(GpuDrawTrace::textured(
            "textured_quad",
            0,
            0,
            DrawBounds {
                left: 360,
                top: 32,
                right: 480,
                bottom: 160,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 360, y: 32 }],
            None,
        ));

        let candidate = FrameBufferWindow {
            x: 320,
            y: 0,
            stats: io.gpu.framebuffer.display_stats(320, 0, width, height),
        };

        assert!(is_detailed_observation(candidate.stats));
        assert!(io.gpu.display_candidate_has_live_draw_overlap(candidate));
        assert!(io.gpu.display_window_is_texture_atlas(candidate));
        assert_eq!(
            io.gpu.display_candidate_resolution_reason(
                io.gpu.current_display_window(),
                screen_observation_score(io.gpu.current_display_window().stats),
                candidate
            ),
            "texture_atlas"
        );
    }

    #[test]
    fn gpu_keeps_recent_gp1_field_pair_from_being_misclassified_as_texture_atlas() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        for x in 0..width {
            let color = match x % 16 {
                0..=3 => 0x00ff_ffff,
                4..=7 => 0x0000_2060,
                8..=11 => 0x00f0_6040,
                _ => 0x0008_1808,
            };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(x as i32, 0, 1, (height * 2) as i32, color);
        }
        io.gpu
            .framebuffer
            .fill_rect_unclipped(48, height as i32 + 96, 160, 48, 0x00f8_d080);
        for _ in 0..2 {
            io.gpu
                .push_image_upload_rect(0, 0, width as i32, height as i32);
            io.gpu
                .push_image_upload_rect(0, height as i32, width as i32, height as i32);
        }

        io.write_u32(GPU_GP1, 0x0500_0000 | ((height as u32) << 10));
        io.write_u32(GPU_GP1, 0x0500_0000);

        let output = io.gpu.current_display_output_window();
        let playability = io.gpu.native_playability_json();

        assert!(output.field_composed, "{playability}");
        assert_eq!((output.width, output.height), (width, height * 2));
        assert!(
            !io.gpu.display_window_is_texture_atlas_with_dimensions(
                output.window,
                output.width,
                output.height
            ),
            "{playability}"
        );
        assert!(
            io.gpu.should_use_field_composed_output(output),
            "{playability}"
        );
        assert!(
            playability.contains("\"actual_display_field_composed\":true"),
            "{playability}"
        );
    }

    #[test]
    fn gpu_keeps_recent_gp1_field_pair_with_many_uploads_from_texture_atlas_classification() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        fill_multicolor_scene(&mut io, 0, 0, width, height);
        fill_multicolor_scene(&mut io, 0, height, width, height);
        for y in (8..height.saturating_sub(8)).step_by(18) {
            io.gpu.framebuffer.fill_rect_unclipped(
                12,
                y as i32,
                width.saturating_sub(24) as i32,
                5,
                0x0018_2028,
            );
            io.gpu.framebuffer.fill_rect_unclipped(
                12,
                height.saturating_add(y) as i32,
                width.saturating_sub(24) as i32,
                5,
                0x0018_2028,
            );
        }
        for x in (0..width).step_by(16) {
            io.gpu
                .push_image_upload_rect(x as i32, 0, 16, (height * 2) as i32);
        }

        io.write_u32(GPU_GP1, 0x0500_0000 | ((height as u32) << 10));
        io.write_u32(GPU_GP1, 0x0500_0000);

        let output = io.gpu.current_display_output_window();
        let playability = io.gpu.native_playability_json();

        assert!(output.field_composed, "{playability}");
        assert_eq!((output.width, output.height), (width, height * 2));
        assert!(
            !io.gpu.display_window_is_texture_atlas_with_dimensions(
                output.window,
                output.width,
                output.height
            ),
            "{playability}"
        );
        assert!(
            io.gpu.should_use_field_composed_output(output),
            "{playability}"
        );

        io.gpu.capture_vblank_presented_frame();
        let playability = io.gpu.native_playability_json();

        assert!(io.gpu.field_composed_display_window.is_some());
        assert!(
            playability.contains("\"actual_display_source\":\"gp1_display_area_fields\"")
                || playability
                    .contains("\"actual_display_source\":\"cached_gp1_display_area_fields\""),
            "{playability}"
        );
        assert!(
            playability.contains("\"actual_display_field_composed\":true"),
            "{playability}"
        );
    }

    #[test]
    fn gpu_best_observation_ignores_texture_upload_atlas() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        let (width, height) = io.gpu.display_dimensions();

        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(512 + x as i32, 0, 1, height as i32, color);
        }
        io.gpu
            .push_image_upload_rect(512, 0, width as i32, height as i32);

        io.gpu.capture_best_observation();

        assert!(io.gpu.best_observation_window.is_none());
        assert_eq!(io.gpu.display_png(), io.gpu.actual_display_png());
    }

    #[test]
    fn gpu_resolves_dense_backbuffer_when_stale_current_display_has_hud_pixels() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        let (width, height) = io.gpu.display_dimensions();
        assert_eq!((width, height), (512, 240));

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, 283, 2, 0x00ff_ffff);
        let current = io.gpu.current_display_window();
        assert!(is_sparse_display(current.stats));
        io.gpu.presentation_captures = 612;
        io.gpu.presented_frame_capture_index = 1;
        io.gpu.presented_frame_window = Some(current);
        assert!(
            io.gpu
                .should_show_current_display_over_stale_candidates_with(current)
        );

        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(512 + x as i32, 24, 1, height as i32, color);
        }

        io.gpu.commands_seen = 295_567;
        io.gpu.image_upload_commands = 45;
        io.gpu.textured_triangle_commands = 18_104;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: 1_366_379,
            color_changes: 611_322,
            ..TexturedDrawStats::default()
        };
        io.gpu.overlap_draw_commands.push(GpuDrawTrace::textured(
            "textured_quad",
            0,
            0,
            DrawBounds {
                left: 576,
                top: 128,
                right: 736,
                bottom: 320,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 576, y: 128 }],
            None,
        ));

        let resolved = io.gpu.display_resolve();
        let playability = io.gpu.native_playability_json();
        let (frame_width, frame_height, frame) = io.gpu.display_rgb_frame();

        assert!(resolved.promoted, "{playability}");
        assert_eq!(resolved.window.x, 512);
        assert_eq!(resolved.window.y, 24);
        assert!(!io.gpu.native_playable_candidate(), "{playability}");
        assert_eq!((frame_width, frame_height), (width, height));
        assert_eq!(frame, io.gpu.framebuffer.rgb_window(512, 24, width, height));
        assert!(playability.contains("\"actual_display_promoted\":true"));
        assert!(playability.contains("\"actual_display_is_live\":false"));
        assert!(playability.contains("\"classification\":\"candidate_not_live_actual\""));
    }

    #[test]
    fn native_playability_rejects_cached_screen_when_actual_display_is_hud_only() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(512 + x as i32, 0, 1, height as i32, color);
        }
        io.gpu.capture_best_observation();
        let best_window = io.gpu.best_observation_window.expect("best observation");
        assert!(is_detailed_observation(best_window.stats));

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, height as i32, 0x0000_0810);
        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, 24, 8, 0x00ff_ffff);
        io.gpu.presented_frame_window = Some(best_window);
        io.gpu.presentation_captures = 1;
        io.gpu.commands_seen = 1;
        io.gpu.image_upload_commands = 1;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: 16_384,
            color_changes: 32,
            ..TexturedDrawStats::default()
        };
        io.gpu.overlap_draw_commands.push(GpuDrawTrace::textured(
            "textured_quad",
            0,
            0,
            DrawBounds {
                left: 64,
                top: 128,
                right: 192,
                bottom: 256,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 64, y: 128 }],
            None,
        ));

        let playability = io.gpu.native_playability_json();
        let resolved = io.gpu.display_resolve();

        assert!(!resolved.promoted, "{playability}");
        assert!(!io.gpu.native_playable_candidate());
        assert!(playability.contains("\"playable_candidate\":false"));
        assert!(
            playability.contains("\"classification\":\"actual_display_too_sparse\""),
            "{playability}"
        );
        assert!(playability.contains("\"has_actual_playfield\":false"));
    }

    #[test]
    fn gpu_display_drops_stale_presented_frame_after_later_visible_progress() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(x as i32, 0, 1, height as i32, color);
        }
        let stale_png = io.gpu.framebuffer.png(0, 0, width, height);
        let stale_stats = io.gpu.framebuffer.display_stats(0, 0, width, height);
        assert!(is_detailed_observation(stale_stats));
        io.gpu.presented_frame_png = Some(stale_png.clone());
        io.gpu.presented_frame_rgb = Some(io.gpu.framebuffer.rgb_window(0, 0, width, height));
        io.gpu.presented_frame_window = Some(FrameBufferWindow {
            x: 0,
            y: 0,
            stats: stale_stats,
        });
        io.gpu.presented_frame_capture_index = 1;
        io.gpu.presentation_captures = 16;

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, height as i32, 0);
        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, 24, 0x00ff_ffff);
        io.gpu.framebuffer.fill_rect_unclipped(
            (width - 96) as i32,
            (height - 24) as i32,
            88,
            16,
            0x00ff_ffff,
        );

        let current_png = io.gpu.framebuffer.png(0, 0, width, height);
        let resolved = io.gpu.display_resolve();

        assert!(!resolved.promoted);
        assert_ne!(current_png, stale_png);
        assert_eq!(io.gpu.actual_display_png(), current_png);
        assert_eq!(io.gpu.display_png(), current_png);
        assert_eq!(io.gpu.screenshot_png(), current_png);
    }

    #[test]
    fn gpu_display_rgb_frame_composes_recent_gp1_field_pair() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        let (width, height) = io.gpu.display_dimensions();
        assert_eq!((width, height), (512, 240));

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, height as i32, 0x00aa_0000);
        io.gpu.framebuffer.fill_rect_unclipped(
            0,
            height as i32,
            width as i32,
            height as i32,
            0x0000_aa00,
        );
        for x in (8..width).step_by(8) {
            io.gpu.framebuffer.fill_rect_unclipped(
                x as i32,
                height as i32,
                2,
                height as i32,
                0x00ff_ffff,
            );
        }
        io.gpu
            .framebuffer
            .fill_rect_unclipped(8, 8, 32, 8, 0x00ff_ffff);
        io.gpu
            .framebuffer
            .fill_rect_unclipped(8, height as i32 + 8, 32, 8, 0x00ff_ffff);
        io.write_u32(GPU_GP1, 0x0500_0000 | ((height as u32) << 10));
        io.write_u32(GPU_GP1, 0x0500_0000);

        let (frame_width, frame_height, frame) = io.gpu.display_rgb_frame();

        assert_eq!((frame_width, frame_height), (width, height * 2));
        let top_pixel = frame[0];
        let bottom_pixel = frame[height * width];
        assert_ne!(top_pixel, bottom_pixel);
        assert!((top_pixel & 0x00ff_0000) > (top_pixel & 0x0000_ff00));
        assert!((bottom_pixel & 0x0000_ff00) > (bottom_pixel & 0x00ff_0000));
    }

    #[test]
    fn gpu_field_composed_display_overrides_promoted_partial_frame_for_gui() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        let (width, height) = io.gpu.display_dimensions();

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, height as i32, 0x0040_0000);
        io.gpu.framebuffer.fill_rect_unclipped(
            0,
            height as i32,
            width as i32,
            height as i32,
            0x0000_4000,
        );
        for x in (8..width).step_by(8) {
            io.gpu.framebuffer.fill_rect_unclipped(
                x as i32,
                height as i32,
                2,
                height as i32,
                0x00ff_ffff,
            );
        }
        io.gpu
            .framebuffer
            .fill_rect_unclipped(16, 16, 96, 16, 0x00ff_ffff);
        io.gpu
            .framebuffer
            .fill_rect_unclipped(16, height as i32 + 16, 96, 16, 0x00ff_ffff);
        io.write_u32(GPU_GP1, 0x0500_0000 | ((height as u32) << 10));
        io.write_u32(GPU_GP1, 0x0500_0000);

        let promoted_stats = FrameBufferStats {
            pixel_count: (width * height) as u64,
            nonzero_pixels: (width * height) as u64,
            bright_pixels: (width * height / 2) as u64,
            luma_sum: (width * height * 96) as u64,
            max_luma: 255,
            detail_edges: (width * height / 8) as u64,
            checksum: 0xfeed_cafe,
        };
        io.gpu.presented_frame_window = Some(FrameBufferWindow {
            x: 0,
            y: 0,
            stats: promoted_stats,
        });
        io.gpu.presented_frame_png = Some(vec![0x89, b'P', b'N', b'G']);
        io.gpu.presented_frame_rgb = Some(vec![0x00ff_00ff; width * height]);
        io.gpu.presentation_captures = 3;
        io.gpu.presented_frame_capture_index = 3;
        io.gpu.image_upload_commands = DISPLAY_RESOLVE_MIN_IMAGE_UPLOAD_COMMANDS;
        io.gpu.textured_triangle_commands = DISPLAY_RESOLVE_MIN_TEXTURED_TRIANGLE_COMMANDS;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: DISPLAY_RESOLVE_MIN_TEXTURED_WRITTEN_PIXELS,
            color_changes: 32,
            ..TexturedDrawStats::default()
        };
        io.gpu.overlap_draw_commands.push(GpuDrawTrace::textured(
            "textured_quad",
            0,
            0,
            DrawBounds {
                left: 0,
                top: 96,
                right: width as i32,
                bottom: (height * 2 - 1) as i32,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 64, y: 128 }],
            None,
        ));

        let resolved = io.gpu.display_resolve();
        let playability = io.gpu.native_playability_json();
        let (frame_width, frame_height, frame) = io.gpu.display_rgb_frame();

        assert!(resolved.promoted, "{playability}");
        assert_eq!((frame_width, frame_height), (width, height * 2));
        assert_eq!(frame.len(), width * height * 2);
        assert!(
            playability.contains("\"display_height\":480"),
            "{playability}"
        );
        assert!(
            playability.contains("\"actual_display_promoted\":false"),
            "{playability}"
        );
        assert!(
            playability.contains("\"actual_display_field_composed\":true"),
            "{playability}"
        );
    }

    #[test]
    fn gpu_display_rgb_frame_keeps_cached_field_composed_frame_after_raw_field_clear() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        let (width, height) = io.gpu.display_dimensions();

        for x in 0..width {
            let top_color = match (x / 8) % 8 {
                0 => 0x00ff_ffff,
                1 => 0x0008_2040,
                2 => 0x00d0_4030,
                3 => 0x0020_a040,
                4 => 0x0040_40c0,
                5 => 0x00e0_9050,
                6 => 0x0030_c0c0,
                _ => 0x0080_6030,
            };
            let bottom_color = match (x / 8) % 8 {
                0 => 0x0008_4008,
                1 => 0x00ff_e0c0,
                2 => 0x00a0_2030,
                3 => 0x0030_b050,
                4 => 0x0050_60d0,
                5 => 0x00f0_a040,
                6 => 0x0040_c0a0,
                _ => 0x0090_7030,
            };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(x as i32, 0, 1, height as i32, top_color);
            io.gpu.framebuffer.fill_rect_unclipped(
                x as i32,
                height as i32,
                1,
                height as i32,
                bottom_color,
            );
        }
        io.write_u32(GPU_GP1, 0x0500_0000 | ((height as u32) << 10));
        io.write_u32(GPU_GP1, 0x0500_0000);

        io.gpu.capture_vblank_presented_frame();
        let (_, cached_height, cached_frame) = io.gpu.display_rgb_frame();
        assert_eq!(cached_height, height * 2);
        assert!(cached_frame.iter().any(|pixel| *pixel != 0));

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, (height * 2) as i32, 0);

        let playability = io.gpu.native_playability_json();
        let compact_playability = io.gpu.native_playability_compact_json();
        let (frame_width, frame_height, frame) = io.gpu.display_rgb_frame();
        let (stable_width, stable_height, stable_frame) = io.gpu.stable_display_rgb_frame();

        assert_eq!((frame_width, frame_height), (width, height * 2));
        assert_eq!(frame, cached_frame);
        assert_eq!((stable_width, stable_height), (width, height));
        assert!(stable_frame.iter().any(|pixel| *pixel != 0));
        assert_ne!(stable_frame, cached_frame);
        assert!(
            playability.contains("\"actual_display_source\":\"cached_gp1_display_area_fields\""),
            "{playability}"
        );
        assert!(
            playability.contains("\"actual_display_cached\":true"),
            "{playability}"
        );
        assert!(
            playability.contains("\"has_actual_color_diversity\":true"),
            "{compact_playability}"
        );
        assert!(
            playability.contains("\"has_scene_color_diversity\":true"),
            "{compact_playability}"
        );
    }

    #[test]
    fn gpu_display_rgb_frame_keeps_stale_cached_field_when_current_mode_is_caption_band() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        let (cached_width, field_height) = io.gpu.display_dimensions();

        fill_multicolor_scene(&mut io, 0, 0, cached_width, field_height);
        fill_multicolor_scene(&mut io, 0, field_height, cached_width, field_height);
        io.write_u32(GPU_GP1, 0x0500_0000 | ((field_height as u32) << 10));
        io.write_u32(GPU_GP1, 0x0500_0000);
        io.gpu.capture_vblank_presented_frame();
        let (cached_frame_width, cached_frame_height, cached_frame) = io.gpu.display_rgb_frame();
        assert_eq!(
            (cached_frame_width, cached_frame_height),
            (cached_width, field_height * 2)
        );

        io.gpu.presentation_captures = 32;
        io.write_u32(GPU_GP1, 0x0800_0001);
        let (current_width, current_height) = io.gpu.display_dimensions();
        io.gpu.framebuffer.fill_rect_unclipped(
            0,
            0,
            cached_width as i32,
            (field_height * 2) as i32,
            0,
        );
        fill_intro_caption_scene(&mut io, 0, 0, current_width, current_height);

        let playability = io.gpu.native_playability_json();
        let (frame_width, frame_height, frame) = io.gpu.display_rgb_frame();

        assert_eq!(
            (frame_width, frame_height),
            (cached_width, field_height * 2)
        );
        assert_eq!(frame, cached_frame);
        assert!(
            playability.contains("\"actual_display_source\":\"cached_gp1_display_area_fields\""),
            "{playability}"
        );
        assert!(
            playability.contains("\"actual_display_cached\":true"),
            "{playability}"
        );
    }

    #[test]
    fn gpu_field_composed_capture_does_not_replace_valid_cache_with_texture_atlas() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        let (width, field_height) = io.gpu.display_dimensions();

        fill_multicolor_scene(&mut io, 0, 0, width, field_height);
        fill_multicolor_scene(&mut io, 0, field_height, width, field_height);
        io.write_u32(GPU_GP1, 0x0500_0000 | ((field_height as u32) << 10));
        io.write_u32(GPU_GP1, 0x0500_0000);
        io.gpu.capture_vblank_presented_frame();
        let (_, _, cached_frame) = io.gpu.display_rgb_frame();
        let cached_window = io
            .gpu
            .field_composed_display_window
            .expect("valid field-composed cache");

        io.gpu.presentation_captures = 32;
        for x in (0..width).step_by(16) {
            io.gpu
                .push_image_upload_rect(x as i32, 0, 16, (field_height * 2) as i32);
        }
        fill_multicolor_scene(&mut io, 0, 0, width, field_height);
        fill_multicolor_scene(&mut io, 0, field_height, width, field_height);
        io.write_u32(GPU_GP1, 0x0500_0000 | ((field_height as u32) << 10));
        io.write_u32(GPU_GP1, 0x0500_0000);
        io.gpu.capture_vblank_presented_frame();
        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, (field_height * 2) as i32, 0);

        let playability = io.gpu.native_playability_json();
        let (frame_width, frame_height, frame) = io.gpu.display_rgb_frame();

        assert_eq!(io.gpu.field_composed_display_window, Some(cached_window));
        assert_eq!((frame_width, frame_height), (width, field_height * 2));
        assert_eq!(frame, cached_frame);
        assert!(
            playability.contains("\"actual_display_source\":\"cached_gp1_display_area_fields\""),
            "{playability}"
        );
    }

    #[test]
    fn gpu_display_rgb_frame_drops_stale_cached_field_after_valid_mode_change() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        let (cached_width, field_height) = io.gpu.display_dimensions();

        fill_multicolor_scene(&mut io, 0, 0, cached_width, field_height);
        fill_multicolor_scene(&mut io, 0, field_height, cached_width, field_height);
        io.write_u32(GPU_GP1, 0x0500_0000 | ((field_height as u32) << 10));
        io.write_u32(GPU_GP1, 0x0500_0000);
        io.gpu.capture_vblank_presented_frame();
        let (_, _, cached_frame) = io.gpu.display_rgb_frame();

        io.gpu.presentation_captures = 32;
        io.write_u32(GPU_GP1, 0x0800_0001);
        let (current_width, current_height) = io.gpu.display_dimensions();
        io.gpu.framebuffer.fill_rect_unclipped(
            0,
            0,
            cached_width as i32,
            (field_height * 2) as i32,
            0,
        );
        fill_multicolor_scene(&mut io, 0, 0, current_width, current_height);

        let playability = io.gpu.native_playability_json();
        let (frame_width, frame_height, frame) = io.gpu.display_rgb_frame();

        assert_eq!((frame_width, frame_height), (current_width, current_height));
        assert_ne!(frame, cached_frame);
        assert!(
            playability.contains("\"actual_display_source\":\"gp1_display_area\""),
            "{playability}"
        );
        assert!(
            playability.contains("\"actual_display_cached\":false"),
            "{playability}"
        );
    }

    #[test]
    fn gpu_display_rgb_frame_drops_stale_cached_field_after_visible_progress() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        let (width, height) = io.gpu.display_dimensions();

        for x in 0..width {
            let top_color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0008_2040 };
            let bottom_color = if x % 10 < 5 { 0x0010_80d0 } else { 0x00f0_a040 };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(x as i32, 0, 1, height as i32, top_color);
            io.gpu.framebuffer.fill_rect_unclipped(
                x as i32,
                height as i32,
                1,
                height as i32,
                bottom_color,
            );
        }
        io.write_u32(GPU_GP1, 0x0500_0000 | ((height as u32) << 10));
        io.write_u32(GPU_GP1, 0x0500_0000);
        io.gpu.capture_vblank_presented_frame();
        let (_, cached_height, cached_frame) = io.gpu.display_rgb_frame();
        assert_eq!(cached_height, height * 2);

        io.gpu.presentation_captures = 32;
        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, (height * 2) as i32, 0);
        for y in 0..height {
            for x in 0..width {
                let red = 24 + ((x * 5 + y * 3) % 160) as u32;
                let green = 56 + ((x * 7 + y * 11) % 176) as u32;
                let blue = 32 + ((x * 13 + y * 17) % 160) as u32;
                io.gpu.framebuffer.fill_rect_unclipped(
                    x as i32,
                    y as i32,
                    1,
                    1,
                    (red << 16) | (green << 8) | blue,
                );
            }
        }

        let (frame_width, frame_height, frame) = io.gpu.display_rgb_frame();
        let playability = io.gpu.native_playability_json();

        assert_eq!((frame_width, frame_height), (width, height));
        assert_ne!(frame, cached_frame);
        assert!(
            playability.contains("\"actual_display_source\":\"gp1_display_area\""),
            "{playability}"
        );
        assert!(
            playability.contains("\"actual_display_cached\":false"),
            "{playability}"
        );
    }

    #[test]
    fn native_playability_rejects_field_composed_text_band_display() {
        let mut io = Io::default();
        io.write_u32(GPU_GP1, 0x0800_0006);
        let (width, height) = io.gpu.display_dimensions();

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, height as i32, 0x0080_0000);
        io.gpu.framebuffer.fill_rect_unclipped(
            0,
            height as i32,
            width as i32,
            height as i32,
            0x00b0_8070,
        );
        io.gpu
            .framebuffer
            .fill_rect_unclipped(360, height as i32 + 220, 48, 8, 0x00ff_ffff);
        for y in 8..28 {
            for x in (0..width).step_by(8) {
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(x as i32, y, 4, 1, 0x00ff_ffff);
            }
        }
        io.write_u32(GPU_GP1, 0x0500_0000 | ((height as u32) << 10));
        io.write_u32(GPU_GP1, 0x0500_0000);

        io.gpu.commands_seen = 1;
        io.gpu.image_upload_commands = 32;
        io.gpu.textured_triangle_commands = 512;
        io.gpu.presentation_captures = 1;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: 16_384,
            color_changes: 32,
            ..TexturedDrawStats::default()
        };
        io.gpu.overlap_draw_commands.push(GpuDrawTrace::textured(
            "textured_quad",
            0,
            0,
            DrawBounds {
                left: 0,
                top: 100,
                right: width as i32,
                bottom: (height * 2 - 1) as i32,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 64, y: 128 }],
            None,
        ));

        let playability = io.gpu.native_playability_json();

        assert_eq!(io.gpu.display_rgb_frame().1, height);
        assert!(!io.gpu.native_playable_candidate(), "{playability}");
        assert!(
            playability.contains("\"display_height\":240"),
            "{playability}"
        );
        assert!(
            playability.contains("\"actual_display_field_composed\":false"),
            "{playability}"
        );
        assert!(
            playability.contains("\"has_actual_full_scene_detail\":false"),
            "{playability}"
        );
        assert!(playability.contains("\"playable_candidate\":false"));
    }

    #[test]
    fn gpu_refreshes_and_resolves_fresh_presented_frame_after_clear() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        for x in 0..width {
            let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(x as i32, 0, 1, height as i32, color);
        }
        io.write_u32(GPU_GP0, 0x0200_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, ((height as u32) << 16) | width as u32);
        let first = io.gpu.presented_frame_window.expect("first frame");

        for x in 0..width {
            let color = if x % 10 < 5 { 0x00e0_f0f0 } else { 0x0010_50d0 };
            io.gpu
                .framebuffer
                .fill_rect_unclipped(x as i32, 0, 1, height as i32, color);
        }
        let second_stats = io.gpu.framebuffer.display_stats(0, 0, width, height);
        assert!(is_detailed_observation(second_stats));
        assert_ne!(first.stats.checksum, second_stats.checksum);

        io.write_u32(GPU_GP0, 0x0200_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, ((height as u32) << 16) | width as u32);

        let refreshed = io.gpu.presented_frame_window.expect("refreshed frame");
        assert_eq!(io.gpu.presentation_captures, 2);
        assert_eq!(io.gpu.presented_frame_capture_index, 2);
        assert_eq!(refreshed.stats.checksum, second_stats.checksum);

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, 24, 0x00ff_ffff);
        io.gpu.commands_seen = 1;
        io.gpu.image_upload_commands = 32;
        io.gpu.textured_triangle_commands = 512;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: 16_384,
            color_changes: 32,
            ..TexturedDrawStats::default()
        };
        io.gpu.overlap_draw_commands.push(GpuDrawTrace::textured(
            "textured_quad",
            0,
            0,
            DrawBounds {
                left: 64,
                top: 128,
                right: 192,
                bottom: 256,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 64, y: 128 }],
            None,
        ));

        let resolved = io.gpu.display_resolve();
        assert!(resolved.promoted);
        assert_eq!(resolved.source, "presented_frame");
        assert_eq!(resolved.window.stats.checksum, second_stats.checksum);
    }

    #[test]
    fn gpu_keeps_repeated_presented_frame_fresh_after_identical_captures() {
        let mut io = Io::default();
        let (width, height) = io.gpu.display_dimensions();

        for _ in 0..2 {
            for x in 0..width {
                let color = if x % 8 < 4 { 0x00ff_ffff } else { 0x0000_40ff };
                io.gpu
                    .framebuffer
                    .fill_rect_unclipped(x as i32, 0, 1, height as i32, color);
            }
            io.write_u32(GPU_GP0, 0x0200_0000);
            io.write_u32(GPU_GP0, 0x0000_0000);
            io.write_u32(GPU_GP0, ((height as u32) << 16) | width as u32);
        }

        let presented = io.gpu.presented_frame_window.expect("presented frame");
        assert_eq!(io.gpu.presentation_captures, 2);
        assert_eq!(io.gpu.presented_frame_capture_index, 2);
        assert!(is_detailed_observation(presented.stats));

        io.gpu
            .framebuffer
            .fill_rect_unclipped(0, 0, width as i32, 24, 0x00ff_ffff);
        io.gpu.commands_seen = 1;
        io.gpu.image_upload_commands = 32;
        io.gpu.textured_triangle_commands = 512;
        io.gpu.textured_draw_stats = TexturedDrawStats {
            written_pixels: 16_384,
            color_changes: 32,
            ..TexturedDrawStats::default()
        };
        io.gpu.overlap_draw_commands.push(GpuDrawTrace::textured(
            "textured_quad",
            0,
            0,
            DrawBounds {
                left: 64,
                top: 128,
                right: 192,
                bottom: 256,
            },
            io.gpu.textured_draw_stats,
            &[0x2c00_0000],
            &[Point { x: 64, y: 128 }],
            None,
        ));

        let resolved = io.gpu.display_resolve();
        assert!(resolved.promoted);
        assert_eq!(resolved.source, "presented_frame");
        assert_eq!(resolved.window.stats.checksum, presented.stats.checksum);
    }

    #[test]
    fn gpu_screenshot_does_not_promote_solid_white_observation() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0x02ff_ffff);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x00f0_0140);

        assert!(io.gpu.best_observation_window.is_none());
    }

    #[test]
    fn gpu_screenshot_does_not_promote_uniform_mid_luma_observation() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0x0280_8080);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x00f0_0140);

        assert!(io.gpu.best_observation_window.is_none());
    }

    #[test]
    fn gpu_gp0_image_upload_and_vram_copy_update_framebuffer() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xa000_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0002);
        io.write_u32(GPU_GP0, 0x03e0_001f);
        io.write_u32(GPU_GP0, 0x8000_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0004_0004);
        io.write_u32(GPU_GP0, 0x0001_0002);

        assert_eq!(io.gpu.gp0_pending_words(), 0);
        assert_eq!(io.gpu.framebuffer_stats().nonzero_pixels, 4);
    }

    #[test]
    fn gpu_gp0_flat_quad_honors_semi_transparency() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0x0200_00ff);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0004_0004);

        io.write_u32(GPU_GP0, 0x2aff_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0003_0000);
        io.write_u32(GPU_GP0, 0x0000_0003);
        io.write_u32(GPU_GP0, 0x0003_0003);

        let blended = io.gpu.framebuffer.pixel(1, 1);
        assert_ne!(blended, 0x00ff_0000);
        assert_ne!(blended, 0x0000_00ff);
        assert!(blended & 0x00ff_0000 != 0);
        assert!(blended & 0x0000_00ff != 0);
    }

    #[test]
    fn gpu_gp0_textured_quad_samples_vram_texture() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xa000_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0010);
        io.write_u32(GPU_GP0, 0x001f_0000);
        io.write_u32(GPU_GP0, 0x001f_001f);
        io.write_u32(GPU_GP0, 0x001f_001f);
        io.write_u32(GPU_GP0, 0x001f_001f);
        io.write_u32(GPU_GP0, 0x001f_001f);
        io.write_u32(GPU_GP0, 0x001f_001f);
        io.write_u32(GPU_GP0, 0x001f_001f);
        io.write_u32(GPU_GP0, 0x001f_001f);

        io.write_u32(GPU_GP0, 0xa000_0000);
        io.write_u32(GPU_GP0, 0x0000_0040);
        io.write_u32(GPU_GP0, 0x0008_0002);
        io.write_u32(GPU_GP0, 0x1111_1111);
        io.write_u32(GPU_GP0, 0x1111_1111);
        io.write_u32(GPU_GP0, 0x1111_1111);
        io.write_u32(GPU_GP0, 0x1111_1111);
        io.write_u32(GPU_GP0, 0x1111_1111);
        io.write_u32(GPU_GP0, 0x1111_1111);
        io.write_u32(GPU_GP0, 0x1111_1111);
        io.write_u32(GPU_GP0, 0x1111_1111);

        io.write_u32(GPU_GP0, 0x2c80_8080);
        io.write_u32(GPU_GP0, 0x000a_000a);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x000a_0014);
        io.write_u32(GPU_GP0, 0x0001_0000);
        io.write_u32(GPU_GP0, 0x0014_000a);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0014_0014);
        io.write_u32(GPU_GP0, 0x0000_0000);

        assert_eq!(io.gpu.gp0_pending_words(), 0);
        assert!(io.gpu.framebuffer_stats().nonzero_pixels > 16);
    }

    #[test]
    fn gpu_gp0_textured_sprite_samples_current_texture_page() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xa000_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0010);
        io.write_u32(GPU_GP0, 0x001f_0000);
        io.write_u32(GPU_GP0, 0x001f_001f);
        io.write_u32(GPU_GP0, 0x001f_001f);
        io.write_u32(GPU_GP0, 0x001f_001f);
        io.write_u32(GPU_GP0, 0x001f_001f);
        io.write_u32(GPU_GP0, 0x001f_001f);
        io.write_u32(GPU_GP0, 0x001f_001f);
        io.write_u32(GPU_GP0, 0x001f_001f);

        io.write_u32(GPU_GP0, 0xa000_0000);
        io.write_u32(GPU_GP0, 0x0000_0040);
        io.write_u32(GPU_GP0, 0x0008_0002);
        io.write_u32(GPU_GP0, 0x1111_1111);
        io.write_u32(GPU_GP0, 0x1111_1111);
        io.write_u32(GPU_GP0, 0x1111_1111);
        io.write_u32(GPU_GP0, 0x1111_1111);
        io.write_u32(GPU_GP0, 0x1111_1111);
        io.write_u32(GPU_GP0, 0x1111_1111);
        io.write_u32(GPU_GP0, 0x1111_1111);
        io.write_u32(GPU_GP0, 0x1111_1111);

        io.write_u32(GPU_GP0, 0xe100_0001);
        io.write_u32(GPU_GP0, 0x7480_8080);
        io.write_u32(GPU_GP0, 0x0010_0010);
        io.write_u32(GPU_GP0, 0x0000_0000);

        assert_eq!(io.gpu.gp0_pending_words(), 0);
        assert!(io.gpu.framebuffer_stats().nonzero_pixels >= 64);
    }

    #[test]
    fn gpu_gp0_draw_mode_preserves_zn_extended_texture_page_y() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xa000_0000);
        io.write_u32(GPU_GP0, 0x0200_0000);
        io.write_u32(GPU_GP0, 0x0001_0001);
        io.write_u32(GPU_GP0, 0x0000_001f);

        io.write_u32(GPU_GP0, 0xe100_0900);
        io.write_u32(GPU_GP0, 0x6500_0000);
        io.write_u32(GPU_GP0, 0x000a_000a);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0001);

        assert_eq!(io.gpu.framebuffer.raw_pixel(10, 10) & 0x7fff, 0x001f);
    }

    #[test]
    fn gpu_gp0_textured_quad_honors_raw_texture_flag() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xa000_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0002_0002);
        io.write_u32(GPU_GP0, 0x7fff_7fff);
        io.write_u32(GPU_GP0, 0x7fff_7fff);

        let direct_15bit_page = 0x0100_u32;
        let texture_page_uv = direct_15bit_page << 16;
        io.write_u32(GPU_GP0, 0x2c40_4040);
        io.write_u32(GPU_GP0, 0x000a_000a);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x000a_000c);
        io.write_u32(GPU_GP0, texture_page_uv | 0x0001);
        io.write_u32(GPU_GP0, 0x000c_000a);
        io.write_u32(GPU_GP0, 0x0000_0100);
        io.write_u32(GPU_GP0, 0x000c_000c);
        io.write_u32(GPU_GP0, 0x0000_0101);

        io.write_u32(GPU_GP0, 0x2d40_4040);
        io.write_u32(GPU_GP0, 0x000a_0014);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x000a_0016);
        io.write_u32(GPU_GP0, texture_page_uv | 0x0001);
        io.write_u32(GPU_GP0, 0x000c_0014);
        io.write_u32(GPU_GP0, 0x0000_0100);
        io.write_u32(GPU_GP0, 0x000c_0016);
        io.write_u32(GPU_GP0, 0x0000_0101);

        let modulated = io.gpu.framebuffer.raw_pixel(10, 10) & 0x7fff;
        let raw = io.gpu.framebuffer.raw_pixel(20, 10) & 0x7fff;
        assert_ne!(modulated, 0);
        assert!(modulated < 0x7fff);
        assert_eq!(raw, 0x7fff);
    }

    #[test]
    fn gpu_gp0_textured_sprite_draws_stp_black_texel() {
        let mut io = Io::default();

        io.gpu
            .framebuffer
            .fill_rect_unclipped(10, 10, 1, 1, 0x0000_ff00);

        io.write_u32(GPU_GP0, 0xa000_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0001);
        io.write_u32(GPU_GP0, 0x0000_8000);

        io.write_u32(GPU_GP0, 0xe100_0100);
        io.write_u32(GPU_GP0, 0x6500_0000);
        io.write_u32(GPU_GP0, 0x000a_000a);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0001);

        assert_eq!(io.gpu.framebuffer.pixel(10, 10), 0);
        assert_eq!(io.gpu.framebuffer.raw_pixel(10, 10), 0x8000);
    }

    #[test]
    fn gpu_gp0_textured_sprite_blends_only_stp_texels() {
        let mut io = Io::default();

        io.gpu
            .framebuffer
            .fill_rect_unclipped(10, 10, 2, 1, 0x0000_00ff);

        io.write_u32(GPU_GP0, 0xa000_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0002);
        io.write_u32(GPU_GP0, 0x801f_001f);

        io.write_u32(GPU_GP0, 0xe100_0100);
        io.write_u32(GPU_GP0, 0x6700_0000);
        io.write_u32(GPU_GP0, 0x000a_000a);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0002);

        let opaque = io.gpu.framebuffer.pixel(10, 10);
        let blended = io.gpu.framebuffer.pixel(11, 10);
        assert_eq!(opaque, 0x00ff_0000);
        assert_ne!(blended, 0x00ff_0000);
        assert_ne!(blended, 0x0000_00ff);
        assert!(blended & 0x00ff_0000 != 0);
        assert!(blended & 0x0000_00ff != 0);
    }

    #[test]
    fn gpu_gp0_accepts_image_upload_command_variants() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xa090_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0002);
        io.write_u32(GPU_GP0, 0x03e0_001f);

        assert_eq!(io.gpu.gp0_pending_words(), 0);
        assert_eq!(io.gpu.gp0_pending_expected_words(), None);
        assert_eq!(io.gpu.image_upload_commands, 1);
        assert_eq!(io.gpu.framebuffer_stats().nonzero_pixels, 2);
    }

    #[test]
    fn gpu_gp0_ignores_non_exact_transfer_opcodes() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xb900_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0002);
        io.write_u32(GPU_GP0, 0x03e0_001f);
        io.write_u32(GPU_GP0, 0x9900_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0004_0004);
        io.write_u32(GPU_GP0, 0x0001_0002);
        io.write_u32(GPU_GP0, 0xd100_0000);
        io.write_u32(GPU_GP0, 0x0004_0004);
        io.write_u32(GPU_GP0, 0x0001_0002);

        assert_eq!(io.gpu.gp0_pending_words(), 0);
        assert_eq!(io.gpu.image_upload_commands, 0);
        assert_eq!(io.gpu.vram_copy_commands, 0);
        assert_eq!(io.gpu.framebuffer_stats().nonzero_pixels, 0);
    }

    #[test]
    fn gpu_gp0_accepts_exact_transfer_opcodes() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xa000_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0002);
        io.write_u32(GPU_GP0, 0x03e0_001f);
        io.write_u32(GPU_GP0, 0x8000_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0004_0004);
        io.write_u32(GPU_GP0, 0x0001_0002);
        io.write_u32(GPU_GP0, 0xc000_0000);
        io.write_u32(GPU_GP0, 0x0004_0004);
        io.write_u32(GPU_GP0, 0x0001_0002);

        assert_eq!(io.gpu.gp0_pending_words(), 0);
        assert_eq!(io.gpu.image_upload_commands, 1);
        assert_eq!(io.gpu.vram_copy_commands, 1);
        assert_eq!(io.gpu.gp0_read, 0);
        assert_eq!(io.gpu.framebuffer_stats().nonzero_pixels, 4);
    }

    #[test]
    fn gpu_gp0_transfers_and_gp1_display_reach_extended_2mb_vram_rows() {
        let mut io = Io::default();
        let source_y = 768_u32;
        let dest_y = 800_u32;

        io.write_u32(GPU_GP0, 0xa000_0000);
        io.write_u32(GPU_GP0, (source_y << 16) | 4);
        io.write_u32(GPU_GP0, 0x0001_0002);
        io.write_u32(GPU_GP0, 0x03e0_001f);
        io.write_u32(GPU_GP0, 0x8000_0000);
        io.write_u32(GPU_GP0, (source_y << 16) | 4);
        io.write_u32(GPU_GP0, (dest_y << 16) | 8);
        io.write_u32(GPU_GP0, 0x0001_0002);
        io.write_u32(GPU_GP1, 0x0500_0000 | (source_y << 10));

        let display = io.gpu.current_display_window();

        assert_eq!(display.y, source_y as usize);
        assert_eq!(io.gpu.image_upload_commands, 1);
        assert_eq!(io.gpu.vram_copy_commands, 1);
        assert_eq!(io.gpu.framebuffer.raw_pixel(4, source_y as i32), 0x001f);
        assert_eq!(io.gpu.framebuffer.raw_pixel(5, source_y as i32), 0x03e0);
        assert_eq!(io.gpu.framebuffer.raw_pixel(8, dest_y as i32), 0x001f);
        assert_eq!(io.gpu.framebuffer.raw_pixel(9, dest_y as i32), 0x03e0);
        assert_eq!(io.gpu.framebuffer.raw_pixel(4, source_y as i32 - 512), 0);
    }

    #[test]
    fn gpu_gp0_rejects_shifted_full_vram_copy_as_corrupt_transfer() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xa000_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0001_0002);
        io.write_u32(GPU_GP0, 0x03e0_001f);
        io.write_u32(GPU_GP0, 0x8000_0000);
        io.write_u32(GPU_GP0, 0x0000_0001);
        io.write_u32(GPU_GP0, 0x0001_0002);
        io.write_u32(GPU_GP0, ((VRAM_HEIGHT as u32) << 16) | VRAM_WIDTH as u32);

        assert_eq!(io.gpu.gp0_pending_words(), 0);
        assert_eq!(io.gpu.vram_copy_commands, 0);
        assert_eq!(io.gpu.invalid_vram_copy_commands, 1);
        assert_eq!(io.gpu.framebuffer_stats().nonzero_pixels, 2);
    }

    #[test]
    fn gpu_gp0_rejects_invalid_fill_rect_dimensions() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0x0201_2b1c);
        io.write_u32(GPU_GP0, 0x03a6_fd95);
        io.write_u32(GPU_GP0, 0xb857_afaf);

        assert_eq!(io.gpu.gp0_pending_words(), 0);
        assert_eq!(io.gpu.fill_rect_commands, 0);
        assert_eq!(io.gpu.invalid_fill_rect_commands, 1);
        assert_eq!(io.gpu.framebuffer_stats().nonzero_pixels, 0);
    }

    #[test]
    fn gpu_gp0_invalid_image_upload_dimensions_do_not_stall_fifo() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xa0a5_9982);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0xffff_ffff);

        assert_eq!(io.gpu.gp0_pending_words(), 0);
        assert_eq!(io.gpu.gp0_pending_expected_words(), None);
        assert_eq!(io.gpu.image_upload_commands, 0);
    }

    #[test]
    fn gpu_gp0_invalid_image_upload_variant_dimensions_recover_fifo() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xb990_0000);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0xffff_ffff);

        assert_eq!(io.gpu.gp0_pending_words(), 0);
        assert_eq!(io.gpu.gp0_pending_expected_words(), None);
        assert_eq!(io.gpu.image_upload_commands, 0);
        assert_eq!(io.gpu.invalid_image_upload_commands, 0);
    }

    #[test]
    fn mdec_control_writes_keep_status_ready() {
        let mut io = Io::default();

        io.write_u32(MDEC_STATUS, 0x6000_0000);
        assert_eq!(io.read_u32(MDEC_STATUS), 0x8004_0000);

        io.write_u32(MDEC_COMMAND, 0x1234_5678);
        assert_eq!(io.read_u32(MDEC_STATUS), 0x8004_0000);

        io.write_u32(MDEC_STATUS, 0x8000_0000);
        assert_eq!(io.read_u32(MDEC_STATUS), 0x8004_0000);
    }

    #[test]
    fn mdec_commands_track_dma_input_requirements() {
        let mut io = Io::default();

        io.write_u32(MDEC_COMMAND, 0x4000_0001);
        assert_eq!(io.mdec.input_words_remaining(), 32);
        assert_eq!(io.read_u32(MDEC_STATUS) & (1 << 29), 1 << 29);

        io.mdec.write_dma_input(0x1111_2222);
        assert_eq!(io.mdec.input_words_remaining(), 31);
        assert_eq!(io.mdec.dma_input_words(), 1);
        assert!(
            io.mdec
                .diagnostic_json()
                .contains("\"command_hex\":\"0x40000001\"")
        );

        for _ in 0..31 {
            io.mdec.write_dma_input(0);
        }

        assert_eq!(io.mdec.input_words_remaining(), 0);
        assert_eq!(io.read_u32(MDEC_STATUS), 0x8004_0000);
    }

    #[test]
    fn mdec_command_port_accepts_data_while_command_is_pending() {
        let mut io = Io::default();

        io.write_u32(MDEC_COMMAND, 0x4000_0000);
        assert_eq!(io.mdec.input_words_remaining(), 16);

        for index in 0..16 {
            io.write_u32(MDEC_COMMAND, 0x0101_0101 + index);
        }

        assert_eq!(io.mdec.input_words_remaining(), 0);
        assert!(
            io.mdec
                .diagnostic_json()
                .contains("\"command_hex\":\"0x40000000\"")
        );
        assert_eq!(io.read_u32(MDEC_STATUS), 0x8004_0000);
    }

    #[test]
    fn mdec_command_port_decodes_payload_data_without_dma() {
        let mut io = Io::default();

        io.write_u32(MDEC_COMMAND, 0x2000_0006);
        io.write_u32(MDEC_COMMAND, 0xfe00_2000);
        io.write_u32(MDEC_COMMAND, 0xfe00_2000);
        io.write_u32(MDEC_COMMAND, 0xfe00_2040);
        io.write_u32(MDEC_COMMAND, 0xfe00_2060);
        io.write_u32(MDEC_COMMAND, 0xfe00_2080);
        io.write_u32(MDEC_COMMAND, 0xfe00_20a0);

        assert_eq!(io.mdec.input_words_remaining(), 0);
        assert_ne!(io.mdec.read_dma_output(), 0);
    }

    #[test]
    fn mdec_dma_input_starts_command_when_idle() {
        let mut io = Io::default();

        io.mdec.write_dma_input(0x4000_0001);

        assert_eq!(io.mdec.input_words_remaining(), 32);
        assert_eq!(io.mdec.dma_input_words(), 1);
        assert!(
            io.mdec
                .diagnostic_json()
                .contains("\"command_hex\":\"0x40000001\"")
        );
    }

    #[test]
    fn mdec_decode_dma_outputs_dc_macroblock_pixels() {
        let mut io = Io::default();

        io.write_u32(MDEC_COMMAND, 0x2000_0006);
        io.mdec.write_dma_input(0xfe00_2000);
        io.mdec.write_dma_input(0xfe00_2000);
        io.mdec.write_dma_input(0xfe00_2040);
        io.mdec.write_dma_input(0xfe00_2060);
        io.mdec.write_dma_input(0xfe00_2080);
        io.mdec.write_dma_input(0xfe00_20a0);

        assert_eq!(io.mdec.input_words_remaining(), 0);
        let first = io.mdec.read_dma_output();
        for _ in 0..3 {
            io.mdec.read_dma_output();
        }
        let second_quadrant = io.mdec.read_dma_output();
        assert_ne!(first, 0);
        assert_ne!(second_quadrant, 0);
        assert_ne!(first, second_quadrant);
    }

    #[test]
    fn mdec_decode_dma_returns_zero_after_decoded_output_is_exhausted() {
        let mut io = Io::default();

        io.write_u32(MDEC_COMMAND, 0x2000_0006);
        io.mdec.write_dma_input(0xfe00_2000);
        io.mdec.write_dma_input(0xfe00_2000);
        io.mdec.write_dma_input(0xfe00_2040);
        io.mdec.write_dma_input(0xfe00_2060);
        io.mdec.write_dma_input(0xfe00_2080);
        io.mdec.write_dma_input(0xfe00_20a0);

        for _ in 0..128 {
            assert_ne!(io.mdec.read_dma_output(), 0);
        }
        assert_eq!(io.mdec.read_dma_output(), 0);
        assert!(
            io.mdec
                .diagnostic_json()
                .contains("\"decoded_output_underflow_reads\":1")
        );
    }

    #[test]
    fn mdec_output_dma_is_zero_without_decode_input() {
        let mut io = Io::default();

        assert_eq!(io.mdec.read_dma_output(), 0);
        assert_eq!(io.mdec.dma_output_words(), 1);
        assert_eq!(io.read_u32(MDEC_STATUS), 0x8004_0000);
    }
}
