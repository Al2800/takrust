use rustak_core::{DetailElement, ExtensionBlob, Kinematics, Track};
use rustak_cot::{decode_extension_element, encode_extension_element, ExtensionRegistry};

struct SpeedTrackRegistry;

impl ExtensionRegistry for SpeedTrackRegistry {
    fn decode(&self, key: &str, bytes: &[u8]) -> Option<DetailElement> {
        if key != "track/speed-v1" {
            return None;
        }

        let speed = std::str::from_utf8(bytes).ok()?.parse::<f64>().ok()?;
        let kin = Kinematics::new(Some(speed), None, None).ok()?;
        let track = Track::new(kin).ok()?;
        Some(DetailElement::Track(track))
    }

    fn encode(&self, element: &DetailElement) -> Option<(String, Vec<u8>)> {
        let DetailElement::Track(track) = element else {
            return None;
        };

        let kin = track.kinematics();
        if kin.course().is_some() || kin.vertical_rate().is_some() {
            return None;
        }

        let speed = kin.speed()?;
        Some(("track/speed-v1".to_string(), speed.to_string().into_bytes()))
    }
}

fn speed_track(speed: f64) -> DetailElement {
    let kin = Kinematics::new(Some(speed), None, None).expect("valid speed track");
    let track = Track::new(kin).expect("valid track");
    DetailElement::Track(track)
}

#[test]
fn typed_extension_registry_roundtrip_is_supported() {
    let registry = SpeedTrackRegistry;
    let original = speed_track(21.25);

    let (key, bytes) = encode_extension_element(&registry, &original)
        .expect("typed track should be encoded by registry");
    assert_eq!(key, "track/speed-v1");

    let decoded = decode_extension_element(&registry, &key, &bytes);
    assert_eq!(decoded, original);
}

#[test]
fn opaque_extension_passthrough_is_preserved() {
    let registry = SpeedTrackRegistry;
    let key = "vendor/raw-v2";
    let bytes = vec![0xAA, 0xBB, 0xCC, 0xDD];

    let decoded = decode_extension_element(&registry, key, &bytes);
    assert_eq!(
        decoded,
        DetailElement::Extension(ExtensionBlob::new(key, bytes.clone()))
    );

    let encoded = encode_extension_element(&registry, &decoded)
        .expect("opaque extension should always pass through");
    assert_eq!(encoded, (key.to_string(), bytes));
}
