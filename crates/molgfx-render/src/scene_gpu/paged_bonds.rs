//! Persistent command arena for one indirect batch of provider-backed bonds.

use super::picking_pages::{ChunkPickPlan, PickPages};
use crate::engine::bond_draw_plan::{PagedBondGpu, ResidentBondPlacement};
use crate::{RenderError, ResidencyConfig};
use molgfx_core::{DrawIndirectArgs, EntityKind};
use molgfx_gpu::Queue as _;
use molgfx_gpu::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType,
    BufferDesc, BufferUsage, Device, ShaderStages,
};

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct BondPlacementGpu {
    model_to_world: [f32; 16],
    bond_base: u32,
    bond_count: u32,
    pick_page: u32,
    color: u32,
    radius: f32,
    _padding: [u32; 3],
}

#[derive(Debug)]
pub(super) struct PagedBondBatch<D: Device> {
    pub(super) layout: D::BindGroupLayout,
    buffers: Option<PagedBondBuffers<D>>,
    group: Option<D::BindGroup>,
    placement_scratch: Vec<BondPlacementGpu>,
    pick_scratch: Vec<ChunkPickPlan>,
    revision: u64,
    pick_revision: u64,
    coordinate_binding_revision: u64,
    bond_binding_revision: u64,
    max_rows: u32,
    active_rows: u32,
    visible_capacity: u32,
    machine_capacity: usize,
}

#[derive(Debug)]
struct PagedBondBuffers<D: Device> {
    placements: D::Buffer,
    visible: D::Buffer,
    args: D::Buffer,
}

pub(crate) struct PagedBondSceneSync<'a, D: Device> {
    pub(crate) device: &'a D,
    pub(crate) queue: &'a D::Queue,
    pub(crate) coordinates: &'a D::Buffer,
    pub(crate) bonds: &'a D::Buffer,
    pub(crate) plan: &'a [ResidentBondPlacement],
    pub(crate) revision: u64,
    pub(crate) coordinate_binding_revision: u64,
    pub(crate) bond_binding_revision: u64,
}

impl<D: Device> PagedBondBatch<D> {
    pub(super) fn new(device: &D, config: ResidencyConfig) -> Result<Self, RenderError> {
        let visible_capacity = u32::try_from(
            config
                .page_size
                .saturating_mul(u64::from(config.page_count))
                / std::mem::size_of::<PagedBondGpu>() as u64,
        )
        .map_err(|_| resident("paged bond visible capacity exceeds local u32 addressing"))?;
        let layout = layout(device);
        Ok(Self {
            buffers: None,
            layout,
            group: None,
            placement_scratch: Vec::with_capacity(config.machine_capacity),
            pick_scratch: Vec::with_capacity(config.machine_capacity),
            revision: 0,
            pick_revision: 0,
            coordinate_binding_revision: 0,
            bond_binding_revision: 0,
            max_rows: 0,
            active_rows: 0,
            visible_capacity,
            machine_capacity: config.machine_capacity,
        })
    }

    pub(super) fn sync(
        &mut self,
        input: &PagedBondSceneSync<'_, D>,
        picks: &mut PickPages,
    ) -> Result<(), RenderError> {
        if self.revision == input.revision
            && self.pick_revision == picks.table_revision()
            && self.coordinate_binding_revision == input.coordinate_binding_revision
            && self.bond_binding_revision == input.bond_binding_revision
        {
            return Ok(());
        }
        self.build_pick_plan(input.plan)?;
        picks.sync_bond_chunks(&self.pick_scratch)?;
        self.placement_scratch.clear();
        self.max_rows = 0;
        self.active_rows = 0;
        for placement in input.plan {
            self.active_rows = self
                .active_rows
                .checked_add(placement.range.span.row_count())
                .ok_or_else(|| resident("paged bond row count exceeds local u32 addressing"))?;
            if self.active_rows > self.visible_capacity {
                return Err(resident("paged bonds exceed the bounded visible-row arena"));
            }
            let page = picks
                .page_for_chunk(placement.dataset(), placement.chunk(), EntityKind::Bond)
                .ok_or(RenderError::PickingPageMissing {
                    dataset: placement.dataset(),
                    kind: EntityKind::Bond,
                })?;
            self.placement_scratch.push(lower(placement, page)?);
            self.max_rows = self.max_rows.max(placement.range.span.row_count());
        }
        if self.placement_scratch.is_empty() {
            self.group = None;
        } else {
            self.ensure_buffers(input.device)?;
            let buffers = self.buffers()?;
            input.queue.write_buffer(
                &buffers.placements,
                0,
                bytemuck::cast_slice(&self.placement_scratch),
            );
            input.queue.write_buffer(
                &buffers.args,
                0,
                bytemuck::bytes_of(&DrawIndirectArgs {
                    vertex_count: 6,
                    instance_count: 0,
                    first_vertex: 0,
                    first_instance: 0,
                }),
            );
            self.bind(input)?;
        }
        self.revision = input.revision;
        self.pick_revision = picks.table_revision();
        self.coordinate_binding_revision = input.coordinate_binding_revision;
        self.bond_binding_revision = input.bond_binding_revision;
        Ok(())
    }

    fn bind(&mut self, input: &PagedBondSceneSync<'_, D>) -> Result<(), RenderError> {
        let buffers = self.buffers()?;
        self.group = Some(input.device.create_bind_group(&BindGroupDesc {
            label: "paged provider bonds",
            layout: &self.layout,
            entries: &[
                BindGroupEntry::Buffer {
                    binding: 0,
                    buffer: input.coordinates,
                },
                BindGroupEntry::Buffer {
                    binding: 1,
                    buffer: input.bonds,
                },
                BindGroupEntry::Buffer {
                    binding: 2,
                    buffer: &buffers.placements,
                },
                BindGroupEntry::Buffer {
                    binding: 3,
                    buffer: &buffers.visible,
                },
                BindGroupEntry::Buffer {
                    binding: 4,
                    buffer: &buffers.args,
                },
                BindGroupEntry::Buffer {
                    binding: 5,
                    buffer: &buffers.visible,
                },
            ],
        }));
        Ok(())
    }

    pub(super) fn cull(&self) -> Option<(&D::BindGroup, [u32; 3])> {
        let placements = u32::try_from(self.placement_scratch.len()).ok()?;
        let x = self.max_rows.div_ceil(64);
        (placements > 0).then_some((self.group.as_ref()?, [x, placements, 1]))
    }

    pub(super) fn draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
        (self.active_rows > 0).then_some((self.group.as_ref()?, &self.buffers.as_ref()?.args))
    }

    fn ensure_buffers(&mut self, device: &D) -> Result<(), RenderError> {
        if self.buffers.is_none() {
            self.buffers = Some(PagedBondBuffers {
                placements: buffer(
                    device,
                    "paged bond placements",
                    bytes_for::<BondPlacementGpu>(self.machine_capacity)?,
                    BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
                )?,
                visible: buffer(
                    device,
                    "paged visible bonds",
                    u64::from(self.visible_capacity).saturating_mul(8).max(8),
                    BufferUsage::STORAGE,
                )?,
                args: buffer(
                    device,
                    "paged bond indirect command arena",
                    std::mem::size_of::<DrawIndirectArgs>() as u64,
                    BufferUsage::STORAGE
                        .union(BufferUsage::INDIRECT)
                        .union(BufferUsage::COPY_DST),
                )?,
            });
        }
        Ok(())
    }

    fn buffers(&self) -> Result<&PagedBondBuffers<D>, RenderError> {
        self.buffers
            .as_ref()
            .ok_or_else(|| resident("paged bond buffers were not allocated for a non-empty plan"))
    }

    fn build_pick_plan(&mut self, plan: &[ResidentBondPlacement]) -> Result<(), RenderError> {
        self.pick_scratch.clear();
        for placement in plan {
            let value = (
                placement.dataset(),
                placement.chunk(),
                placement.range.span,
                EntityKind::Bond,
            );
            if !self.pick_scratch.contains(&value) {
                if self.pick_scratch.len() == self.pick_scratch.capacity() {
                    return Err(molgfx_core::PickingError::WorkingSetFull.into());
                }
                self.pick_scratch.push(value);
            }
        }
        Ok(())
    }
}

fn lower(value: &ResidentBondPlacement, page: u32) -> Result<BondPlacementGpu, RenderError> {
    let stride = std::mem::size_of::<PagedBondGpu>() as u64;
    let bytes = value.range.allocation.byte_offset();
    if !bytes.is_multiple_of(stride) {
        return Err(resident("paged bond arena offset is misaligned"));
    }
    let bond_base = u32::try_from(bytes / stride)
        .map_err(|_| resident("paged bond arena offset exceeds local u32 addressing"))?;
    Ok(BondPlacementGpu {
        model_to_world: value.placement.model_to_world.to_cols_array(),
        bond_base,
        bond_count: value.range.span.row_count(),
        pick_page: page,
        color: u32::from_le_bytes([
            value.placement.color.r,
            value.placement.color.g,
            value.placement.color.b,
            value.placement.color.a,
        ]),
        radius: value.placement.radius,
        _padding: [0; 3],
    })
}

fn layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "paged provider bonds",
        entries: &[
            storage(0, true, ShaderStages::VERTEX.union(ShaderStages::COMPUTE)),
            storage(1, true, ShaderStages::VERTEX.union(ShaderStages::COMPUTE)),
            storage(2, true, ShaderStages::VERTEX.union(ShaderStages::COMPUTE)),
            storage(3, false, ShaderStages::COMPUTE),
            storage(4, false, ShaderStages::COMPUTE),
            storage(5, true, ShaderStages::VERTEX),
        ],
    })
}

const fn storage(binding: u32, read_only: bool, visibility: ShaderStages) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility,
        ty: BindingType::Storage { read_only },
    }
}

fn buffer<D: Device>(
    device: &D,
    label: &'static str,
    size: u64,
    usage: BufferUsage,
) -> Result<D::Buffer, RenderError> {
    Ok(device.create_buffer(&BufferDesc { label, size, usage })?)
}

fn bytes_for<T>(count: usize) -> Result<u64, RenderError> {
    let bytes = std::mem::size_of::<T>()
        .checked_mul(count.max(1))
        .ok_or_else(|| resident("paged bond buffer size overflow"))?;
    u64::try_from(bytes).map_err(|_| resident("paged bond buffer exceeds host address space"))
}

const fn resident(reason: &'static str) -> RenderError {
    RenderError::Residency { reason }
}

impl<D: Device> super::GpuScene<D> {
    pub(crate) const fn paged_bond_layout(&self) -> &D::BindGroupLayout {
        &self.paged_bonds.layout
    }

    pub(crate) fn sync_paged_bonds(
        &mut self,
        input: &PagedBondSceneSync<'_, D>,
    ) -> Result<(), RenderError> {
        self.paged_bonds.sync(input, &mut self.picking_pages)
    }

    pub(crate) fn paged_bond_cull(&self) -> Option<(&D::BindGroup, [u32; 3])> {
        self.paged_bonds.cull()
    }

    pub(crate) fn paged_bond_draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
        self.paged_bonds.draw()
    }
}
