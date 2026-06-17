use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::action::ActionButtons;

#[derive(Clone, Debug)]
pub struct Observation {
    pub frame: u64,
    pub player_health: f32,
    pub opponent_health: f32,
    pub beast_meter: f32,
    pub round_time: f32,
    pub terminal: bool,
    pub screenshot_b64: Option<String>,
}

impl Observation {
    pub fn json(&self) -> String {
        let screenshot = match &self.screenshot_b64 {
            Some(value) => format!("\"{}\"", escape_json(value)),
            None => "null".to_string(),
        };

        format!(
            "{{\"frame\":{},\"player_health\":{},\"opponent_health\":{},\"beast_meter\":{},\"round_time\":{},\"terminal\":{},\"screenshot_b64\":{}}}",
            self.frame,
            self.player_health,
            self.opponent_health,
            self.beast_meter,
            self.round_time,
            self.terminal,
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
    fn reset(&mut self) -> Result<Observation, BackendError>;
    fn step(&mut self, buttons: ActionButtons, frames: u32) -> Result<Observation, BackendError>;
}

#[derive(Debug)]
pub struct NullBackend {
    frame: u64,
    player_health: f32,
    opponent_health: f32,
    beast_meter: f32,
}

impl Default for NullBackend {
    fn default() -> Self {
        Self {
            frame: 0,
            player_health: 1.0,
            opponent_health: 1.0,
            beast_meter: 0.0,
        }
    }
}

impl NullBackend {
    fn observe(&self) -> Observation {
        Observation {
            frame: self.frame,
            player_health: self.player_health,
            opponent_health: self.opponent_health,
            beast_meter: self.beast_meter,
            round_time: (99.0 - (self.frame as f32 / 60.0)).max(0.0),
            terminal: self.player_health <= 0.0 || self.opponent_health <= 0.0,
            screenshot_b64: None,
        }
    }
}

impl Backend for NullBackend {
    fn reset(&mut self) -> Result<Observation, BackendError> {
        *self = Self::default();
        Ok(self.observe())
    }

    fn step(&mut self, buttons: ActionButtons, frames: u32) -> Result<Observation, BackendError> {
        let frame_delta = frames.max(1) as u64;
        self.frame += frame_delta;

        if buttons.punch || buttons.kick || buttons.beast {
            let damage = if buttons.beast { 0.018 } else { 0.01 };
            self.opponent_health = (self.opponent_health - damage).max(0.0);
        }

        if !buttons.guard && self.frame.is_multiple_of(12) {
            self.player_health = (self.player_health - 0.004).max(0.0);
        }

        self.beast_meter = (self.beast_meter + 0.002 * frame_delta as f32).min(1.0);
        Ok(self.observe())
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
