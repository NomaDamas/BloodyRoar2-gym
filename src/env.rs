use crate::action::Action;
use crate::backend::{Backend, BackendError, Observation};

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
            "{{\"observation\":{},\"reward\":{},\"terminated\":{},\"truncated\":{},\"info\":{{}}}}",
            self.observation.json(),
            self.reward,
            self.terminated,
            self.truncated
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

    pub fn step(&mut self, action: Action, frames: u32) -> Result<StepResult, BackendError> {
        let previous = match self.last_observation.clone() {
            Some(observation) => observation,
            None => self.backend.reset()?,
        };
        let observation = self.backend.step(action.buttons(), frames)?;
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
    use crate::{Action, BloodyRoar2Env, NullBackend};

    #[test]
    fn null_environment_steps() {
        let mut env = BloodyRoar2Env::new(NullBackend::default());
        let reset = env.reset().expect("reset");
        assert_eq!(reset.frame, 0);

        let step = env.step(Action::Punch, 4).expect("step");
        assert!(step.reward > 0.0);
        assert_eq!(step.observation.frame, 4);
    }
}
