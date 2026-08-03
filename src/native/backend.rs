use std::path::PathBuf;
use std::time::Instant;

use crate::action::ActionButtons;
use crate::backend::{Backend, BackendError, Observation};
use crate::moves::{BLOODY_ROAR_2_ROSTER, canonical_character_name};
use crate::native::emulator::{
    NativeDisplayFrame, NativeEmulator, native_aspect_corrected_gui_frame,
};
use crate::native::framebuffer::{bytes_base64, png_from_rgb888_pixels};
use crate::native::playable::{
    NativePlayableStartup, prepare_native_character_select_emulator,
    prepare_native_selected_match_emulator,
};

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
const NATIVE_P2_BEAST_X: std::ops::Range<usize> = 382..476;
const NATIVE_BEAST_Y: std::ops::Range<usize> = 431..440;

#[derive(Clone, Debug)]
pub struct NativeBackend {
    emulator: NativeEmulator,
    character_select_checkpoint: NativeEmulator,
    reset_checkpoint: NativeEmulator,
    frame: u64,
    instructions_per_frame: u64,
    player_health: f32,
    opponent_health: f32,
    beast_meter: f32,
    opponent_beast_meter: f32,
    include_screenshot: bool,
    requires_reset: bool,
    startup: NativePlayableStartup,
    last_observation_vblank: u64,
    last_observation_draw_sequence: u64,
    last_observation_frame_checksum: Option<u32>,
    p1_character: &'static str,
    p2_character: &'static str,
}

impl NativeBackend {
    pub fn from_rom_zip(
        rom_path: impl Into<PathBuf>,
        instructions_per_frame: u64,
    ) -> Result<Self, BackendError> {
        let mut emulator = NativeEmulator::from_rom_zip(rom_path.into())?;
        emulator.rom_compatibility().validate_native_runtime()?;
        configure_native_backend_boot(&mut emulator);
        let select_frames = prepare_native_character_select_emulator(&mut emulator)?;
        let character_select_checkpoint = emulator.clone();
        let match_startup = prepare_native_selected_match_emulator(&mut emulator, 0, 0)?;
        let startup = NativePlayableStartup {
            frames: select_frames.saturating_add(match_startup.frames),
            playable: match_startup.playable,
        };
        configure_native_backend_runtime(&mut emulator);
        let reset_checkpoint = emulator.clone();
        let last_observation_vblank = emulator.vblank_count();
        let last_observation_draw_sequence = emulator.draw_sequence();
        Ok(Self {
            emulator,
            character_select_checkpoint,
            reset_checkpoint,
            frame: 0,
            instructions_per_frame: instructions_per_frame.max(1),
            player_health: 1.0,
            opponent_health: 1.0,
            beast_meter: 0.0,
            opponent_beast_meter: 0.0,
            include_screenshot: false,
            requires_reset: false,
            startup,
            last_observation_vblank,
            last_observation_draw_sequence,
            last_observation_frame_checksum: None,
            p1_character: BLOODY_ROAR_2_ROSTER[0],
            p2_character: BLOODY_ROAR_2_ROSTER[0],
        })
    }

    fn make_observation(
        &mut self,
        requested_buttons: ActionButtons,
        activity_baseline: crate::native::NativeInputActivity,
        emulation_elapsed_seconds: Option<f64>,
    ) -> Observation {
        let (display_frame, frame_checksum) =
            self.emulator.fast_gameplay_window_frame_with_checksum();
        let native_playable_candidate = native_backend_playable_candidate(
            self.emulator.native_playable_candidate(),
            self.startup.playable,
            self.emulator
                .br2_native_credit_hle_game_start_accepted_seen(),
            !self.emulator.is_terminal(),
        );
        self.update_hud_observation(native_playable_candidate, &display_frame);
        let input_activity = self.emulator.input_activity();
        let input_delta = input_activity.saturating_subtracted(activity_baseline);
        let current_vblank = self.emulator.vblank_count();
        let current_draw_sequence = self.emulator.draw_sequence();
        let vblank_delta = current_vblank.saturating_sub(self.last_observation_vblank);
        let emulated_fps =
            if let Some(elapsed) = emulation_elapsed_seconds.filter(|elapsed| *elapsed > 0.0) {
                (vblank_delta as f64 / elapsed) as f32
            } else {
                0.0
            };
        let frame_changed = self
            .last_observation_frame_checksum
            .is_some_and(|previous| previous != frame_checksum);
        let draw_progressed = current_draw_sequence > self.last_observation_draw_sequence;
        let render_progressing = vblank_delta > 0 && (frame_changed || draw_progressed);
        let effects_requested = requested_buttons.punch
            || requested_buttons.kick
            || requested_buttons.beast
            || requested_buttons.p2_punch
            || requested_buttons.p2_kick
            || requested_buttons.p2_beast;
        let effects_progressing = effects_requested && render_progressing;
        let audio_health = self.emulator.audio_health();
        let audio_progressing = audio_health.is_some_and(|health| health.render_progressing);
        let audio_health_json = audio_health.map_or_else(
            || "{\"state\":\"unavailable\",\"render_progressing\":false}".to_string(),
            |health| health.json(),
        );
        let screenshot_b64 = self.include_screenshot.then(|| {
            let gui_frame = native_aspect_corrected_gui_frame(&display_frame);
            bytes_base64(&png_from_rgb888_pixels(
                gui_frame.width,
                gui_frame.height,
                &gui_frame.pixels,
            ))
        });
        self.last_observation_vblank = current_vblank;
        self.last_observation_draw_sequence = current_draw_sequence;
        self.last_observation_frame_checksum = Some(frame_checksum);
        Observation {
            frame: self.frame,
            player_health: self.player_health,
            opponent_health: self.opponent_health,
            beast_meter: self.beast_meter,
            opponent_beast_meter: self.opponent_beast_meter,
            round_time: (99.0 - (self.frame as f32 / 60.0)).max(0.0),
            terminal: self.emulator.is_terminal()
                || self.player_health <= f32::EPSILON
                || self.opponent_health <= f32::EPSILON,
            native_playable: native_playable_candidate,
            rendered_frame_checksum: frame_checksum,
            render_progressing,
            effects_progressing,
            audio_progressing,
            emulated_fps,
            screenshot_b64,
            info_json: format!(
                "{{\"requested_buttons\":{},\"input_activity\":{},\"input_activity_delta\":{},\"native_playable_candidate\":{},\"runtime\":{{\"vblank\":{},\"vblank_delta\":{},\"draw_sequence\":{},\"frame_checksum\":{},\"frame_changed\":{},\"draw_progressed\":{},\"render_progressing\":{},\"effects_requested\":{},\"effects_progressing\":{},\"audio_progressing\":{},\"emulated_fps\":{},\"audio\":{}}},\"players\":{{\"p1\":{{\"character\":\"{}\",\"health\":{},\"beast_meter\":{}}},\"p2\":{{\"character\":\"{}\",\"health\":{},\"beast_meter\":{}}}}},\"startup\":{{\"mode\":\"two_player_character_checkpoint\",\"frames\":{},\"playable\":{}}},\"screenshot\":{{\"included\":{},\"width\":{},\"height\":{},\"source\":\"native_fast_gameplay_gui_aspect_corrected\"}},\"guest_input_proof\":{{\"any\":{},\"play_controls\":{},\"full_controls\":{},\"p1_any\":{},\"p1_play_controls\":{},\"p1_full_controls\":{},\"p2_any\":{},\"p2_play_controls\":{},\"p2_full_controls\":{},\"combined_any\":{},\"combined_full_controls\":{}}}}}",
                requested_buttons.json(),
                input_activity.json(),
                input_delta.json(),
                native_playable_candidate,
                current_vblank,
                vblank_delta,
                current_draw_sequence,
                frame_checksum,
                frame_changed,
                draw_progressed,
                render_progressing,
                effects_requested,
                effects_progressing,
                audio_progressing,
                emulated_fps,
                audio_health_json,
                self.p1_character,
                self.player_health,
                self.beast_meter,
                self.p2_character,
                self.opponent_health,
                self.opponent_beast_meter,
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

    fn update_hud_observation(
        &mut self,
        native_playable_candidate: bool,
        frame: &NativeDisplayFrame,
    ) {
        if !native_playable_candidate {
            return;
        }
        if let Some(health) = native_hud_bar_fill(frame, NATIVE_P1_HEALTH_X, NATIVE_HEALTH_Y) {
            self.player_health = health;
        }
        if let Some(health) = native_hud_bar_fill(frame, NATIVE_P2_HEALTH_X, NATIVE_HEALTH_Y) {
            self.opponent_health = health;
        }
        if let Some(meter) = native_hud_bar_fill(frame, NATIVE_P1_BEAST_X, NATIVE_BEAST_Y) {
            self.beast_meter = meter;
        }
        if let Some(meter) = native_hud_bar_fill(frame, NATIVE_P2_BEAST_X, NATIVE_BEAST_Y) {
            self.opponent_beast_meter = meter;
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

    fn diagnostic_json(&self) -> Option<String> {
        Some(self.emulator.diagnostic_json())
    }

    fn reset(&mut self) -> Result<Observation, BackendError> {
        self.emulator = self.reset_checkpoint.clone();
        self.frame = 0;
        self.player_health = 1.0;
        self.opponent_health = 1.0;
        self.beast_meter = 0.0;
        self.opponent_beast_meter = 0.0;
        self.requires_reset = false;
        self.last_observation_vblank = self.emulator.vblank_count();
        self.last_observation_draw_sequence = self.emulator.draw_sequence();
        self.last_observation_frame_checksum = None;
        let activity_baseline = self.emulator.input_activity();
        Ok(self.make_observation(ActionButtons::default(), activity_baseline, None))
    }

    fn reset_match(
        &mut self,
        p1_character: &str,
        p2_character: &str,
    ) -> Result<Observation, BackendError> {
        let p1_character = canonical_character_name(p1_character).ok_or_else(|| {
            BackendError::new(format!("unknown Bloody Roar 2 character: {p1_character}"))
        })?;
        let p2_character = canonical_character_name(p2_character).ok_or_else(|| {
            BackendError::new(format!("unknown Bloody Roar 2 character: {p2_character}"))
        })?;
        if self.p1_character == p1_character && self.p2_character == p2_character {
            return self.reset();
        }

        let p1_slot = BLOODY_ROAR_2_ROSTER
            .iter()
            .position(|character| *character == p1_character)
            .expect("canonical P1 character is in the roster");
        let p2_slot = BLOODY_ROAR_2_ROSTER
            .iter()
            .position(|character| *character == p2_character)
            .expect("canonical P2 character is in the roster");
        let mut emulator = self.character_select_checkpoint.clone();
        let startup = prepare_native_selected_match_emulator(&mut emulator, p1_slot, p2_slot)?;
        configure_native_backend_runtime(&mut emulator);
        self.emulator = emulator;
        self.reset_checkpoint = self.emulator.clone();
        self.startup = startup;
        self.p1_character = p1_character;
        self.p2_character = p2_character;
        self.reset()
    }

    fn active_characters(&self) -> Option<(&'static str, &'static str)> {
        Some((self.p1_character, self.p2_character))
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
        let step_started_at = Instant::now();
        self.emulator.set_input(buttons);
        for _ in 0..requested_frames {
            let advanced = self.advance_one_vblank()?;
            self.frame = self.frame.saturating_add(advanced);
            if self.emulator.is_terminal() {
                break;
            }
        }
        Ok(self.make_observation(
            buttons,
            activity_baseline,
            Some(step_started_at.elapsed().as_secs_f64()),
        ))
    }

    fn observe(&mut self) -> Result<Observation, BackendError> {
        let activity_baseline = self.emulator.input_activity();
        Ok(self.make_observation(ActionButtons::default(), activity_baseline, None))
    }
}

fn native_backend_playable_candidate(
    emulator_candidate: bool,
    startup_playable: bool,
    game_start_accepted: bool,
    runtime_running: bool,
) -> bool {
    emulator_candidate || (startup_playable && game_start_accepted && runtime_running)
}

fn configure_native_backend_boot(emulator: &mut NativeEmulator) {
    emulator.apply_auto_coin_input_mapping();
}

fn configure_native_backend_runtime(emulator: &mut NativeEmulator) {
    emulator.set_native_otc_dma_recovery_interval(NATIVE_BACKEND_OTC_RECOVERY_INTERVAL);
    emulator.set_vblank_presentation_capture_interval(Some(NATIVE_BACKEND_VBLANK_CAPTURE_INTERVAL));
    let (replay_enabled, replay_interval) = native_backend_unlinked_primitive_replay_settings();
    emulator.set_unlinked_primitive_replay_enabled(replay_enabled);
    emulator.set_unlinked_primitive_replay_interval(replay_interval);
}

fn native_backend_unlinked_primitive_replay_settings() -> (bool, Option<u64>) {
    // Match interactive play and headless roster QA: automatic replay is
    // enabled, but only the stale/sparse-scene policy may trigger it.
    (true, None)
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
        NativeDisplayFrame, native_backend_playable_candidate,
        native_backend_unlinked_primitive_replay_settings, native_hud_bar_fill,
    };

    #[test]
    fn native_backend_keeps_conditional_primitive_recovery_enabled() {
        assert_eq!(
            native_backend_unlinked_primitive_replay_settings(),
            (true, None)
        );
    }

    #[test]
    fn native_backend_accepts_a_proven_rendered_match_when_gpu_heuristic_is_false() {
        assert!(native_backend_playable_candidate(false, true, true, true));
        assert!(native_backend_playable_candidate(true, false, false, false));
        assert!(!native_backend_playable_candidate(false, false, true, true));
        assert!(!native_backend_playable_candidate(false, true, false, true));
        assert!(!native_backend_playable_candidate(false, true, true, false));
    }

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
