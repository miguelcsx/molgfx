//! One revision-diffed GPU table for analytic primitives.

use super::buffers::{count, ensure_upload_buffer};
use super::primitive_draw::{PackedPrimitive, PrimitiveDrawGroup, regroup};
use super::primitive_packing::{pack_primitive, placement_revision};
use super::structure::GpuStructure;
use crate::error::RenderError;
use molgfx_core::{ParticleMotionGpu, PrimitiveGpu, Scene};
use molgfx_gpu::{BindGroupDesc, BindGroupEntry, Device, Queue};

/// The three bind-group layouts a primitive sync binds: the gbuffer table, the
/// particle-motion compute group, and the shadow-caster group.
pub(super) struct PrimitiveLayouts<'a, D: Device> {
    pub(super) table: &'a D::BindGroupLayout,
    pub(super) motion: &'a D::BindGroupLayout,
    pub(super) shadow: &'a D::BindGroupLayout,
}
/// Above this count, realtime shadows use SSAO/contact shadows instead of a
/// second full primitive raster. Cinematic mode retains the complete caster set.
const REALTIME_SHADOW_PRIMITIVES: u32 = 4_096;
const RETAINED_STAGING_BYTES: usize = 8 * 1024 * 1024;

#[cfg(test)]
#[path = "primitive_table_tests.rs"]
mod tests;

#[derive(Debug)]
pub(super) struct GpuPrimitives<D: Device> {
    buffer: Option<D::Buffer>,
    previous: Option<D::Buffer>,
    motion: Option<D::Buffer>,
    group: Option<D::BindGroup>,
    shadow_group: Option<D::BindGroup>,
    motion_group: Option<D::BindGroup>,
    capacity: u64,
    previous_capacity: u64,
    motion_capacity: u64,
    count: u32,
    opaque_count: u32,
    translucent: bool,
    motion_count: u32,
    synced: Option<(u64, u64, u64)>,
    scratch: Vec<PrimitiveGpu>,
    previous_scratch: Vec<[f32; 4]>,
    motion_scratch: Vec<ParticleMotionGpu>,
    rows: Vec<PackedPrimitive>,
    groups: Vec<PrimitiveDrawGroup>,
}

impl<D: Device> GpuPrimitives<D> {
    pub(super) const fn new() -> Self {
        Self {
            buffer: None,
            previous: None,
            motion: None,
            group: None,
            shadow_group: None,
            motion_group: None,
            capacity: 0,
            previous_capacity: 0,
            motion_capacity: 0,
            count: 0,
            opaque_count: 0,
            translucent: false,
            motion_count: 0,
            synced: None,
            scratch: Vec::new(),
            previous_scratch: Vec::new(),
            motion_scratch: Vec::new(),
            rows: Vec::new(),
            groups: Vec::new(),
        }
    }

    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        layouts: &PrimitiveLayouts<'_, D>,
        scene: &Scene,
        structures: &[GpuStructure<D>],
    ) -> Result<bool, RenderError> {
        let revision = (
            scene.primitive_revision(),
            scene.structure_revision(),
            placement_revision(scene),
        );
        if self.synced == Some(revision) {
            return Ok(false);
        }
        self.rows.clear();
        self.translucent = false;
        self.motion_count = 0;
        for (handle, primitive) in scene.primitives() {
            if !primitive.visible() {
                continue;
            }
            let owner = primitive.owner();
            let Some(placed) = scene.structure(owner) else {
                continue;
            };
            let Some(pick_page) = structures
                .iter()
                .find(|structure| structure.handle == owner)
                .map(|structure| structure.pick_page(molgfx_core::EntityKind::Primitive))
            else {
                continue;
            };
            let row = Scene::primitive_row(handle);
            if let Some((record, motion)) =
                pack_primitive(*primitive, row, pick_page, placed.model_to_world)?
            {
                self.translucent |= record.color[3] < 0.999;
                self.motion_count += motion.metadata[0];
                self.rows.push(PackedPrimitive::new(record, motion));
            }
        }
        regroup(
            &mut self.rows,
            &mut self.scratch,
            &mut self.previous_scratch,
            &mut self.motion_scratch,
            &mut self.groups,
            self.motion_count > 0,
        );
        let packed_count = count(self.scratch.len());
        self.opaque_count = self
            .groups
            .iter()
            .filter(|group| !group.translucent)
            .map(|group| group.len)
            .sum();
        let auxiliary = self.motion_count > 0;
        let rebind = self.prepare_buffers(device, packed_count, auxiliary)?;
        let (Some(buffer), Some(previous), Some(motion)) =
            (&self.buffer, &self.previous, &self.motion)
        else {
            return Ok(false);
        };
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(&self.scratch));
        if auxiliary {
            queue.write_buffer(previous, 0, bytemuck::cast_slice(&self.previous_scratch));
            queue.write_buffer(motion, 0, bytemuck::cast_slice(&self.motion_scratch));
        } else {
            queue.write_buffer(previous, 0, bytemuck::cast_slice(&[[0.0_f32; 4]]));
            queue.write_buffer(
                motion,
                0,
                bytemuck::cast_slice(&[ParticleMotionGpu::default()]),
            );
        }
        if rebind || self.group.is_none() || self.motion_group.is_none() {
            self.bind(device, layouts);
        }
        self.count = packed_count;
        self.synced = Some(revision);
        release_staging(&mut self.rows);
        release_staging(&mut self.scratch);
        release_staging(&mut self.previous_scratch);
        release_staging(&mut self.motion_scratch);
        Ok(true)
    }

    fn prepare_buffers(
        &mut self,
        device: &D,
        upper_count: u32,
        auxiliary: bool,
    ) -> Result<bool, RenderError> {
        let needed = required_bytes::<PrimitiveGpu>(upper_count);
        let previous_needed = if auxiliary {
            required_bytes::<[f32; 4]>(upper_count)
        } else {
            std::mem::size_of::<[f32; 4]>() as u64
        };
        let motion_needed = if auxiliary {
            required_bytes::<ParticleMotionGpu>(upper_count)
        } else {
            std::mem::size_of::<ParticleMotionGpu>() as u64
        };
        if !auxiliary {
            release_large_buffer(&mut self.previous, &mut self.previous_capacity);
            release_large_buffer(&mut self.motion, &mut self.motion_capacity);
        }
        let mut rebind = ensure_upload_buffer(
            device,
            "primitive records",
            needed,
            &mut self.buffer,
            &mut self.capacity,
        )?;
        rebind |= ensure_upload_buffer(
            device,
            "primitive previous centers",
            previous_needed,
            &mut self.previous,
            &mut self.previous_capacity,
        )?;
        rebind |= ensure_upload_buffer(
            device,
            "primitive particle motion table",
            motion_needed,
            &mut self.motion,
            &mut self.motion_capacity,
        )?;
        Ok(rebind)
    }

    fn bind(&mut self, device: &D, layouts: &PrimitiveLayouts<'_, D>) {
        let (Some(buffer), Some(previous), Some(motion)) =
            (&self.buffer, &self.previous, &self.motion)
        else {
            return;
        };
        // The shadow caster reads the same records at binding 6, sharing one
        // primitive buffer between the gbuffer and shadow draws.
        self.shadow_group = Some(device.create_bind_group(&BindGroupDesc {
            label: "group2: primitive shadow casters",
            layout: layouts.shadow,
            entries: &[BindGroupEntry::Buffer { binding: 6, buffer }],
        }));
        self.group = Some(device.create_bind_group(&BindGroupDesc {
            label: "group2: primitive table",
            layout: layouts.table,
            entries: &[
                BindGroupEntry::Buffer { binding: 0, buffer },
                BindGroupEntry::Buffer {
                    binding: 1,
                    buffer: previous,
                },
                BindGroupEntry::Buffer {
                    binding: 2,
                    buffer: motion,
                },
            ],
        }));
        self.motion_group = Some(device.create_bind_group(&BindGroupDesc {
            label: "group2: primitive particle motion",
            layout: layouts.motion,
            entries: &[
                BindGroupEntry::Buffer { binding: 0, buffer },
                BindGroupEntry::Buffer {
                    binding: 1,
                    buffer: previous,
                },
                BindGroupEntry::Buffer {
                    binding: 2,
                    buffer: motion,
                },
            ],
        }));
    }

    /// The shadow table and exact opaque class ranges.
    pub(super) fn shadow_draw(
        &self,
        quality: bool,
    ) -> Option<(&D::BindGroup, &[PrimitiveDrawGroup])> {
        shadows_enabled(self.opaque_count, quality)
            .then_some((self.shadow_group.as_ref()?, self.groups.as_slice()))
    }

    /// The shape-sorted table with its per-class draw ranges, for the
    /// specialized gbuffer and transparency passes.
    pub(super) fn groups(&self) -> Option<(&D::BindGroup, &[PrimitiveDrawGroup])> {
        (self.count > 0).then_some((self.group.as_ref()?, self.groups.as_slice()))
    }

    pub(super) fn particle_motion(&self) -> Option<(&D::BindGroup, u32)> {
        (self.motion_count > 0).then_some((self.motion_group.as_ref()?, self.count))
    }

    pub(super) const fn has_translucency(&self) -> bool {
        self.translucent
    }
}

const fn shadows_enabled(opaque_count: u32, quality: bool) -> bool {
    opaque_count > 0 && (quality || opaque_count <= REALTIME_SHADOW_PRIMITIVES)
}

fn release_staging<T>(values: &mut Vec<T>) {
    if values.capacity().saturating_mul(std::mem::size_of::<T>()) > RETAINED_STAGING_BYTES {
        *values = Vec::new();
    } else {
        values.clear();
    }
}

fn release_large_buffer<B>(buffer: &mut Option<B>, capacity: &mut u64) {
    if *capacity > 256 {
        *buffer = None;
        *capacity = 0;
    }
}

const fn required_bytes<T>(count: u32) -> u64 {
    (count as u64).saturating_mul(std::mem::size_of::<T>() as u64)
}
