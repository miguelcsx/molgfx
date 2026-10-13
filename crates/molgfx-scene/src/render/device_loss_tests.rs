use super::DeviceLossReport;
use molgfx_gpu::{DeviceLoss, DeviceLossReason};

#[test]
fn the_report_serializes_reason_recoverable_and_message() {
    let report = DeviceLossReport::from(&DeviceLoss {
        reason: DeviceLossReason::Unknown,
        message: "gpu \"hung\"".to_owned(),
    });
    let Ok(json) = serde_json::to_string(&report) else {
        panic!("the report serializes")
    };
    assert_eq!(
        json,
        r#"{"reason":"unknown","recoverable":true,"message":"gpu \"hung\""}"#
    );
}

#[test]
fn a_destroyed_device_is_reported_unrecoverable() {
    let report = DeviceLossReport::from(&DeviceLoss {
        reason: DeviceLossReason::Destroyed,
        message: String::new(),
    });
    assert_eq!(report.reason, "destroyed");
    assert!(!report.recoverable);
}
