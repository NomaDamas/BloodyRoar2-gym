use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::action::ActionButtons;
use crate::moves::{BLOODY_ROAR_2_ROSTER, canonical_character_name};

#[derive(Clone, Debug)]
pub struct Observation {
    pub frame: u64,
    pub player_health: f32,
    pub opponent_health: f32,
    pub beast_meter: f32,
    pub opponent_beast_meter: f32,
    pub round_time: f32,
    pub terminal: bool,
    pub native_playable: bool,
    pub rendered_frame_checksum: u32,
    pub render_progressing: bool,
    pub effects_progressing: bool,
    pub audio_progressing: bool,
    pub emulated_fps: f32,
    pub screenshot_b64: Option<String>,
    pub info_json: String,
}

impl Observation {
    pub fn json(&self) -> String {
        let screenshot = match &self.screenshot_b64 {
            Some(value) => format!("\"{}\"", escape_json(value)),
            None => "null".to_string(),
        };

        format!(
            "{{\"frame\":{},\"player_health\":{},\"opponent_health\":{},\"beast_meter\":{},\"opponent_beast_meter\":{},\"round_time\":{},\"terminal\":{},\"native_playable\":{},\"rendered_frame_checksum\":{},\"render_progressing\":{},\"effects_progressing\":{},\"audio_progressing\":{},\"emulated_fps\":{},\"screenshot_b64\":{}}}",
            self.frame,
            self.player_health,
            self.opponent_health,
            self.beast_meter,
            self.opponent_beast_meter,
            self.round_time,
            self.terminal,
            self.native_playable,
            self.rendered_frame_checksum,
            self.render_progressing,
            self.effects_progressing,
            self.audio_progressing,
            self.emulated_fps,
            screenshot
        )
    }
}

#[derive(Debug)]
pub struct BackendError {
    message: String,
}

impl BackendError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for BackendError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for BackendError {}

pub trait Backend {
    fn set_observation_screenshot(&mut self, _enabled: bool) {}

    fn diagnostic_json(&self) -> Option<String> {
        None
    }

    fn reset(&mut self) -> Result<Observation, BackendError>;
    fn reset_match(
        &mut self,
        _p1_character: &str,
        _p2_character: &str,
    ) -> Result<Observation, BackendError> {
        Err(BackendError::new(
            "character-select reset is not supported by this backend",
        ))
    }
    fn active_characters(&self) -> Option<(&'static str, &'static str)> {
        None
    }
    fn step(&mut self, buttons: ActionButtons, frames: u32) -> Result<Observation, BackendError>;
    fn observe(&mut self) -> Result<Observation, BackendError> {
        Err(BackendError::new(
            "current observation is not supported by this backend",
        ))
    }
}

#[derive(Debug)]
pub struct NullBackend {
    frame: u64,
    player_health: f32,
    opponent_health: f32,
    beast_meter: f32,
    p1_character: &'static str,
    p2_character: &'static str,
}

impl Default for NullBackend {
    fn default() -> Self {
        Self {
            frame: 0,
            player_health: 1.0,
            opponent_health: 1.0,
            beast_meter: 0.0,
            p1_character: BLOODY_ROAR_2_ROSTER[0],
            p2_character: BLOODY_ROAR_2_ROSTER[0],
        }
    }
}

impl NullBackend {
    fn make_observation(&self) -> Observation {
        Observation {
            frame: self.frame,
            player_health: self.player_health,
            opponent_health: self.opponent_health,
            beast_meter: self.beast_meter,
            opponent_beast_meter: self.beast_meter,
            round_time: (99.0 - (self.frame as f32 / 60.0)).max(0.0),
            terminal: self.player_health <= 0.0 || self.opponent_health <= 0.0,
            native_playable: true,
            rendered_frame_checksum: self.frame as u32,
            render_progressing: self.frame > 0,
            effects_progressing: false,
            audio_progressing: false,
            emulated_fps: 60.0,
            screenshot_b64: None,
            info_json: format!(
                "{{\"players\":{{\"p1\":{{\"character\":\"{}\"}},\"p2\":{{\"character\":\"{}\"}}}}}}",
                self.p1_character, self.p2_character
            ),
        }
    }
}

impl Backend for NullBackend {
    fn reset(&mut self) -> Result<Observation, BackendError> {
        let p1_character = self.p1_character;
        let p2_character = self.p2_character;
        *self = Self {
            p1_character,
            p2_character,
            ..Self::default()
        };
        Ok(self.make_observation())
    }

    fn reset_match(
        &mut self,
        p1_character: &str,
        p2_character: &str,
    ) -> Result<Observation, BackendError> {
        self.p1_character = canonical_character_name(p1_character).ok_or_else(|| {
            BackendError::new(format!("unknown Bloody Roar 2 character: {p1_character}"))
        })?;
        self.p2_character = canonical_character_name(p2_character).ok_or_else(|| {
            BackendError::new(format!("unknown Bloody Roar 2 character: {p2_character}"))
        })?;
        self.reset()
    }

    fn active_characters(&self) -> Option<(&'static str, &'static str)> {
        Some((self.p1_character, self.p2_character))
    }

    fn step(&mut self, buttons: ActionButtons, frames: u32) -> Result<Observation, BackendError> {
        let frame_delta = frames.max(1) as u64;
        self.frame += frame_delta;

        let attack_requested = buttons.punch
            || buttons.kick
            || buttons.beast
            || buttons.p2_punch
            || buttons.p2_kick
            || buttons.p2_beast;
        if attack_requested {
            let damage = if buttons.beast || buttons.p2_beast {
                0.018
            } else {
                0.01
            };
            self.opponent_health = (self.opponent_health - damage).max(0.0);
        }

        if !buttons.guard && self.frame.is_multiple_of(12) {
            self.player_health = (self.player_health - 0.004).max(0.0);
        }

        self.beast_meter = (self.beast_meter + 0.002 * frame_delta as f32).min(1.0);
        let mut observation = self.make_observation();
        observation.effects_progressing = attack_requested;
        Ok(observation)
    }

    fn observe(&mut self) -> Result<Observation, BackendError> {
        Ok(self.make_observation())
    }
}

fn escape_json(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}
