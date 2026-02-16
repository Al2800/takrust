use serde_yaml::Value;

use crate::{schema::RustakConfigDocument, ConfigError, RustakConfig};

const REDACTED: &str = "[REDACTED]";
const DEFAULT_REDACT_PATHS: [&str; 3] = [
    "certificates.client_key",
    "certificates.client_cert",
    "crypto.server_spki_pin",
];

pub(crate) fn to_redacted_yaml(config: &RustakConfig) -> Result<String, ConfigError> {
    let document = RustakConfigDocument::from(config);
    let mut value = serde_yaml::to_value(document).map_err(ConfigError::SerializeConfig)?;

    let mut redact_paths: Vec<String> = DEFAULT_REDACT_PATHS
        .iter()
        .map(|path| (*path).to_owned())
        .collect();

    if let Some(logging) = &config.logging {
        redact_paths.extend(logging.redact.iter().cloned());
    }

    redact_paths.sort();
    redact_paths.dedup();

    for path in redact_paths {
        redact_path(&mut value, &path);
    }

    serde_yaml::to_string(&value).map_err(ConfigError::SerializeConfig)
}

fn redact_path(root: &mut Value, path: &str) {
    let mut cursor = root;
    let mut parts = path.split('.').peekable();

    while let Some(part) = parts.next() {
        let is_leaf = parts.peek().is_none();
        let Value::Mapping(mapping) = cursor else {
            return;
        };

        let key = Value::String(part.to_owned());
        if is_leaf {
            if mapping.contains_key(&key) {
                mapping.insert(key, Value::String(REDACTED.to_owned()));
            }
            return;
        }

        let Some(next) = mapping.get_mut(&key) else {
            return;
        };
        cursor = next;
    }
}
