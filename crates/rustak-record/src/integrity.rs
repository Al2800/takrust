use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityLink {
    pub sequence: u64,
    pub payload_hash: [u8; 32],
    pub previous_chain_hash: Option<[u8; 32]>,
    pub chain_hash: [u8; 32],
    pub signature: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IntegrityChain {
    pub links: Vec<IntegrityLink>,
}

pub trait SignatureProvider {
    fn sign(&self, sequence: u64, chain_hash: &[u8; 32]) -> Option<Vec<u8>>;
}

pub trait SignatureVerifier {
    fn verify(&self, sequence: u64, chain_hash: &[u8; 32], signature: &[u8]) -> bool;
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IntegrityError {
    #[error("payload count {payload_count} does not match chain length {chain_len}")]
    PayloadCountMismatch {
        payload_count: usize,
        chain_len: usize,
    },

    #[error("chain sequence mismatch at index {index}: expected {expected}, found {found}")]
    SequenceMismatch {
        index: usize,
        expected: u64,
        found: u64,
    },

    #[error("payload hash mismatch at sequence {sequence}")]
    PayloadHashMismatch { sequence: u64 },

    #[error("chain hash mismatch at sequence {sequence}")]
    ChainHashMismatch { sequence: u64 },

    #[error("missing signature at sequence {sequence}")]
    MissingSignature { sequence: u64 },

    #[error("no signature verifier configured")]
    MissingVerifier,

    #[error("invalid signature at sequence {sequence}")]
    InvalidSignature { sequence: u64 },
}

#[must_use]
pub fn build_integrity_chain(payloads: &[Vec<u8>]) -> IntegrityChain {
    build_integrity_chain_internal(payloads, None::<&NoopSigner>)
}

#[must_use]
pub fn build_integrity_chain_with_signer<S: SignatureProvider>(
    payloads: &[Vec<u8>],
    signer: &S,
) -> IntegrityChain {
    build_integrity_chain_internal(payloads, Some(signer))
}

pub fn verify_integrity_chain<V: SignatureVerifier>(
    payloads: &[Vec<u8>],
    chain: &IntegrityChain,
    verifier: Option<&V>,
    require_signatures: bool,
) -> Result<(), IntegrityError> {
    if payloads.len() != chain.links.len() {
        return Err(IntegrityError::PayloadCountMismatch {
            payload_count: payloads.len(),
            chain_len: chain.links.len(),
        });
    }

    let mut previous_chain_hash = [0u8; 32];
    for (index, (payload, link)) in payloads.iter().zip(chain.links.iter()).enumerate() {
        let expected_sequence = index as u64;
        if link.sequence != expected_sequence {
            return Err(IntegrityError::SequenceMismatch {
                index,
                expected: expected_sequence,
                found: link.sequence,
            });
        }

        let payload_hash = digest_bytes(payload);
        if link.payload_hash != payload_hash {
            return Err(IntegrityError::PayloadHashMismatch {
                sequence: link.sequence,
            });
        }

        let expected_previous = if index == 0 {
            None
        } else {
            Some(previous_chain_hash)
        };
        if link.previous_chain_hash != expected_previous {
            return Err(IntegrityError::ChainHashMismatch {
                sequence: link.sequence,
            });
        }

        let expected_chain_hash = chain_hash(expected_sequence, payload_hash, previous_chain_hash);
        if link.chain_hash != expected_chain_hash {
            return Err(IntegrityError::ChainHashMismatch {
                sequence: link.sequence,
            });
        }

        if require_signatures {
            let signature = link
                .signature
                .as_ref()
                .ok_or(IntegrityError::MissingSignature {
                    sequence: link.sequence,
                })?;
            let verifier = verifier.ok_or(IntegrityError::MissingVerifier)?;
            if !verifier.verify(link.sequence, &link.chain_hash, signature) {
                return Err(IntegrityError::InvalidSignature {
                    sequence: link.sequence,
                });
            }
        }

        previous_chain_hash = link.chain_hash;
    }

    Ok(())
}

fn build_integrity_chain_internal<S: SignatureProvider>(
    payloads: &[Vec<u8>],
    signer: Option<&S>,
) -> IntegrityChain {
    let mut links = Vec::with_capacity(payloads.len());
    let mut previous_chain_hash = [0u8; 32];

    for (index, payload) in payloads.iter().enumerate() {
        let sequence = index as u64;
        let payload_hash = digest_bytes(payload);
        let chain_hash = chain_hash(sequence, payload_hash, previous_chain_hash);
        let previous_link_hash = if index == 0 {
            None
        } else {
            Some(previous_chain_hash)
        };
        let signature = signer.and_then(|provider| provider.sign(sequence, &chain_hash));

        links.push(IntegrityLink {
            sequence,
            payload_hash,
            previous_chain_hash: previous_link_hash,
            chain_hash,
            signature,
        });
        previous_chain_hash = chain_hash;
    }

    IntegrityChain { links }
}

fn digest_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn chain_hash(sequence: u64, payload_hash: [u8; 32], previous_chain_hash: [u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(previous_chain_hash);
    hasher.update(sequence.to_le_bytes());
    hasher.update(payload_hash);
    hasher.finalize().into()
}

#[derive(Debug)]
struct NoopSigner;

impl SignatureProvider for NoopSigner {
    fn sign(&self, _sequence: u64, _chain_hash: &[u8; 32]) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::integrity::{
        build_integrity_chain, build_integrity_chain_with_signer, verify_integrity_chain,
        IntegrityError, SignatureProvider, SignatureVerifier,
    };

    #[derive(Debug)]
    struct PrefixSigner;

    impl SignatureProvider for PrefixSigner {
        fn sign(&self, _sequence: u64, chain_hash: &[u8; 32]) -> Option<Vec<u8>> {
            Some(chain_hash[..8].to_vec())
        }
    }

    #[derive(Debug)]
    struct PrefixVerifier;

    impl SignatureVerifier for PrefixVerifier {
        fn verify(&self, _sequence: u64, chain_hash: &[u8; 32], signature: &[u8]) -> bool {
            signature == &chain_hash[..8]
        }
    }

    fn sample_payloads() -> Vec<Vec<u8>> {
        vec![
            b"first-packet".to_vec(),
            b"second-packet".to_vec(),
            b"third-packet".to_vec(),
        ]
    }

    #[test]
    fn chain_roundtrip_verifies_without_signatures() {
        let payloads = sample_payloads();
        let chain = build_integrity_chain(&payloads);

        let result = verify_integrity_chain(&payloads, &chain, None::<&PrefixVerifier>, false);
        assert!(result.is_ok());
    }

    #[test]
    fn chain_verification_detects_payload_tampering() {
        let payloads = sample_payloads();
        let chain = build_integrity_chain(&payloads);

        let mut tampered = payloads;
        tampered[1][0] ^= 0xFF;
        let error = verify_integrity_chain(&tampered, &chain, None::<&PrefixVerifier>, false)
            .expect_err("tampered payload should fail");
        assert!(matches!(
            error,
            IntegrityError::PayloadHashMismatch { sequence: 1 }
        ));
    }

    #[test]
    fn signature_hooks_support_optional_verification() {
        let payloads = sample_payloads();
        let signer = PrefixSigner;
        let verifier = PrefixVerifier;
        let chain = build_integrity_chain_with_signer(&payloads, &signer);

        let result = verify_integrity_chain(&payloads, &chain, Some(&verifier), true);
        assert!(result.is_ok());
    }

    #[test]
    fn missing_signature_is_reported_when_required() {
        let payloads = sample_payloads();
        let chain = build_integrity_chain(&payloads);

        let error = verify_integrity_chain(&payloads, &chain, Some(&PrefixVerifier), true)
            .expect_err("missing signatures should fail");
        assert!(matches!(
            error,
            IntegrityError::MissingSignature { sequence: 0 }
        ));
    }
}
