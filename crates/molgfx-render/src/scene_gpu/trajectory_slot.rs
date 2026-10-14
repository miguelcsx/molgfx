//! Persistent coordinate storage for one placed structure: two frames, plus the
//! neighbouring `before`/`after` frames only while cubic Hermite is selected.

use super::buffers::buffer_entry;
use super::grow_buffer::grow_capacity;
use crate::error::RenderError;
use molgfx_core::{InterpolationKind, TrajectoryFrame, TrajectorySegment};
use molgfx_gpu::{BindGroupDesc, BufferDesc, BufferUsage, ComputePassEncoder, Device, Queue};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TrajectoryUniforms {
    alpha: f32,
    previous_alpha: f32,
    count: u32,
    /// 0 = linear, 1 = cubic Hermite.
    mode: u32,
    /// Blend weights over `[before, start, end, after]` at the current sample.
    weights: [f32; 4],
    /// The same weights at the previous sample, so motion vectors follow the spline.
    previous_weights: [f32; 4],
}

/// What a sync changed, so the caller can re-record or rebind.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrajectorySync {
    pub(super) changed: bool,
    pub(super) coordinate_binding_changed: bool,
}

/// Four equal buffers are held per trajectory, so the rounding waste is paid
/// four times: grow by an eighth of an octave and never past the device limit
/// rather than to the next power of two, which doubles a 10M-atom frame and
/// can exceed the limit for a frame that fits.
fn trajectory_capacity(byte_len: u64, limit: u64) -> Result<u64, RenderError> {
    grow_capacity(byte_len, limit, "trajectory frame")
}

/// Resident frames and interpolated outputs for one placed trajectory.
#[derive(Debug)]
pub struct GpuTrajectory<D: Device> {
    start: Option<D::Buffer>,
    end: Option<D::Buffer>,
    /// Allocated only while a Hermite segment has a frame before its start.
    before: Option<D::Buffer>,
    /// Allocated only while a Hermite segment has a frame after its end.
    after: Option<D::Buffer>,
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
    hermite: bool,
    before: Option<(u64, usize)>,
    after: Option<(u64, usize)>,
}

fn resident(frame: Option<&TrajectoryFrame>) -> Option<(u64, usize)> {
    frame.map(|frame| (frame.index(), frame.positions().as_ptr() as usize))
}

fn or_alias<'a, B>(slot: Option<&'a B>, alias: &'a B) -> &'a B {
    match slot {
        Some(buffer) => buffer,
        None => alias,
    }
}

/// Narrows `f64` blend weights to the `f32` the GPU consumes.
fn narrow_weights(weights: [f64; 4]) -> Result<[f32; 4], RenderError> {
    let mut narrowed = [0.0_f32; 4];
    for (slot, weight) in narrowed.iter_mut().zip(weights) {
        *slot = molgfx_core::narrow_to_f32(weight).ok_or(
            molgfx_core::CoreError::InvalidTrajectory {
                reason: "trajectory blend weights must be representable as f32",
            },
        )?;
    }
    Ok(narrowed)
}

impl<D: Device> Default for GpuTrajectory<D> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D: Device> GpuTrajectory<D> {
    /// An inactive slot that holds no buffers yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            start: None,
            end: None,
            before: None,
            after: None,
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

    /// Uploads whatever the segment changed and prepares the dispatch.
    ///
    /// # Errors
    ///
    /// Fails when a buffer cannot be allocated or a blend weight is not an `f32`.
    pub fn sync(
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
            // Resident frames survive deactivation, but temporal provenance
            // does not: reactivation initializes both outputs at its sample.
            self.alpha = None;
            return Ok(TrajectorySync {
                changed,
                coordinate_binding_changed: changed,
            });
        };
        let was_active = self.active;
        self.active = true;
        let count = crate::fallback(u32::try_from(segment.atom_count()), u32::MAX);
        let byte_len = (segment.atom_count() as u64).saturating_mul(12);
        let hermite = segment.interpolation_kind() == InterpolationKind::CubicHermite;
        let (before, after) = if hermite {
            (segment.before(), segment.after())
        } else {
            (None, None)
        };
        let output_reallocated =
            self.ensure_buffers(device, byte_len, before.is_some(), after.is_some())?;
        let pair = FramePair {
            start_index: segment.start().index(),
            start_storage: segment.start().positions().as_ptr() as usize,
            end_index: segment.end().index(),
            end_storage: segment.end().positions().as_ptr() as usize,
            hermite,
            before: resident(before),
            after: resident(after),
        };
        let pair_changed = self.pair != Some(pair);
        if pair_changed {
            if let (Some(start), Some(end)) = (&self.start, &self.end) {
                queue.write_buffer(start, 0, bytemuck::cast_slice(segment.start().positions()));
                queue.write_buffer(end, 0, bytemuck::cast_slice(segment.end().positions()));
            }
            for (buffer, frame) in [(&self.before, before), (&self.after, after)] {
                if let (Some(buffer), Some(frame)) = (buffer, frame) {
                    queue.write_buffer(buffer, 0, bytemuck::cast_slice(frame.positions()));
                }
            }
            self.pair = Some(pair);
        }
        // The GPU consumes f32 blend weights; narrow once and cache the f32 bits.
        let alpha_value = molgfx_core::narrow_to_f32(segment.interpolation()).ok_or(
            molgfx_core::CoreError::InvalidTrajectory {
                reason: "trajectory interpolation must be representable as f32",
            },
        )?;
        let alpha = alpha_value.to_bits();
        let had_prior_sample = self.alpha.is_some();
        let sample_changed = self.alpha != Some(alpha) || self.count != count;
        let settle = !pair_changed && !sample_changed && self.needs_settle;
        if pair_changed || sample_changed || settle {
            let previous_alpha = if pair_changed || self.alpha.is_none() {
                alpha_value
            } else if sample_changed {
                f32::from_bits(crate::fallback(self.alpha, alpha))
            } else {
                alpha_value
            };
            let (weights, previous_weights) = if hermite {
                (
                    narrow_weights(segment.blend_weights(segment.interpolation()))?,
                    narrow_weights(segment.blend_weights(f64::from(previous_alpha)))?,
                )
            } else {
                ([0.0; 4], [0.0; 4])
            };
            if let Some(uniforms) = &self.uniforms {
                queue.write_buffer(
                    uniforms,
                    0,
                    bytemuck::bytes_of(&TrajectoryUniforms {
                        alpha: alpha_value,
                        previous_alpha,
                        count,
                        mode: u32::from(hermite),
                        weights,
                        previous_weights,
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

    fn ensure_buffers(
        &mut self,
        device: &D,
        byte_len: u64,
        want_before: bool,
        want_after: bool,
    ) -> Result<bool, RenderError> {
        let mut output_reallocated = false;
        if self.start.is_none() || byte_len > self.capacity {
            let capacity =
                trajectory_capacity(byte_len, device.capabilities().max_storage_buffer_bytes)?;
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
                usage: OUTPUT_USAGE,
            })?);
            self.previous_output = Some(device.create_buffer(&BufferDesc {
                label: "previous interpolated trajectory coordinates",
                size: capacity,
                usage: OUTPUT_USAGE,
            })?);
            self.capacity = capacity;
            self.before = None;
            self.after = None;
            self.pair = None;
            self.group = None;
            output_reallocated = true;
        }
        let capacity = self.capacity;
        let neighbour = |label| BufferDesc {
            label,
            size: capacity,
            usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
        };
        let changed = Self::fit_neighbour(
            device,
            &mut self.before,
            want_before,
            &neighbour("trajectory frame before"),
        )?;
        let changed = Self::fit_neighbour(
            device,
            &mut self.after,
            want_after,
            &neighbour("trajectory frame after"),
        )? || changed;
        if changed {
            // The binding set or its contents changed: rebind and re-upload.
            self.group = None;
            self.pair = None;
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

    /// Allocates or releases one neighbour buffer; true when it changed.
    fn fit_neighbour(
        device: &D,
        slot: &mut Option<D::Buffer>,
        wanted: bool,
        desc: &BufferDesc,
    ) -> Result<bool, RenderError> {
        match (slot.is_some(), wanted) {
            (false, true) => {
                *slot = Some(device.create_buffer(desc)?);
                Ok(true)
            }
            (true, false) => {
                *slot = None;
                Ok(true)
            }
            _ => Ok(false),
        }
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
                // An absent neighbour aliases a resident frame: its weight is zero.
                buffer_entry(5, or_alias(self.before.as_ref(), start)),
                buffer_entry(6, or_alias(self.after.as_ref(), end)),
            ],
        }));
    }

    /// The interpolated coordinates at the current sample, while active.
    pub fn output(&self) -> Option<&D::Buffer> {
        if self.active {
            self.output.as_ref()
        } else {
            None
        }
    }

    /// The interpolated coordinates at the previous sample, while active.
    pub fn previous_output(&self) -> Option<&D::Buffer> {
        if self.active {
            self.previous_output.as_ref()
        } else {
            None
        }
    }

    pub(super) const fn dirty(&self) -> bool {
        self.active && self.dirty
    }

    /// Records the interpolation dispatch when the slot is dirty.
    pub fn record<P: ComputePassEncoder<D>>(&mut self, pass: &mut P, pipeline: &D::Pipeline) {
        if !self.dirty() {
            return;
        }
        let Some(group) = &self.group else {
            return;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, group, &[]);
        let groups = trajectory_dispatch_groups(self.count);
        pass.dispatch(groups[0], groups[1], 1);
        self.dirty = false;
    }
}

/// Outputs are storage; the copy source lets a test read them back.
const OUTPUT_USAGE: BufferUsage = BufferUsage::STORAGE.union(BufferUsage::COPY_SRC);

fn trajectory_dispatch_groups(atom_count: u32) -> [u32; 2] {
    let coordinates = u64::from(atom_count) * 3;
    super::dispatch::workgroups_2d(coordinates.div_ceil(64))
}

#[cfg(test)]
#[path = "trajectory_slot_tests.rs"]
mod tests;
