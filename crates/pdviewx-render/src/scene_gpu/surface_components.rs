//! Persistent working-set scratch for sampled-surface component filtering.

use super::surface_slot::FieldTexture;
use crate::error::RenderError;
use pdviewx_core::{SurfaceComponentPolicy, SurfaceComponentThreshold};
use pdviewx_gpu::{
    BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, Device, Queue, TextureFormat,
};

#[cfg(test)]
#[path = "surface_components_tests.rs"]
mod tests;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ComponentConfig {
    dimensions: [u32; 4],
    metric: [u32; 4],
    values: [f32; 4],
}

#[derive(Debug)]
pub(super) struct SurfaceComponents<D: Device> {
    filtered: Option<FieldTexture<D>>,
    parents: Option<D::Buffer>,
    statistics: Option<D::Buffer>,
    uniforms: Option<D::Buffer>,
    group: Option<D::BindGroup>,
    voxel_capacity: u64,
    dimensions: [u32; 3],
}

pub(super) struct SurfaceComponentSync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) layout: &'a D::BindGroupLayout,
    pub(super) source: &'a D::TextureView,
    pub(super) dimensions: [u32; 3],
    pub(super) cell: f32,
    pub(super) isolevel: f32,
    pub(super) gaussian: bool,
    pub(super) policy: SurfaceComponentPolicy,
}

impl<D: Device> SurfaceComponents<D> {
    pub(super) const fn new() -> Self {
        Self {
            filtered: None,
            parents: None,
            statistics: None,
            uniforms: None,
            group: None,
            voxel_capacity: 0,
            dimensions: [0; 3],
        }
    }

    pub(super) fn sync(&mut self, input: &SurfaceComponentSync<'_, D>) -> Result<(), RenderError> {
        if !input.policy.is_enabled() {
            self.release();
            return Ok(());
        }
        let voxels = voxel_count(input.dimensions)?;
        let resized = input.dimensions != self.dimensions;
        if resized || self.filtered.is_none() {
            self.filtered = Some(FieldTexture::create(
                input.device,
                input.dimensions,
                "component-filtered surface field",
                TextureFormat::R32Float,
            )?);
            self.dimensions = input.dimensions;
        }
        if voxels > self.voxel_capacity {
            self.parents = Some(storage_buffer(
                input.device,
                "surface component parents",
                voxels,
                4,
            )?);
            self.statistics = Some(storage_buffer(
                input.device,
                "surface component statistics",
                voxels,
                8,
            )?);
            self.voxel_capacity = voxels;
        }
        if self.uniforms.is_none() {
            self.uniforms = Some(input.device.create_buffer(&BufferDesc {
                label: "surface component policy",
                size: std::mem::size_of::<ComponentConfig>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?);
        }
        let config = component_config(
            input.dimensions,
            input.cell,
            input.isolevel,
            input.gaussian,
            input.policy,
        )?;
        let (Some(filtered), Some(parents), Some(statistics), Some(uniforms)) = (
            &self.filtered,
            &self.parents,
            &self.statistics,
            &self.uniforms,
        ) else {
            return Err(RenderError::SurfaceComponents {
                reason: "component scratch was not allocated",
            });
        };
        input
            .queue
            .write_buffer(uniforms, 0, bytemuck::bytes_of(&config));
        self.group = Some(input.device.create_bind_group(&BindGroupDesc {
            label: "surface component working set",
            layout: input.layout,
            entries: &[
                BindGroupEntry::Texture {
                    binding: 0,
                    view: input.source,
                },
                BindGroupEntry::Texture {
                    binding: 1,
                    view: &filtered.view,
                },
                BindGroupEntry::Buffer {
                    binding: 2,
                    buffer: parents,
                },
                BindGroupEntry::Buffer {
                    binding: 3,
                    buffer: statistics,
                },
                BindGroupEntry::Buffer {
                    binding: 4,
                    buffer: uniforms,
                },
            ],
        }));
        Ok(())
    }

    pub(super) fn group(&self) -> Option<&D::BindGroup> {
        self.group.as_ref()
    }

    pub(super) fn field(&self) -> Option<&D::TextureView> {
        self.filtered.as_ref().map(|field| &field.view)
    }

    pub(super) fn release(&mut self) {
        self.filtered = None;
        self.parents = None;
        self.statistics = None;
        self.uniforms = None;
        self.group = None;
        self.voxel_capacity = 0;
        self.dimensions = [0; 3];
    }
}

fn voxel_count(dimensions: [u32; 3]) -> Result<u64, RenderError> {
    let count = dimensions.into_iter().try_fold(1_u64, |total, axis| {
        total
            .checked_mul(u64::from(axis))
            .ok_or(RenderError::SurfaceComponents {
                reason: "surface component working set overflows address space",
            })
    })?;
    if count > u64::from(u32::MAX) {
        return Err(RenderError::SurfaceComponents {
            reason: "surface component working set exceeds local u32 labels",
        });
    }
    Ok(count)
}

fn storage_buffer<D: Device>(
    device: &D,
    label: &'static str,
    elements: u64,
    stride: u64,
) -> Result<D::Buffer, RenderError> {
    let size = elements
        .checked_mul(stride)
        .ok_or(RenderError::SurfaceComponents {
            reason: "surface component scratch size overflows",
        })?;
    if size > device.capabilities().max_storage_buffer_bytes {
        return Err(RenderError::SurfaceComponents {
            reason: "surface component scratch exceeds the device storage-buffer limit",
        });
    }
    Ok(device.create_buffer(&BufferDesc {
        label,
        size,
        usage: BufferUsage::STORAGE,
    })?)
}

fn component_config(
    dimensions: [u32; 3],
    cell: f32,
    isolevel: f32,
    gaussian: bool,
    policy: SurfaceComponentPolicy,
) -> Result<ComponentConfig, RenderError> {
    let (mode, minimum_voxels, threshold) = match policy.threshold() {
        SurfaceComponentThreshold::Disabled => (0, 0, 0.0),
        SurfaceComponentThreshold::Area(value) => (1, 0, finite_f32(value)?),
        SurfaceComponentThreshold::Volume(value) => (2, 0, finite_f32(value)?),
        SurfaceComponentThreshold::Voxels(value) => match u32::try_from(value) {
            Ok(value) => (3, value, 0.0),
            Err(_) => (4, 0, 0.0),
        },
    };
    Ok(ComponentConfig {
        dimensions: [dimensions[0], dimensions[1], dimensions[2], 0],
        metric: [mode, u32::from(gaussian), minimum_voxels, 0],
        values: [isolevel, cell, threshold, 0.0],
    })
}

fn finite_f32(value: f64) -> Result<f32, RenderError> {
    let converted =
        value
            .to_string()
            .parse::<f32>()
            .map_err(|_| RenderError::SurfaceComponents {
                reason: "surface component threshold cannot be lowered to GPU precision",
            })?;
    if converted.is_finite() {
        Ok(converted)
    } else {
        Err(RenderError::SurfaceComponents {
            reason: "surface component threshold exceeds GPU numeric range",
        })
    }
}
