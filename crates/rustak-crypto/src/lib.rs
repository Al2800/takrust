use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, CryptoError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoProviderMode {
    Ring,
    AwsLcRs,
    AwsLcRsFips,
}

impl CryptoProviderMode {
    pub fn validate(self, support: ProviderSupport) -> Result<()> {
        if matches!(self, Self::AwsLcRsFips) && !support.fips_enabled {
            return Err(CryptoError::FipsProviderUnavailable);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProviderSupport {
    pub fips_enabled: bool,
}

impl ProviderSupport {
    #[must_use]
    pub const fn with_fips_enabled(fips_enabled: bool) -> Self {
        Self { fips_enabled }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevocationPolicy {
    Off,
    Prefer,
    Require,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CryptoConfig {
    pub provider: CryptoProviderMode,
    pub revocation: RevocationPolicy,
    pub identity: IdentitySource,
}

impl CryptoConfig {
    pub fn validate(&self, support: ProviderSupport) -> Result<()> {
        self.provider.validate(support)?;
        self.identity.validate()
    }

    pub fn load_identity(&self) -> Result<LoadedIdentity> {
        self.identity.load()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentitySource {
    PemFiles {
        ca_cert_path: PathBuf,
        client_cert_path: PathBuf,
        client_key_path: PathBuf,
    },
    Pkcs12File {
        archive_path: PathBuf,
        password: Option<String>,
    },
}

impl IdentitySource {
    pub fn validate(&self) -> Result<()> {
        match self {
            IdentitySource::PemFiles {
                ca_cert_path,
                client_cert_path,
                client_key_path,
            } => {
                validate_path(ca_cert_path, "ca_cert_path")?;
                validate_path(client_cert_path, "client_cert_path")?;
                validate_path(client_key_path, "client_key_path")?;
                Ok(())
            }
            IdentitySource::Pkcs12File {
                archive_path,
                password,
            } => {
                validate_path(archive_path, "archive_path")?;
                if let Some(value) = password {
                    if value.trim().is_empty() {
                        return Err(CryptoError::EmptyPkcs12Password);
                    }
                }
                Ok(())
            }
        }
    }

    pub fn load(&self) -> Result<LoadedIdentity> {
        self.validate()?;

        match self {
            IdentitySource::PemFiles {
                ca_cert_path,
                client_cert_path,
                client_key_path,
            } => {
                let ca_cert_pem = read_text(ca_cert_path)?;
                ensure_pem_block(ca_cert_path, &ca_cert_pem, "CERTIFICATE")?;

                let client_cert_pem = read_text(client_cert_path)?;
                ensure_pem_block(client_cert_path, &client_cert_pem, "CERTIFICATE")?;

                let client_key_pem = read_text(client_key_path)?;
                ensure_private_key_block(client_key_path, &client_key_pem)?;

                Ok(LoadedIdentity::Pem(PemIdentity {
                    ca_cert_pem,
                    client_cert_pem,
                    client_key_pem,
                }))
            }
            IdentitySource::Pkcs12File {
                archive_path,
                password,
            } => {
                let archive_bytes = read_bytes(archive_path)?;
                if archive_bytes.is_empty() {
                    return Err(CryptoError::EmptyPkcs12Archive {
                        path: archive_path.display().to_string(),
                    });
                }

                Ok(LoadedIdentity::Pkcs12(Pkcs12Identity {
                    archive_bytes,
                    password: password.clone(),
                }))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadedIdentity {
    Pem(PemIdentity),
    Pkcs12(Pkcs12Identity),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PemIdentity {
    pub ca_cert_pem: String,
    pub client_cert_pem: String,
    pub client_key_pem: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkcs12Identity {
    pub archive_bytes: Vec<u8>,
    pub password: Option<String>,
}

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("fips provider mode requested but runtime support is disabled")]
    FipsProviderUnavailable,
    #[error("required path for `{field}` is empty")]
    EmptyPath { field: &'static str },
    #[error("pkcs12 password is empty")]
    EmptyPkcs12Password,
    #[error("failed reading `{path}`: {source}")]
    ReadPath {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("expected PEM block `{block}` in `{path}`")]
    MissingPemBlock { path: String, block: &'static str },
    #[error("pkcs12 archive at `{path}` is empty")]
    EmptyPkcs12Archive { path: String },
}

fn validate_path(path: &Path, field: &'static str) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(CryptoError::EmptyPath { field });
    }
    Ok(())
}

fn read_text(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|source| CryptoError::ReadPath {
        path: path.display().to_string(),
        source,
    })
}

fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|source| CryptoError::ReadPath {
        path: path.display().to_string(),
        source,
    })
}

fn ensure_pem_block(path: &Path, contents: &str, block: &'static str) -> Result<()> {
    if contains_pem_block(contents, block) {
        return Ok(());
    }

    Err(CryptoError::MissingPemBlock {
        path: path.display().to_string(),
        block,
    })
}

fn ensure_private_key_block(path: &Path, contents: &str) -> Result<()> {
    const PRIVATE_KEY_BLOCKS: [&str; 3] = ["PRIVATE KEY", "RSA PRIVATE KEY", "EC PRIVATE KEY"];
    if PRIVATE_KEY_BLOCKS
        .iter()
        .any(|block| contains_pem_block(contents, block))
    {
        return Ok(());
    }

    Err(CryptoError::MissingPemBlock {
        path: path.display().to_string(),
        block: "PRIVATE KEY",
    })
}

fn contains_pem_block(contents: &str, block: &str) -> bool {
    let begin = format!("-----BEGIN {block}-----");
    let end = format!("-----END {block}-----");
    contents.contains(&begin) && contents.contains(&end)
}

#[cfg(test)]
mod tests {
    use super::{
        CryptoConfig, CryptoError, CryptoProviderMode, IdentitySource, LoadedIdentity,
        ProviderSupport, RevocationPolicy,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn provider_mode_rejects_fips_without_support() {
        let error = CryptoProviderMode::AwsLcRsFips
            .validate(ProviderSupport::default())
            .expect_err("fips mode should require runtime support");
        assert!(matches!(error, CryptoError::FipsProviderUnavailable));

        assert!(CryptoProviderMode::AwsLcRsFips
            .validate(ProviderSupport::with_fips_enabled(true))
            .is_ok());
    }

    #[test]
    fn pem_identity_loads_with_required_blocks() {
        let dir = test_dir("pem_identity_loads_with_required_blocks");
        let ca_path = dir.join("ca.pem");
        let cert_path = dir.join("client.pem");
        let key_path = dir.join("client.key");

        write_file(&ca_path, pem_certificate("ca"));
        write_file(&cert_path, pem_certificate("client"));
        write_file(&key_path, pem_private_key("key"));

        let identity = IdentitySource::PemFiles {
            ca_cert_path: ca_path,
            client_cert_path: cert_path,
            client_key_path: key_path,
        };

        let loaded = identity.load().expect("pem identity should load");
        assert!(matches!(loaded, LoadedIdentity::Pem(_)));
    }

    #[test]
    fn pem_identity_rejects_missing_private_key_block() {
        let dir = test_dir("pem_identity_rejects_missing_private_key_block");
        let ca_path = dir.join("ca.pem");
        let cert_path = dir.join("client.pem");
        let key_path = dir.join("client.key");

        write_file(&ca_path, pem_certificate("ca"));
        write_file(&cert_path, pem_certificate("client"));
        write_file(&key_path, "not-a-private-key");

        let identity = IdentitySource::PemFiles {
            ca_cert_path: ca_path,
            client_cert_path: cert_path,
            client_key_path: key_path,
        };

        let error = identity
            .load()
            .expect_err("invalid key fixture should fail validation");
        assert!(matches!(
            error,
            CryptoError::MissingPemBlock {
                block: "PRIVATE KEY",
                ..
            }
        ));
    }

    #[test]
    fn pkcs12_identity_rejects_empty_archive() {
        let dir = test_dir("pkcs12_identity_rejects_empty_archive");
        let archive_path = dir.join("identity.p12");
        write_file(&archive_path, "");

        let identity = IdentitySource::Pkcs12File {
            archive_path,
            password: Some("secret".to_owned()),
        };

        let error = identity
            .load()
            .expect_err("empty p12 fixture should fail validation");
        assert!(matches!(error, CryptoError::EmptyPkcs12Archive { .. }));
    }

    #[test]
    fn config_validate_rejects_blank_pkcs12_password() {
        let config = CryptoConfig {
            provider: CryptoProviderMode::Ring,
            revocation: RevocationPolicy::Prefer,
            identity: IdentitySource::Pkcs12File {
                archive_path: PathBuf::from("tests/fixtures/certs/dev_identity.p12"),
                password: Some("   ".to_owned()),
            },
        };

        let error = config
            .validate(ProviderSupport::default())
            .expect_err("blank password should fail");
        assert!(matches!(error, CryptoError::EmptyPkcs12Password));
    }

    fn test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "rustak_crypto_tests_{}_{}_{}",
            label,
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&dir).expect("temp test dir should be created");
        dir
    }

    fn write_file(path: &PathBuf, contents: impl AsRef<[u8]>) {
        fs::write(path, contents).expect("fixture file should be written");
    }

    fn pem_certificate(body: &str) -> String {
        format!("-----BEGIN CERTIFICATE-----\n{body}\n-----END CERTIFICATE-----\n")
    }

    fn pem_private_key(body: &str) -> String {
        format!("-----BEGIN PRIVATE KEY-----\n{body}\n-----END PRIVATE KEY-----\n")
    }
}
