use std::cell::Cell;

use crate::action::ActionButtons;
use crate::native::framebuffer::{
    ClipRect, DEFAULT_DISPLAY_HEIGHT, DEFAULT_DISPLAY_WIDTH, FrameBufferBounds, FrameBufferStats,
    FrameBufferWindow, NativeFrameBuffer, Point, TextureCoordinate, TextureWindow, TexturedPoint,
    VRAM_HEIGHT, VRAM_WIDTH,
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
const DMA_CHANNEL_COUNT: usize = 7;
const DMA_INTERRUPT_IRQ_ENABLE_MASK: u32 = 0x007f_0000;
const DMA_INTERRUPT_FLAG_MASK: u32 = 0x7f00_0000;
const GP0_RECENT_COMMAND_LIMIT: usize = 16;
const GP0_RECENT_COMMAND_WORD_LIMIT: usize = 12;

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

    pub fn tick(&mut self, cycles: u64) {
        self.timers.tick(cycles);
    }

    pub fn json(&self) -> String {
        let framebuffer_stats = self.gpu.framebuffer_stats();
        let vram_stats = self.gpu.vram_stats();
        let vram_bounds = self.gpu.vram_nonzero_bounds_json();
        let screenshot_window = self.gpu.screenshot_window();
        format!(
            "{{\"irq_status\":{},\"irq_mask\":{},\"dma_control\":{},\"dma_interrupt\":{},\"gpu_status\":{},\"gpu_commands_seen\":{},\"gpu_gp0_pending_words\":{},\"gpu_gp0_pending_head\":{},\"gpu_gp0_expected_words\":{},\"gpu_frame_nonzero_pixels\":{},\"gpu_frame_checksum\":{},\"gpu_vram_nonzero_pixels\":{},\"gpu_vram_checksum\":{},\"gpu_vram_nonzero_bounds\":{},\"gpu_screenshot_x\":{},\"gpu_screenshot_y\":{},\"gpu_screenshot_nonzero_pixels\":{},\"gpu_screenshot_checksum\":{},\"gpu_display_area_start\":{},\"gpu_horizontal_range\":{},\"gpu_vertical_range\":{},\"gpu_drawing_area_top_left\":{},\"gpu_drawing_area_bottom_right\":{},\"gpu_drawing_offset\":{},\"gpu_texture_page\":{},\"gpu_fill_rect_commands\":{},\"gpu_flat_triangle_commands\":{},\"gpu_textured_triangle_commands\":{},\"gpu_textured_rect_commands\":{},\"gpu_flat_line_commands\":{},\"gpu_image_upload_commands\":{},\"gpu_vram_copy_commands\":{},\"gpu_gp0_command_counts\":[{}],\"gpu_recent_gp0_commands\":[{}],\"timer0_counter\":{},\"timer1_counter\":{},\"timer2_counter\":{},\"sio_status\":{},\"p1_state\":{}}}",
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
            self.gpu.display_area_start,
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
            self.gpu.image_upload_commands,
            self.gpu.vram_copy_commands,
            self.gpu.gp0_command_counts_json(),
            self.gpu.recent_gp0_commands_json(),
            self.timers.0[0].counter,
            self.timers.0[1].counter,
            self.timers.0[2].counter,
            self.controller.status,
            self.controller.p1_state
        )
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
                self.controller.write_u32(address, value)
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
    drawing_area_top_left: u32,
    drawing_area_bottom_right: u32,
    drawing_offset: u32,
    texture_page: u16,
    texture_window: TextureWindow,
    gp0_command_counts: [u64; 256],
    fill_rect_commands: u64,
    flat_triangle_commands: u64,
    textured_triangle_commands: u64,
    textured_rect_commands: u64,
    flat_line_commands: u64,
    image_upload_commands: u64,
    vram_copy_commands: u64,
    recent_gp0_commands: Vec<Gp0CommandTrace>,
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
            drawing_area_top_left: 0,
            drawing_area_bottom_right: 0,
            drawing_offset: 0,
            texture_page: 0,
            texture_window: TextureWindow::default(),
            gp0_command_counts: [0; 256],
            fill_rect_commands: 0,
            flat_triangle_commands: 0,
            textured_triangle_commands: 0,
            textured_rect_commands: 0,
            flat_line_commands: 0,
            image_upload_commands: 0,
            vram_copy_commands: 0,
            recent_gp0_commands: Vec::new(),
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
        self.gp0_read = value;
        self.commands_seen += 1;
        self.gp0_fifo.push(value);
        self.drain_gp0_fifo();
    }

    pub fn write_gp1(&mut self, value: u32) {
        let command = value >> 24;
        match command {
            0x00 => *self = Self::default(),
            0x01 => {
                self.gp0_read = 0;
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
            0x05 => self.display_area_start = value & 0x00ff_ffff,
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
        let window = self.screenshot_window();
        self.framebuffer.png_base64(
            window.x,
            window.y,
            DEFAULT_DISPLAY_WIDTH,
            DEFAULT_DISPLAY_HEIGHT,
        )
    }

    pub fn screenshot_png(&self) -> Vec<u8> {
        let window = self.screenshot_window();
        self.framebuffer.png(
            window.x,
            window.y,
            DEFAULT_DISPLAY_WIDTH,
            DEFAULT_DISPLAY_HEIGHT,
        )
    }

    pub fn display_png(&self) -> Vec<u8> {
        let (start_x, start_y) = display_area_start_xy(self.display_area_start);
        self.framebuffer.png(
            start_x,
            start_y,
            DEFAULT_DISPLAY_WIDTH,
            DEFAULT_DISPLAY_HEIGHT,
        )
    }

    pub fn screenshot_window(&self) -> FrameBufferWindow {
        let (start_x, start_y) = display_area_start_xy(self.display_area_start);
        let stats = self.framebuffer.display_stats(
            start_x,
            start_y,
            DEFAULT_DISPLAY_WIDTH,
            DEFAULT_DISPLAY_HEIGHT,
        );
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
            stats: self.framebuffer.display_stats(
                drawing_x,
                drawing_y,
                DEFAULT_DISPLAY_WIDTH,
                DEFAULT_DISPLAY_HEIGHT,
            ),
        };

        let mut best_window = display_window;
        if should_use_observation_fallback(best_window.stats, drawing_window.stats) {
            best_window = drawing_window;
        }

        let Some(densest_window) =
            self.framebuffer
                .densest_window(DEFAULT_DISPLAY_WIDTH, DEFAULT_DISPLAY_HEIGHT, 8)
        else {
            return best_window;
        };

        if should_use_observation_fallback(best_window.stats, densest_window.stats) {
            best_window = densest_window;
        }
        best_window
    }

    pub fn vram_png(&self) -> Vec<u8> {
        self.framebuffer.png(0, 0, VRAM_WIDTH, VRAM_HEIGHT)
    }

    pub fn framebuffer_stats(&self) -> FrameBufferStats {
        let (start_x, start_y) = display_area_start_xy(self.display_area_start);
        self.framebuffer.display_stats(
            start_x,
            start_y,
            DEFAULT_DISPLAY_WIDTH,
            DEFAULT_DISPLAY_HEIGHT,
        )
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

    fn drain_gp0_fifo(&mut self) {
        loop {
            let Some(expected_words) = gp0_expected_words(&self.gp0_fifo) else {
                return;
            };
            if self.gp0_fifo.len() < expected_words {
                return;
            }

            let command = self.gp0_fifo[..expected_words].to_vec();
            self.gp0_fifo.drain(..expected_words);
            self.execute_gp0_command(&command);
        }
    }

    fn execute_gp0_command(&mut self, words: &[u32]) {
        if words.is_empty() {
            return;
        }

        let command = (words[0] >> 24) as u8;
        self.gp0_command_counts[command as usize] =
            self.gp0_command_counts[command as usize].saturating_add(1);
        self.push_recent_gp0_command(words);
        match command {
            0x02 if words.len() >= 3 => {
                let (x, y) = xy(words[1]);
                let width = (words[2] & 0xffff) as i32;
                let height = (words[2] >> 16) as i32;
                self.fill_rect_commands += 1;
                self.framebuffer
                    .fill_rect_unclipped(x, y, width, height, color(words[0]));
            }
            0x20..=0x23 if words.len() >= 4 => {
                self.draw_flat_triangle(words[1], words[2], words[3], color(words[0]));
            }
            0x24..=0x27 if words.len() >= 7 => {
                self.draw_textured_triangle([
                    (words[1], words[2]),
                    (words[3], words[4]),
                    (words[5], words[6]),
                ]);
            }
            0x28..=0x2b if words.len() >= 5 => {
                self.draw_flat_quad(words[1], words[2], words[3], words[4], color(words[0]));
            }
            0x2c..=0x2f if words.len() >= 9 => {
                self.draw_textured_quad([
                    (words[1], words[2]),
                    (words[3], words[4]),
                    (words[5], words[6]),
                    (words[7], words[8]),
                ]);
            }
            0x30..=0x33 if words.len() >= 6 => {
                self.draw_flat_triangle(words[1], words[3], words[5], color(words[0]));
            }
            0x34..=0x37 if words.len() >= 9 => {
                self.draw_textured_triangle([
                    (words[1], words[2]),
                    (words[4], words[5]),
                    (words[7], words[8]),
                ]);
            }
            0x38..=0x3b if words.len() >= 8 => {
                self.draw_flat_quad(words[1], words[3], words[5], words[7], color(words[0]));
            }
            0x3c..=0x3f if words.len() >= 12 => {
                self.draw_textured_quad([
                    (words[1], words[2]),
                    (words[4], words[5]),
                    (words[7], words[8]),
                    (words[10], words[11]),
                ]);
            }
            0x40..=0x47 if words.len() >= 3 => {
                self.draw_flat_line(words[1], words[2], color(words[0]));
            }
            0x50..=0x57 if words.len() >= 4 => {
                self.draw_flat_line(words[1], words[3], color(words[0]));
            }
            0x60..=0x63 if words.len() >= 3 => {
                let (x, y) = xy(words[1]);
                let width = (words[2] & 0xffff) as i32;
                let height = (words[2] >> 16) as i32;
                self.fill_rect_commands += 1;
                self.framebuffer
                    .fill_rect(x, y, width, height, color(words[0]));
            }
            0x64..=0x67 if words.len() >= 4 => {
                let width = (words[3] & 0xffff) as i32;
                let height = (words[3] >> 16) as i32;
                self.draw_textured_rect(words[1], words[2], width, height);
            }
            0x68..=0x6f if words.len() >= 2 => {
                let (x, y) = xy(words[1]);
                self.fill_rect_commands += 1;
                self.framebuffer.fill_rect(x, y, 1, 1, color(words[0]));
            }
            0x70..=0x73 if words.len() >= 2 => {
                let (x, y) = xy(words[1]);
                self.fill_rect_commands += 1;
                self.framebuffer.fill_rect(x, y, 8, 8, color(words[0]));
            }
            0x74..=0x77 if words.len() >= 3 => {
                self.draw_textured_rect(words[1], words[2], 8, 8);
            }
            0x78..=0x7b if words.len() >= 2 => {
                let (x, y) = xy(words[1]);
                self.fill_rect_commands += 1;
                self.framebuffer.fill_rect(x, y, 16, 16, color(words[0]));
            }
            0x7c..=0x7f if words.len() >= 3 => {
                self.draw_textured_rect(words[1], words[2], 16, 16);
            }
            0x80 if words.len() >= 4 => {
                let (source_x, source_y) = unsigned_xy(words[1]);
                let (dest_x, dest_y) = unsigned_xy(words[2]);
                let (width, height) = dimensions(words[3]);
                self.vram_copy_commands += 1;
                self.framebuffer
                    .copy_rect(source_x, source_y, dest_x, dest_y, width, height);
            }
            0xa0 if words.len() >= 3 => {
                let (x, y) = unsigned_xy(words[1]);
                if let Some((width, height)) = image_transfer_dimensions(words[2]) {
                    self.image_upload_commands += 1;
                    self.framebuffer.write_rgb555_image(
                        x,
                        y,
                        width as i32,
                        height as i32,
                        &words[3..],
                    );
                }
            }
            0xc0 if words.len() >= 3 => {
                self.gp0_read = 0;
            }
            0xe1 => self.texture_page = (words[0] & 0x07ff) as u16,
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
            _ => {}
        }
    }

    fn draw_flat_triangle(&mut self, a: u32, b: u32, c: u32, color: u32) {
        self.flat_triangle_commands += 1;
        self.framebuffer.draw_triangle(
            self.offset_point(point(a)),
            self.offset_point(point(b)),
            self.offset_point(point(c)),
            color,
        );
    }

    fn draw_flat_quad(&mut self, a: u32, b: u32, c: u32, d: u32, color: u32) {
        self.flat_triangle_commands += 2;
        let a = self.offset_point(point(a));
        let b = self.offset_point(point(b));
        let c = self.offset_point(point(c));
        let d = self.offset_point(point(d));
        self.framebuffer.draw_triangle(a, b, c, color);
        self.framebuffer.draw_triangle(b, c, d, color);
    }

    fn draw_flat_line(&mut self, a: u32, b: u32, color: u32) {
        self.flat_line_commands += 1;
        self.framebuffer.draw_line(
            self.offset_point(point(a)),
            self.offset_point(point(b)),
            color,
        );
    }

    fn draw_textured_triangle(&mut self, vertices: [(u32, u32); 3]) {
        self.textured_triangle_commands += 1;
        let [(a, a_uv), (b, b_uv), (c, c_uv)] = vertices;
        let clut = clut(a_uv);
        let texture_page = texture_page(b_uv);
        self.texture_page = texture_page;
        self.framebuffer.draw_textured_triangle(
            self.textured_point(a, a_uv),
            self.textured_point(b, b_uv),
            self.textured_point(c, c_uv),
            texture_page,
            clut,
            self.texture_window,
        );
    }

    fn draw_textured_quad(&mut self, vertices: [(u32, u32); 4]) {
        self.textured_triangle_commands += 2;
        let [(a, a_uv), (b, b_uv), (c, c_uv), (d, d_uv)] = vertices;
        let clut = clut(a_uv);
        let texture_page = texture_page(b_uv);
        self.texture_page = texture_page;
        let a = self.textured_point(a, a_uv);
        let b = self.textured_point(b, b_uv);
        let c = self.textured_point(c, c_uv);
        let d = self.textured_point(d, d_uv);
        self.framebuffer
            .draw_textured_triangle(a, b, c, texture_page, clut, self.texture_window);
        self.framebuffer
            .draw_textured_triangle(b, c, d, texture_page, clut, self.texture_window);
    }

    fn draw_textured_rect(&mut self, xy: u32, uv: u32, width: i32, height: i32) {
        self.textured_rect_commands += 1;
        let point = self.offset_point(point(xy));
        self.framebuffer.draw_textured_rect(
            point,
            (width, height),
            self.texture_page,
            clut(uv),
            texture_coordinate(uv),
            self.texture_window,
        );
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

    fn push_recent_gp0_command(&mut self, words: &[u32]) {
        self.recent_gp0_commands.push(Gp0CommandTrace::new(words));
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
}

#[derive(Clone, Debug)]
struct Gp0CommandTrace {
    opcode: u8,
    word_count: usize,
    words: Vec<u32>,
}

impl Gp0CommandTrace {
    fn new(words: &[u32]) -> Self {
        Self {
            opcode: (words[0] >> 24) as u8,
            word_count: words.len(),
            words: words
                .iter()
                .copied()
                .take(GP0_RECENT_COMMAND_WORD_LIMIT)
                .collect(),
        }
    }

    fn json(&self) -> String {
        let words = self
            .words
            .iter()
            .map(|word| format!("{{\"value\":{},\"value_hex\":\"0x{word:08x}\"}}", word))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"opcode\":{},\"opcode_hex\":\"0x{:02x}\",\"word_count\":{},\"words\":[{}]}}",
            self.opcode, self.opcode, self.word_count, words
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
        return Some(3);
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
    let x = (value & 0x03ff) as usize;
    let y = ((value >> 10) & 0x01ff) as usize;
    (x.min(VRAM_WIDTH - 1), y.min(VRAM_HEIGHT - 1))
}

fn xy(value: u32) -> (i32, i32) {
    (
        sign_extend_11(value & 0x07ff),
        sign_extend_11((value >> 16) & 0x07ff),
    )
}

fn unsigned_xy(value: u32) -> (i32, i32) {
    (
        (value & 0x03ff).min((VRAM_WIDTH - 1) as u32) as i32,
        ((value >> 16) & 0x01ff).min((VRAM_HEIGHT - 1) as u32) as i32,
    )
}

fn dimensions(value: u32) -> (i32, i32) {
    let width = (value & 0xffff).max(1).min(VRAM_WIDTH as u32) as i32;
    let height = ((value >> 16) & 0xffff).max(1).min(VRAM_HEIGHT as u32) as i32;
    (width, height)
}

fn drawing_offset_xy(value: u32) -> (i32, i32) {
    (
        sign_extend_11(value & 0x07ff),
        sign_extend_11((value >> 11) & 0x07ff),
    )
}

fn drawing_area_xy(value: u32) -> (i32, i32) {
    (
        (value & 0x03ff).min((VRAM_WIDTH - 1) as u32) as i32,
        ((value >> 10) & 0x01ff).min((VRAM_HEIGHT - 1) as u32) as i32,
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

fn texture_coordinate(value: u32) -> TextureCoordinate {
    TextureCoordinate {
        u: (value & 0xff) as u8,
        v: ((value >> 8) & 0xff) as u8,
    }
}

fn optional_usize_json(value: Option<usize>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn should_use_observation_fallback(
    display_stats: FrameBufferStats,
    candidate_stats: FrameBufferStats,
) -> bool {
    is_sparse_display(display_stats)
        && candidate_stats.nonzero_pixels > display_stats.nonzero_pixels.saturating_mul(4)
}

fn is_sparse_display(display_stats: FrameBufferStats) -> bool {
    let sparse_display_cutoff = (DEFAULT_DISPLAY_WIDTH * DEFAULT_DISPLAY_HEIGHT / 128) as u64;
    display_stats.nonzero_pixels < sparse_display_cutoff
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
    matches!(channel, 2 | 6)
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
    pub fn tick(&mut self, cycles: u64) {
        for timer in &mut self.0 {
            timer.tick(cycles);
        }
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
}

impl Timer {
    fn tick(&mut self, cycles: u64) {
        const TIMER_COUNTER_DIVISOR: u64 = 128;

        self.cycle_accumulator = self.cycle_accumulator.saturating_add(cycles);
        let increments = self.cycle_accumulator / TIMER_COUNTER_DIVISOR;
        self.cycle_accumulator %= TIMER_COUNTER_DIVISOR;
        self.counter = self.counter.wrapping_add(increments as u16);
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
            TimerRegister::Counter => self.counter = value,
            TimerRegister::Mode => self.mode = value & 0x03ff,
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
    response: u8,
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
            response: 0xff,
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

    pub fn set_security_selects(&mut self, cat702_1_select: bool, cat702_2_select: bool) {
        if let Some(cat702) = &mut self.cat702[0] {
            cat702.write_select(cat702_1_select);
        }
        if let Some(cat702) = &mut self.cat702[1] {
            cat702.write_select(cat702_2_select);
        }
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
        self.status |= 0x0003;
    }

    fn write_control(&mut self, value: u16) {
        self.control = value;
        if value & 0x0040 != 0 {
            self.transfer_index = 0;
            self.security_transfer_index = 0;
            self.response = 0xff;
            self.status = 0x0007;
        } else if value & 0x0003 == 0x0003 {
            self.security_transfer_index = 0;
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
}

impl Default for Mdec {
    fn default() -> Self {
        Self {
            command: 0,
            control: 0,
            status: 0x8004_0000,
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
            MDEC_COMMAND => {
                self.command = value;
                self.status = mdec_ready_status();
            }
            MDEC_STATUS => {
                if value & 0x8000_0000 != 0 {
                    *self = Self::default();
                } else {
                    self.control = value & 0x6000_0000;
                    self.status = mdec_ready_status();
                }
            }
            _ => {}
        }
    }
}

fn mdec_ready_status() -> u32 {
    0x8004_0000
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
        ACCESS_WIDTH_16, ACCESS_WIDTH_32, DMA_GPU_CHCR, DMA_INTERRUPT, GPU_GP0, GPU_GP1,
        IO_REGISTER_MAP, IRQ_MASK, IRQ_STATUS, Io, IoAccess, IoDevice, MDEC_COMMAND, MDEC_STATUS,
        SIO_DATA, SPU_REGION_START, io_register, io_register_range, is_io_register_address,
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
    fn gpu_gp0_fill_rect_updates_screenshot() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0x0200_00ff);
        io.write_u32(GPU_GP0, 0x0000_0000);
        io.write_u32(GPU_GP0, 0x0008_0008);

        assert!(io.gpu.screenshot_png_base64().starts_with("iVBORw0KGgo"));
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
    fn gpu_gp0_ignores_non_command_image_data_when_fifo_is_idle() {
        let mut io = Io::default();

        io.write_u32(GPU_GP0, 0xb990_0000);
        io.write_u32(GPU_GP0, 0xb8a5_9982);

        assert_eq!(io.gpu.gp0_pending_words(), 0);
        assert_eq!(io.gpu.gp0_pending_expected_words(), None);
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
}
