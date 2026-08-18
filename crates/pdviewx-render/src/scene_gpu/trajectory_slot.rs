//! Persistent two-frame coordinate storage for one placed structure.

use super::buffers::buffer_entry;
use crate::error::RenderError;
use pdviewx_core::TrajectorySegment;
use pdviewx_gpu::{BindGroupDesc, BufferDesc, BufferUsage, ComputePassEncoder, Device, Queue};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TrajectoryUniforms {
    alpha: f32,
    previous_alpha: f32,
    count: u32,
    padding: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TrajectorySync {
    pub(super) changed: bool,
    pub(super) coordinate_binding_changed: bool,
}

#[derive(Debug)]
pub(super) struct GpuTrajectory<D: Device> {
    start: Option<D::Buffer>,
    end: Option<D::Buffer>,
    output: Option<D::Buffer>,
    previous_output: Option<D::Buffer>,
    uniforms: Option<D::Buffer>,
    group: Option<D::BindGroup>,
    capacity: u64,
    pair: Option<FramePair>,
    alpha: Option<u32>,
    count: u32,
    dirty: bool,
    active: bool,
    needs_settle: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct FramePair {
    start_index: u64,
    start_storage: usize,
    end_index: u64,
    end_storage: usize,
}

impl<D: Device> GpuTrajectory<D> {
    pub(super) const fn new() -> Self {
        Self {
            start: None,
            end: None,
            output: None,
            previous_output: None,
            uniforms: None,
            group: None,
            capacity: 0,
            pair: None,
            alpha: None,
            count: 0,
            dirty: false,
            active: false,
            needs_settle: false,
        }
    }

    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        layout: &D::BindGroupLayout,
        segment: Option<&TrajectorySegment>,
    ) -> Result<TrajectorySync, RenderError> {
        let Some(segment) = segment else {
            let changed = self.active;
            self.active = false;
            self.dirty = false;
            self.needs_settle = false;
            return Ok(TrajectorySync {
                changed,
                coordinate_binding_changed: changed,
            });
        };
        let was_active = self.active;
        self.active = true;
        let count = u32::try_from(segment.atom_count()).map_or(u32::MAX, |value| value);
        let byte_len = (segment.atom_count() as u64).saturating_mul(12);
        let output_reallocated = self.ensure_buffers(device, byte_len)?;
        let pair = FramePair {
            start_index: segment.start().index(),
            start_storage: segment.start().positions().as_ptr() as usize,
            end_index: segment.end().index(),
            end_storage: segment.end().positions().as_ptr() as usize,
        };
        let pair_changed = self.pair != Some(pair);
        if pair_changed {
            if let (Some(start), Some(end)) = (&self.start, &self.end) {
                queue.write_buffer(start, 0, bytemuck::cast_slice(segment.start().positions()));
                queue.write_buffer(end, 0, bytemuck::cast_slice(segment.end().positions()));
            }
            self.pair = Some(pair);
        }
        let alpha = segment.interpolation().to_bits();
        let had_prior_sample = self.alpha.is_some();
        let sample_changed = self.alpha != Some(alpha) || self.count != count;
        let settle = !pair_changed && !sample_changed && self.needs_settle;
        if pair_changed || sample_changed || settle {
            let previous_alpha = if pair_changed || self.alpha.is_none() {
                segment.interpolation()
            } else if sample_changed {
                f32::from_bits(self.alpha.map_or(alpha, |value| value))
            } else {
                segment.interpolation()
            };
            if let Some(uniforms) = &self.uniforms {
                queue.write_buffer(
                    uniforms,
                    0,
                    bytemuck::bytes_of(&TrajectoryUniforms {
                        alpha: segment.interpolation(),
                        previous_alpha,
                        count,
                        padding: 0,
                    }),
                );
            }
            self.alpha = Some(alpha);
            self.count = count;
            // A new resident pair initializes previous and current to the
            // same sample. Only a later time advance carries real motion that
            // needs one subsequent settling dispatch.
            self.needs_settle = sample_changed && !pair_changed && had_prior_sample;
            if settle {
                self.needs_settle = false;
            }
        }
        if self.group.is_none() {
            self.bind(device, layout);
        }
        self.dirty |= pair_changed || sample_changed || settle || output_reallocated || !was_active;
        Ok(TrajectorySync {
            changed: self.dirty || !was_active,
            coordinate_binding_changed: output_reallocated || !was_active,
        })
    }

    fn ensure_buffers(&mut self, device: &D, byte_len: u64) -> Result<bool, RenderError> {
        let mut output_reallocated = false;
        if self.start.is_none() || byte_len > self.capacity {
            let capacity = byte_len.next_power_of_two().max(256);
            let input = |label| BufferDesc {
                label,
                size: capacity,
                usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
            };
            self.start = Some(device.create_buffer(&input("trajectory frame start"))?);
            self.end = Some(device.create_buffer(&input("trajectory frame end"))?);
            self.output = Some(device.create_buffer(&BufferDesc {
                label: "interpolated trajectory coordinates",
                size: capacity,
                usage: BufferUsage::STORAGE,
            })?);
            self.previous_output = Some(device.create_buffer(&BufferDesc {
                label: "previous interpolated trajectory coordinates",
                size: capacity,
                usage: BufferUsage::STORAGE,
            })?);
            self.capacity = capacity;
            self.pair = None;
            self.group = None;
            output_reallocated = true;
        }
        if self.uniforms.is_none() {
            self.uniforms = Some(device.create_buffer(&BufferDesc {
                label: "trajectory interpolation uniforms",
                size: std::mem::size_of::<TrajectoryUniforms>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?);
            self.group = None;
        }
        Ok(output_reallocated)
    }

    fn bind(&mut self, device: &D, layout: &D::BindGroupLayout) {
        let (Some(start), Some(end), Some(output), Some(previous_output), Some(uniforms)) = (
            &self.start,
            &self.end,
            &self.output,
            &self.previous_output,
            &self.uniforms,
        ) else {
            return;
        };
        self.group = Some(device.create_bind_group(&BindGroupDesc {
            label: "trajectory interpolation",
            layout,
            entries: &[
                buffer_entry(0, start),
                buffer_entry(1, end),
                buffer_entry(2, output),
                buffer_entry(3, previous_output),
                buffer_entry(4, uniforms),
            ],
        }));
    }

    pub(super) fn output(&self) -> Option<&D::Buffer> {
        if self.active {
            self.output.as_ref()
        } else {
            None
        }
    }

    pub(super) fn previous_output(&self) -> Option<&D::Buffer> {
        if self.active {
            self.previous_output.as_ref()
        } else {
            None
        }
    }

    pub(super) const fn dirty(&self) -> bool {
        self.active && self.dirty
    }

    pub(super) fn record<P: ComputePassEncoder<D>>(
        &mut self,
        pass: &mut P,
        pipeline: &D::Pipeline,
    ) {
        if !self.dirty() {
            return;
        }
        let Some(group) = &self.group else {
            return;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.dispatch(self.count.div_ceil(64), 1, 1);
        self.dirty = false;
    }
}
