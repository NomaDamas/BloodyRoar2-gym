use std::path::PathBuf;

use crate::action::ActionButtons;
use crate::backend::{Backend, BackendError, Observation};
use crate::native::emulator::NativeEmulator;

const NATIVE_BACKEND_VBLANK_CAPTURE_INTERVAL: u64 = 6_000;
const NATIVE_BACKEND_UNLINKED_PRIMITIVE_REPLAY_INTERVAL: u64 = 5;

#[derive(Clone, Debug)]
pub struct NativeBackend {
    emulator: NativeEmulator,
    rom_path: PathBuf,
    frame: u64,
    instructions_per_frame: u64,
}

impl NativeBackend {
    pub fn from_rom_zip(
        rom_path: impl Into<PathBuf>,
        instructions_per_frame: u64,
    ) -> Result<Self, BackendError> {
        let rom_path = rom_path.into();
        let mut emulator = NativeEmulator::from_rom_zip(rom_path.clone())?;
        emulator.rom_compatibility().validate_native_runtime()?;
        configure_native_backend_emulator(&mut emulator);
        Ok(Self {
            emulator,
            rom_path,
            frame: 0,
            instructions_per_frame: instructions_per_frame.max(1),
        })
    }

    fn observe(&self) -> Observation {
        Observation {
            frame: self.frame,
            player_health: 1.0,
            opponent_health: 1.0,
            beast_meter: self.emulator.progress_signal(),
            round_time: (99.0 - (self.frame as f32 / 60.0)).max(0.0),
            terminal: self.emulator.is_terminal(),
            screenshot_b64: Some(self.emulator.display_window_png_base64()),
        }
    }
}

impl Backend for NativeBackend {
    fn reset(&mut self) -> Result<Observation, BackendError> {
        self.emulator = NativeEmulator::from_rom_zip(self.rom_path.clone())?;
        self.emulator
            .rom_compatibility()
            .validate_native_runtime()?;
        configure_native_backend_emulator(&mut self.emulator);
        self.frame = 0;
        Ok(self.observe())
    }

    fn step(&mut self, buttons: ActionButtons, frames: u32) -> Result<Observation, BackendError> {
        let frames = frames.max(1) as u64;
        self.emulator.set_input(buttons);
        for _ in 0..frames {
            self.emulator.step_instructions(self.instructions_per_frame);
            self.frame += 1;
            if self.emulator.is_terminal() {
                break;
            }
        }
        Ok(self.observe())
    }
}

fn configure_native_backend_emulator(emulator: &mut NativeEmulator) {
    emulator.apply_auto_coin_input_mapping();
    emulator.set_vblank_presentation_capture_interval(Some(NATIVE_BACKEND_VBLANK_CAPTURE_INTERVAL));
    emulator.set_unlinked_primitive_replay_interval(Some(
        NATIVE_BACKEND_UNLINKED_PRIMITIVE_REPLAY_INTERVAL,
    ));
}
