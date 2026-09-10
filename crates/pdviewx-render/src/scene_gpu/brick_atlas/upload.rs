//! Fence-safe upload, publication and eviction over fixed storage.

use super::page_table::{GpuPageTable, same_generation};
use super::types::{
    BrickAtlasConfig, BrickAtlasError, BrickAtlasKind, BrickAtlasMetrics, BrickAtlasPoll,
    BrickAtlasUpload,
};
use pdviewx_core::{BrickAddress, BrickCatalog, BrickDescriptor, BrickId, BrickValueRange};
use pdviewx_gpu::{
    BufferDesc, BufferUsage, Device, FenceValue, Queue, TextureDesc, TextureDimension,
    TextureFormat, TextureUsage, TextureViewDesc, TextureWrite, UploadRing,
};
use pdviewx_semantic::BrickWorkingSet;

#[derive(Clone, Copy, Debug, Default)]
enum Pending {
    #[default]
    Vacant,
    Upload {
        descriptor: BrickDescriptor,
        fence: FenceValue,
    },
    Eviction {
        brick: BrickId,
        fence: FenceValue,
    },
}

/// Fixed-capacity physical atlas and generational sparse page table.
#[derive(Debug)]
pub struct GpuBrickAtlas<D: Device> {
    config: BrickAtlasConfig,
    texture: D::Texture,
    view: D::TextureView,
    page_buffer: D::Buffer,
    page_table: GpuPageTable,
    working_set: BrickWorkingSet,
    published: Vec<Option<BrickDescriptor>>,
    pending: Vec<Pending>,
    upload_ring: UploadRing,
    metrics: BrickAtlasMetrics,
}

impl<D: Device> GpuBrickAtlas<D> {
    /// Allocates the complete atlas, page table and staging storage once.
    ///
    /// # Errors
    ///
    /// Rejects zero, overflowing or device-incompatible capacities.
    pub fn new(
        device: &D,
        queue: &D::Queue,
        catalog: &BrickCatalog,
        config: BrickAtlasConfig,
    ) -> Result<Self, BrickAtlasError> {
        let [width, height, brick_depth] = config.stored_shape.map(u32::from);
        if width == 0 || height == 0 || brick_depth == 0 || config.resident_capacity == 0 {
            return Err(BrickAtlasError::InvalidConfiguration);
        }
        for descriptor in catalog.descriptors() {
            if descriptor.metadata.shape.stored() != config.stored_shape
                || !kind_matches(config.kind, descriptor.metadata.range)
            {
                return Err(BrickAtlasError::InvalidConfiguration);
            }
        }
        let Ok(capacity) = u32::try_from(config.resident_capacity) else {
            return Err(BrickAtlasError::InvalidConfiguration);
        };
        let Some(depth) = brick_depth.checked_mul(capacity) else {
            return Err(BrickAtlasError::InvalidConfiguration);
        };
        let limit = device.capabilities().max_texture_dim_3d;
        if width > limit || height > limit || depth > limit {
            return Err(pdviewx_gpu::GpuError::LimitExceeded {
                resource: "sparse brick atlas",
                limit: u64::from(limit),
            }
            .into());
        }
        let page_table = GpuPageTable::new(config.resident_capacity)?;
        if page_table.byte_len() > device.capabilities().max_storage_buffer_bytes {
            return Err(pdviewx_gpu::GpuError::LimitExceeded {
                resource: "sparse brick page table",
                limit: device.capabilities().max_storage_buffer_bytes,
            }
            .into());
        }
        let format = texture_format(config.kind);
        let texture = device.create_texture(&TextureDesc {
            label: atlas_label(config.kind),
            width,
            height,
            depth,
            dimension: TextureDimension::D3,
            format,
            usage: TextureUsage::TEXTURE_BINDING.union(TextureUsage::COPY_DST),
        })?;
        let view = device.create_texture_view(&texture, &TextureViewDesc::default());
        let page_buffer = device.create_buffer(&BufferDesc {
            label: "sparse brick generational page table",
            size: page_table.byte_len(),
            usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
        })?;
        queue.write_buffer(&page_buffer, 0, page_table.bytes());
        let atlas_bytes = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|value| value.checked_mul(u64::from(depth)))
            .and_then(|value| value.checked_mul(4))
            .ok_or(BrickAtlasError::InvalidConfiguration)?;
        let page_table_bytes = page_table.byte_len();
        Ok(Self {
            config,
            texture,
            view,
            page_buffer,
            page_table,
            working_set: BrickWorkingSet::new(config.resident_capacity)?,
            published: vec![None; config.resident_capacity],
            pending: vec![Pending::Vacant; config.resident_capacity.saturating_mul(2)],
            upload_ring: UploadRing::new(config.uploads)?,
            metrics: BrickAtlasMetrics {
                atlas_bytes,
                page_table_bytes,
                page_table_writes: 1,
                ..BrickAtlasMetrics::default()
            },
        })
    }

    /// Resets only the bounded per-frame upload allowance.
    pub fn begin_frame(&mut self) {
        self.upload_ring.begin_epoch();
    }

    /// Stages one brick and submits an ordered fence for its publication.
    ///
    /// The provider allocation is borrowed only for this call. Its bytes are
    /// copied once into the persistent upload ring and never into scene state.
    ///
    /// # Errors
    ///
    /// Returns typed shape, semantic, lifecycle or upload backpressure.
    pub fn stage(
        &mut self,
        device: &D,
        queue: &D::Queue,
        upload: BrickAtlasUpload<'_>,
    ) -> Result<FenceValue, BrickAtlasError> {
        self.validate(upload)?;
        let id = upload.descriptor.metadata.id;
        if self.eviction_pending(id) {
            return Err(BrickAtlasError::EvictionPending { brick: id });
        }
        let Some(pending_index) = self
            .pending
            .iter()
            .position(|entry| matches!(entry, Pending::Vacant))
        else {
            return Err(BrickAtlasError::LifecycleCapacity {
                capacity: self.pending.len(),
            });
        };
        let reservation = self.upload_ring.reserve(upload.bytes.len())?;
        self.upload_ring
            .bytes_mut(reservation)?
            .copy_from_slice(upload.bytes);
        self.upload_ring.commit(reservation.ticket())?;
        self.upload_ring.ensure_submittable(reservation.ticket())?;
        let slot = match self.working_set.commit(upload.descriptor) {
            Ok(slot) => slot.get(),
            Err(error) => {
                let _ = self.upload_ring.cancel(reservation.ticket());
                return Err(error.into());
            }
        };
        let offset = reservation.offset();
        let end = offset.saturating_add(reservation.len());
        let Some(staged) = self.upload_ring.staging_bytes().get(offset..end) else {
            let _ = self.upload_ring.cancel(reservation.ticket());
            return Err(BrickAtlasError::InvalidConfiguration);
        };
        let [width, height, depth] = self.config.stored_shape.map(u32::from);
        queue.write_texture(
            &self.texture,
            &TextureWrite {
                origin: [0, 0, slot.saturating_mul(depth)],
                size: [width, height, depth],
                bytes_per_row: width.saturating_mul(4),
                rows_per_image: height,
                data: staged,
            },
        );
        let fence = queue.submit_tracked(device.create_command_encoder());
        self.upload_ring.submit(reservation.ticket(), fence)?;
        self.pending[pending_index] = Pending::Upload {
            descriptor: upload.descriptor,
            fence,
        };
        self.refresh_metrics();
        Ok(fence)
    }

    /// Invalidates a mapping and releases its slot only after an ordered fence.
    ///
    /// # Errors
    ///
    /// Returns a typed absent-page, duplicate-eviction or capacity error.
    pub fn request_eviction(
        &mut self,
        device: &D,
        queue: &D::Queue,
        brick: BrickId,
    ) -> Result<FenceValue, BrickAtlasError> {
        if self.eviction_pending(brick) {
            return Err(BrickAtlasError::EvictionPending { brick });
        }
        let Some(page) = self.working_set.get(brick).copied() else {
            return Err(pdviewx_semantic::BrickWorkingSetError::NotResident { brick }.into());
        };
        let Some(pending_index) = self
            .pending
            .iter()
            .position(|entry| matches!(entry, Pending::Vacant))
        else {
            return Err(BrickAtlasError::LifecycleCapacity {
                capacity: self.pending.len(),
            });
        };
        let Ok(slot) = usize::try_from(page.slot.get()) else {
            return Err(BrickAtlasError::InvalidConfiguration);
        };
        if let Some(target) = self.published.get_mut(slot) {
            *target = None;
        }
        self.flush_page_table(queue)?;
        let fence = queue.submit_tracked(device.create_command_encoder());
        self.pending[pending_index] = Pending::Eviction { brick, fence };
        self.refresh_metrics();
        Ok(fence)
    }

    /// Publishes current generations and releases fence-safe evictions.
    ///
    /// # Errors
    ///
    /// Device loss or an inconsistent working-set transition is returned.
    pub fn poll(
        &mut self,
        device: &D,
        queue: &D::Queue,
    ) -> Result<BrickAtlasPoll, BrickAtlasError> {
        let completed = queue.completed_fence(device)?;
        let _ = self.upload_ring.retire(completed);
        let mut result = BrickAtlasPoll {
            completed_fence: completed,
            ..BrickAtlasPoll::default()
        };
        let mut table_changed = false;
        for index in 0..self.pending.len() {
            match self.pending[index] {
                Pending::Upload { descriptor, fence } if fence <= completed => {
                    let current = self.working_set.get(descriptor.metadata.id).copied();
                    let publish = current.is_some_and(|page| {
                        same_generation(
                            descriptor,
                            BrickDescriptor {
                                chunk: page.chunk,
                                metadata: page.metadata,
                            },
                        )
                    }) && !self.eviction_pending(descriptor.metadata.id);
                    if publish {
                        if let Some(page) = current
                            && let Ok(slot) = usize::try_from(page.slot.get())
                            && let Some(target) = self.published.get_mut(slot)
                        {
                            *target = Some(descriptor);
                            result.uploads_published += 1;
                            table_changed = true;
                        }
                    } else {
                        result.stale_completions += 1;
                        self.metrics.stale_completions =
                            self.metrics.stale_completions.saturating_add(1);
                    }
                    self.pending[index] = Pending::Vacant;
                }
                Pending::Eviction { brick, fence } if fence <= completed => {
                    let page = self.working_set.evict(brick)?;
                    let Ok(slot) = usize::try_from(page.slot.get()) else {
                        return Err(BrickAtlasError::InvalidConfiguration);
                    };
                    if let Some(target) = self.published.get_mut(slot) {
                        *target = None;
                    }
                    self.pending[index] = Pending::Vacant;
                    result.evictions_completed += 1;
                    table_changed = true;
                }
                Pending::Vacant | Pending::Upload { .. } | Pending::Eviction { .. } => {}
            }
        }
        if table_changed {
            self.flush_page_table(queue)?;
        }
        self.refresh_metrics();
        Ok(result)
    }

    /// Resolves an exact global logical address to an atlas-local slot.
    #[must_use]
    pub fn resolve(&self, address: BrickAddress) -> Option<u32> {
        self.published.iter().enumerate().find_map(|(slot, value)| {
            value
                .filter(|descriptor| descriptor.metadata.address == address)
                .and_then(|_| u32::try_from(slot).ok())
        })
    }

    /// Physical atlas view used by scalar, segmentation and field shaders.
    #[must_use]
    pub const fn view(&self) -> &D::TextureView {
        &self.view
    }

    /// Persistent generational hash table consumed by sparse shader lookup.
    #[must_use]
    pub const fn page_buffer(&self) -> &D::Buffer {
        &self.page_buffer
    }

    /// Hash mask supplied to sparse lookup uniforms.
    #[must_use]
    pub const fn page_mask(&self) -> u32 {
        self.page_table.mask()
    }

    /// Common physical shape, including halo voxels.
    #[must_use]
    pub const fn stored_shape(&self) -> [u16; 3] {
        self.config.stored_shape
    }

    /// Current bounded memory and lifecycle counters.
    #[must_use]
    pub const fn metrics(&self) -> BrickAtlasMetrics {
        self.metrics
    }

    fn validate(&self, upload: BrickAtlasUpload<'_>) -> Result<(), BrickAtlasError> {
        let metadata = upload.descriptor.metadata;
        if metadata.shape.stored() != self.config.stored_shape {
            return Err(BrickAtlasError::ShapeMismatch { brick: metadata.id });
        }
        let expected = usize::try_from(metadata.shape.voxel_count())
            .ok()
            .and_then(|count| count.checked_mul(4))
            .ok_or(BrickAtlasError::InvalidConfiguration)?;
        if upload.bytes.len() != expected {
            return Err(BrickAtlasError::PayloadSize {
                brick: metadata.id,
                expected,
                received: upload.bytes.len(),
            });
        }
        if !kind_matches(self.config.kind, metadata.range) {
            return Err(BrickAtlasError::KindMismatch { brick: metadata.id });
        }
        Ok(())
    }

    fn flush_page_table(&mut self, queue: &D::Queue) -> Result<(), BrickAtlasError> {
        self.page_table.rebuild(&self.published, self.config.kind)?;
        queue.write_buffer(&self.page_buffer, 0, self.page_table.bytes());
        self.metrics.page_table_writes = self.metrics.page_table_writes.saturating_add(1);
        Ok(())
    }

    fn eviction_pending(&self, brick: BrickId) -> bool {
        self.pending.iter().any(
            |entry| matches!(entry, Pending::Eviction { brick: pending, .. } if *pending == brick),
        )
    }

    fn refresh_metrics(&mut self) {
        self.metrics.resident_pages = self
            .published
            .iter()
            .filter(|value| value.is_some())
            .count();
        self.metrics.pending_uploads = self
            .pending
            .iter()
            .filter(|entry| matches!(entry, Pending::Upload { .. }))
            .count();
        self.metrics.pending_evictions = self
            .pending
            .iter()
            .filter(|entry| matches!(entry, Pending::Eviction { .. }))
            .count();
    }
}

const fn kind_matches(kind: BrickAtlasKind, range: BrickValueRange) -> bool {
    matches!(
        (kind, range),
        (
            BrickAtlasKind::Scalar | BrickAtlasKind::Surface,
            BrickValueRange::Scalar { .. }
        ) | (
            BrickAtlasKind::Segmentation,
            BrickValueRange::Segmentation { .. }
        ) | (BrickAtlasKind::Occupancy, BrickValueRange::Occupancy { .. })
    )
}

const fn texture_format(kind: BrickAtlasKind) -> TextureFormat {
    match kind {
        BrickAtlasKind::Segmentation => TextureFormat::R32Uint,
        BrickAtlasKind::Scalar | BrickAtlasKind::Occupancy | BrickAtlasKind::Surface => {
            TextureFormat::R32Float
        }
    }
}

const fn atlas_label(kind: BrickAtlasKind) -> &'static str {
    match kind {
        BrickAtlasKind::Scalar => "sparse scalar brick atlas",
        BrickAtlasKind::Segmentation => "sparse segmentation brick atlas",
        BrickAtlasKind::Occupancy => "sparse occupancy brick atlas",
        BrickAtlasKind::Surface => "sparse surface brick atlas",
    }
}
