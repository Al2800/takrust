use crate::truth::TruthSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SensorObservation {
    pub tick: u64,
    pub observed_x_mm: i64,
    pub observed_y_mm: i64,
    pub quality: u8,
}

pub trait SensorModel {
    type Observation;

    fn observe(&self, truth: &TruthSnapshot) -> Self::Observation;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeterministicSensorModel {
    pub seed: u64,
    pub position_quantum_mm: i64,
    pub max_noise_mm: i64,
}

impl Default for DeterministicSensorModel {
    fn default() -> Self {
        Self {
            seed: 0xA2F3_5B13,
            position_quantum_mm: 25,
            max_noise_mm: 50,
        }
    }
}

impl SensorModel for DeterministicSensorModel {
    type Observation = SensorObservation;

    fn observe(&self, truth: &TruthSnapshot) -> Self::Observation {
        let noise_x = signed_noise(self.seed, truth.tick, self.max_noise_mm);
        let noise_y = signed_noise(self.seed ^ 0xC7D2_4A11, truth.tick, self.max_noise_mm);

        let observed_x = quantize(
            truth.state.x_mm.saturating_add(noise_x),
            self.position_quantum_mm,
        );
        let observed_y = quantize(
            truth.state.y_mm.saturating_add(noise_y),
            self.position_quantum_mm,
        );
        let speed =
            i64::from(truth.state.vx_mm_per_s.abs()) + i64::from(truth.state.vy_mm_per_s.abs());
        let quality = ((255_i64.saturating_sub(speed / 20)).clamp(0, 255)) as u8;

        SensorObservation {
            tick: truth.tick,
            observed_x_mm: observed_x,
            observed_y_mm: observed_y,
            quality,
        }
    }
}

fn signed_noise(seed: u64, tick: u64, amplitude: i64) -> i64 {
    if amplitude <= 0 {
        return 0;
    }

    let mixed = splitmix64(seed ^ tick.rotate_left(17));
    let span = amplitude.saturating_mul(2).saturating_add(1);
    (mixed % span as u64) as i64 - amplitude
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut mixed = value;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}

fn quantize(value: i64, quantum: i64) -> i64 {
    if quantum <= 1 {
        return value;
    }

    let half = quantum / 2;
    ((value + half) / quantum) * quantum
}

#[cfg(test)]
mod tests {
    use crate::sensor::{DeterministicSensorModel, SensorModel};
    use crate::truth::TruthSnapshot;
    use crate::truth::TruthState;

    #[test]
    fn deterministic_sensor_is_stable_for_same_tick() {
        let model = DeterministicSensorModel::default();
        let truth = TruthSnapshot {
            tick: 42,
            elapsed_millis: 4_200,
            state: TruthState {
                x_mm: 12_345,
                y_mm: -7_654,
                vx_mm_per_s: 350,
                vy_mm_per_s: -120,
            },
        };

        let first = model.observe(&truth);
        let second = model.observe(&truth);
        assert_eq!(first, second);
    }

    #[test]
    fn deterministic_sensor_changes_with_tick() {
        let model = DeterministicSensorModel {
            position_quantum_mm: 1,
            max_noise_mm: 500,
            ..DeterministicSensorModel::default()
        };
        let base_state = TruthState {
            x_mm: 0,
            y_mm: 0,
            vx_mm_per_s: 0,
            vy_mm_per_s: 0,
        };

        let first = model.observe(&TruthSnapshot {
            tick: 1,
            elapsed_millis: 100,
            state: base_state,
        });
        let second = model.observe(&TruthSnapshot {
            tick: 2,
            elapsed_millis: 200,
            state: base_state,
        });
        assert_ne!(
            (first.observed_x_mm, first.observed_y_mm),
            (second.observed_x_mm, second.observed_y_mm)
        );
    }
}
