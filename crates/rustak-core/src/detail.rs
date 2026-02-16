use crate::model::{DetailElement, ExtensionBlob};

/// Typed extension registry used by CoT XML codecs and bridge logic.
///
/// Implementors can decode known extension payloads into typed `DetailElement`s
/// and encode typed `DetailElement`s back into extension key/bytes pairs.
pub trait ExtensionRegistry: Send + Sync {
    fn decode(&self, key: &str, bytes: &[u8]) -> Option<DetailElement>;
    fn encode(&self, element: &DetailElement) -> Option<(String, Vec<u8>)>;
}

/// Decodes a raw extension payload with typed registry lookup and opaque fallback.
///
/// Deterministic behavior:
/// - If the registry decodes the payload, the typed `DetailElement` is returned.
/// - Otherwise the original key/bytes are preserved as `DetailElement::Extension`.
#[must_use]
pub fn decode_extension_element<R: ExtensionRegistry + ?Sized>(
    registry: &R,
    key: &str,
    bytes: &[u8],
) -> DetailElement {
    registry
        .decode(key, bytes)
        .unwrap_or_else(|| DetailElement::Extension(ExtensionBlob::new(key, bytes.to_vec())))
}

/// Encodes a detail element with typed registry lookup and opaque passthrough.
///
/// Deterministic behavior:
/// - `DetailElement::Extension` is passed through verbatim.
/// - Non-extension elements are delegated to the registry.
#[must_use]
pub fn encode_extension_element<R: ExtensionRegistry + ?Sized>(
    registry: &R,
    element: &DetailElement,
) -> Option<(String, Vec<u8>)> {
    match element {
        DetailElement::Extension(blob) => Some((blob.key.clone(), blob.bytes.clone())),
        _ => registry.encode(element),
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_extension_element, encode_extension_element, ExtensionRegistry};
    use crate::model::{DetailElement, ExtensionBlob, Kinematics, Track};

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

    struct AggressiveRegistry;

    impl ExtensionRegistry for AggressiveRegistry {
        fn decode(&self, _: &str, _: &[u8]) -> Option<DetailElement> {
            None
        }

        fn encode(&self, _: &DetailElement) -> Option<(String, Vec<u8>)> {
            Some(("mutated".to_string(), vec![9, 9, 9]))
        }
    }

    fn speed_track(speed: f64) -> DetailElement {
        let kin = Kinematics::new(Some(speed), None, None).expect("valid speed track");
        let track = Track::new(kin).expect("valid track");
        DetailElement::Track(track)
    }

    #[test]
    fn typed_extension_roundtrip_uses_registry() {
        let registry = SpeedTrackRegistry;
        let original = speed_track(42.5);

        let (key, bytes) = encode_extension_element(&registry, &original)
            .expect("track should encode into typed extension payload");
        assert_eq!(key, "track/speed-v1");

        let decoded = decode_extension_element(&registry, &key, &bytes);
        assert_eq!(decoded, original);
    }

    #[test]
    fn unknown_extension_decodes_as_opaque_blob() {
        let registry = SpeedTrackRegistry;
        let key = "vendor/raw-v2";
        let bytes = vec![0xCA, 0xFE, 0x01];

        let decoded = decode_extension_element(&registry, key, &bytes);
        assert_eq!(
            decoded,
            DetailElement::Extension(ExtensionBlob::new(key, bytes.clone()))
        );

        let encoded = encode_extension_element(&registry, &decoded)
            .expect("opaque extension should encode as passthrough");
        assert_eq!(encoded, (key.to_string(), bytes));
    }

    #[test]
    fn opaque_passthrough_is_deterministic_even_with_aggressive_registry() {
        let registry = AggressiveRegistry;
        let extension = DetailElement::Extension(ExtensionBlob::new("original", vec![1, 2, 3]));

        for _ in 0..3 {
            let encoded = encode_extension_element(&registry, &extension)
                .expect("opaque extension passthrough should always encode");
            assert_eq!(encoded, ("original".to_string(), vec![1, 2, 3]));
        }
    }
}
