use rustak_transport::{apply_mtu_policy, MtuSafety, UdpPolicyError, UdpSendDecision};

#[test]
fn drop_policy_rejects_oversize_payload() {
    let mtu_safety = MtuSafety {
        max_udp_payload_bytes: 1200,
        drop_oversize: true,
    };
    let payload = vec![0xAB; 1400];

    let decision = apply_mtu_policy(&payload, &mtu_safety).expect("policy should evaluate");
    assert_eq!(
        decision,
        UdpSendDecision::DropOversize {
            payload_bytes: 1400,
            max_udp_payload_bytes: 1200,
        }
    );
}

#[test]
fn split_policy_fragments_oversize_payload_deterministically() {
    let mtu_safety = MtuSafety {
        max_udp_payload_bytes: 512,
        drop_oversize: false,
    };
    let payload = vec![0xCD; 1300];

    let decision = apply_mtu_policy(&payload, &mtu_safety).expect("policy should evaluate");
    let UdpSendDecision::SendDatagrams(datagrams) = decision else {
        panic!("split policy should produce datagrams");
    };

    let sizes: Vec<usize> = datagrams.iter().map(Vec::len).collect();
    assert_eq!(sizes, vec![512, 512, 276]);
    assert!(datagrams
        .iter()
        .all(|chunk| chunk.iter().all(|byte| *byte == 0xCD)));
}

#[test]
fn payload_within_limit_is_single_datagram() {
    let mtu_safety = MtuSafety {
        max_udp_payload_bytes: 1200,
        drop_oversize: false,
    };
    let payload = vec![0x7E; 1200];

    let decision = apply_mtu_policy(&payload, &mtu_safety).expect("policy should evaluate");
    assert_eq!(decision, UdpSendDecision::SendDatagrams(vec![payload]));
}

#[test]
fn zero_mtu_limit_is_rejected() {
    let mtu_safety = MtuSafety {
        max_udp_payload_bytes: 0,
        drop_oversize: true,
    };
    let payload = vec![0x11; 64];

    let error = apply_mtu_policy(&payload, &mtu_safety).expect_err("zero mtu should fail");
    assert_eq!(error, UdpPolicyError::ZeroMaxPayload);
}
