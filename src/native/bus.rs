#[derive(Clone, Debug)]
pub struct Bus {
    ram: Vec<u8>,
    rom: Vec<u8>,
}

impl Bus {
    pub fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Self {
            ram: vec![0; ram_size],
            rom,
        }
    }

    pub fn read_u32(&self, address: u32) -> u32 {
        let bytes = self.read_bytes(address, 4);
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    pub fn write_u32(&mut self, address: u32, value: u32) {
        let bytes = value.to_le_bytes();
        self.write_bytes(address, &bytes);
    }

    pub fn rom_len(&self) -> usize {
        self.rom.len()
    }

    pub fn ram_len(&self) -> usize {
        self.ram.len()
    }

    fn read_bytes(&self, address: u32, len: usize) -> Vec<u8> {
        if let Some(offset) = ram_offset(address, self.ram.len()) {
            return self.ram[offset..offset + len].to_vec();
        }

        if let Some(offset) = rom_offset(address, self.rom.len()) {
            return self.rom[offset..offset + len].to_vec();
        }

        vec![0; len]
    }

    fn write_bytes(&mut self, address: u32, bytes: &[u8]) {
        if let Some(offset) = ram_offset(address, self.ram.len()) {
            self.ram[offset..offset + bytes.len()].copy_from_slice(bytes);
        }
    }
}

fn ram_offset(address: u32, ram_len: usize) -> Option<usize> {
    let masked = address & 0x1fff_ffff;
    let offset = masked as usize;
    (offset + 4 <= ram_len).then_some(offset)
}

fn rom_offset(address: u32, rom_len: usize) -> Option<usize> {
    let masked = address & 0x1fff_ffff;
    let base = 0x1fc0_0000;
    if masked < base {
        return None;
    }

    let offset = (masked - base) as usize;
    (offset + 4 <= rom_len).then_some(offset)
}
