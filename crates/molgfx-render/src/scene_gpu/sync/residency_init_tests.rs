use super::frame_upload_config;

#[test]
fn frame_staging_is_reduced_to_one_aligned_uniform() {
    let config = molgfx_gpu::UploadRingConfig {
        capacity_bytes: 16 * 1024 * 1024,
        ticket_capacity: 256,
        epoch_budget_bytes: 16 * 1024 * 1024,
        in_flight_budget_bytes: 16 * 1024 * 1024,
        alignment: 256,
    };
    let reduced = match frame_upload_config(config, 1_040) {
        Ok(value) => value,
        Err(error) => panic!("frame staging must fit: {error}"),
    };
    assert_eq!(reduced.capacity_bytes, 1_280);
    assert_eq!(reduced.ticket_capacity, 1);
    assert_eq!(reduced.epoch_budget_bytes, 1_040);
    assert_eq!(reduced.in_flight_budget_bytes, 1_040);
}

#[test]
fn reducing_frame_staging_preserves_a_smaller_runtime_budget() {
    let mut config = crate::ResidencyConfig::default().uploads;
    config.epoch_budget_bytes = 1;
    config.in_flight_budget_bytes = 2;
    let reduced = frame_upload_config(config, 1_040).expect("valid runtime budget");
    assert_eq!(reduced.epoch_budget_bytes, 1);
    assert_eq!(reduced.in_flight_budget_bytes, 2);
    assert_eq!(reduced.capacity_bytes, 1_280);
}

#[test]
fn reducing_frame_staging_does_not_normalize_invalid_configuration() {
    let config = crate::ResidencyConfig::default().uploads;
    for invalid in [
        molgfx_gpu::UploadRingConfig {
            ticket_capacity: 0,
            ..config
        },
        molgfx_gpu::UploadRingConfig {
            alignment: 3,
            ..config
        },
        molgfx_gpu::UploadRingConfig {
            epoch_budget_bytes: 0,
            ..config
        },
        molgfx_gpu::UploadRingConfig {
            in_flight_budget_bytes: config.capacity_bytes + 1,
            ..config
        },
    ] {
        assert!(frame_upload_config(invalid, 1_040).is_err());
    }
}
