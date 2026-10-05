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

#[cfg(target_vendor = "apple")]
#[test]
fn native_metal_wins_when_the_same_physical_adapter_also_exposes_vulkan() {
    let mut metal = wgpu::AdapterInfo::new(wgpu::DeviceType::IntegratedGpu, wgpu::Backend::Metal);
    metal.name = "Z native Metal".to_owned();
    let vulkan = wgpu::AdapterInfo {
        name: "A Vulkan portability".to_owned(),
        backend: wgpu::Backend::Vulkan,
        ..metal.clone()
    };
    assert!(
        super::rank_info(&metal, PowerPreference::HighPerformance)
            < super::rank_info(&vulkan, PowerPreference::HighPerformance)
    );
}
