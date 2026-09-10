//! Scene-wide immutable visual-program instruction residency.
//!
//! Program discovery is linear in representation count and new program
//! insertion is linear in the number of distinct programs, which is normally
//! tiny. Each instruction stream is uploaded once and addressed by a stable
//! offset from per-draw configuration. A million entities therefore do not
//! duplicate program storage or add CPU work.

use super::grow_buffer::GrowBuffer;
use crate::error::RenderError;
use pdviewx_core::{Scene, VisualInstructionGpu, VisualProgram};
use pdviewx_gpu::Device;

#[cfg(test)]
#[path = "visual_programs_tests.rs"]
mod tests;

#[derive(Clone, Debug)]
struct ResidentProgram {
    program: VisualProgram,
    offset: u32,
}

#[derive(Debug)]
pub(super) struct VisualProgramTable<D: Device> {
    buffer: GrowBuffer<D>,
    programs: Vec<ResidentProgram>,
    instructions: Vec<VisualInstructionGpu>,
    scratch: Vec<VisualInstructionGpu>,
    scene_identity: Option<u64>,
    binding_revision: u64,
}

impl<D: Device> VisualProgramTable<D> {
    pub(super) const fn new() -> Self {
        Self {
            buffer: GrowBuffer::new(),
            programs: Vec::new(),
            instructions: Vec::new(),
            scratch: Vec::new(),
            scene_identity: None,
            binding_revision: 0,
        }
    }

    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
    ) -> Result<bool, RenderError> {
        if self.scene_identity != Some(scene.cache_identity()) {
            self.programs.clear();
            self.instructions.clear();
            self.scene_identity = Some(scene.cache_identity());
        }
        let mut changed = false;
        for (_, representation) in scene.representations() {
            let Some(style) = representation.visual.as_ref() else {
                continue;
            };
            changed |= self.intern(style.program())?;
        }
        for (_, descriptor) in scene.domain_visuals() {
            changed |= self.intern(descriptor.style().program())?;
        }
        let rebound = if self.instructions.is_empty() {
            self.buffer
                .reserve(device, "visual program instructions", 32)?
        } else if changed {
            self.buffer.upload(
                device,
                queue,
                "visual program instructions",
                &self.instructions,
            )?
        } else {
            false
        };
        if rebound {
            self.binding_revision = self.binding_revision.wrapping_add(1);
        }
        Ok(changed || rebound)
    }

    fn intern(&mut self, program: &VisualProgram) -> Result<bool, RenderError> {
        if self
            .programs
            .iter()
            .any(|resident| resident.program == *program)
        {
            return Ok(false);
        }
        let offset = u32::try_from(self.instructions.len()).map_err(|_| {
            pdviewx_gpu::GpuError::LimitExceeded {
                resource: "visual program instruction index",
                limit: u64::from(u32::MAX),
            }
        })?;
        program.write_gpu_instructions(&mut self.scratch);
        self.instructions.extend_from_slice(&self.scratch);
        self.programs.push(ResidentProgram {
            program: program.clone(),
            offset,
        });
        Ok(true)
    }

    pub(super) fn offset(&self, program: &VisualProgram) -> Option<u32> {
        self.programs
            .iter()
            .find(|resident| resident.program == *program)
            .map(|resident| resident.offset)
    }

    pub(super) const fn buffer(&self) -> Option<&D::Buffer> {
        self.buffer.get()
    }

    pub(super) const fn binding_revision(&self) -> u64 {
        self.binding_revision
    }
}
