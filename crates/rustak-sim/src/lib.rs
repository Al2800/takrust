pub mod scenario;
pub mod sensor;
pub mod sweep;
pub mod truth;

use rustak_io::{MessageEnvelope, MessageSink, MessageSource};
use sensor::SensorModel;

pub use rustak_io::{
    CotEnvelope, CotMessage, CotSink, CotSource, IoError, MessageEnvelope as IoMessageEnvelope,
    MessageSink as IoMessageSink, MessageSource as IoMessageSource, ObservedTime,
};
pub use scenario::{Scenario, ScenarioComposition, ScenarioError, ScenarioOverlay};
pub use sensor::{DeterministicSensorModel, SensorObservation};
pub use sweep::{SweepAxis, SweepCase, SweepReport, SweepRunOptions, SweepRunner};
pub use truth::{TruthEngine, TruthEngineConfig, TruthEngineError, TruthSnapshot, TruthState};

pub type SimEnvelope<T> = MessageEnvelope<T>;
pub type SimSink<T> = dyn MessageSink<T>;
pub type SimSource<T> = dyn MessageSource<T>;

#[must_use]
pub fn scenario_envelope<T>(message: T) -> SimEnvelope<T> {
    MessageEnvelope::new(message)
}

#[must_use]
pub fn observe_with_model<M: SensorModel>(
    snapshot: &TruthSnapshot,
    model: &M,
) -> SimEnvelope<M::Observation> {
    scenario_envelope(model.observe(snapshot))
}

#[must_use]
pub fn simulate_step<M: SensorModel>(
    engine: &mut TruthEngine,
    model: &M,
) -> SimEnvelope<M::Observation> {
    let snapshot = engine.advance();
    observe_with_model(&snapshot, model)
}

#[cfg(test)]
mod tests {
    use crate::sensor::SensorModel;
    use crate::truth::{TruthEngine, TruthEngineConfig, TruthState};
    use crate::{scenario_envelope, simulate_step, DeterministicSensorModel};

    #[test]
    fn scenario_envelope_wraps_message() {
        let env = scenario_envelope(String::from("sim-observation"));
        assert_eq!(env.message, "sim-observation");
        assert!(env.peer.is_none());
        assert!(env.raw_frame.is_none());
    }

    #[test]
    fn simulate_step_bridges_truth_engine_and_sensor_model() {
        let mut engine = TruthEngine::new(
            11,
            TruthState {
                x_mm: 0,
                y_mm: 0,
                vx_mm_per_s: 120,
                vy_mm_per_s: 80,
            },
            TruthEngineConfig::default(),
        )
        .expect("engine");
        let sensor = DeterministicSensorModel::default();

        let envelope = simulate_step(&mut engine, &sensor);
        let direct = sensor.observe(&engine.advance());

        assert!(envelope.peer.is_none());
        assert!(envelope.raw_frame.is_none());
        assert_eq!(envelope.message.tick, 1);
        assert_ne!(envelope.message.tick, direct.tick);
    }
}
