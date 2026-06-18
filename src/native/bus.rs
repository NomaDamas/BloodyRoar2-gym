use crate::native::io::Io;

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
        self.read_bytes(address, 1)[0]
    }

    pub fn read_u16(&self, address: u32) -> u16 {
        let bytes = self.read_bytes(address, 2);
        u16::from_le_bytes([bytes[0], bytes[1]])
    }

    pub fn read_u32(&self, address: u32) -> u32 {
        if let Some(io_address) = io_address(address) {
            return self.io.read_u32(io_address);
        }

        let bytes = self.read_bytes(address, 4);
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    pub fn write_u8(&mut self, address: u32, value: u8) {
        self.write_bytes(address, &[value]);
    }

    pub fn write_u16(&mut self, address: u32, value: u16) {
        self.write_bytes(address, &value.to_le_bytes());
    }

    pub fn write_u32(&mut self, address: u32, value: u32) {
        if let Some(io_address) = io_address(address) {
            self.io.write_u32(io_address, value);
            return;
        }

        let bytes = value.to_le_bytes();
        self.write_bytes(address, &bytes);
    }

    pub fn rom_len(&self) -> usize {
        self.rom.len()
    }

    pub fn ram_len(&self) -> usize {
        self.ram.len()
    }

    pub fn io_json(&self) -> String {
        format!(
            "{{\"irq_status\":{},\"irq_mask\":{},\"gpu_status\":{},\"gpu_commands_seen\":{},\"p1_state\":{}}}",
            self.io.irq.status,
            self.io.irq.mask,
            self.io.gpu.status,
            self.io.gpu.commands_seen,
            self.io.controller.p1_state
        )
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
    (0x1f80_1000..=0x1f80_1fff)
        .contains(&physical)
        .then_some(physical)
}

fn physical_address(address: u32) -> u32 {
    address & 0x1fff_ffff
}
