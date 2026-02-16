use crate::truth::TruthSnapshot;
use rustak_core::Position;
use rustak_geo::{interpolate_great_circle, GeoError};

#[derive(Debug, PartialEq)]
pub enum GeoInterpolationError {
    ZeroDurationTicks,
    TickOutOfRange { tick: u64, duration_ticks: u64 },
    Geo(GeoError),
}

pub fn interpolate_route_position(
    start: &Position,
    end: &Position,
    tick: u64,
    duration_ticks: u64,
) -> Result<Position, GeoInterpolationError> {
    if duration_ticks == 0 {
        return Err(GeoInterpolationError::ZeroDurationTicks);
    }
    if tick > duration_ticks {
        return Err(GeoInterpolationError::TickOutOfRange {
            tick,
            duration_ticks,
        });
    }

    let fraction = tick as f64 / duration_ticks as f64;
    interpolate_great_circle(start, end, fraction).map_err(GeoInterpolationError::Geo)
}

pub fn interpolate_snapshot_route_position(
    start: &Position,
    end: &Position,
    snapshot: &TruthSnapshot,
    duration_ticks: u64,
) -> Result<Position, GeoInterpolationError> {
    interpolate_route_position(start, end, snapshot.tick, duration_ticks)
}

#[cfg(test)]
mod tests {
    use crate::geo::{
        interpolate_route_position, interpolate_snapshot_route_position, GeoInterpolationError,
    };
    use crate::truth::{TruthSnapshot, TruthState};
    use rustak_core::Position;

    fn approx_equal(left: f64, right: f64, tolerance: f64) {
        let delta = (left - right).abs();
        assert!(
            delta <= tolerance,
            "expected {left} ~= {right} within {tolerance}, delta={delta}"
        );
    }

    #[test]
    fn interpolate_snapshot_route_position_tracks_fractional_tick() {
        let start = Position::new(0.0, 0.0).expect("start");
        let end = Position::new(0.0, 90.0).expect("end");
        let snapshot = TruthSnapshot {
            tick: 5,
            elapsed_millis: 500,
            state: TruthState {
                x_mm: 0,
                y_mm: 0,
                vx_mm_per_s: 0,
                vy_mm_per_s: 0,
            },
        };

        let position = interpolate_snapshot_route_position(&start, &end, &snapshot, 10)
            .expect("interpolation should succeed");
        approx_equal(position.latitude(), 0.0, 1e-9);
        approx_equal(position.longitude(), 45.0, 1e-6);
    }

    #[test]
    fn interpolation_rejects_tick_beyond_duration() {
        let start = Position::new(0.0, 0.0).expect("start");
        let end = Position::new(0.0, 1.0).expect("end");
        let error =
            interpolate_route_position(&start, &end, 11, 10).expect_err("tick should be bounded");
        assert_eq!(
            error,
            GeoInterpolationError::TickOutOfRange {
                tick: 11,
                duration_ticks: 10,
            }
        );
    }
}
