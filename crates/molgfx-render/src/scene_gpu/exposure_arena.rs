//! Three completion-protected banks of distinct temporal uniform ranges.
//!
//! The CPU staging storage and ranged bind groups are created once. One upload
//! and one submission publish an exposure; bank reuse requires its final fence.

use super::FrameUniforms;
use super::sync::GpuScene;
use crate::RenderError;
use molgfx_gpu::{
    BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, Device, FenceValue, Queue,
};

pub(crate) const EXPOSURE_SAMPLES: usize = 64;
const BANKS: usize = 3;

#[derive(Debug)]
pub(super) struct ExposureArena<D: Device> {
    buffer: D::Buffer,
    groups: Vec<D::BindGroup>,
    staging: Vec<u8>,
    stride: usize,
    fences: [FenceValue; BANKS],
    active: Option<usize>,
    pub(super) uploaded_bytes: u64,
}

impl<D: Device> ExposureArena<D> {
    pub(super) fn new(device: &D, layout: &D::BindGroupLayout) -> Result<Self, RenderError> {
        let size = std::mem::size_of::<FrameUniforms>();
        let alignment = device.capabilities().min_uniform_buffer_offset_alignment as usize;
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(error("invalid uniform buffer offset alignment"));
        }
        let stride = size.div_ceil(alignment) * alignment;
        let buffer = device.create_buffer(&BufferDesc {
            label: "exposure uniform arena",
            size: (stride * EXPOSURE_SAMPLES * BANKS) as u64,
            usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
        })?;
        let groups = (0..EXPOSURE_SAMPLES * BANKS)
            .map(|index| {
                device.create_bind_group(&BindGroupDesc {
                    label: "group0: exposure sample",
                    layout,
                    entries: &[BindGroupEntry::BufferRange {
                        binding: 0,
                        buffer: &buffer,
                        offset: (index * stride) as u64,
                        size: size as u64,
                    }],
                })
            })
            .collect();
        Ok(Self {
            buffer,
            groups,
            staging: vec![0; stride * EXPOSURE_SAMPLES],
            stride,
            fences: [FenceValue::default(); BANKS],
            active: None,
            uploaded_bytes: 0,
        })
    }

    pub(super) fn available(&self, completed: FenceValue) -> Option<usize> {
        self.fences.iter().position(|fence| *fence <= completed)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn oldest(&self) -> FenceValue {
        self.fences[1..]
            .iter()
            .copied()
            .fold(self.fences[0], std::cmp::min)
    }

    pub(super) fn stage(
        &mut self,
        bank: usize,
        sample: usize,
        uniforms: &FrameUniforms,
        group: &mut D::BindGroup,
    ) -> Result<(), RenderError> {
        if bank >= BANKS || sample >= EXPOSURE_SAMPLES {
            return Err(error("exposure exceeds its uniform arena budget"));
        }
        self.restore(group);
        let bytes = bytemuck::bytes_of(uniforms);
        let offset = sample * self.stride;
        self.staging[offset..offset + bytes.len()].copy_from_slice(bytes);
        let index = bank * EXPOSURE_SAMPLES + sample;
        std::mem::swap(group, &mut self.groups[index]);
        self.active = Some(index);
        Ok(())
    }

    pub(super) fn upload(&mut self, queue: &D::Queue, bank: usize, samples: usize) {
        queue.write_buffer(
            &self.buffer,
            (bank * EXPOSURE_SAMPLES * self.stride) as u64,
            &self.staging[..samples * self.stride],
        );
        self.uploaded_bytes = self
            .uploaded_bytes
            .saturating_add((samples * self.stride) as u64);
    }

    pub(super) fn submit(&mut self, bank: usize, fence: FenceValue, group: &mut D::BindGroup) {
        self.fences[bank] = fence;
        self.restore(group);
    }

    pub(super) fn restore(&mut self, group: &mut D::BindGroup) {
        if let Some(index) = self.active.take() {
            std::mem::swap(group, &mut self.groups[index]);
        }
    }
}

impl<D: Device> GpuScene<D> {
    pub(crate) fn acquire_exposure_bank(
        &mut self,
        device: &D,
        queue: &D::Queue,
    ) -> Result<usize, RenderError> {
        if self.exposure_uniforms.is_none() {
            self.exposure_uniforms = Some(ExposureArena::new(device, &self.group0_layout)?);
        }
        let Some(arena) = &mut self.exposure_uniforms else {
            return Err(error("exposure uniform arena is unavailable"));
        };
        arena.restore(&mut self.group0);
        let completed = queue.completed_fence(device)?;
        if let Some(bank) = arena.available(completed) {
            return Ok(bank);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            queue.wait_fence_blocking(device, arena.oldest())?;
            if let Some(bank) = arena.available(queue.completed_fence(device)?) {
                return Ok(bank);
            }
        }
        Err(error("exposure uniform arena has three outputs in flight"))
    }

    pub(crate) fn stage_exposure_uniforms(
        &mut self,
        bank: usize,
        sample: usize,
        uniforms: &FrameUniforms,
    ) -> Result<(), RenderError> {
        let Some(arena) = &mut self.exposure_uniforms else {
            return Err(error("exposure uniform arena is unavailable"));
        };
        arena.stage(bank, sample, uniforms, &mut self.group0)
    }

    pub(crate) fn submit_exposure_uniforms(
        &mut self,
        queue: &D::Queue,
        bank: usize,
        samples: usize,
        encoder: D::CommandEncoder,
    ) -> Result<FenceValue, RenderError> {
        let Some(arena) = &mut self.exposure_uniforms else {
            return Err(error("exposure uniform arena is unavailable"));
        };
        arena.upload(queue, bank, samples);
        let fence = queue.submit_tracked(encoder);
        arena.submit(bank, fence, &mut self.group0);
        Ok(fence)
    }
}

fn error(reason: &'static str) -> RenderError {
    RenderError::Residency { reason }
}

#[cfg(test)]
#[path = "exposure_arena_tests.rs"]
mod tests;
