//! Persistent categorical-volume textures, lookup tables and bindings.

use super::segmentation_lookup::{LookupMode, SegmentLookup};
use super::segmentation_uniforms::SegmentationUniforms;
use crate::error::RenderError;
use pdviewx_core::{Representation, RepresentationHandle, SegmentationHandle, SegmentedVolume};
use pdviewx_gpu::{
    BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, Device, Queue, TextureDesc,
    TextureDimension, TextureFormat, TextureUsage, TextureViewDesc, TextureWrite,
};

#[derive(Debug)]
pub(super) struct GpuSegmentationSlot<D: Device> {
    pub(super) representation: RepresentationHandle,
    uniforms: Option<D::Buffer>,
    lookup: Option<D::Buffer>,
    sparse_pages: Option<D::Buffer>,
    sparse_config: Option<D::Buffer>,
    lookup_size: u64,
    group: Option<D::BindGroup>,
    pipeline: SegmentationPipelineKey,
    synced: Option<(u64, SegmentationHandle, u64, u32)>,
    has_styles: bool,
}

/// Pipeline specialization selected once when categorical state changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SegmentationPipelineKey {
    /// Compact style lookup over the complete volume.
    #[default]
    Direct,
    /// Compact style lookup sampled on one plane.
    DirectSlice,
    /// Sparse hashed style lookup over the complete volume.
    Hash,
    /// Sparse hashed style lookup sampled on one plane.
    HashSlice,
}

impl SegmentationPipelineKey {
    const fn new(slice: bool, lookup: LookupMode) -> Self {
        match (slice, lookup) {
            (false, LookupMode::Direct) => Self::Direct,
            (true, LookupMode::Direct) => Self::DirectSlice,
            (false, LookupMode::Hash) => Self::Hash,
            (true, LookupMode::Hash) => Self::HashSlice,
        }
    }

    pub(crate) const fn is_slice(self) -> bool {
        matches!(self, Self::DirectSlice | Self::HashSlice)
    }

    pub(crate) const fn is_hash(self) -> bool {
        matches!(self, Self::Hash | Self::HashSlice)
    }
}

#[derive(Debug)]
pub(super) struct GpuSegmentationResource<D: Device> {
    pub(super) handle: SegmentationHandle,
    texture: Option<D::Texture>,
    view: Option<D::TextureView>,
    synced_revision: Option<u64>,
    binding_revision: u64,
    source_id: u32,
}

pub(super) struct SegmentationSync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) layout: &'a D::BindGroupLayout,
    pub(super) volume: &'a SegmentedVolume,
    pub(super) representation: &'a Representation,
    pub(super) representation_revision: u64,
    pub(super) segmentation_handle: SegmentationHandle,
    pub(super) volume_view: &'a D::TextureView,
    pub(super) volume_binding_revision: u64,
    pub(super) source_id: u32,
}

impl<D: Device> GpuSegmentationResource<D> {
    pub(super) const fn new(handle: SegmentationHandle) -> Self {
        Self {
            handle,
            texture: None,
            view: None,
            synced_revision: None,
            binding_revision: 0,
            source_id: u32::MAX,
        }
    }

    pub(super) const fn set_source_id(&mut self, source_id: u32) {
        self.source_id = source_id;
    }

    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        volume: &SegmentedVolume,
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
                resource: "3-D categorical segmentation texture",
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
        volume: &SegmentedVolume,
    ) -> Result<(), RenderError> {
        let dimensions = volume.dimensions();
        let texture = device.create_texture(&TextureDesc {
            label: "caller categorical segmentation",
            width: dimensions[0],
            height: dimensions[1],
            depth: dimensions[2],
            dimension: TextureDimension::D3,
            format: TextureFormat::R32Uint,
            usage: TextureUsage::TEXTURE_BINDING.union(TextureUsage::COPY_DST),
        })?;
        queue.write_texture(
            &texture,
            &TextureWrite {
                origin: [0; 3],
                size: dimensions,
                bytes_per_row: dimensions[0].saturating_mul(4),
                rows_per_image: dimensions[1],
                data: bytemuck::cast_slice(volume.labels()),
            },
        );
        self.view = Some(device.create_texture_view(&texture, &TextureViewDesc::default()));
        self.texture = Some(texture);
        Ok(())
    }

    pub(super) fn binding(&self) -> Option<(&D::TextureView, u64)> {
        Some((self.view.as_ref()?, self.binding_revision))
    }

    pub(super) const fn source_id(&self) -> u32 {
        self.source_id
    }
}

impl<D: Device> GpuSegmentationSlot<D> {
    pub(super) fn new(representation: RepresentationHandle) -> Self {
        Self {
            representation,
            uniforms: None,
            lookup: None,
            sparse_pages: None,
            sparse_config: None,
            lookup_size: 0,
            group: None,
            pipeline: SegmentationPipelineKey::Direct,
            synced: None,
            has_styles: false,
        }
    }

    pub(super) fn sync(&mut self, input: &SegmentationSync<'_, D>) -> Result<bool, RenderError> {
        let current = (
            input.representation_revision,
            input.segmentation_handle,
            input.volume_binding_revision,
            input.source_id,
        );
        if self.synced == Some(current) {
            return Ok(false);
        }
        if self.uniforms.is_none() {
            self.uniforms = Some(input.device.create_buffer(&BufferDesc {
                label: "categorical segmentation uniforms",
                size: std::mem::size_of::<SegmentationUniforms>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?);
        }
        if self.sparse_pages.is_none() {
            self.sparse_pages = Some(input.device.create_buffer(&BufferDesc {
                label: "empty sparse segmentation pages",
                size: 64,
                usage: BufferUsage::STORAGE,
            })?);
        }
        if self.sparse_config.is_none() {
            self.sparse_config = Some(input.device.create_buffer(&BufferDesc {
                label: "empty sparse segmentation configuration",
                size: 48,
                usage: BufferUsage::UNIFORM,
            })?);
        }
        let lookup = SegmentLookup::new(input.representation.segmentation.styles.styles());
        let pipeline = SegmentationPipelineKey::new(
            input.representation.segmentation.slice.is_some(),
            lookup.mode(),
        );
        let bytes = bytemuck::cast_slice(lookup.entries());
        let size = match u64::try_from(bytes.len()) {
            Ok(size) => size.max(4),
            Err(_) => u64::MAX,
        };
        if self.lookup.is_none() || self.lookup_size != size {
            self.lookup = Some(input.device.create_buffer(&BufferDesc {
                label: "categorical segment style lookup",
                size,
                usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
            })?);
            self.lookup_size = size;
        }
        if let Some(uniforms) = &self.uniforms {
            input.queue.write_buffer(
                uniforms,
                0,
                bytemuck::bytes_of(&SegmentationUniforms::new(
                    input.volume,
                    input.representation,
                    input.source_id,
                    &lookup,
                )),
            );
        }
        if let Some(lookup_buffer) = &self.lookup {
            input.queue.write_buffer(lookup_buffer, 0, bytes);
        }
        self.bind(input.device, input.layout, input.volume_view);
        self.pipeline = pipeline;
        self.has_styles = !input.representation.segmentation.styles.styles().is_empty();
        self.synced = Some(current);
        Ok(true)
    }

    fn bind(&mut self, device: &D, layout: &D::BindGroupLayout, view: &D::TextureView) {
        let (Some(uniforms), Some(lookup), Some(sparse_pages), Some(sparse_config)) = (
            &self.uniforms,
            &self.lookup,
            &self.sparse_pages,
            &self.sparse_config,
        ) else {
            return;
        };
        self.group = Some(device.create_bind_group(&BindGroupDesc {
            label: "categorical segmentation representation",
            layout,
            entries: &[
                BindGroupEntry::Texture { binding: 0, view },
                BindGroupEntry::Buffer {
                    binding: 1,
                    buffer: uniforms,
                },
                BindGroupEntry::Buffer {
                    binding: 2,
                    buffer: lookup,
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

    pub(super) fn draw(&self) -> Option<(SegmentationPipelineKey, &D::BindGroup)> {
        if self.has_styles {
            Some((self.pipeline, self.group.as_ref()?))
        } else {
            None
        }
    }

    pub(super) const fn source_is_drawable(&self) -> bool {
        self.has_styles
    }
}
