use super::{DEVICE_LOST, DeviceErrors};
use molgfx_gpu::{DeviceLoss, DeviceLossReason, GpuError};
use std::sync::atomic::Ordering;

#[test]
fn asynchronous_validation_preserves_the_first_cause_without_panicking() {
    let errors = DeviceErrors::default();
    assert!(errors.check().is_ok());
    errors.record("binding too small".to_owned());
    errors.record("command buffer invalid".to_owned());
    assert!(
        matches!(errors.check(), Err(GpuError::Runtime { detail }) if detail == "binding too small")
    );
    assert!(errors.check().is_ok());
}

#[test]
fn device_loss_remains_sticky_after_the_originating_validation_error_is_reported() {
    let errors = DeviceErrors::default();
    errors.record("allocation failed".to_owned());
    errors.status.fetch_or(DEVICE_LOST, Ordering::Release);
    assert!(matches!(errors.check(), Err(GpuError::Runtime { .. })));
    for _ in 0..3 {
        assert!(matches!(errors.check(), Err(GpuError::DeviceLost)));
    }
}

#[test]
fn a_recorded_loss_carries_its_reason_and_keeps_the_first() {
    let errors = DeviceErrors::default();
    assert_eq!(errors.loss(), None);
    errors.record_loss(DeviceLoss {
        reason: DeviceLossReason::Unknown,
        message: "gpu hung".to_owned(),
    });
    errors.record_loss(DeviceLoss {
        reason: DeviceLossReason::Destroyed,
        message: "later".to_owned(),
    });
    let Some(loss) = errors.loss() else {
        panic!("the loss was recorded")
    };
    assert_eq!(loss.reason, DeviceLossReason::Unknown);
    assert_eq!(loss.message, "gpu hung");
    assert!(loss.recoverable());
    assert!(matches!(errors.check(), Err(GpuError::DeviceLost)));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn native_validation_callback_returns_the_original_error_without_panicking() {
    use crate::backend::device::WgpuDevice;
    use molgfx_gpu::{Device as _, DeviceDesc};

    let opened = WgpuDevice::open_blocking(&DeviceDesc::default(), None)
        .expect("native adapter opens for callback validation");
    let _invalid = opened.device.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("invalid buffer callback probe"),
        size: 16,
        usage: wgpu::BufferUsages::empty(),
        mapped_at_creation: false,
    });
    opened
        .device
        .device
        .poll(wgpu::PollType::Poll)
        .expect("device remains available");
    let error = opened
        .device
        .check_errors()
        .expect_err("validation callback must report failure");
    assert!(
        matches!(error, GpuError::Runtime { detail } if detail.contains("invalid buffer callback probe"))
    );
    assert!(opened.device.check_errors().is_ok());
}

/// Opens a native device, or reports that this machine has no adapter so the
/// caller can skip. Any other failure is a real defect and fails the test.
#[cfg(not(target_arch = "wasm32"))]
fn open_or_skip() -> Option<molgfx_gpu::Opened<crate::backend::device::WgpuDevice>> {
    use crate::backend::device::WgpuDevice;
    use molgfx_gpu::{Device as _, DeviceDesc};

    match WgpuDevice::open_blocking(&DeviceDesc::default(), None) {
        Ok(opened) => Some(opened),
        Err(GpuError::NoAdapter { detail }) => {
            eprintln!("skipping: no native GPU adapter ({detail})");
            None
        }
        Err(other) => panic!("adapter probe failed for a reason other than absence: {other}"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_destroyed_real_device_reports_a_typed_loss_and_a_fresh_device_replaces_it() {
    use molgfx_gpu::Device as _;

    let Some(lost) = open_or_skip() else { return };
    assert_eq!(lost.device.device_loss(), None);
    assert!(lost.device.check_errors().is_ok());

    // wgpu completes a destroy on the first poll that finds the queue empty.
    lost.device.device.destroy();
    lost.queue.queue.submit([]);
    for _ in 0..50 {
        lost.device
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("a destroyed device still drains its queue");
        if lost.device.device_loss().is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let Some(recorded) = lost.device.device_loss() else {
        panic!("destroying the device must record a typed loss")
    };
    assert_eq!(recorded.reason, DeviceLossReason::Destroyed);
    assert!(!recorded.recoverable());
    for _ in 0..2 {
        assert!(matches!(
            lost.device.check_errors(),
            Err(GpuError::DeviceLost)
        ));
    }

    let Some(fresh) = open_or_skip() else { return };
    assert_eq!(fresh.device.device_loss(), None);
    assert!(fresh.device.check_errors().is_ok());
    assert_eq!(
        lost.device.device_loss().map(|cause| cause.reason),
        Some(DeviceLossReason::Destroyed)
    );
}
