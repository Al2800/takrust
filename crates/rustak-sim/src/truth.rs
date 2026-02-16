#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TruthState {
    pub x_mm: i64,
    pub y_mm: i64,
    pub vx_mm_per_s: i32,
    pub vy_mm_per_s: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TruthSnapshot {
    pub tick: u64,
    pub elapsed_millis: u64,
    pub state: TruthState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TruthEngineConfig {
    pub step_millis: u64,
    pub velocity_limit_mm_per_s: i32,
    pub velocity_jitter_mm_per_s: i32,
}

impl Default for TruthEngineConfig {
    fn default() -> Self {
        Self {
            step_millis: 100,
            velocity_limit_mm_per_s: 3_000,
            velocity_jitter_mm_per_s: 60,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruthEngineError {
    ZeroStepMillis,
    NonPositiveVelocityLimit,
    NegativeVelocityJitter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TruthEngine {
    config: TruthEngineConfig,
    state: TruthState,
    tick: u64,
    elapsed_millis: u64,
    rng_state: u64,
}

impl TruthEngine {
    pub fn new(
        seed: u64,
        initial_state: TruthState,
        config: TruthEngineConfig,
    ) -> Result<Self, TruthEngineError> {
        validate_config(config)?;

        Ok(Self {
            config,
            state: initial_state,
            tick: 0,
            elapsed_millis: 0,
            rng_state: normalize_seed(seed),
        })
    }

    #[must_use]
    pub fn state(&self) -> TruthState {
        self.state
    }

    #[must_use]
    pub fn tick(&self) -> u64 {
        self.tick
    }

    #[must_use]
    pub fn elapsed_millis(&self) -> u64 {
        self.elapsed_millis
    }

    pub fn advance(&mut self) -> TruthSnapshot {
        self.tick = self.tick.saturating_add(1);
        self.elapsed_millis = self.elapsed_millis.saturating_add(self.config.step_millis);

        let jitter_x = signed_jitter(&mut self.rng_state, self.config.velocity_jitter_mm_per_s);
        let jitter_y = signed_jitter(&mut self.rng_state, self.config.velocity_jitter_mm_per_s);

        self.state.vx_mm_per_s = clamp_velocity(
            self.state.vx_mm_per_s.saturating_add(jitter_x),
            self.config.velocity_limit_mm_per_s,
        );
        self.state.vy_mm_per_s = clamp_velocity(
            self.state.vy_mm_per_s.saturating_add(jitter_y),
            self.config.velocity_limit_mm_per_s,
        );

        let step_ms_i64 = self.config.step_millis as i64;
        self.state.x_mm = self
            .state
            .x_mm
            .saturating_add((i64::from(self.state.vx_mm_per_s) * step_ms_i64) / 1_000);
        self.state.y_mm = self
            .state
            .y_mm
            .saturating_add((i64::from(self.state.vy_mm_per_s) * step_ms_i64) / 1_000);

        TruthSnapshot {
            tick: self.tick,
            elapsed_millis: self.elapsed_millis,
            state: self.state,
        }
    }
}

fn validate_config(config: TruthEngineConfig) -> Result<(), TruthEngineError> {
    if config.step_millis == 0 {
        return Err(TruthEngineError::ZeroStepMillis);
    }
    if config.velocity_limit_mm_per_s <= 0 {
        return Err(TruthEngineError::NonPositiveVelocityLimit);
    }
    if config.velocity_jitter_mm_per_s < 0 {
        return Err(TruthEngineError::NegativeVelocityJitter);
    }
    Ok(())
}

fn normalize_seed(seed: u64) -> u64 {
    if seed == 0 {
        0x9E37_79B9_7F4A_7C15
    } else {
        seed
    }
}

fn next_rand(state: &mut u64) -> u64 {
    let mut value = *state;
    value ^= value << 13;
    value ^= value >> 7;
    value ^= value << 17;
    *state = value;
    value
}

fn signed_jitter(state: &mut u64, amplitude: i32) -> i32 {
    if amplitude == 0 {
        return 0;
    }

    let span = (amplitude as i64)
        .saturating_mul(2)
        .saturating_add(1)
        .clamp(1, i64::from(i32::MAX)) as u64;
    let sample = next_rand(state) % span;
    sample as i32 - amplitude
}

fn clamp_velocity(value: i32, limit: i32) -> i32 {
    value.clamp(-limit, limit)
}

#[cfg(test)]
mod tests {
    use crate::truth::{TruthEngine, TruthEngineConfig, TruthEngineError, TruthState};

    fn default_state() -> TruthState {
        TruthState {
            x_mm: 0,
            y_mm: 0,
            vx_mm_per_s: 120,
            vy_mm_per_s: -80,
        }
    }

    #[test]
    fn deterministic_sequence_is_stable_for_same_seed() {
        let config = TruthEngineConfig::default();
        let mut left = TruthEngine::new(7, default_state(), config).expect("engine");
        let mut right = TruthEngine::new(7, default_state(), config).expect("engine");

        let mut left_states = Vec::new();
        let mut right_states = Vec::new();
        for _ in 0..32 {
            left_states.push(left.advance());
            right_states.push(right.advance());
        }

        assert_eq!(left_states, right_states);
    }

    #[test]
    fn different_seed_changes_evolution_path() {
        let config = TruthEngineConfig::default();
        let mut left = TruthEngine::new(1, default_state(), config).expect("engine");
        let mut right = TruthEngine::new(2, default_state(), config).expect("engine");

        let mut diverged = false;
        for _ in 0..16 {
            if left.advance() != right.advance() {
                diverged = true;
                break;
            }
        }

        assert!(diverged);
    }

    #[test]
    fn invalid_config_is_rejected() {
        let state = default_state();
        let invalid = TruthEngineConfig {
            step_millis: 0,
            ..TruthEngineConfig::default()
        };
        let error = TruthEngine::new(1, state, invalid).expect_err("must fail");
        assert_eq!(error, TruthEngineError::ZeroStepMillis);
    }
}
