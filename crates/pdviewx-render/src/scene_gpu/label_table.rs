//! Persistent analytic label table and its GPU decluttering resources.

use super::buffers::{count, upload_grow, write_draw_args};
use super::label_pack::pack_labels;
use super::label_types::{LabelCountsGpu, LabelGpu, LabelHeaderGpu, OCCUPANCY_SLOTS};
use super::structure::GpuStructure;
use crate::error::RenderError;
use pdviewx_core::Scene;
use pdviewx_gpu::{BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, Device, Queue};

#[derive(Debug)]
pub(super) struct GpuLabels<D: Device> {
    headers: Option<D::Buffer>,
    source: Option<D::Buffer>,
    visible: Option<D::Buffer>,
    occupancy: Option<D::Buffer>,
    args: Option<D::Buffer>,
    counts: Option<D::Buffer>,
    compute_group: Option<D::BindGroup>,
    render_group: Option<D::BindGroup>,
    header_capacity: u64,
    source_capacity: u64,
    visible_capacity: u64,
    synced: Option<(u64, u64)>,
    header_count: u32,
    source_count: u32,
    headers_scratch: Vec<LabelHeaderGpu>,
    records_scratch: Vec<LabelGpu>,
}

impl<D: Device> GpuLabels<D> {
    pub(super) const fn new() -> Self {
        Self {
            headers: None,
            source: None,
            visible: None,
            occupancy: None,
            args: None,
            counts: None,
            compute_group: None,
            render_group: None,
            header_capacity: 0,
            source_capacity: 0,
            visible_capacity: 0,
            synced: None,
            header_count: 0,
            source_count: 0,
            headers_scratch: Vec::new(),
            records_scratch: Vec::new(),
        }
    }

    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        compute_layout: &D::BindGroupLayout,
        render_layout: &D::BindGroupLayout,
        scene: &Scene,
        structures: &[GpuStructure<D>],
    ) -> Result<bool, RenderError> {
        let revision = (scene.label_revision(), scene.structure_revision());
        if self.synced == Some(revision) {
            return Ok(false);
        }
        pack_labels(
            scene,
            structures,
            &mut self.headers_scratch,
            &mut self.records_scratch,
        );
        let header_needed = byte_len::<LabelHeaderGpu>(self.headers_scratch.len());
        let record_needed = byte_len::<LabelGpu>(self.records_scratch.len());
        let rebind = self.headers.is_none()
            || self.source.is_none()
            || header_needed > self.header_capacity
            || record_needed > self.source_capacity
            || record_needed > self.visible_capacity;
        upload_grow(
            device,
            queue,
            "label headers",
            &self.headers_scratch,
            &mut self.headers,
            &mut self.header_capacity,
        )?;
        upload_grow(
            device,
            queue,
            "label source records",
            &self.records_scratch,
            &mut self.source,
            &mut self.source_capacity,
        )?;
        self.ensure_outputs(device, record_needed)?;
        write_draw_args(
            device,
            queue,
            "label indirect arguments",
            6,
            0,
            &mut self.args,
        )?;
        self.header_count = count(self.headers_scratch.len());
        self.source_count = count(self.records_scratch.len());
        self.write_counts(device, queue)?;
        if rebind || self.compute_group.is_none() || self.render_group.is_none() {
            self.bind(device, compute_layout, render_layout);
        }
        self.synced = Some(revision);
        Ok(true)
    }

    fn ensure_outputs(&mut self, device: &D, needed: u64) -> Result<(), RenderError> {
        if self.visible.is_none() || needed > self.visible_capacity {
            self.visible_capacity = needed.next_power_of_two().max(256);
            self.visible = Some(device.create_buffer(&BufferDesc {
                label: "decluttered label records",
                size: self.visible_capacity,
                usage: BufferUsage::STORAGE,
            })?);
        }
        if self.occupancy.is_none() {
            self.occupancy = Some(device.create_buffer(&BufferDesc {
                label: "label declutter occupancy",
                size: OCCUPANCY_SLOTS * size_u64::<u32>(),
                usage: BufferUsage::STORAGE,
            })?);
        }
        Ok(())
    }

    fn write_counts(&mut self, device: &D, queue: &D::Queue) -> Result<(), RenderError> {
        if self.counts.is_none() {
            self.counts = Some(device.create_buffer(&BufferDesc {
                label: "label logical counts",
                size: size_u64::<LabelCountsGpu>(),
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?);
        }
        if let Some(buffer) = &self.counts {
            let counts = LabelCountsGpu {
                header_count: self.header_count,
                source_count: self.source_count,
                occupancy_count: u32::try_from(OCCUPANCY_SLOTS).map_or(u32::MAX, |value| value),
                padding: 0,
            };
            queue.write_buffer(buffer, 0, bytemuck::bytes_of(&counts));
        }
        Ok(())
    }

    fn bind(
        &mut self,
        device: &D,
        compute_layout: &D::BindGroupLayout,
        render_layout: &D::BindGroupLayout,
    ) {
        let (Some(headers), Some(source), Some(visible), Some(occupancy), Some(args), Some(counts)) = (
            &self.headers,
            &self.source,
            &self.visible,
            &self.occupancy,
            &self.args,
            &self.counts,
        ) else {
            return;
        };
        self.compute_group = Some(device.create_bind_group(&BindGroupDesc {
            label: "group2: label declutter",
            layout: compute_layout,
            entries: &[
                entry(0, headers),
                entry(1, source),
                entry(2, visible),
                entry(3, occupancy),
                entry(4, args),
                entry(5, counts),
            ],
        }));
        self.render_group = Some(device.create_bind_group(&BindGroupDesc {
            label: "group2: visible labels",
            layout: render_layout,
            entries: &[entry(0, visible)],
        }));
    }

    pub(super) fn declutter(&self) -> Option<&D::BindGroup> {
        (self.header_count > 0).then_some(self.compute_group.as_ref()?)
    }

    pub(super) fn draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
        (self.source_count > 0).then_some((self.render_group.as_ref()?, self.args.as_ref()?))
    }

    pub(super) const fn has_visible(&self) -> bool {
        self.source_count > 0
    }
}

fn entry<D: Device>(binding: u32, buffer: &D::Buffer) -> BindGroupEntry<'_, D> {
    BindGroupEntry::Buffer { binding, buffer }
}

fn byte_len<T>(count: usize) -> u64 {
    u64::try_from(count.saturating_mul(std::mem::size_of::<T>())).map_or(u64::MAX, |value| value)
}

fn size_u64<T>() -> u64 {
    u64::try_from(std::mem::size_of::<T>()).map_or(u64::MAX, |value| value)
}
