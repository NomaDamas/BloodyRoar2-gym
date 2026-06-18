use crate::action::ActionButtons;

#[derive(Clone, Debug, Default)]
pub struct Io {
    pub irq: InterruptController,
    pub gpu: Gpu,
    pub dma: Dma,
    pub controller: Controller,
}

impl Io {
    pub fn read_u32(&self, address: u32) -> u32 {
        match address {
            0x1f80_1070 => self.irq.status,
            0x1f80_1074 => self.irq.mask,
            0x1f80_1810 => self.gpu.gp0_read,
            0x1f80_1814 => self.gpu.status,
            0x1f80_1080..=0x1f80_10ff => self.dma.read_u32(address),
            0x1f80_1040 => self.controller.p1_state as u32,
            _ => 0,
        }
    }

    pub fn write_u32(&mut self, address: u32, value: u32) {
        match address {
            0x1f80_1070 => self.irq.status &= value,
            0x1f80_1074 => self.irq.mask = value,
            0x1f80_1810 => self.gpu.write_gp0(value),
            0x1f80_1814 => self.gpu.write_gp1(value),
            0x1f80_1080..=0x1f80_10ff => self.dma.write_u32(address, value),
            0x1f80_1040 => self.controller.last_write = value as u16,
            _ => {}
        }
    }

    pub fn set_input(&mut self, buttons: ActionButtons) {
        self.controller.set_buttons(buttons);
    }
}

#[derive(Clone, Debug)]
pub struct InterruptController {
    pub status: u32,
    pub mask: u32,
}

impl Default for InterruptController {
    fn default() -> Self {
        Self { status: 0, mask: 0 }
    }
}

#[derive(Clone, Debug)]
pub struct Gpu {
    pub gp0_read: u32,
    pub status: u32,
    pub commands_seen: u64,
}

impl Default for Gpu {
    fn default() -> Self {
        Self {
            gp0_read: 0,
            status: 0x1480_2000,
            commands_seen: 0,
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
        if command == 0x00 {
            *self = Self::default();
        } else {
            self.status = (self.status & 0x00ff_ffff) | (command << 24);
        }
        self.commands_seen += 1;
    }
}

#[derive(Clone, Debug)]
pub struct Dma {
    registers: [u32; 32],
}

impl Default for Dma {
    fn default() -> Self {
        Self { registers: [0; 32] }
    }
}

impl Dma {
    pub fn read_u32(&self, address: u32) -> u32 {
        self.registers[index(address)]
    }

    pub fn write_u32(&mut self, address: u32, value: u32) {
        self.registers[index(address)] = value;
    }
}

fn index(address: u32) -> usize {
    (((address - 0x1f80_1080) / 4) as usize).min(31)
}

#[derive(Clone, Debug)]
pub struct Controller {
    pub p1_state: u16,
    pub last_write: u16,
}

impl Default for Controller {
    fn default() -> Self {
        Self {
            p1_state: 0xffff,
            last_write: 0,
        }
    }
}

impl Controller {
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

fn clear_if(state: &mut u16, mask: u16, pressed: bool) {
    if pressed {
        *state &= !mask;
    }
}
