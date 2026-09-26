//! What graphics the build can use and what this machine offers.

/// One adapter the host exposes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterReport {
    /// Driver-reported adapter name, e.g. `llvmpipe`.
    pub name: String,
    /// Graphics API the adapter is reached through (`vulkan`, `gl`, ...).
    pub backend: &'static str,
    /// `discrete`, `integrated`, `virtual`, `cpu` or `other`.
    pub device_type: &'static str,
    /// Maximum storage-buffer bindings visible to one shader stage.
    pub max_storage_buffers_per_shader_stage: u32,
    /// Largest 3-D texture dimension, texels.
    pub max_texture_dim_3d: u32,
    /// Whether the renderer's mandatory baseline is satisfiable.
    pub renderer_core_compatible: bool,
    /// Concrete reasons this adapter cannot open the core renderer.
    pub incompatibility_reasons: Vec<&'static str>,
    /// Whether temporal occupancy can use its native RG32 bounds path.
    pub occupancy_rg32_storage: bool,
    /// Whether temporal occupancy can use its RGBA32 bounds fallback.
    pub occupancy_rgba32_storage: bool,
}

/// Compiled backends and the adapters found through them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemInfo {
    /// `std::env::consts::OS`.
    pub platform: &'static str,
    /// Backends this build carries; a backend missing here can never be found.
    pub compiled_backends: Vec<&'static str>,
    /// Adapters visible right now.
    pub adapters: Vec<AdapterReport>,
}

const fn backend_name(backend: wgpu::Backend) -> &'static str {
    match backend {
        wgpu::Backend::Vulkan => "vulkan",
        wgpu::Backend::Metal => "metal",
        wgpu::Backend::Dx12 => "dx12",
        wgpu::Backend::Gl => "gl",
        wgpu::Backend::BrowserWebGpu => "webgpu",
        wgpu::Backend::Noop => "noop",
    }
}

const fn device_type_name(kind: wgpu::DeviceType) -> &'static str {
    match kind {
        wgpu::DeviceType::DiscreteGpu => "discrete",
        wgpu::DeviceType::IntegratedGpu => "integrated",
        wgpu::DeviceType::VirtualGpu => "virtual",
        wgpu::DeviceType::Cpu => "cpu",
        wgpu::DeviceType::Other => "other",
    }
}

/// Report the compiled backends and enumerate the adapters they can reach.
#[must_use]
pub fn system_info() -> SystemInfo {
    let compiled = wgpu::Instance::enabled_backend_features();
    let compiled_backends = [
        (wgpu::Backends::VULKAN, "vulkan"),
        (wgpu::Backends::GL, "gl"),
        (wgpu::Backends::METAL, "metal"),
        (wgpu::Backends::DX12, "dx12"),
    ]
    .into_iter()
    .filter(|(flag, _)| compiled.contains(*flag))
    .map(|(_, name)| name)
    .collect();

    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
        .iter()
        .map(adapter_report)
        .collect();

    SystemInfo {
        platform: std::env::consts::OS,
        compiled_backends,
        adapters,
    }
}

fn adapter_report(adapter: &wgpu::Adapter) -> AdapterReport {
    let info = adapter.get_info();
    let limits = adapter.limits();
    let density_format = adapter.get_texture_format_features(wgpu::TextureFormat::R32Float);
    let bounds_rg_format = adapter.get_texture_format_features(wgpu::TextureFormat::Rg32Float);
    let bounds_rgba_format = adapter.get_texture_format_features(wgpu::TextureFormat::Rgba32Float);
    let mut incompatibility_reasons = Vec::new();
    if limits.max_storage_buffers_per_shader_stage
        < super::device_caps::REQUIRED_STORAGE_BUFFERS_PER_STAGE
    {
        incompatibility_reasons.push("max_storage_buffers_per_shader_stage is below 8");
    }
    if !density_format
        .allowed_usages
        .contains(wgpu::TextureUsages::STORAGE_BINDING)
    {
        incompatibility_reasons.push("r32float storage writes are unavailable");
    }
    AdapterReport {
        name: info.name,
        backend: backend_name(info.backend),
        device_type: device_type_name(info.device_type),
        max_storage_buffers_per_shader_stage: limits.max_storage_buffers_per_shader_stage,
        max_texture_dim_3d: limits.max_texture_dimension_3d,
        renderer_core_compatible: incompatibility_reasons.is_empty(),
        incompatibility_reasons,
        occupancy_rg32_storage: bounds_rg_format
            .allowed_usages
            .contains(wgpu::TextureUsages::STORAGE_BINDING),
        occupancy_rgba32_storage: bounds_rgba_format
            .allowed_usages
            .contains(wgpu::TextureUsages::STORAGE_BINDING),
    }
}
