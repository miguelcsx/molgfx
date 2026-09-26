//! Persistent direct-volume texture and representation binding.

use super::asset_arena::AssetArena;
use super::occupancy_slot::GpuOccupancy;
use super::structure::GpuStructure;
use super::volume_uniforms::VolumeUniforms;
use crate::error::RenderError;
use crate::passes::OccupancyPass;
use molgfx_core::{
    OccupancyStream, PlacedStructure, Representation, RepresentationHandle, ScalarVolume,
    VolumeHandle, VolumeRendering,
};
use molgfx_gpu::{
    BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, ComputePassEncoder, Device, Queue,
    TextureDesc, TextureDimension, TextureFormat, TextureUsage, TextureViewDesc, TextureWrite,
};

#[derive(Debug)]
pub(super) struct GpuVolumeSlot<D: Device> {
    pub(super) representation: RepresentationHandle,
    uniforms: Option<D::Buffer>,
    sparse_pages: Option<D::Buffer>,
    sparse_config: Option<D::Buffer>,
    group: Option<D::BindGroup>,
    rendering: VolumeRendering,
    synced: Option<(u64, VolumeHandle, u64)>,
}

#[derive(Debug)]
pub(super) struct GpuVolumeResource<D: Device> {
    pub(super) handle: VolumeHandle,
    texture: Option<D::Texture>,
    view: Option<D::TextureView>,
    bounds_texture: Option<D::Texture>,
    bounds_view: Option<D::TextureView>,
    occupancy: Option<GpuOccupancy<D>>,
    synced_revision: Option<u64>,
    binding_revision: u64,
}

pub(super) struct VolumeSync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) layout: &'a D::BindGroupLayout,
    pub(super) uniforms: VolumeUniforms,
    pub(super) representation: &'a Representation,
    pub(super) representation_revision: u64,
    pub(super) volume_handle: VolumeHandle,
    pub(super) volume_view: &'a D::TextureView,
    pub(super) empty_space_bounds_view: &'a D::TextureView,
    pub(super) volume_binding_revision: u64,
}

pub(super) struct OccupancySync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) layout: &'a D::BindGroupLayout,
    pub(super) bounds_format: TextureFormat,
    pub(super) stream: &'a OccupancyStream,
    pub(super) selected_rows: &'a [u32],
    pub(super) placed: &'a PlacedStructure,
    pub(super) structure: &'a GpuStructure<D>,
    pub(super) asset_arena: &'a AssetArena<D>,
    pub(super) revision: u64,
}

impl<D: Device> GpuVolumeResource<D> {
    pub(super) const fn new(handle: VolumeHandle) -> Self {
        Self {
            handle,
            texture: None,
            view: None,
            bounds_texture: None,
            bounds_view: None,
            occupancy: None,
            synced_revision: None,
            binding_revision: 0,
        }
    }

    pub(super) fn sync_static(
        &mut self,
        device: &D,
        queue: &D::Queue,
        volume: &ScalarVolume,
        revision: u64,
    ) -> Result<bool, RenderError> {
        if self.synced_revision == Some(revision) {
            return Ok(false);
        }
        self.occupancy = None;
        if volume
            .dimensions()
            .iter()
            .any(|&dimension| dimension > device.capabilities().max_texture_dim_3d)
        {
            return Err(molgfx_gpu::GpuError::LimitExceeded {
                resource: "3-D density texture",
                limit: u64::from(device.capabilities().max_texture_dim_3d),
            }
            .into());
        }
        self.upload_texture(device, queue, volume)?;
        self.synced_revision = Some(revision);
        self.binding_revision = self.binding_revision.wrapping_add(1);
        Ok(true)
    }

    pub(super) fn sync_occupancy(
        &mut self,
        input: &OccupancySync<'_, D>,
    ) -> Result<bool, RenderError> {
        if self.synced_revision == Some(input.revision)
            && let Some(occupancy) = &mut self.occupancy
        {
            occupancy.sync(
                input.device,
                input.layout,
                input.placed,
                input.structure,
                input.asset_arena,
            );
            return Ok(false);
        }
        self.texture = None;
        self.view = None;
        self.bounds_texture = None;
        self.bounds_view = None;
        self.synced_revision = None;
        self.occupancy = Some(GpuOccupancy::new(input)?);
        self.synced_revision = Some(input.revision);
        self.binding_revision = self.binding_revision.wrapping_add(1);
        Ok(true)
    }

    fn upload_texture(
        &mut self,
        device: &D,
        queue: &D::Queue,
        volume: &ScalarVolume,
    ) -> Result<(), RenderError> {
        let dimensions = volume.dimensions();
        let texture = device.create_texture(&TextureDesc {
            label: "caller density volume",
            width: dimensions[0],
            height: dimensions[1],
            depth: dimensions[2],
            dimension: TextureDimension::D3,
            format: TextureFormat::R32Float,
            usage: TextureUsage::TEXTURE_BINDING.union(TextureUsage::COPY_DST),
        })?;
        queue.write_texture(
            &texture,
            &TextureWrite {
                origin: [0; 3],
                size: dimensions,
                bytes_per_row: dimensions[0].saturating_mul(4),
                rows_per_image: dimensions[1],
                data: bytemuck::cast_slice(volume.values()),
            },
        );
        self.view = Some(device.create_texture_view(&texture, &TextureViewDesc::default()));
        self.texture = Some(texture);
        self.upload_empty_space_bounds(device, queue, volume)?;
        Ok(())
    }

    fn upload_empty_space_bounds(
        &mut self,
        device: &D,
        queue: &D::Queue,
        volume: &ScalarVolume,
    ) -> Result<(), RenderError> {
        let dimensions = volume.empty_space_dimensions();
        self.bounds_texture = Some(upload_bounds_texture(
            device,
            queue,
            "density empty-space bounds",
            dimensions,
            volume.empty_space_bounds(),
        )?);
        self.bounds_view = self
            .bounds_texture
            .as_ref()
            .map(|texture| device.create_texture_view(texture, &TextureViewDesc::default()));
        Ok(())
    }

    pub(super) fn binding(&self) -> Option<(&D::TextureView, u64)> {
        let view = self
            .occupancy
            .as_ref()
            .map_or_else(|| self.view.as_ref(), |occupancy| Some(occupancy.view()))?;
        Some((view, self.binding_revision))
    }

    pub(super) fn empty_space_binding(&self) -> Option<&D::TextureView> {
        self.occupancy.as_ref().map_or_else(
            || self.bounds_view.as_ref(),
            |occupancy| Some(occupancy.bounds_view()),
        )
    }

    pub(super) fn record_occupancy<P: ComputePassEncoder<D>>(
        &mut self,
        pass: &mut P,
        pipelines: &OccupancyPass<D>,
    ) {
        let Some(occupancy) = &mut self.occupancy else {
            return;
        };
        occupancy.record(pass, pipelines);
    }

    pub(super) fn occupancy_dirty(&self) -> bool {
        self.occupancy.as_ref().is_some_and(GpuOccupancy::is_dirty)
    }
}

impl<D: Device> GpuVolumeSlot<D> {
    pub(super) fn new(representation: RepresentationHandle) -> Self {
        Self {
            representation,
            uniforms: None,
            sparse_pages: None,
            sparse_config: None,
            group: None,
            rendering: VolumeRendering::Direct,
            synced: None,
        }
    }

    pub(super) fn sync(&mut self, input: &VolumeSync<'_, D>) -> Result<bool, RenderError> {
        let current = (
            input.representation_revision,
            input.volume_handle,
            input.volume_binding_revision,
        );
        if self.synced == Some(current) {
            return Ok(false);
        }
        if self.uniforms.is_none() {
            self.uniforms = Some(input.device.create_buffer(&BufferDesc {
                label: "volume representation uniforms",
                size: std::mem::size_of::<VolumeUniforms>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?);
        }
        if self.sparse_pages.is_none() {
            self.sparse_pages = Some(input.device.create_buffer(&BufferDesc {
                label: "empty sparse volume pages",
                size: 64,
                usage: BufferUsage::STORAGE,
            })?);
        }
        if self.sparse_config.is_none() {
            self.sparse_config = Some(input.device.create_buffer(&BufferDesc {
                label: "empty sparse volume configuration",
                size: 48,
                usage: BufferUsage::UNIFORM,
            })?);
        }
        if let Some(uniforms) = &self.uniforms {
            input
                .queue
                .write_buffer(uniforms, 0, bytemuck::bytes_of(&input.uniforms));
        }
        self.bind(
            input.device,
            input.layout,
            input.volume_view,
            input.empty_space_bounds_view,
        );
        self.rendering = input.representation.volume.rendering;
        self.synced = Some(current);
        Ok(true)
    }

    fn bind(
        &mut self,
        device: &D,
        layout: &D::BindGroupLayout,
        view: &D::TextureView,
        bounds_view: &D::TextureView,
    ) {
        let (Some(uniforms), Some(sparse_pages), Some(sparse_config)) =
            (&self.uniforms, &self.sparse_pages, &self.sparse_config)
        else {
            return;
        };
        self.group = Some(device.create_bind_group(&BindGroupDesc {
            label: "density volume representation",
            layout,
            entries: &[
                BindGroupEntry::Texture { binding: 0, view },
                BindGroupEntry::Buffer {
                    binding: 1,
                    buffer: uniforms,
                },
                BindGroupEntry::Texture {
                    binding: 2,
                    view: bounds_view,
                },
                BindGroupEntry::Buffer {
                    binding: 3,
                    buffer: sparse_pages,
                },
                BindGroupEntry::Buffer {
                    binding: 4,
                    buffer: sparse_config,
                },
            ],
        }));
    }

    pub(super) fn draw(&self) -> Option<(VolumeRendering, &D::BindGroup)> {
        Some((self.rendering, self.group.as_ref()?))
    }
}

fn upload_bounds_texture<D: Device>(
    device: &D,
    queue: &D::Queue,
    label: &'static str,
    dimensions: [u32; 3],
    values: &[f32],
) -> Result<D::Texture, RenderError> {
    let texture = device.create_texture(&TextureDesc {
        label,
        width: dimensions[0],
        height: dimensions[1],
        depth: dimensions[2],
        dimension: TextureDimension::D3,
        format: TextureFormat::Rg32Float,
        usage: TextureUsage::TEXTURE_BINDING.union(TextureUsage::COPY_DST),
    })?;
    queue.write_texture(
        &texture,
        &TextureWrite {
            origin: [0; 3],
            size: dimensions,
            bytes_per_row: dimensions[0].saturating_mul(8),
            rows_per_image: dimensions[1],
            data: bytemuck::cast_slice(values),
        },
    );
    Ok(texture)
}
