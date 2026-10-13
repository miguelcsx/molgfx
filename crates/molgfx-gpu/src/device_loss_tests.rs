use super::{DeviceLoss, DeviceLossReason};

#[test]
fn only_an_intentional_destroy_is_unrecoverable() {
    let unknown = DeviceLoss {
        reason: DeviceLossReason::Unknown,
        message: "adapter removed".to_owned(),
    };
    let destroyed = DeviceLoss {
        reason: DeviceLossReason::Destroyed,
        message: String::new(),
    };
    assert!(unknown.recoverable());
    assert!(!destroyed.recoverable());
}

#[test]
fn reasons_have_stable_names() {
    assert_eq!(DeviceLossReason::Destroyed.as_str(), "destroyed");
    assert_eq!(DeviceLossReason::Unknown.as_str(), "unknown");
}
