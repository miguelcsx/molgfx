//! Adapter capability negotiation and requested limits.

use super::device::WgpuDevice;
use molgfx_gpu::{Capabilities, CapabilityFlags, GpuError, TextureFormatCapabilities};

pub(super) const REQUIRED_STORAGE_BUFFERS_PER_STAGE: u32 = 8;

impl WgpuDevice {
    #[cfg(test)]
    pub(super) fn probe_capabilities(
        features: wgpu::Features,
        limits: &wgpu::Limits,
    ) -> Capabilities {
        Self::probe_with_formats(features, limits, [TextureFormatCapabilities::default(); 3])
    }

    pub(super) fn probe_adapter_capabilities(
        adapter: &wgpu::Adapter,
        features: wgpu::Features,
        limits: &wgpu::Limits,
    ) -> Capabilities {
        Self::probe_with_formats(features, limits, texture_capabilities(adapter))
    }

    fn probe_with_formats(
        features: wgpu::Features,
        limits: &wgpu::Limits,
        formats: [TextureFormatCapabilities; 3],
    ) -> Capabilities {
        let mut flags = CapabilityFlags::empty();
        let feature_map = [
            (
                wgpu::Features::EXPERIMENTAL_RAY_QUERY,
                CapabilityFlags::RAY_QUERY,
            ),
            (
                wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
                CapabilityFlags::BINDLESS,
            ),
            (
                wgpu::Features::TIMESTAMP_QUERY,
                CapabilityFlags::TIMESTAMP_QUERIES,
            ),
            (wgpu::Features::SUBGROUP, CapabilityFlags::SUBGROUP_OPS),
        ];
        for (feature, capability) in feature_map {
            if features.contains(feature) {
                flags |= capability;
            }
        }
        Capabilities {
            flags,
            max_storage_buffer_bytes: limits.max_storage_buffer_binding_size,
            max_storage_buffers_per_shader_stage: limits.max_storage_buffers_per_shader_stage,
            max_texture_dim: limits.max_texture_dimension_2d,
            max_texture_dim_3d: limits.max_texture_dimension_3d,
            r32float: formats[0],
            rg32float: formats[1],
            rgba32float: formats[2],
        }
    }

    pub(super) fn required_limits(supported: &wgpu::Limits, ray_query: bool) -> wgpu::Limits {
        let limits = wgpu::Limits {
            max_storage_buffer_binding_size: supported.max_storage_buffer_binding_size,
            max_buffer_size: supported.max_buffer_size,
            max_storage_buffers_per_shader_stage: REQUIRED_STORAGE_BUFFERS_PER_STAGE,
            ..wgpu::Limits::default()
                .using_resolution(supported.clone())
                .using_alignment(supported.clone())
        };
        if ray_query {
            limits.using_acceleration_structure_values(supported.clone())
        } else {
            limits
        }
    }

    pub(super) fn negotiated_features(supported: wgpu::Features) -> wgpu::Features {
        let mut requested = wgpu::Features::empty();
        if supported.contains(wgpu::Features::TIMESTAMP_QUERY) {
            requested |= wgpu::Features::TIMESTAMP_QUERY;
        }
        requested
    }

    pub(super) fn require_ray_query(&self) -> Result<(), GpuError> {
        crate::backend::ray_query::require_capability(&self.capabilities)
    }

    pub(super) fn validate_required_limits(supported: &wgpu::Limits) -> Result<(), GpuError> {
        if supported.max_storage_buffers_per_shader_stage < REQUIRED_STORAGE_BUFFERS_PER_STAGE {
            return Err(GpuError::Capability {
                name: "eight storage buffers per shader stage",
            });
        }
        Ok(())
    }
}

fn texture_capabilities(adapter: &wgpu::Adapter) -> [TextureFormatCapabilities; 3] {
    [
        format_capabilities(adapter.get_texture_format_features(wgpu::TextureFormat::R32Float)),
        format_capabilities(adapter.get_texture_format_features(wgpu::TextureFormat::Rg32Float)),
        format_capabilities(adapter.get_texture_format_features(wgpu::TextureFormat::Rgba32Float)),
    ]
}

fn format_capabilities(features: wgpu::TextureFormatFeatures) -> TextureFormatCapabilities {
    TextureFormatCapabilities {
        sampled: features
            .allowed_usages
            .contains(wgpu::TextureUsages::TEXTURE_BINDING),
        storage_write: features
            .allowed_usages
            .contains(wgpu::TextureUsages::STORAGE_BINDING),
    }
}
