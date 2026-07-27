use crate::action::{Action, ActionButtons};
use crate::backend::{Backend, BackendError, Observation};

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

    pub fn set_observation_screenshot(&mut self, enabled: bool) {
        self.backend.set_observation_screenshot(enabled);
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
}

fn reward(previous: &Observation, current: &Observation) -> f32 {
    let damage_dealt = previous.opponent_health - current.opponent_health;
    let damage_taken = previous.player_health - current.player_health;
    (damage_dealt * 10.0) - (damage_taken * 8.0)
}

#[cfg(test)]
mod tests {
    use crate::{Action, ActionButtons, BloodyRoar2Env, NullBackend};

    use super::MAX_STEP_FRAMES;

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
}
