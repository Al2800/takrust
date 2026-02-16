use thiserror::Error;

use crate::MtuSafety;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UdpSendDecision {
    SendDatagrams(Vec<Vec<u8>>),
    DropOversize {
        payload_bytes: usize,
        max_udp_payload_bytes: usize,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UdpPolicyError {
    #[error("max_udp_payload_bytes must be greater than zero")]
    ZeroMaxPayload,
}

pub fn apply_mtu_policy(
    payload: &[u8],
    mtu_safety: &MtuSafety,
) -> Result<UdpSendDecision, UdpPolicyError> {
    if mtu_safety.max_udp_payload_bytes == 0 {
        return Err(UdpPolicyError::ZeroMaxPayload);
    }

    if payload.len() <= mtu_safety.max_udp_payload_bytes {
        return Ok(UdpSendDecision::SendDatagrams(vec![payload.to_vec()]));
    }

    if mtu_safety.drop_oversize {
        return Ok(UdpSendDecision::DropOversize {
            payload_bytes: payload.len(),
            max_udp_payload_bytes: mtu_safety.max_udp_payload_bytes,
        });
    }

    let datagrams = payload
        .chunks(mtu_safety.max_udp_payload_bytes)
        .map(|chunk| chunk.to_vec())
        .collect();
    Ok(UdpSendDecision::SendDatagrams(datagrams))
}

#[cfg(test)]
mod tests {
    use crate::MtuSafety;

    use super::{apply_mtu_policy, UdpPolicyError, UdpSendDecision};

    #[test]
    fn sends_single_datagram_when_payload_fits_limit() {
        let mtu_safety = MtuSafety {
            max_udp_payload_bytes: 8,
            drop_oversize: true,
        };

        let decision = apply_mtu_policy(b"tak", &mtu_safety).expect("policy should evaluate");
        assert_eq!(
            decision,
            UdpSendDecision::SendDatagrams(vec![b"tak".to_vec()])
        );
    }

    #[test]
    fn drops_oversize_payload_when_drop_policy_enabled() {
        let mtu_safety = MtuSafety {
            max_udp_payload_bytes: 4,
            drop_oversize: true,
        };

        let decision = apply_mtu_policy(b"oversize", &mtu_safety).expect("policy should evaluate");
        assert_eq!(
            decision,
            UdpSendDecision::DropOversize {
                payload_bytes: 8,
                max_udp_payload_bytes: 4,
            }
        );
    }

    #[test]
    fn fragments_oversize_payload_when_drop_policy_disabled() {
        let mtu_safety = MtuSafety {
            max_udp_payload_bytes: 3,
            drop_oversize: false,
        };

        let decision = apply_mtu_policy(b"abcdefg", &mtu_safety).expect("policy should evaluate");
        assert_eq!(
            decision,
            UdpSendDecision::SendDatagrams(vec![b"abc".to_vec(), b"def".to_vec(), b"g".to_vec()])
        );
    }

    #[test]
    fn rejects_zero_udp_payload_limit() {
        let mtu_safety = MtuSafety {
            max_udp_payload_bytes: 0,
            drop_oversize: true,
        };

        let error = apply_mtu_policy(b"tak", &mtu_safety).expect_err("zero max payload must fail");
        assert_eq!(error, UdpPolicyError::ZeroMaxPayload);
    }
}
