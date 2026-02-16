use std::cmp::Ordering;
use std::fmt;

/// WGS84 position with optional altitude and accuracy fields.
#[derive(Debug, Clone, PartialEq)]
pub struct Position {
    latitude: f64,
    longitude: f64,
    hae: Option<f64>,
    ce: Option<f64>,
    le: Option<f64>,
}

impl Position {
    pub fn new(latitude: f64, longitude: f64) -> Result<Self, CoreError> {
        Ok(Self {
            latitude: validate_bounded("latitude", latitude, -90.0, 90.0)?,
            longitude: validate_bounded("longitude", longitude, -180.0, 180.0)?,
            hae: None,
            ce: None,
            le: None,
        })
    }

    pub fn with_hae(mut self, hae_m: f64) -> Result<Self, CoreError> {
        self.hae = Some(validate_finite("hae", hae_m)?);
        Ok(self)
    }

    pub fn with_ce(mut self, ce_m: f64) -> Result<Self, CoreError> {
        self.ce = Some(validate_non_negative("ce", ce_m)?);
        Ok(self)
    }

    pub fn with_le(mut self, le_m: f64) -> Result<Self, CoreError> {
        self.le = Some(validate_non_negative("le", le_m)?);
        Ok(self)
    }

    #[must_use]
    pub fn latitude(&self) -> f64 {
        self.latitude
    }

    #[must_use]
    pub fn longitude(&self) -> f64 {
        self.longitude
    }

    #[must_use]
    pub fn hae(&self) -> Option<f64> {
        self.hae
    }

    #[must_use]
    pub fn ce(&self) -> Option<f64> {
        self.ce
    }

    #[must_use]
    pub fn le(&self) -> Option<f64> {
        self.le
    }
}

/// Course/speed vertical motion representation for moving entities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kinematics {
    speed: Option<f64>,
    course: Option<f64>,
    vertical_rate: Option<f64>,
}

impl Kinematics {
    pub fn new(
        speed: Option<f64>,
        course: Option<f64>,
        vertical_rate: Option<f64>,
    ) -> Result<Self, CoreError> {
        Ok(Self {
            speed: speed
                .map(|value| validate_non_negative("speed", value))
                .transpose()?,
            course: course.map(validate_course).transpose()?,
            vertical_rate: vertical_rate
                .map(|value| validate_finite("vertical_rate", value))
                .transpose()?,
        })
    }

    #[must_use]
    pub fn speed(&self) -> Option<f64> {
        self.speed
    }

    #[must_use]
    pub fn course(&self) -> Option<f64> {
        self.course
    }

    #[must_use]
    pub fn vertical_rate(&self) -> Option<f64> {
        self.vertical_rate
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.speed.is_none() && self.course.is_none() && self.vertical_rate.is_none()
    }
}

/// Canonical track detail wrapper.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Track {
    kin: Kinematics,
}

impl Track {
    pub fn new(kin: Kinematics) -> Result<Self, CoreError> {
        if kin.is_empty() {
            return Err(CoreError::EmptyTrack);
        }

        Ok(Self { kin })
    }

    #[must_use]
    pub fn kinematics(&self) -> Kinematics {
        self.kin
    }
}

/// Raw XML element placeholder used for unknown detail payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlElement {
    pub name: String,
    pub payload: String,
}

impl XmlElement {
    #[must_use]
    pub fn new(name: impl Into<String>, payload: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            payload: payload.into(),
        }
    }
}

/// Opaque extension payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionBlob {
    pub key: String,
    pub bytes: Vec<u8>,
}

impl ExtensionBlob {
    #[must_use]
    pub fn new(key: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            key: key.into(),
            bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DetailElement {
    Track(Track),
    Unknown(XmlElement),
    Extension(ExtensionBlob),
}

/// Canonicalized detail payload preserving deterministic ordering.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CotDetail {
    elements: Vec<DetailElement>,
}

impl CotDetail {
    pub fn new(mut elements: Vec<DetailElement>) -> Result<Self, CoreError> {
        normalize_detail_elements(&mut elements)?;
        Ok(Self { elements })
    }

    #[must_use]
    pub fn elements(&self) -> &[DetailElement] {
        &self.elements
    }

    pub fn push(&mut self, element: DetailElement) -> Result<(), CoreError> {
        self.elements.push(element);
        normalize_detail_elements(&mut self.elements)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CoreError {
    NonFiniteValue {
        field: &'static str,
        value: f64,
    },
    OutOfRange {
        field: &'static str,
        value: f64,
        min: f64,
        max: f64,
    },
    NegativeValue {
        field: &'static str,
        value: f64,
    },
    EmptyTrack,
    DuplicateTrackElements {
        count: usize,
    },
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteValue { field, value } => {
                write!(f, "{field} must be finite, got {value}")
            }
            Self::OutOfRange {
                field,
                value,
                min,
                max,
            } => write!(f, "{field} must be in [{min}, {max}], got {value}"),
            Self::NegativeValue { field, value } => {
                write!(f, "{field} must be >= 0.0, got {value}")
            }
            Self::EmptyTrack => write!(f, "track requires at least one kinematic component"),
            Self::DuplicateTrackElements { count } => {
                write!(
                    f,
                    "detail contains {count} track elements; expected at most one"
                )
            }
        }
    }
}

impl std::error::Error for CoreError {}

fn normalize_detail_elements(elements: &mut [DetailElement]) -> Result<(), CoreError> {
    let track_count = elements
        .iter()
        .filter(|element| matches!(element, DetailElement::Track(_)))
        .count();
    if track_count > 1 {
        return Err(CoreError::DuplicateTrackElements { count: track_count });
    }

    elements.sort_by(detail_element_cmp);
    Ok(())
}

fn detail_element_cmp(left: &DetailElement, right: &DetailElement) -> Ordering {
    let left_rank = detail_element_rank(left);
    let right_rank = detail_element_rank(right);
    if left_rank != right_rank {
        return left_rank.cmp(&right_rank);
    }

    match (left, right) {
        (DetailElement::Track(left_track), DetailElement::Track(right_track)) => {
            kinematics_cmp(left_track.kinematics(), right_track.kinematics())
        }
        (DetailElement::Unknown(left_xml), DetailElement::Unknown(right_xml)) => left_xml
            .name
            .cmp(&right_xml.name)
            .then_with(|| left_xml.payload.cmp(&right_xml.payload)),
        (DetailElement::Extension(left_blob), DetailElement::Extension(right_blob)) => left_blob
            .key
            .cmp(&right_blob.key)
            .then_with(|| left_blob.bytes.cmp(&right_blob.bytes)),
        _ => Ordering::Equal,
    }
}

fn detail_element_rank(element: &DetailElement) -> u8 {
    match element {
        DetailElement::Track(_) => 0,
        DetailElement::Unknown(_) => 1,
        DetailElement::Extension(_) => 2,
    }
}

fn kinematics_cmp(left: Kinematics, right: Kinematics) -> Ordering {
    option_f64_cmp(left.speed(), right.speed())
        .then_with(|| option_f64_cmp(left.course(), right.course()))
        .then_with(|| option_f64_cmp(left.vertical_rate(), right.vertical_rate()))
}

fn option_f64_cmp(left: Option<f64>, right: Option<f64>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(left_value), Some(right_value)) => left_value.total_cmp(&right_value),
    }
}

fn validate_finite(field: &'static str, value: f64) -> Result<f64, CoreError> {
    if !value.is_finite() {
        return Err(CoreError::NonFiniteValue { field, value });
    }

    Ok(canonicalize_zero(value))
}

fn validate_non_negative(field: &'static str, value: f64) -> Result<f64, CoreError> {
    let value = validate_finite(field, value)?;
    if value < 0.0 {
        return Err(CoreError::NegativeValue { field, value });
    }

    Ok(value)
}

fn validate_bounded(field: &'static str, value: f64, min: f64, max: f64) -> Result<f64, CoreError> {
    let value = validate_finite(field, value)?;
    if value < min || value > max {
        return Err(CoreError::OutOfRange {
            field,
            value,
            min,
            max,
        });
    }

    Ok(value)
}

fn validate_course(course: f64) -> Result<f64, CoreError> {
    let course = validate_bounded("course", course, 0.0, 360.0)?;
    if course == 360.0 {
        return Ok(0.0);
    }

    Ok(canonicalize_zero(course))
}

fn canonicalize_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CoreError, CotDetail, DetailElement, ExtensionBlob, Kinematics, Position, Track, XmlElement,
    };

    #[test]
    fn position_constructor_enforces_ranges() {
        assert!(Position::new(42.0, -73.0).is_ok());
        assert_eq!(
            Position::new(91.0, 0.0),
            Err(CoreError::OutOfRange {
                field: "latitude",
                value: 91.0,
                min: -90.0,
                max: 90.0
            })
        );
        assert_eq!(
            Position::new(0.0, -181.0),
            Err(CoreError::OutOfRange {
                field: "longitude",
                value: -181.0,
                min: -180.0,
                max: 180.0
            })
        );
    }

    #[test]
    fn position_optionals_validate_finite_and_non_negative() {
        let position = Position::new(10.0, 20.0)
            .expect("valid position")
            .with_hae(1200.5)
            .expect("hae should be valid")
            .with_ce(10.0)
            .expect("ce should be valid")
            .with_le(15.0)
            .expect("le should be valid");

        assert_eq!(position.hae(), Some(1200.5));
        assert_eq!(position.ce(), Some(10.0));
        assert_eq!(position.le(), Some(15.0));

        let error = Position::new(10.0, 20.0)
            .expect("valid position")
            .with_ce(-0.5)
            .expect_err("negative ce should fail");
        assert_eq!(
            error,
            CoreError::NegativeValue {
                field: "ce",
                value: -0.5
            }
        );
    }

    #[test]
    fn kinematics_canonicalizes_course_and_validates() {
        let kin = Kinematics::new(Some(12.5), Some(360.0), Some(-0.0)).expect("valid kinematics");
        assert_eq!(kin.course(), Some(0.0));
        assert_eq!(kin.vertical_rate(), Some(0.0));

        let error = Kinematics::new(None, Some(361.0), None).expect_err("course must be bounded");
        assert_eq!(
            error,
            CoreError::OutOfRange {
                field: "course",
                value: 361.0,
                min: 0.0,
                max: 360.0
            }
        );
    }

    #[test]
    fn track_requires_at_least_one_kinematic_component() {
        let empty = Kinematics::new(None, None, None).expect("empty representation is valid");
        assert_eq!(Track::new(empty), Err(CoreError::EmptyTrack));
    }

    #[test]
    fn detail_rejects_duplicate_track_elements() {
        let track = Track::new(Kinematics::new(Some(1.0), None, None).expect("valid kinematics"))
            .expect("track should be valid");
        let duplicate = CotDetail::new(vec![
            DetailElement::Track(track),
            DetailElement::Track(track),
        ]);
        assert_eq!(
            duplicate,
            Err(CoreError::DuplicateTrackElements { count: 2 })
        );
    }

    #[test]
    fn detail_is_canonicalized_deterministically() {
        let track = Track::new(Kinematics::new(Some(1.0), Some(270.0), None).expect("valid kin"))
            .expect("valid track");

        let detail = CotDetail::new(vec![
            DetailElement::Extension(ExtensionBlob::new("beta", vec![2])),
            DetailElement::Unknown(XmlElement::new("contact", "<contact />")),
            DetailElement::Track(track),
            DetailElement::Extension(ExtensionBlob::new("alpha", vec![1])),
        ])
        .expect("detail should canonicalize");

        let ordered_kinds: Vec<&str> = detail
            .elements()
            .iter()
            .map(|element| match element {
                DetailElement::Track(_) => "track",
                DetailElement::Unknown(_) => "unknown",
                DetailElement::Extension(blob) if blob.key == "alpha" => "extension-alpha",
                DetailElement::Extension(_) => "extension-other",
            })
            .collect();

        assert_eq!(
            ordered_kinds,
            vec!["track", "unknown", "extension-alpha", "extension-other"]
        );
    }
}
