use crate::native::io::{IO_REGION_END, IO_REGION_START, Io, io_access_for};
use crate::native::platform::{NativePlatformOps, PreferredNativePlatform};

#[derive(Clone, Debug)]
pub struct Bus {
    ram: Vec<u8>,
    rom: Vec<u8>,
    pub io: Io,
}

impl Bus {
    pub fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Self {
            ram: vec![0; ram_size],
            rom,
            io: Io::default(),
        }
    }

    pub fn read_u8(&self, address: u32) -> u8 {
        if let Some(io_address) = mapped_io_address(address, 1) {
            return self.io.read_u8(io_address);
        }

        self.read_bytes(address, 1)[0]
    }

    pub fn read_u16(&self, address: u32) -> u16 {
        if let Some(io_address) = mapped_io_address(address, 2) {
            return self.io.read_u16(io_address);
        }

        let bytes = self.read_bytes(address, 2);
        PreferredNativePlatform::read_le_u16(&bytes)
    }

    pub fn read_u32(&self, address: u32) -> u32 {
        if let Some(io_address) = mapped_io_address(address, 4) {
            return self.io.read_u32(io_address);
        }

        let bytes = self.read_bytes(address, 4);
        PreferredNativePlatform::read_le_u32(&bytes)
    }

    pub fn write_u8(&mut self, address: u32, value: u8) {
        if let Some(io_address) = mapped_io_address(address, 1) {
            self.io.write_u8(io_address, value);
            return;
        }

        self.write_bytes(address, &[value]);
    }

    pub fn write_u16(&mut self, address: u32, value: u16) {
        if let Some(io_address) = mapped_io_address(address, 2) {
            self.io.write_u16(io_address, value);
            return;
        }

        self.write_bytes(address, &PreferredNativePlatform::write_le_u16(value));
    }

    pub fn write_u32(&mut self, address: u32, value: u32) {
        if let Some(io_address) = mapped_io_address(address, 4) {
            self.io.write_u32(io_address, value);
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

    pub fn io_json(&self) -> String {
        self.io.json()
    }

    pub fn set_input(&mut self, buttons: crate::action::ActionButtons) {
        self.io.set_input(buttons);
    }

    fn read_bytes(&self, address: u32, len: usize) -> Vec<u8> {
        if let Some(offset) = ram_offset(address, self.ram.len(), len) {
            return self.ram[offset..offset + len].to_vec();
        }

        if let Some(offset) = rom_offset(address, self.rom.len(), len) {
            return self.rom[offset..offset + len].to_vec();
        }

        vec![0; len]
    }

    fn write_bytes(&mut self, address: u32, bytes: &[u8]) {
        if let Some(offset) = ram_offset(address, self.ram.len(), bytes.len()) {
            self.ram[offset..offset + bytes.len()].copy_from_slice(bytes);
        }
    }
}

fn ram_offset(address: u32, ram_len: usize, access_len: usize) -> Option<usize> {
    let physical = physical_address(address);
    let offset = physical as usize;
    (offset + access_len <= ram_len).then_some(offset)
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

fn physical_address(address: u32) -> u32 {
    address & 0x1fff_ffff
}

#[cfg(test)]
mod tests {
    use super::Bus;
    use crate::native::io::{
        DMA_GPU_MADR, GPU_GP0, IRQ_MASK, IRQ_STATUS, SIO_DATA, SPU_REGION_START,
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

        bus.write_u8(SIO_DATA, 0x5a);

        assert_eq!(bus.io.controller.last_write, 0x005a);
        assert_eq!(bus.read_u8(SIO_DATA), 0xcd);
        assert_eq!(bus.read_u8(SIO_DATA + 1), 0xab);
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
    fn bus_preserves_mapped_but_unmodeled_register_range_state() {
        let mut bus = Bus::new(Vec::new(), 2 * 1024 * 1024);

        bus.write_u16(SPU_REGION_START + 2, 0xbeef);

        assert_eq!(bus.read_u16(SPU_REGION_START + 2), 0xbeef);
        assert_eq!(bus.read_u16(SPU_REGION_START + 4), 0);
    }
}
