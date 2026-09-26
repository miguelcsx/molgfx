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
        .map(|adapter| {
            let info = adapter.get_info();
            AdapterReport {
                name: info.name,
                backend: backend_name(info.backend),
                device_type: device_type_name(info.device_type),
            }
        })
        .collect();

    SystemInfo {
        platform: std::env::consts::OS,
        compiled_backends,
        adapters,
    }
}
