//! Deterministic hardware and software adapter fallback order.

use pdviewx_gpu::PowerPreference;

pub(super) fn wgpu_power(power: PowerPreference) -> wgpu::PowerPreference {
    match power {
        PowerPreference::HighPerformance => wgpu::PowerPreference::HighPerformance,
        PowerPreference::LowPower => wgpu::PowerPreference::LowPower,
    }
}

pub(super) fn opposite_power(power: wgpu::PowerPreference) -> wgpu::PowerPreference {
    match power {
        wgpu::PowerPreference::HighPerformance => wgpu::PowerPreference::LowPower,
        wgpu::PowerPreference::LowPower | wgpu::PowerPreference::None => {
            wgpu::PowerPreference::HighPerformance
        }
    }
}

pub(super) async fn request_adapter<'a>(
    instance: &'a wgpu::Instance,
    surface: Option<&'a wgpu::Surface<'static>>,
    power: wgpu::PowerPreference,
    force_fallback: bool,
) -> Result<wgpu::Adapter, wgpu::RequestAdapterError> {
    instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: power,
            compatible_surface: surface,
            force_fallback_adapter: force_fallback,
            apply_limit_buckets: false,
        })
        .await
}
