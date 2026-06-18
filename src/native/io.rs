use crate::action::ActionButtons;

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

    pub fn json(&self) -> String {
        format!(
            "{{\"irq_status\":{},\"irq_mask\":{},\"dma_control\":{},\"dma_interrupt\":{},\"gpu_status\":{},\"gpu_commands_seen\":{},\"timer0_counter\":{},\"timer1_counter\":{},\"timer2_counter\":{},\"sio_status\":{},\"p1_state\":{}}}",
            self.irq.status,
            self.irq.mask,
            self.dma.control,
            self.dma.interrupt,
            self.gpu.status,
            self.gpu.commands_seen,
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
            GPU_GP1 => self.gpu.status,
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
        }
    }
}

impl Gpu {
    pub fn write_gp0(&mut self, value: u32) {
        self.gp0_read = value;
        self.commands_seen += 1;
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
            _ => {
                self.status = (self.status & 0x00ff_ffff) | (command << 24);
            }
        }
        self.commands_seen += 1;
    }
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
                self.channels[channel].chcr = value & !(1 << 24);
                if value & (1 << 24) != 0 {
                    self.mark_channel_complete(channel);
                }
            }
            DmaRegisterSlot::Control => self.control = value,
            DmaRegisterSlot::Interrupt => self.write_interrupt(value),
            DmaRegisterSlot::Padding(index) => self.padding[index] = value,
        }
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

#[derive(Clone, Copy, Debug, Default)]
struct DmaChannel {
    madr: u32,
    bcr: u32,
    chcr: u32,
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
}

impl Timer {
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
            response: 0xff,
        }
    }
}

impl Controller {
    fn read_u32(&self, address: u32) -> u32 {
        match address {
            SIO_DATA => self.p1_state as u32,
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
        self.status |= 0x0001;
    }

    fn write_control(&mut self, value: u16) {
        self.control = value;
        if value & 0x0040 != 0 {
            self.transfer_index = 0;
            self.response = 0xff;
            self.status = 0x0005;
        }
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
        self.p1_state = state;
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
    status: u32,
}

impl Default for Mdec {
    fn default() -> Self {
        Self {
            command: 0,
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
                self.status &= !0x2000_0000;
            }
            MDEC_STATUS => self.status = value,
            _ => {}
        }
    }
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
        ACCESS_WIDTH_16, ACCESS_WIDTH_32, DMA_GPU_CHCR, DMA_INTERRUPT, GPU_GP1, IO_REGISTER_MAP,
        IRQ_MASK, IRQ_STATUS, Io, IoAccess, IoDevice, SIO_DATA, SPU_REGION_START, io_register,
        io_register_range, is_io_register_address,
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
}
