pub mod detail;
pub mod model;
pub mod time;

pub use detail::{decode_extension_element, encode_extension_element, ExtensionRegistry};
pub use model::{
    CoreError, CotDetail, DetailElement, ExtensionBlob, Kinematics, Position, Track, XmlElement,
};
pub use time::{TimestampError, TimestampUtc};
