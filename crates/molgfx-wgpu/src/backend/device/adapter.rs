//! Deterministic capability-based adapter selection.

use molgfx_gpu::PowerPreference;

pub(super) const REQUIRED_STORAGE_BUFFERS_PER_STAGE: u32 = 8;

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

/// Select a headless adapter before device creation, rejecting candidates that
/// cannot satisfy the renderer's already-eager portable baseline.
pub(super) async fn select_headless_adapter(
    instance: &wgpu::Instance,
    preference: PowerPreference,
) -> Result<wgpu::Adapter, String> {
    let mut rejected = Vec::new();
    let mut candidates = Vec::new();
    for adapter in instance.enumerate_adapters(wgpu::Backends::all()).await {
        let limits = adapter.limits();
        let r32_storage = adapter
            .get_texture_format_features(wgpu::TextureFormat::R32Float)
            .allowed_usages
            .contains(wgpu::TextureUsages::STORAGE_BINDING);
        let reasons =
            core_rejection_reasons(limits.max_storage_buffers_per_shader_stage, r32_storage);
        if reasons.is_empty() {
            candidates.push(adapter);
        } else {
            let info = adapter.get_info();
            rejected.push(format!(
                "{} / {:?}: {}",
                info.name,
                info.backend,
                reasons.join(", ")
            ));
        }
    }
    candidates.sort_by_key(|adapter| rank(adapter, preference));
    candidates.into_iter().next().ok_or_else(|| {
        if rejected.is_empty() {
            "no adapters were enumerated".to_owned()
        } else {
            format!(
                "no adapter satisfies MolGFX core requirements: {}",
                rejected.join("; ")
            )
        }
    })
}

fn core_rejection_reasons(storage_buffers_per_stage: u32, r32_storage: bool) -> Vec<String> {
    let mut reasons = Vec::new();
    if storage_buffers_per_stage < REQUIRED_STORAGE_BUFFERS_PER_STAGE {
        reasons.push(format!(
            "max_storage_buffers_per_shader_stage={storage_buffers_per_stage} (required {REQUIRED_STORAGE_BUFFERS_PER_STAGE})"
        ));
    }
    if !r32_storage {
        reasons.push("r32float storage writes unavailable".to_owned());
    }
    reasons
}

fn rank(adapter: &wgpu::Adapter, preference: PowerPreference) -> (u8, String) {
    let info = adapter.get_info();
    (rank_device(preference, info.device_type), info.name)
}

const fn rank_device(preference: PowerPreference, device_type: wgpu::DeviceType) -> u8 {
    match (preference, device_type) {
        (PowerPreference::HighPerformance, wgpu::DeviceType::DiscreteGpu)
        | (PowerPreference::LowPower, wgpu::DeviceType::IntegratedGpu) => 0,
        (PowerPreference::HighPerformance, wgpu::DeviceType::IntegratedGpu)
        | (PowerPreference::LowPower, wgpu::DeviceType::DiscreteGpu) => 1,
        (_, wgpu::DeviceType::VirtualGpu) => 2,
        (_, wgpu::DeviceType::Cpu) => 3,
        (_, wgpu::DeviceType::Other) => 4,
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

#[cfg(test)]
mod tests {
    use super::{core_rejection_reasons, rank_device};
    use molgfx_gpu::PowerPreference;

    #[test]
    fn headless_selection_prefers_power_matched_physical_adapters() {
        assert!(
            rank_device(
                PowerPreference::HighPerformance,
                wgpu::DeviceType::DiscreteGpu
            ) < rank_device(
                PowerPreference::HighPerformance,
                wgpu::DeviceType::IntegratedGpu
            )
        );
        assert!(
            rank_device(PowerPreference::LowPower, wgpu::DeviceType::IntegratedGpu)
                < rank_device(PowerPreference::LowPower, wgpu::DeviceType::DiscreteGpu)
        );
    }

    #[test]
    fn headless_selection_prefers_physical_adapters_over_software() {
        assert!(
            rank_device(
                PowerPreference::HighPerformance,
                wgpu::DeviceType::DiscreteGpu
            ) < rank_device(PowerPreference::HighPerformance, wgpu::DeviceType::Cpu)
        );
    }

    #[test]
    fn headless_selection_rejects_missing_core_capabilities() {
        assert_eq!(core_rejection_reasons(8, true), Vec::<String>::new());
        assert_eq!(
            core_rejection_reasons(7, false),
            vec![
                "max_storage_buffers_per_shader_stage=7 (required 8)".to_owned(),
                "r32float storage writes unavailable".to_owned(),
            ]
        );
    }
}
