use std::time::Instant;

use crate::action::{Action, ActionButtons};
use crate::backend::{Backend, BackendError, Observation};
use crate::moves::{ActionSegment, MAX_ACTION_SEQUENCE_SEGMENTS, Player, canonical_character_name};

pub const MAX_STEP_FRAMES: u32 = 600;

#[derive(Clone, Debug)]
pub struct StepResult {
    pub observation: Observation,
    pub reward: f32,
    pub terminated: bool,
    pub truncated: bool,
}

impl StepResult {
    pub fn json(&self) -> String {
        format!(
            "{{\"observation\":{},\"reward\":{},\"terminated\":{},\"truncated\":{},\"info\":{}}}",
            self.observation.json(),
            self.reward,
            self.terminated,
            self.truncated,
            self.observation.info_json
        )
    }
}

pub struct BloodyRoar2Env<B: Backend> {
    backend: B,
    last_observation: Option<Observation>,
    max_frames: u64,
}

impl<B: Backend> BloodyRoar2Env<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            last_observation: None,
            max_frames: 60 * 99,
        }
    }

    pub fn reset(&mut self) -> Result<Observation, BackendError> {
        let observation = self.backend.reset()?;
        self.last_observation = Some(observation.clone());
        Ok(observation)
    }

    pub fn reset_match(
        &mut self,
        p1_character: &str,
        p2_character: &str,
    ) -> Result<Observation, BackendError> {
        let observation = self.backend.reset_match(p1_character, p2_character)?;
        self.last_observation = Some(observation.clone());
        Ok(observation)
    }

    pub fn active_characters(&self) -> Option<(&'static str, &'static str)> {
        self.backend.active_characters()
    }

    pub fn ensure_character_selected(
        &mut self,
        character: &str,
        player: Player,
    ) -> Result<bool, BackendError> {
        let character = canonical_character_name(character).ok_or_else(|| {
            BackendError::new(format!("unknown Bloody Roar 2 character: {character}"))
        })?;
        let (p1_character, p2_character) = self.backend.active_characters().ok_or_else(|| {
            BackendError::new("current backend does not expose active character selection")
        })?;
        let already_selected = match player {
            Player::One => p1_character == character,
            Player::Two => p2_character == character,
        };
        if already_selected {
            return Ok(false);
        }
        let (p1_character, p2_character) = match player {
            Player::One => (character, p2_character),
            Player::Two => (p1_character, character),
        };
        self.reset_match(p1_character, p2_character)?;
        Ok(true)
    }

    pub fn set_observation_screenshot(&mut self, enabled: bool) {
        self.backend.set_observation_screenshot(enabled);
    }

    pub fn diagnostic_json(&self) -> Option<String> {
        self.backend.diagnostic_json()
    }

    pub fn observe(&mut self) -> Result<Observation, BackendError> {
        let observation = self.backend.observe()?;
        self.last_observation = Some(observation.clone());
        Ok(observation)
    }

    pub fn last_observation(&self) -> Option<&Observation> {
        self.last_observation.as_ref()
    }

    pub fn step(&mut self, action: Action, frames: u32) -> Result<StepResult, BackendError> {
        self.step_buttons(action.buttons(), frames)
    }

    pub fn step_buttons(
        &mut self,
        buttons: ActionButtons,
        frames: u32,
    ) -> Result<StepResult, BackendError> {
        if frames == 0 || frames > MAX_STEP_FRAMES {
            return Err(BackendError::new(format!(
                "frames must be between 1 and {MAX_STEP_FRAMES}"
            )));
        }
        let previous = match self.last_observation.clone() {
            Some(observation) => observation,
            None => self.backend.reset()?,
        };
        let observation = self.backend.step(buttons, frames)?;
        let reward = reward(&previous, &observation);
        let truncated = observation.frame >= self.max_frames;
        let terminated = observation.terminal;
        self.last_observation = Some(observation.clone());

        Ok(StepResult {
            observation,
            reward,
            terminated,
            truncated,
        })
    }

    pub fn step_sequence(
        &mut self,
        segments: &[ActionSegment],
        include_screenshot: bool,
    ) -> Result<StepResult, BackendError> {
        if segments.is_empty() {
            return Err(BackendError::new(
                "action sequence must contain at least one segment",
            ));
        }
        if segments.len() > MAX_ACTION_SEQUENCE_SEGMENTS {
            return Err(BackendError::new(format!(
                "action sequence must contain at most {MAX_ACTION_SEQUENCE_SEGMENTS} segments"
            )));
        }
        let total_frames = segments.iter().try_fold(0_u32, |total, segment| {
            if segment.frames == 0 {
                return Err(BackendError::new(
                    "action sequence segment frames must be positive",
                ));
            }
            total
                .checked_add(segment.frames)
                .ok_or_else(|| BackendError::new("action sequence frame count overflow"))
        })?;
        if total_frames > MAX_STEP_FRAMES {
            return Err(BackendError::new(format!(
                "action sequence total frames must be between 1 and {MAX_STEP_FRAMES}"
            )));
        }

        let started_at = Instant::now();
        let start_frame = self
            .last_observation
            .as_ref()
            .map(|observation| observation.frame)
            .unwrap_or(0);
        let mut total_reward = 0.0;
        let mut final_step = None;
        let mut executed_segments = 0usize;
        let mut render_progressing = false;
        let mut effects_progressing = false;
        let mut audio_progressing = false;
        let mut input_activity_delta = serde_json::Map::new();
        let mut guest_input_proof = serde_json::Map::new();
        for (index, segment) in segments.iter().enumerate() {
            let is_last = index + 1 == segments.len();
            self.set_observation_screenshot(include_screenshot && is_last);
            let step = self.step_buttons(segment.buttons, segment.frames)?;
            total_reward += step.reward;
            executed_segments = executed_segments.saturating_add(1);
            render_progressing |= step.observation.render_progressing;
            effects_progressing |= step.observation.effects_progressing;
            audio_progressing |= step.observation.audio_progressing;
            accumulate_sequence_input_info(
                &step.observation.info_json,
                &mut input_activity_delta,
                &mut guest_input_proof,
            );
            let finished = step.terminated || step.truncated;
            final_step = Some(step);
            if finished {
                break;
            }
        }

        let mut final_step = final_step.expect("non-empty action sequence produced a step");
        if include_screenshot && final_step.observation.screenshot_b64.is_none() {
            self.set_observation_screenshot(true);
            final_step.observation = self.observe()?;
            final_step.terminated = final_step.observation.terminal;
            final_step.truncated = final_step.observation.frame >= self.max_frames;
        }
        final_step.reward = total_reward;
        final_step.observation.render_progressing |= render_progressing;
        final_step.observation.effects_progressing |= effects_progressing;
        final_step.observation.audio_progressing |= audio_progressing;
        let executed_frames = final_step.observation.frame.saturating_sub(start_frame);
        let elapsed_seconds = started_at.elapsed().as_secs_f64();
        if elapsed_seconds > 0.0 {
            final_step.observation.emulated_fps = (executed_frames as f64 / elapsed_seconds) as f32;
        }
        final_step.observation.info_json = sequence_info_json(
            &final_step.observation.info_json,
            segments.len(),
            executed_segments,
            total_frames,
            executed_frames,
            elapsed_seconds,
            render_progressing,
            effects_progressing,
            audio_progressing,
            &input_activity_delta,
            &guest_input_proof,
        );
        Ok(final_step)
    }
}

#[allow(clippy::too_many_arguments)]
fn sequence_info_json(
    info_json: &str,
    requested_segments: usize,
    executed_segments: usize,
    requested_frames: u32,
    executed_frames: u64,
    elapsed_seconds: f64,
    render_progressing: bool,
    effects_progressing: bool,
    audio_progressing: bool,
    input_activity_delta: &serde_json::Map<String, serde_json::Value>,
    guest_input_proof: &serde_json::Map<String, serde_json::Value>,
) -> String {
    let mut info = serde_json::from_str::<serde_json::Value>(info_json)
        .unwrap_or_else(|_| serde_json::json!({ "backend_info": info_json }));
    if !info.is_object() {
        info = serde_json::json!({ "backend_info": info });
    }
    let emulated_fps = if elapsed_seconds > 0.0 {
        executed_frames as f64 / elapsed_seconds
    } else {
        0.0
    };
    let info_object = info.as_object_mut().expect("info was normalized");
    if !input_activity_delta.is_empty() {
        info_object.insert(
            "input_activity_delta".to_string(),
            serde_json::Value::Object(input_activity_delta.clone()),
        );
    }
    if !guest_input_proof.is_empty() {
        info_object.insert(
            "guest_input_proof".to_string(),
            serde_json::Value::Object(guest_input_proof.clone()),
        );
    }
    info_object.insert(
        "sequence".to_string(),
        serde_json::json!({
            "requested_segments": requested_segments,
            "executed_segments": executed_segments,
            "requested_frames": requested_frames,
            "executed_frames": executed_frames,
            "elapsed_seconds": elapsed_seconds,
            "emulated_fps": emulated_fps,
            "render_progressing": render_progressing,
            "effects_progressing": effects_progressing,
            "audio_progressing": audio_progressing,
            "released": true,
            "input_activity_delta": input_activity_delta,
            "guest_input_proof": guest_input_proof,
        }),
    );
    info.to_string()
}

fn accumulate_sequence_input_info(
    info_json: &str,
    input_activity_delta: &mut serde_json::Map<String, serde_json::Value>,
    guest_input_proof: &mut serde_json::Map<String, serde_json::Value>,
) {
    let Ok(info) = serde_json::from_str::<serde_json::Value>(info_json) else {
        return;
    };
    if let Some(delta) = info
        .get("input_activity_delta")
        .and_then(|value| value.as_object())
    {
        merge_sequence_observation_object(input_activity_delta, delta);
    }
    if let Some(proof) = info
        .get("guest_input_proof")
        .and_then(|value| value.as_object())
    {
        merge_sequence_observation_object(guest_input_proof, proof);
    }
}

fn merge_sequence_observation_object(
    aggregate: &mut serde_json::Map<String, serde_json::Value>,
    current: &serde_json::Map<String, serde_json::Value>,
) {
    for (key, value) in current {
        match value {
            serde_json::Value::Number(number) if !key.starts_with("last_") => {
                let sum = aggregate
                    .get(key)
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default()
                    .saturating_add(number.as_u64().unwrap_or_default());
                aggregate.insert(key.clone(), serde_json::Value::from(sum));
            }
            serde_json::Value::Bool(value) => {
                let merged = aggregate
                    .get(key)
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
                    || *value;
                aggregate.insert(key.clone(), serde_json::Value::from(merged));
            }
            _ => {
                aggregate.insert(key.clone(), value.clone());
            }
        }
    }
}

fn reward(previous: &Observation, current: &Observation) -> f32 {
    let damage_dealt = previous.opponent_health - current.opponent_health;
    let damage_taken = previous.player_health - current.player_health;
    (damage_dealt * 10.0) - (damage_taken * 8.0)
}

#[cfg(test)]
mod tests {
    use crate::{Action, ActionButtons, ActionSegment, BloodyRoar2Env, NullBackend};

    use super::{MAX_STEP_FRAMES, accumulate_sequence_input_info, sequence_info_json};

    #[test]
    fn null_environment_steps() {
        let mut env = BloodyRoar2Env::new(NullBackend::default());
        let reset = env.reset().expect("reset");
        assert_eq!(reset.frame, 0);

        let step = env.step(Action::Punch, 4).expect("step");
        assert!(step.reward > 0.0);
        assert_eq!(step.observation.frame, 4);
    }

    #[test]
    fn environment_accepts_arbitrary_button_combinations() {
        let mut env = BloodyRoar2Env::new(NullBackend::default());
        let step = env
            .step_buttons(
                ActionButtons {
                    up: true,
                    punch: true,
                    kick: true,
                    guard: true,
                    ..ActionButtons::default()
                },
                1,
            )
            .expect("combined step");

        assert!(step.reward > 0.0);
    }

    #[test]
    fn environment_rejects_unbounded_frame_requests() {
        let mut env = BloodyRoar2Env::new(NullBackend::default());
        assert!(env.step(Action::Noop, 0).is_err());
        assert!(env.step(Action::Noop, MAX_STEP_FRAMES).is_ok());
        assert!(env.step(Action::Noop, MAX_STEP_FRAMES + 1).is_err());
    }

    #[test]
    fn environment_executes_bounded_action_sequences() {
        let mut env = BloodyRoar2Env::new(NullBackend::default());
        let result = env
            .step_sequence(
                &[
                    ActionSegment::new(Action::Punch.buttons(), 2),
                    ActionSegment::new(Action::P2Punch.buttons(), 4),
                    ActionSegment::new(ActionButtons::default(), 3),
                ],
                false,
            )
            .expect("action sequence");

        assert_eq!(result.observation.frame, 9);
        assert!(result.reward > 0.0);
        assert!(result.observation.effects_progressing);
        let info: serde_json::Value =
            serde_json::from_str(&result.observation.info_json).expect("sequence info");
        assert_eq!(info["sequence"]["executed_segments"], 3);
        assert_eq!(info["sequence"]["executed_frames"], 9);
        assert_eq!(info["sequence"]["released"], true);
        assert!(
            env.step_sequence(
                &[ActionSegment::new(
                    ActionButtons::default(),
                    MAX_STEP_FRAMES + 1,
                )],
                false,
            )
            .is_err()
        );
    }

    #[test]
    fn sequence_info_aggregates_guest_input_across_release_segments() {
        let mut input_activity_delta = serde_json::Map::new();
        let mut guest_input_proof = serde_json::Map::new();
        accumulate_sequence_input_info(
            r#"{"input_activity_delta":{"p1_punch_active_reads":4,"last_system_input":128},"guest_input_proof":{"any":true,"p1_any":true,"p2_any":false}}"#,
            &mut input_activity_delta,
            &mut guest_input_proof,
        );
        accumulate_sequence_input_info(
            r#"{"input_activity_delta":{"p1_punch_active_reads":0,"p2_kick_active_reads":3,"last_system_input":0},"guest_input_proof":{"any":false,"p1_any":false,"p2_any":true}}"#,
            &mut input_activity_delta,
            &mut guest_input_proof,
        );

        let info: serde_json::Value = serde_json::from_str(&sequence_info_json(
            "{}",
            2,
            2,
            5,
            5,
            0.5,
            true,
            true,
            false,
            &input_activity_delta,
            &guest_input_proof,
        ))
        .expect("sequence info");
        assert_eq!(info["input_activity_delta"]["p1_punch_active_reads"], 4);
        assert_eq!(info["input_activity_delta"]["p2_kick_active_reads"], 3);
        assert_eq!(info["input_activity_delta"]["last_system_input"], 0);
        assert_eq!(info["guest_input_proof"]["any"], true);
        assert_eq!(info["guest_input_proof"]["p1_any"], true);
        assert_eq!(info["guest_input_proof"]["p2_any"], true);
        assert_eq!(
            info["sequence"]["guest_input_proof"]["p2_any"], true,
            "the final release segment must not erase earlier P2 activity"
        );
    }
}
