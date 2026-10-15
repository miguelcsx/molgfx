//! GPU-resident temporal occupancy accumulation pipelines.

use crate::error::RenderError;
use molgfx_gpu::{
    Capabilities, ComputePipelineDesc, Device, GpuError, ShaderModuleDesc, TextureFormat,
};

/// The physical storage representation selected for temporal occupancy bounds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum OccupancyBoundsFormat {
    /// Native two-channel min/max texture.
    Rg32Float,
    /// Portable four-channel texture with minimum and maximum in `.xy`.
    Rgba32Float,
}

impl OccupancyBoundsFormat {
    /// Resolves the occupancy implementation from normalized device capabilities.
    pub(crate) fn resolve(capabilities: &Capabilities) -> Result<Self, RenderError> {
        if !capabilities.r32float.sampled || !capabilities.r32float.storage_write {
            return Err(GpuError::Capability {
                name: "temporal occupancy r32float sampled storage texture",
            }
            .into());
        }
        if capabilities.rg32float.sampled && capabilities.rg32float.storage_write {
            return Ok(Self::Rg32Float);
        }
        if capabilities.rgba32float.sampled && capabilities.rgba32float.storage_write {
            return Ok(Self::Rgba32Float);
        }
        Err(GpuError::Capability {
            name: "temporal occupancy bounds sampled storage texture",
        }
        .into())
    }

    pub(crate) const fn texture_format(self) -> TextureFormat {
        match self {
            Self::Rg32Float => TextureFormat::Rg32Float,
            Self::Rgba32Float => TextureFormat::Rgba32Float,
        }
    }

    fn shader(self) -> &'static str {
        match self {
            Self::Rg32Float => molgfx_shaders::OCCUPANCY,
            Self::Rgba32Float => molgfx_shaders::OCCUPANCY_RGBA,
        }
    }
}

#[derive(Debug)]
pub(crate) struct OccupancyPass<D: Device> {
    pub(crate) clear: D::Pipeline,
    pub(crate) decay: D::Pipeline,
    pub(crate) deposit: D::Pipeline,
    pub(crate) resolve: D::Pipeline,
    pub(crate) bounds: D::Pipeline,
}

impl<D: Device> OccupancyPass<D> {
    pub(crate) fn new(
        device: &D,
        layout: &D::BindGroupLayout,
        format: OccupancyBoundsFormat,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "temporal occupancy accumulation",
            wgsl: format.shader(),
        })?;
        let pipeline = |label, entry| {
            device.create_compute_pipeline(&ComputePipelineDesc {
                label,
                layouts: &[Some(layout)],
                shader: &shader,
                entry,
            })
        };
        Ok(Self {
            clear: pipeline("clear temporal occupancy", "clear_occupancy")?,
            decay: pipeline("decay temporal occupancy", "decay_occupancy")?,
            deposit: pipeline("deposit temporal occupancy", "deposit_occupancy")?,
            resolve: pipeline("resolve temporal occupancy", "resolve_occupancy")?,
            bounds: pipeline(
                "resolve temporal occupancy bounds",
                "resolve_occupancy_bounds",
            )?,
        })
    }
}

#[cfg(test)]
#[path = "occupancy_tests.rs"]
mod tests;
