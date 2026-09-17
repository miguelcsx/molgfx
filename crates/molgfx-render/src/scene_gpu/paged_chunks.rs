//! Bounded GPU command arena for resident provider-backed atom chunks.

#[path = "paged_chunks/lowering.rs"]
mod lowering;
use lowering::{bytes_for, lower, lower_window, representation_command};

use super::picking_pages::{ChunkPickPlan, PickPages};
use crate::ResidencyConfig;
use crate::engine::chunk_draw_plan::ResidentChunkPlacement;
use crate::engine::chunk_draw_plan::ResidentTrajectoryWindow;
use crate::error::RenderError;
use molgfx_core::{DrawIndirectArgs, ResidencyTicket};
use molgfx_gpu::Queue as _;
use molgfx_gpu::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType,
    BufferDesc, BufferUsage, Device, ShaderStages,
};

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct PlacementGpu {
    model_to_world: [f32; 16],
    coordinate_base: u32,
    radius_base: u32,
    cluster_base: u32,
    cluster_count: u32,
    local_rows: u32,
    pick_page: u32,
    color: u32,
    representation: u32,
    size: f32,
    cluster_padding: f32,
    dynamic_coordinates: u32,
    _padding: u32,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TrajectoryWindowGpu {
    output_base: u32,
    start_base: u32,
    end_base: u32,
    count: u32,
    interpolation: f32,
    _padding: [u32; 3],
}

const POINT_COMMAND: usize = 0;
const POINT_REPRESENTATION: u32 = 0;
const SPACEFILL_COMMAND: usize = 1;
const SPACEFILL_REPRESENTATION: u32 = 1;
const COMMAND_COUNT: usize = 2;

/// Persistent buffers and bindings reused by stable paged frames.
#[derive(Debug)]
pub(super) struct PagedChunkBatch<D: Device> {
    pub(super) layout: D::BindGroupLayout,
    buffers: Option<PagedChunkBuffers<D>>,
    group: Option<D::BindGroup>,
    placement_scratch: Vec<PlacementGpu>,
    pick_scratch: Vec<ChunkPickPlan>,
    window_scratch: Vec<TrajectoryWindowGpu>,
    dynamic_ticket_scratch: Vec<ResidencyTicket>,
    revision: u64,
    pick_revision: u64,
    binding_revision: u64,
    max_clusters: u32,
    visible_capacity: u32,
    machine_capacity: usize,
    active_rows: [u32; COMMAND_COUNT],
    max_window_rows: u32,
}

#[derive(Debug)]
struct PagedChunkBuffers<D: Device> {
    placements: D::Buffer,
    visible: D::Buffer,
    args: D::Buffer,
    windows: D::Buffer,
}

pub(crate) struct PagedChunkSceneSync<'a, D: Device> {
    pub(crate) device: &'a D,
    pub(crate) queue: &'a D::Queue,
    pub(crate) display_coordinates: &'a D::Buffer,
    pub(crate) frame_coordinates: &'a D::Buffer,
    pub(crate) clusters: &'a D::Buffer,
    pub(crate) plan: &'a [ResidentChunkPlacement],
    pub(crate) trajectory: &'a [ResidentTrajectoryWindow],
    pub(crate) revision: u64,
    pub(crate) binding_revision: u64,
}

struct PagedChunkSync<'a, D: Device> {
    scene: PagedChunkSceneSync<'a, D>,
    picks: &'a mut PickPages,
}

impl<D: Device> PagedChunkBatch<D> {
    pub(super) fn new(device: &D, config: ResidencyConfig) -> Result<Self, RenderError> {
        let visible_capacity = u32::try_from(
            (config
                .page_size
                .saturating_mul(u64::from(config.page_count)))
                / 16,
        )
        .map_err(|_| RenderError::Residency {
            reason: "paged visible row capacity exceeds local u32 addressing",
        })?;
        let layout = layout(device);
        Ok(Self {
            layout,
            buffers: None,
            group: None,
            placement_scratch: Vec::with_capacity(config.machine_capacity),
            pick_scratch: Vec::with_capacity(config.machine_capacity),
            window_scratch: Vec::with_capacity(config.machine_capacity),
            dynamic_ticket_scratch: Vec::with_capacity(config.machine_capacity),
            revision: 0,
            pick_revision: 0,
            binding_revision: 0,
            max_clusters: 0,
            visible_capacity,
            machine_capacity: config.machine_capacity,
            active_rows: [0; COMMAND_COUNT],
            max_window_rows: 0,
        })
    }

    fn sync(&mut self, input: &mut PagedChunkSync<'_, D>) -> Result<(), RenderError> {
        if self.revision == input.scene.revision
            && self.pick_revision == input.picks.table_revision()
            && self.binding_revision == input.scene.binding_revision
        {
            return Ok(());
        }
        self.sync_placements(input)?;
        self.sync_windows(&input.scene)?;
        if input.scene.plan.is_empty() && input.scene.trajectory.is_empty() {
            self.group = None;
        } else {
            let allocated = self.ensure_buffers(input.scene.device)?;
            self.write_placements(input.scene.queue)?;
            self.write_windows(input.scene.queue)?;
            self.sync_commands(input.scene.queue)?;
            if allocated
                || self.group.is_none()
                || self.binding_revision != input.scene.binding_revision
            {
                self.bind(&input.scene)?;
            }
        }
        self.revision = input.scene.revision;
        self.pick_revision = input.picks.table_revision();
        self.binding_revision = input.scene.binding_revision;
        Ok(())
    }

    fn sync_placements(&mut self, input: &mut PagedChunkSync<'_, D>) -> Result<(), RenderError> {
        self.build_pick_plan(input.scene.plan)?;
        input.picks.sync_draw_chunks(&self.pick_scratch)?;
        self.placement_scratch.clear();
        self.dynamic_ticket_scratch.clear();
        self.dynamic_ticket_scratch.extend(
            input
                .scene
                .trajectory
                .iter()
                .map(|window| window.structure.ticket),
        );
        self.dynamic_ticket_scratch.sort_unstable();
        self.max_clusters = 0;
        self.active_rows = [0; COMMAND_COUNT];
        for placement in input.scene.plan {
            let command = representation_command(placement.representation());
            self.active_rows[command] = self.active_rows[command]
                .checked_add(placement.span().row_count())
                .ok_or(RenderError::Residency {
                    reason: "paged representation row count exceeds local u32 addressing",
                })?;
            let visible_rows = self.active_rows.iter().try_fold(0_u32, |sum, count| {
                sum.checked_add(*count).ok_or(RenderError::Residency {
                    reason: "paged visible row count exceeds local u32 addressing",
                })
            })?;
            if visible_rows > self.visible_capacity {
                return Err(RenderError::Residency {
                    reason: "paged placements exceed the bounded visible-row arena",
                });
            }
            let page = input
                .picks
                .page_for_chunk(
                    placement.dataset(),
                    placement.chunk(),
                    placement.entity_kind(),
                )
                .ok_or(RenderError::PickingPageMissing {
                    dataset: placement.dataset(),
                    kind: placement.entity_kind(),
                })?;
            let dynamic = self
                .dynamic_ticket_scratch
                .binary_search(&placement.range.ticket)
                .is_ok();
            self.placement_scratch
                .push(lower(placement, page, dynamic)?);
            self.max_clusters = self.max_clusters.max(placement.range.cluster_count);
        }
        Ok(())
    }

    fn sync_windows(&mut self, scene: &PagedChunkSceneSync<'_, D>) -> Result<(), RenderError> {
        self.window_scratch.clear();
        self.max_window_rows = 0;
        for window in scene.trajectory {
            self.window_scratch.push(lower_window(window)?);
            self.max_window_rows = self.max_window_rows.max(window.start.local_rows);
        }
        Ok(())
    }

    fn write_placements(&self, queue: &D::Queue) -> Result<(), RenderError> {
        if !self.placement_scratch.is_empty() {
            queue.write_buffer(
                &self.buffers()?.placements,
                0,
                bytemuck::cast_slice(&self.placement_scratch),
            );
        }
        Ok(())
    }

    fn write_windows(&self, queue: &D::Queue) -> Result<(), RenderError> {
        if !self.window_scratch.is_empty() {
            queue.write_buffer(
                &self.buffers()?.windows,
                0,
                bytemuck::cast_slice(&self.window_scratch),
            );
        }
        Ok(())
    }

    fn sync_commands(&self, queue: &D::Queue) -> Result<(), RenderError> {
        let commands = [
            DrawIndirectArgs {
                vertex_count: 6,
                instance_count: 0,
                first_vertex: 0,
                first_instance: 0,
            },
            DrawIndirectArgs {
                vertex_count: 6,
                instance_count: 0,
                first_vertex: 0,
                first_instance: self.active_rows[POINT_COMMAND],
            },
        ];
        queue.write_buffer(&self.buffers()?.args, 0, bytemuck::cast_slice(&commands));
        Ok(())
    }

    fn bind(&mut self, scene: &PagedChunkSceneSync<'_, D>) -> Result<(), RenderError> {
        let buffers = self.buffers()?;
        self.group = Some(scene.device.create_bind_group(&BindGroupDesc {
            label: "paged structure chunks",
            layout: &self.layout,
            entries: &[
                BindGroupEntry::Buffer {
                    binding: 0,
                    buffer: scene.display_coordinates,
                },
                BindGroupEntry::Buffer {
                    binding: 1,
                    buffer: scene.clusters,
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
                BindGroupEntry::Buffer {
                    binding: 6,
                    buffer: scene.frame_coordinates,
                },
                BindGroupEntry::Buffer {
                    binding: 7,
                    buffer: &buffers.windows,
                },
                BindGroupEntry::Buffer {
                    binding: 8,
                    buffer: scene.display_coordinates,
                },
            ],
        }));
        Ok(())
    }

    pub(super) fn cull(&self) -> Option<(&D::BindGroup, [u32; 3])> {
        let group = self.group.as_ref()?;
        let placements = u32::try_from(self.placement_scratch.len()).ok()?;
        (placements > 0).then_some((group, [self.max_clusters, placements, 1]))
    }

    pub(super) fn interpolate(&self) -> Option<(&D::BindGroup, [u32; 3])> {
        let windows = u32::try_from(self.window_scratch.len()).ok()?;
        let groups = self.max_window_rows.div_ceil(64);
        (windows > 0).then_some((self.group.as_ref()?, [groups, windows, 1]))
    }

    fn draw(&self, command: usize) -> Option<(&D::BindGroup, &D::Buffer, u64)> {
        if self.active_rows[command] == 0 {
            return None;
        }
        let offset =
            u64::try_from(command.checked_mul(std::mem::size_of::<DrawIndirectArgs>())?).ok()?;
        Some((self.group.as_ref()?, &self.buffers.as_ref()?.args, offset))
    }

    fn ensure_buffers(&mut self, device: &D) -> Result<bool, RenderError> {
        if self.buffers.is_some() {
            return Ok(false);
        }
        self.buffers = Some(PagedChunkBuffers {
            placements: buffer(
                device,
                "paged chunk placements",
                bytes_for::<PlacementGpu>(self.machine_capacity)?,
                BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
            )?,
            visible: buffer(
                device,
                "paged visible rows",
                u64::from(self.visible_capacity).saturating_mul(8).max(8),
                BufferUsage::STORAGE,
            )?,
            args: buffer(
                device,
                "paged indirect command arena",
                bytes_for::<DrawIndirectArgs>(COMMAND_COUNT)?,
                BufferUsage::STORAGE
                    .union(BufferUsage::INDIRECT)
                    .union(BufferUsage::COPY_DST),
            )?,
            windows: buffer(
                device,
                "paged trajectory windows",
                bytes_for::<TrajectoryWindowGpu>(self.machine_capacity)?,
                BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
            )?,
        });
        Ok(true)
    }

    fn buffers(&self) -> Result<&PagedChunkBuffers<D>, RenderError> {
        self.buffers.as_ref().ok_or(RenderError::Residency {
            reason: "paged chunk buffers were not allocated for a non-empty plan",
        })
    }

    fn build_pick_plan(&mut self, plan: &[ResidentChunkPlacement]) -> Result<(), RenderError> {
        self.pick_scratch.clear();
        for placement in plan {
            let value = (
                placement.dataset(),
                placement.chunk(),
                placement.span(),
                placement.entity_kind(),
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

fn buffer<D: Device>(
    device: &D,
    label: &'static str,
    size: u64,
    usage: BufferUsage,
) -> Result<D::Buffer, RenderError> {
    Ok(device.create_buffer(&BufferDesc { label, size, usage })?)
}

fn layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "paged structure chunks",
        entries: &[
            storage(0, false, ShaderStages::COMPUTE),
            storage(1, true, ShaderStages::COMPUTE),
            storage(2, true, ShaderStages::VERTEX.union(ShaderStages::COMPUTE)),
            storage(3, false, ShaderStages::COMPUTE),
            storage(4, false, ShaderStages::COMPUTE),
            storage(5, true, ShaderStages::VERTEX),
            storage(6, true, ShaderStages::COMPUTE),
            storage(7, true, ShaderStages::COMPUTE),
            storage(8, true, ShaderStages::VERTEX),
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

impl<D: Device> super::GpuScene<D> {
    pub(crate) const fn paged_chunk_layout(&self) -> &D::BindGroupLayout {
        &self.paged_chunks.layout
    }

    pub(crate) fn sync_paged_chunks(
        &mut self,
        input: PagedChunkSceneSync<'_, D>,
    ) -> Result<(), RenderError> {
        self.paged_chunks.sync(&mut PagedChunkSync {
            scene: input,
            picks: &mut self.picking_pages,
        })
    }

    pub(crate) fn paged_chunk_cull(&self) -> Option<(&D::BindGroup, [u32; 3])> {
        self.paged_chunks.cull()
    }

    pub(crate) fn paged_trajectory_interpolation(&self) -> Option<(&D::BindGroup, [u32; 3])> {
        self.paged_chunks.interpolate()
    }

    pub(crate) fn paged_point_draw(&self) -> Option<(&D::BindGroup, &D::Buffer, u64)> {
        self.paged_chunks.draw(POINT_COMMAND)
    }

    pub(crate) fn paged_spacefill_draw(&self) -> Option<(&D::BindGroup, &D::Buffer, u64)> {
        self.paged_chunks.draw(SPACEFILL_COMMAND)
    }
}
