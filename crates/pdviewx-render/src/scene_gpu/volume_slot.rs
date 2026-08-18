//! Persistent direct-volume texture and representation binding.

use super::volume_uniforms::VolumeUniforms;
use crate::error::RenderError;
use pdviewx_core::{DensityVolume, Representation, RepresentationHandle, VolumeHandle};
use pdviewx_gpu::{
    BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, Device, Queue, TextureDesc,
    TextureDimension, TextureFormat, TextureUsage, TextureViewDesc, TextureWrite,
};

#[derive(Debug)]
pub(super) struct GpuVolumeSlot<D: Device> {
    pub(super) representation: RepresentationHandle,
    uniforms: Option<D::Buffer>,
    group: Option<D::BindGroup>,
    synced: Option<(u64, VolumeHandle, u64)>,
}

#[derive(Debug)]
pub(super) struct GpuVolumeResource<D: Device> {
    pub(super) handle: VolumeHandle,
    texture: Option<D::Texture>,
    view: Option<D::TextureView>,
    minimum_texture: Option<D::Texture>,
    minimum_view: Option<D::TextureView>,
    maximum_texture: Option<D::Texture>,
    maximum_view: Option<D::TextureView>,
    synced_revision: Option<u64>,
    binding_revision: u64,
}

pub(super) struct VolumeSync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) layout: &'a D::BindGroupLayout,
    pub(super) volume: &'a DensityVolume,
    pub(super) representation: &'a Representation,
    pub(super) representation_revision: u64,
    pub(super) volume_handle: VolumeHandle,
    pub(super) volume_view: &'a D::TextureView,
    pub(super) empty_space_minimum_view: &'a D::TextureView,
    pub(super) empty_space_maximum_view: &'a D::TextureView,
    pub(super) volume_binding_revision: u64,
}

impl<D: Device> GpuVolumeResource<D> {
    pub(super) const fn new(handle: VolumeHandle) -> Self {
        Self {
            handle,
            texture: None,
            view: None,
            minimum_texture: None,
            minimum_view: None,
            maximum_texture: None,
            maximum_view: None,
            synced_revision: None,
            binding_revision: 0,
        }
    }

    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        volume: &DensityVolume,
        revision: u64,
    ) -> Result<bool, RenderError> {
        if self.synced_revision == Some(revision) {
            return Ok(false);
        }
        if volume
            .dimensions()
            .iter()
            .any(|&dimension| dimension > device.capabilities().max_texture_dim_3d)
        {
            return Err(pdviewx_gpu::GpuError::LimitExceeded {
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

    fn upload_texture(
        &mut self,
        device: &D,
        queue: &D::Queue,
        volume: &DensityVolume,
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
        volume: &DensityVolume,
    ) -> Result<(), RenderError> {
        let dimensions = volume.empty_space_dimensions();
        let minimum = volume
            .empty_space_bounds()
            .chunks_exact(2)
            .map(|bounds| bounds[0])
            .collect::<Vec<_>>();
        let maximum = volume
            .empty_space_bounds()
            .chunks_exact(2)
            .map(|bounds| bounds[1])
            .collect::<Vec<_>>();
        self.minimum_texture = Some(upload_bounds_texture(
            device,
            queue,
            "density empty-space minimum",
            dimensions,
            &minimum,
        )?);
        self.maximum_texture = Some(upload_bounds_texture(
            device,
            queue,
            "density empty-space maximum",
            dimensions,
            &maximum,
        )?);
        self.minimum_view = self
            .minimum_texture
            .as_ref()
            .map(|texture| device.create_texture_view(texture, &TextureViewDesc::default()));
        self.maximum_view = self
            .maximum_texture
            .as_ref()
            .map(|texture| device.create_texture_view(texture, &TextureViewDesc::default()));
        Ok(())
    }

    pub(super) fn binding(&self) -> Option<(&D::TextureView, u64)> {
        Some((self.view.as_ref()?, self.binding_revision))
    }

    pub(super) fn empty_space_binding(&self) -> Option<(&D::TextureView, &D::TextureView)> {
        Some((self.minimum_view.as_ref()?, self.maximum_view.as_ref()?))
    }
}

impl<D: Device> GpuVolumeSlot<D> {
    pub(super) fn new(representation: RepresentationHandle) -> Self {
        Self {
            representation,
            uniforms: None,
            group: None,
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
        if let Some(uniforms) = &self.uniforms {
            input.queue.write_buffer(
                uniforms,
                0,
                bytemuck::bytes_of(&VolumeUniforms::new(input.volume, input.representation)),
            );
        }
        self.bind(
            input.device,
            input.layout,
            input.volume_view,
            input.empty_space_minimum_view,
            input.empty_space_maximum_view,
        );
        self.synced = Some(current);
        Ok(true)
    }

    fn bind(
        &mut self,
        device: &D,
        layout: &D::BindGroupLayout,
        view: &D::TextureView,
        minimum_view: &D::TextureView,
        maximum_view: &D::TextureView,
    ) {
        let Some(uniforms) = &self.uniforms else {
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
                    view: minimum_view,
                },
                BindGroupEntry::Texture {
                    binding: 3,
                    view: maximum_view,
                },
            ],
        }));
    }

    pub(super) fn draw(&self) -> Option<&D::BindGroup> {
        self.group.as_ref()
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
            data: bytemuck::cast_slice(values),
        },
    );
    Ok(texture)
}
