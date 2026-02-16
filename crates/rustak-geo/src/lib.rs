use rustak_core::Position;
use thiserror::Error;

pub const WGS84_AUTHALIC_RADIUS_METERS: f64 = 6_371_008.8;

const EPSILON: f64 = 1e-12;

#[derive(Debug, Error, PartialEq)]
pub enum GeoError {
    #[error("fraction must be in [0.0, 1.0], got {fraction}")]
    InvalidFraction { fraction: f64 },

    #[error(transparent)]
    Core(#[from] rustak_core::CoreError),
}

pub fn haversine_distance_meters(from: &Position, to: &Position) -> f64 {
    let lat1 = degrees_to_radians(from.latitude());
    let lon1 = degrees_to_radians(from.longitude());
    let lat2 = degrees_to_radians(to.latitude());
    let lon2 = degrees_to_radians(to.longitude());

    let delta_lat = lat2 - lat1;
    let delta_lon = lon2 - lon1;

    let haversine = ((delta_lat / 2.0).sin().powi(2)
        + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2))
    .clamp(0.0, 1.0);
    let central_angle = 2.0 * haversine.sqrt().asin();

    WGS84_AUTHALIC_RADIUS_METERS * central_angle
}

pub fn initial_bearing_degrees(from: &Position, to: &Position) -> f64 {
    let lat1 = degrees_to_radians(from.latitude());
    let lon1 = degrees_to_radians(from.longitude());
    let lat2 = degrees_to_radians(to.latitude());
    let lon2 = degrees_to_radians(to.longitude());

    let delta_lon = lon2 - lon1;
    let y = delta_lon.sin() * lat2.cos();
    let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * delta_lon.cos();
    let bearing = y.atan2(x).to_degrees();
    normalize_bearing_degrees(bearing)
}

pub fn interpolate_great_circle(
    from: &Position,
    to: &Position,
    fraction: f64,
) -> Result<Position, GeoError> {
    if !(0.0..=1.0).contains(&fraction) {
        return Err(GeoError::InvalidFraction { fraction });
    }

    if fraction <= EPSILON {
        return Ok(from.clone());
    }

    if (1.0 - fraction).abs() <= EPSILON {
        return Ok(to.clone());
    }

    let lat1 = degrees_to_radians(from.latitude());
    let lon1 = degrees_to_radians(from.longitude());
    let lat2 = degrees_to_radians(to.latitude());
    let lon2 = degrees_to_radians(to.longitude());

    let central_angle = haversine_central_angle(lat1, lon1, lat2, lon2);
    if central_angle.abs() <= EPSILON {
        return interpolate_linearly(from, to, fraction);
    }

    let sin_angle = central_angle.sin();
    if sin_angle.abs() <= EPSILON {
        return interpolate_linearly(from, to, fraction);
    }
    let weight_a = ((1.0 - fraction) * central_angle).sin() / sin_angle;
    let weight_b = (fraction * central_angle).sin() / sin_angle;

    let x = weight_a * lat1.cos() * lon1.cos() + weight_b * lat2.cos() * lon2.cos();
    let y = weight_a * lat1.cos() * lon1.sin() + weight_b * lat2.cos() * lon2.sin();
    let z = weight_a * lat1.sin() + weight_b * lat2.sin();

    let latitude = z.atan2((x.powi(2) + y.powi(2)).sqrt()).to_degrees();
    let longitude = normalize_longitude_degrees(y.atan2(x).to_degrees());

    let mut position = Position::new(latitude, longitude)?;
    if let Some(hae) = interpolate_optional(from.hae(), to.hae(), fraction) {
        position = position.with_hae(hae)?;
    }
    if let Some(ce) = interpolate_optional(from.ce(), to.ce(), fraction) {
        position = position.with_ce(ce)?;
    }
    if let Some(le) = interpolate_optional(from.le(), to.le(), fraction) {
        position = position.with_le(le)?;
    }

    Ok(position)
}

fn interpolate_linearly(
    from: &Position,
    to: &Position,
    fraction: f64,
) -> Result<Position, GeoError> {
    let latitude = from.latitude() + (to.latitude() - from.latitude()) * fraction;
    let longitude = normalize_longitude_degrees(
        from.longitude() + shortest_longitude_delta(from.longitude(), to.longitude()) * fraction,
    );

    let mut position = Position::new(latitude, longitude)?;
    if let Some(hae) = interpolate_optional(from.hae(), to.hae(), fraction) {
        position = position.with_hae(hae)?;
    }
    if let Some(ce) = interpolate_optional(from.ce(), to.ce(), fraction) {
        position = position.with_ce(ce)?;
    }
    if let Some(le) = interpolate_optional(from.le(), to.le(), fraction) {
        position = position.with_le(le)?;
    }

    Ok(position)
}

fn interpolate_optional(from: Option<f64>, to: Option<f64>, fraction: f64) -> Option<f64> {
    match (from, to) {
        (Some(left), Some(right)) => Some(left + (right - left) * fraction),
        _ => None,
    }
}

fn haversine_central_angle(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let delta_lat = lat2 - lat1;
    let delta_lon = lon2 - lon1;

    let haversine = ((delta_lat / 2.0).sin().powi(2)
        + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2))
    .clamp(0.0, 1.0);
    2.0 * haversine.sqrt().asin()
}

fn shortest_longitude_delta(from: f64, to: f64) -> f64 {
    let mut delta = to - from;
    while delta > 180.0 {
        delta -= 360.0;
    }
    while delta < -180.0 {
        delta += 360.0;
    }
    delta
}

fn degrees_to_radians(value: f64) -> f64 {
    value.to_radians()
}

fn normalize_longitude_degrees(value: f64) -> f64 {
    let mut normalized = (value + 180.0).rem_euclid(360.0) - 180.0;
    if (normalized + 180.0).abs() <= EPSILON {
        normalized = 180.0;
    }
    normalized
}

fn normalize_bearing_degrees(value: f64) -> f64 {
    value.rem_euclid(360.0)
}

#[cfg(test)]
mod tests {
    use rustak_core::Position;

    use crate::{
        haversine_distance_meters, initial_bearing_degrees, interpolate_great_circle, GeoError,
    };

    fn approx_equal(left: f64, right: f64, tolerance: f64) {
        let delta = (left - right).abs();
        assert!(
            delta <= tolerance,
            "expected {left} ~= {right} within {tolerance}, delta={delta}"
        );
    }

    #[test]
    fn distance_is_zero_for_identical_points() {
        let point = Position::new(34.0, -117.0).expect("point should validate");

        let distance = haversine_distance_meters(&point, &point);
        approx_equal(distance, 0.0, 1e-6);
    }

    #[test]
    fn distance_matches_known_city_pair_within_tolerance() {
        let san_francisco = Position::new(37.7749, -122.4194).expect("point should validate");
        let los_angeles = Position::new(34.0522, -118.2437).expect("point should validate");

        let distance = haversine_distance_meters(&san_francisco, &los_angeles);
        approx_equal(distance, 559_121.0, 2_000.0);
    }

    #[test]
    fn initial_bearing_matches_expected_reference_value() {
        let san_francisco = Position::new(37.7749, -122.4194).expect("point should validate");
        let los_angeles = Position::new(34.0522, -118.2437).expect("point should validate");

        let bearing = initial_bearing_degrees(&san_francisco, &los_angeles);
        approx_equal(bearing, 136.5, 1.0);
    }

    #[test]
    fn interpolation_returns_expected_equatorial_midpoint() {
        let left = Position::new(0.0, 0.0).expect("point should validate");
        let right = Position::new(0.0, 90.0).expect("point should validate");

        let midpoint = interpolate_great_circle(&left, &right, 0.5).expect("midpoint should work");
        approx_equal(midpoint.latitude(), 0.0, 1e-9);
        approx_equal(midpoint.longitude(), 45.0, 1e-9);
    }

    #[test]
    fn interpolation_interpolates_optional_accuracy_fields() {
        let left = Position::new(10.0, 20.0)
            .expect("point should validate")
            .with_hae(100.0)
            .expect("hae should validate")
            .with_ce(2.0)
            .expect("ce should validate")
            .with_le(4.0)
            .expect("le should validate");

        let right = Position::new(15.0, 25.0)
            .expect("point should validate")
            .with_hae(200.0)
            .expect("hae should validate")
            .with_ce(6.0)
            .expect("ce should validate")
            .with_le(8.0)
            .expect("le should validate");

        let interpolated =
            interpolate_great_circle(&left, &right, 0.5).expect("interpolation should work");

        assert_eq!(interpolated.hae(), Some(150.0));
        assert_eq!(interpolated.ce(), Some(4.0));
        assert_eq!(interpolated.le(), Some(6.0));
    }

    #[test]
    fn interpolation_rejects_invalid_fraction() {
        let left = Position::new(0.0, 0.0).expect("point should validate");
        let right = Position::new(1.0, 1.0).expect("point should validate");

        let error = interpolate_great_circle(&left, &right, 1.1)
            .expect_err("fraction above one should be rejected");
        assert_eq!(error, GeoError::InvalidFraction { fraction: 1.1 });
    }

    #[test]
    fn interpolation_for_antipodal_points_stays_finite() {
        let left = Position::new(0.0, 0.0).expect("point should validate");
        let right = Position::new(0.0, 180.0).expect("point should validate");

        let midpoint = interpolate_great_circle(&left, &right, 0.5).expect("midpoint should work");
        assert!(midpoint.latitude().is_finite());
        assert!(midpoint.longitude().is_finite());
    }

    #[test]
    fn antipodal_distance_is_half_circumference() {
        let left = Position::new(0.0, 0.0).expect("point should validate");
        let right = Position::new(0.0, 180.0).expect("point should validate");

        let distance = haversine_distance_meters(&left, &right);
        approx_equal(
            distance,
            std::f64::consts::PI * crate::WGS84_AUTHALIC_RADIUS_METERS,
            1e-6,
        );
    }
}
