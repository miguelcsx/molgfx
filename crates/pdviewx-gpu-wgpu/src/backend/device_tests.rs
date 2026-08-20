use super::*;
use pdviewx_gpu::BufferUsage;

#[test]
fn buffer_validation_returns_limits_instead_of_reaching_wgpu_validation() {
    let limits = wgpu::Limits::default();
    let storage = BufferDesc {
        label: "large storage",
        size: limits.max_storage_buffer_binding_size + 1,
        usage: BufferUsage::STORAGE,
    };
    let plain = BufferDesc {
        label: "large buffer",
        size: limits.max_buffer_size + 1,
        usage: BufferUsage::COPY_DST,
    };

    assert!(matches!(
        WgpuDevice::validate_buffer(&storage, &limits),
        Err(GpuError::LimitExceeded {
            resource: "large storage",
            limit
        }) if limit == limits.max_storage_buffer_binding_size
    ));
    assert!(matches!(
        WgpuDevice::validate_buffer(&plain, &limits),
        Err(GpuError::LimitExceeded {
            resource: "large buffer",
            limit
        }) if limit == limits.max_buffer_size
    ));
}

#[test]
fn requested_limits_keep_the_adapters_large_buffer_capacity() {
    let supported = wgpu::Limits {
        max_buffer_size: 2 << 30,
        max_storage_buffer_binding_size: 1 << 30,
        ..wgpu::Limits::default()
    };

    let requested = WgpuDevice::required_limits(&supported);

    assert_eq!(requested.max_buffer_size, supported.max_buffer_size);
    assert_eq!(
        requested.max_storage_buffer_binding_size,
        supported.max_storage_buffer_binding_size
    );
}
