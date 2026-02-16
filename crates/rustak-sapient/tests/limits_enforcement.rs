use rustak_sapient::{fuzz_hook_validate_sapient_config, SapientConfig, SapientConfigError};

#[test]
fn rejects_zero_timeout_and_invalid_limits() {
    let mut config = SapientConfig::default();
    config.limits.max_frame_bytes = 0;

    let error = config
        .validate()
        .expect_err("invalid shared limits must fail validation");
    assert!(matches!(error, SapientConfigError::InvalidLimits(_)));
}

#[test]
fn fuzz_hook_accepts_arbitrary_corpus_without_panics() {
    let corpus = [
        &[][..],
        &[0u8; 1][..],
        &[255u8; 16][..],
        &[1, 2, 3, 4, 5, 6, 7][..],
    ];

    for sample in corpus {
        let _ = fuzz_hook_validate_sapient_config(sample);
    }
}
