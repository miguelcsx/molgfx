use super::*;
use pdviewx_gpu::BufferUsage;
#[cfg(not(target_arch = "wasm32"))]
use pdviewx_gpu::Device as _;

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn this_host_can_open_the_portable_headless_device() {
    let opened = WgpuDevice::open_blocking(&DeviceDesc::default(), None);
    assert!(opened.is_ok(), "{opened:?}");
}

#[test]
fn capabilities_include_only_features_enabled_on_the_device() {
    let capabilities =
        WgpuDevice::probe_capabilities(wgpu::Features::TIMESTAMP_QUERY, &wgpu::Limits::default());

    assert!(capabilities.timestamp_queries());
    assert!(!capabilities.hardware_ray_tracing());
    assert!(!capabilities.bindless());
    assert!(!capabilities.subgroup_ops());
    assert_eq!(
        capabilities.max_storage_buffers_per_shader_stage,
        wgpu::Limits::default().max_storage_buffers_per_shader_stage
    );
}

#[test]
fn ray_query_capability_requires_the_enabled_device_feature() {
    let capabilities = WgpuDevice::probe_capabilities(
        wgpu::Features::EXPERIMENTAL_RAY_QUERY,
        &wgpu::Limits::default().using_minimum_supported_acceleration_structure_values(),
    );

    assert!(capabilities.ray_query());
    assert!(crate::backend::ray_query::require_capability(&capabilities).is_ok());
    assert!(matches!(
        crate::backend::ray_query::require_capability(&Capabilities::default()),
        Err(GpuError::Capability { name: "ray query" })
    ));
}

#[test]
fn portable_negotiation_never_requests_unsafe_experimental_features() {
    let requested = WgpuDevice::negotiated_features(wgpu::Features::EXPERIMENTAL_RAY_QUERY);
    let baseline = WgpuDevice::negotiated_features(wgpu::Features::empty());

    assert!(!requested.contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY));
    assert!(!baseline.contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY));
}

#[test]
fn portable_baseline_does_not_request_vertex_writable_storage() {
    let supported = wgpu::Features::VERTEX_WRITABLE_STORAGE | wgpu::Features::TIMESTAMP_QUERY;
    let requested = WgpuDevice::negotiated_features(supported);
    assert!(!requested.contains(wgpu::Features::VERTEX_WRITABLE_STORAGE));
    assert!(requested.contains(wgpu::Features::TIMESTAMP_QUERY));
}

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

    let requested = WgpuDevice::required_limits(&supported, false);

    assert_eq!(requested.max_buffer_size, supported.max_buffer_size);
    assert_eq!(
        requested.max_storage_buffer_binding_size,
        supported.max_storage_buffer_binding_size
    );
    assert_eq!(
        requested.max_storage_buffers_per_shader_stage,
        REQUIRED_STORAGE_BUFFERS_PER_STAGE
    );
}

#[test]
fn ray_query_limits_are_requested_only_for_the_hardware_path() {
    let supported = wgpu::Limits::default().using_minimum_supported_acceleration_structure_values();

    let portable = WgpuDevice::required_limits(&supported, false);
    let ray_query = WgpuDevice::required_limits(&supported, true);

    assert_eq!(portable.max_blas_primitive_count, 0);
    assert_eq!(
        ray_query.max_blas_primitive_count,
        supported.max_blas_primitive_count
    );
    assert_eq!(
        ray_query.max_tlas_instance_count,
        supported.max_tlas_instance_count
    );
    assert_eq!(
        ray_query.max_acceleration_structures_per_shader_stage,
        supported.max_acceleration_structures_per_shader_stage
    );
}

#[test]
fn adapters_below_the_engine_binding_floor_are_rejected_before_pipeline_creation() {
    let supported = wgpu::Limits {
        max_storage_buffers_per_shader_stage: REQUIRED_STORAGE_BUFFERS_PER_STAGE - 1,
        ..wgpu::Limits::default()
    };

    assert!(matches!(
        WgpuDevice::validate_required_limits(&supported),
        Err(GpuError::Capability {
            name: "eight storage buffers per shader stage"
        })
    ));
}
