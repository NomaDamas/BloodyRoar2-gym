use std::path::PathBuf;

use crate::action::ActionButtons;
use crate::backend::{Backend, BackendError, Observation};
use crate::native::emulator::{NativeDisplayFrame, NativeEmulator};
use crate::native::playable::{NativePlayableStartup, prepare_native_playable_emulator};

const NATIVE_BACKEND_VBLANK_CAPTURE_INTERVAL: u64 = 6_000;
const NATIVE_BACKEND_OTC_RECOVERY_INTERVAL: u64 = 1;
const NATIVE_BACKEND_MIN_VBLANK_INSTRUCTION_BUDGET: u64 = 2_000_000;
const NATIVE_HUD_WIDTH: usize = 512;
const NATIVE_HUD_HEIGHT: usize = 480;
const NATIVE_OBSERVATION_SCREENSHOT_WIDTH: usize = 640;
const NATIVE_OBSERVATION_SCREENSHOT_HEIGHT: usize = 480;
const NATIVE_P1_HEALTH_X: std::ops::Range<usize> = 35..230;
const NATIVE_P2_HEALTH_X: std::ops::Range<usize> = 282..477;
const NATIVE_HEALTH_Y: std::ops::Range<usize> = 42..51;
const NATIVE_P1_BEAST_X: std::ops::Range<usize> = 36..130;
const NATIVE_BEAST_Y: std::ops::Range<usize> = 431..440;

#[derive(Clone, Debug)]
pub struct NativeBackend {
    emulator: NativeEmulator,
    reset_checkpoint: NativeEmulator,
    frame: u64,
    instructions_per_frame: u64,
    player_health: f32,
    opponent_health: f32,
    beast_meter: f32,
    include_screenshot: bool,
    requires_reset: bool,
    startup: NativePlayableStartup,
}

impl NativeBackend {
    pub fn from_rom_zip(
        rom_path: impl Into<PathBuf>,
        instructions_per_frame: u64,
    ) -> Result<Self, BackendError> {
        let mut emulator = NativeEmulator::from_rom_zip(rom_path.into())?;
        emulator.rom_compatibility().validate_native_runtime()?;
        configure_native_backend_emulator(&mut emulator);
        let startup = prepare_native_playable_emulator(&mut emulator)?;
        let reset_checkpoint = emulator.clone();
        Ok(Self {
            emulator,
            reset_checkpoint,
            frame: 0,
            instructions_per_frame: instructions_per_frame.max(1),
            player_health: 1.0,
            opponent_health: 1.0,
            beast_meter: 0.0,
            include_screenshot: false,
            requires_reset: false,
            startup,
        })
    }

    fn observe(
        &mut self,
        requested_buttons: ActionButtons,
        activity_baseline: crate::native::NativeInputActivity,
    ) -> Observation {
        let native_playable_candidate = self.emulator.native_playable_candidate();
        self.update_hud_observation(native_playable_candidate);
        let input_activity = self.emulator.input_activity();
        let input_delta = input_activity.saturating_subtracted(activity_baseline);
        let screenshot_b64 = self
            .include_screenshot
            .then(|| self.emulator.display_gui_png_base64());
        Observation {
            frame: self.frame,
            player_health: self.player_health,
            opponent_health: self.opponent_health,
            beast_meter: self.beast_meter,
            round_time: (99.0 - (self.frame as f32 / 60.0)).max(0.0),
            terminal: self.emulator.is_terminal()
                || self.player_health <= f32::EPSILON
                || self.opponent_health <= f32::EPSILON,
            screenshot_b64,
            info_json: format!(
                "{{\"requested_buttons\":{},\"input_activity\":{},\"input_activity_delta\":{},\"native_playable_candidate\":{},\"startup\":{{\"mode\":\"playable_checkpoint\",\"frames\":{},\"playable\":{}}},\"screenshot\":{{\"included\":{},\"width\":{},\"height\":{},\"source\":\"native_gui_aspect_corrected\"}},\"guest_input_proof\":{{\"any\":{},\"play_controls\":{},\"full_controls\":{},\"p1_any\":{},\"p1_play_controls\":{},\"p1_full_controls\":{},\"p2_any\":{},\"p2_play_controls\":{},\"p2_full_controls\":{},\"combined_any\":{},\"combined_full_controls\":{}}}}}",
                requested_buttons.json(),
                input_activity.json(),
                input_delta.json(),
                native_playable_candidate,
                self.startup.frames,
                self.startup.playable,
                self.include_screenshot,
                NATIVE_OBSERVATION_SCREENSHOT_WIDTH,
                NATIVE_OBSERVATION_SCREENSHOT_HEIGHT,
                input_delta.has_any_control_activity(),
                input_delta.has_play_control_activity(),
                input_delta.has_full_control_activity(),
                input_delta.has_p1_any_control_activity(),
                input_delta.has_p1_play_control_activity(),
                input_delta.has_p1_full_control_activity(),
                input_delta.has_p2_any_control_activity(),
                input_delta.has_p2_play_control_activity(),
                input_delta.has_p2_full_control_activity(),
                input_delta.has_any_control_activity(),
                input_delta.has_combined_full_control_activity(),
            ),
        }
    }

    fn update_hud_observation(&mut self, native_playable_candidate: bool) {
        if !native_playable_candidate {
            return;
        }
        let frame = self.emulator.display_frame();
        if let Some(health) = native_hud_bar_fill(&frame, NATIVE_P1_HEALTH_X, NATIVE_HEALTH_Y) {
            self.player_health = health;
        }
        if let Some(health) = native_hud_bar_fill(&frame, NATIVE_P2_HEALTH_X, NATIVE_HEALTH_Y) {
            self.opponent_health = health;
        }
        if let Some(meter) = native_hud_bar_fill(&frame, NATIVE_P1_BEAST_X, NATIVE_BEAST_Y) {
            self.beast_meter = meter;
        }
    }

    fn advance_one_vblank(&mut self) -> Result<u64, BackendError> {
        let start_vblank = self.emulator.vblank_count();
        let instruction_budget = self
            .instructions_per_frame
            .max(NATIVE_BACKEND_MIN_VBLANK_INSTRUCTION_BUDGET);
        let mut executed = 0_u64;

        while self.emulator.vblank_count() == start_vblank
            && !self.emulator.is_terminal()
            && executed < instruction_budget
        {
            let remaining = instruction_budget.saturating_sub(executed);
            let batch = self.instructions_per_frame.min(remaining).max(1);
            let batch_executed = self
                .emulator
                .step_instructions_until_vblank_untracked(batch);
            executed = executed.saturating_add(batch_executed);
            if batch_executed == 0 {
                break;
            }
        }

        let advanced = self.emulator.vblank_count().saturating_sub(start_vblank);
        if advanced == 0 {
            self.requires_reset = true;
            return Err(BackendError::new(format!(
                "native backend did not reach the next vblank within {instruction_budget} instructions; reset is required before the next step"
            )));
        }
        Ok(advanced)
    }
}

impl Backend for NativeBackend {
    fn set_observation_screenshot(&mut self, enabled: bool) {
        self.include_screenshot = enabled;
    }

    fn reset(&mut self) -> Result<Observation, BackendError> {
        self.emulator = self.reset_checkpoint.clone();
        self.frame = 0;
        self.player_health = 1.0;
        self.opponent_health = 1.0;
        self.beast_meter = 0.0;
        self.requires_reset = false;
        let activity_baseline = self.emulator.input_activity();
        Ok(self.observe(ActionButtons::default(), activity_baseline))
    }

    fn step(&mut self, buttons: ActionButtons, frames: u32) -> Result<Observation, BackendError> {
        if self.requires_reset {
            return Err(BackendError::new(
                "native backend requires reset after a failed vblank step",
            ));
        }
        if frames == 0 || frames > crate::env::MAX_STEP_FRAMES {
            return Err(BackendError::new(format!(
                "frames must be between 1 and {}",
                crate::env::MAX_STEP_FRAMES
            )));
        }
        let requested_frames = frames as u64;
        let activity_baseline = self.emulator.input_activity();
        self.emulator.set_input(buttons);
        for _ in 0..requested_frames {
            let advanced = self.advance_one_vblank()?;
            self.frame = self.frame.saturating_add(advanced);
            if self.emulator.is_terminal() {
                break;
            }
        }
        Ok(self.observe(buttons, activity_baseline))
    }
}

fn configure_native_backend_emulator(emulator: &mut NativeEmulator) {
    emulator.apply_auto_coin_input_mapping();
    emulator.set_native_otc_dma_recovery_interval(NATIVE_BACKEND_OTC_RECOVERY_INTERVAL);
    emulator.set_vblank_presentation_capture_interval(Some(NATIVE_BACKEND_VBLANK_CAPTURE_INTERVAL));
    emulator.set_unlinked_primitive_replay_enabled(false);
    emulator.set_unlinked_primitive_replay_interval(None);
}

fn native_hud_bar_fill(
    frame: &NativeDisplayFrame,
    x_range: std::ops::Range<usize>,
    y_range: std::ops::Range<usize>,
) -> Option<f32> {
    if frame.width < NATIVE_HUD_WIDTH
        || frame.height < NATIVE_HUD_HEIGHT
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
        || x_range.is_empty()
        || y_range.is_empty()
    {
        return None;
    }

    let active_columns = x_range
        .clone()
        .filter(|&x| {
            let active_pixels = y_range
                .clone()
                .filter(|&y| native_hud_meter_pixel(frame.pixels[y * frame.width + x]))
                .count();
            active_pixels * 2 >= y_range.len()
        })
        .count();
    Some(active_columns as f32 / x_range.len() as f32)
}

fn native_hud_meter_pixel(pixel: u32) -> bool {
    let red = ((pixel >> 16) & 0xff) as i32;
    let green = ((pixel >> 8) & 0xff) as i32;
    let blue = (pixel & 0xff) as i32;
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    maximum >= 80
        && maximum - minimum >= 24
        && (red >= blue.saturating_add(20) || green >= blue.saturating_add(20))
}

#[cfg(test)]
mod tests {
    use super::{
        NATIVE_HEALTH_Y, NATIVE_HUD_HEIGHT, NATIVE_HUD_WIDTH, NATIVE_P1_HEALTH_X,
        NativeDisplayFrame, native_hud_bar_fill,
    };

    #[test]
    fn native_hud_bar_fill_tracks_colored_meter_columns() {
        let mut frame = NativeDisplayFrame {
            width: NATIVE_HUD_WIDTH,
            height: NATIVE_HUD_HEIGHT,
            pixels: vec![0; NATIVE_HUD_WIDTH * NATIVE_HUD_HEIGHT],
        };
        let filled = NATIVE_P1_HEALTH_X.len() / 2;
        for y in NATIVE_HEALTH_Y {
            for x in NATIVE_P1_HEALTH_X.start..NATIVE_P1_HEALTH_X.start + filled {
                frame.pixels[y * frame.width + x] = 0x00e0_c020;
            }
        }

        let fill = native_hud_bar_fill(&frame, NATIVE_P1_HEALTH_X, NATIVE_HEALTH_Y)
            .expect("valid HUD frame");

        assert!((fill - 0.5).abs() < 0.01);
    }

    #[test]
    fn native_hud_bar_fill_rejects_partial_frames() {
        let frame = NativeDisplayFrame {
            width: 512,
            height: 240,
            pixels: vec![0; 512 * 240],
        };

        assert_eq!(
            native_hud_bar_fill(&frame, NATIVE_P1_HEALTH_X, NATIVE_HEALTH_Y),
            None
        );
    }
}
