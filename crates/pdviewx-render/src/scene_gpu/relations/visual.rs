//! Native typed visual lowering for scene-independent relation chunks.

use super::super::grow_buffer::GrowBuffer;
use super::super::visual::{VisualBase, VisualCullEntries, VisualSlot, VisualSync};
use crate::engine::chunk_draw_plan::{ResidentAttributeColumn, ResidentRelationChunkPlacement};
use crate::error::RenderError;
use pdviewx_core::{
    ChunkOccurrenceId, ChunkVisualDescriptor, MAX_VISUAL_PARAMETERS, VisualInstructionGpu,
    VisualProgram, VisualStyle,
};
use pdviewx_gpu::Device;
use std::sync::Arc;

#[derive(Clone, Debug)]
struct ProgramEntry {
    program: VisualProgram,
    offset: u32,
}

#[derive(Clone, Debug)]
struct ParameterEntry {
    style: VisualStyle,
    offset: u32,
}

#[derive(Debug)]
pub(super) struct PagedRelationVisualArenas<D: Device> {
    instructions: GrowBuffer<D>,
    parameters: GrowBuffer<D>,
    programs: Vec<ProgramEntry>,
    styles: Vec<ParameterEntry>,
    instruction_scratch: Vec<VisualInstructionGpu>,
    parameter_scratch: Vec<[f32; 4]>,
    signature: Vec<(ChunkOccurrenceId, u64, u64)>,
    binding_revision: u64,
}

impl<D: Device> PagedRelationVisualArenas<D> {
    pub(super) const fn new() -> Self {
        Self {
            instructions: GrowBuffer::new(),
            parameters: GrowBuffer::new(),
            programs: Vec::new(),
            styles: Vec::new(),
            instruction_scratch: Vec::new(),
            parameter_scratch: Vec::new(),
            signature: Vec::new(),
            binding_revision: 0,
        }
    }

    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        plans: &[ResidentRelationChunkPlacement],
    ) -> Result<bool, RenderError> {
        let next = plans
            .iter()
            .filter_map(|plan| {
                plan.visual.as_ref().map(|visual| {
                    (
                        plan.id,
                        visual.style().program().fingerprint(),
                        visual.style().parameter_fingerprint(),
                    )
                })
            })
            .collect::<Vec<_>>();
        if next == self.signature {
            return Ok(false);
        }
        self.signature = next;
        self.rebuild(plans)?;
        let instruction_rebound = if self.instruction_scratch.is_empty() {
            self.instructions
                .reserve(device, "paged visual instructions", 32)?
        } else {
            self.instructions.upload(
                device,
                queue,
                "paged visual instructions",
                &self.instruction_scratch,
            )?
        };
        let parameter_rebound = if self.parameter_scratch.is_empty() {
            self.parameters
                .reserve(device, "paged visual parameters", 16)?
        } else {
            self.parameters.upload(
                device,
                queue,
                "paged visual parameters",
                &self.parameter_scratch,
            )?
        };
        if instruction_rebound || parameter_rebound {
            self.binding_revision = self.binding_revision.wrapping_add(1);
        }
        Ok(true)
    }

    fn rebuild(&mut self, plans: &[ResidentRelationChunkPlacement]) -> Result<(), RenderError> {
        self.programs.clear();
        self.styles.clear();
        self.instruction_scratch.clear();
        self.parameter_scratch.clear();
        for visual in plans.iter().filter_map(|plan| plan.visual.as_ref()) {
            self.intern_program(visual.style().program())?;
            self.intern_style(visual.style())?;
        }
        Ok(())
    }

    fn intern_program(&mut self, program: &VisualProgram) -> Result<(), RenderError> {
        if self.programs.iter().any(|entry| entry.program == *program) {
            return Ok(());
        }
        let offset = u32::try_from(self.instruction_scratch.len()).map_err(|_| limit())?;
        let mut instructions = Vec::new();
        program.write_gpu_instructions(&mut instructions);
        self.instruction_scratch.extend_from_slice(&instructions);
        self.programs.push(ProgramEntry {
            program: program.clone(),
            offset,
        });
        Ok(())
    }

    fn intern_style(&mut self, style: &VisualStyle) -> Result<(), RenderError> {
        if self.styles.iter().any(|entry| entry.style == *style) {
            return Ok(());
        }
        let offset = u32::try_from(self.parameter_scratch.len()).map_err(|_| limit())?;
        self.parameter_scratch.extend_from_slice(style.parameters());
        self.styles.push(ParameterEntry {
            style: style.clone(),
            offset,
        });
        Ok(())
    }

    fn program_offset(&self, program: &VisualProgram) -> Option<u32> {
        self.programs
            .iter()
            .find(|entry| entry.program == *program)
            .map(|entry| entry.offset)
    }

    fn parameter_offset(&self, style: &VisualStyle) -> Option<u32> {
        self.styles
            .iter()
            .find(|entry| entry.style == *style)
            .map(|entry| entry.offset)
    }

    fn buffers(&self) -> Option<(&D::Buffer, &D::Buffer)> {
        Some((self.instructions.get()?, self.parameters.get()?))
    }

    pub(super) const fn binding_revision(&self) -> u64 {
        self.binding_revision
    }
}

#[derive(Debug)]
pub(super) struct PagedRelationVisualState<D: Device> {
    descriptor: Arc<ChunkVisualDescriptor>,
    slot: VisualSlot<D>,
}

impl<D: Device> PagedRelationVisualState<D> {
    pub(super) fn new(descriptor: Arc<ChunkVisualDescriptor>) -> Self {
        Self {
            descriptor,
            slot: VisualSlot::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn sync<F>(
        &mut self,
        device: &D,
        queue: &D::Queue,
        descriptor: Arc<ChunkVisualDescriptor>,
        row_count: usize,
        color: pdviewx_math::Rgba8,
        opacity: f32,
        time_seconds: f32,
        arenas: &PagedRelationVisualArenas<D>,
        mut resolve: F,
    ) -> Result<bool, RenderError>
    where
        F: FnMut(pdviewx_core::ResidencyTicket) -> Option<ResidentAttributeColumn>,
    {
        self.descriptor = descriptor;
        let style = self.descriptor.style();
        let (_, parameters) = arenas.buffers().ok_or_else(limit)?;
        let mut offsets = [0; 4];
        let mut layouts = [0; 4];
        let mut end_offsets = [0; 4];
        let mut alphas = [0.0; 4];
        for (slot, binding) in self.descriptor.bindings().iter().enumerate() {
            let column = resolve(binding.ticket()).ok_or_else(limit)?;
            offsets[slot] = u32::try_from(column.byte_offset / 4).map_err(|_| limit())?;
            layouts[slot] = (column.kind.stride() / 4) | ((column.kind as u32) << 8);
            if let Some(timeline) = column.timeline {
                if let Some(materialized) = timeline.materialized_byte_offset {
                    offsets[slot] = u32::try_from(materialized / 4).map_err(|_| limit())?;
                } else {
                    offsets[slot] =
                        u32::try_from(timeline.start_byte_offset / 4).map_err(|_| limit())?;
                    end_offsets[slot] =
                        u32::try_from(timeline.end_byte_offset / 4).map_err(|_| limit())?;
                    alphas[slot] = timeline.interpolation;
                    layouts[slot] |= 1 << 17;
                }
            }
        }
        self.slot.sync(&VisualSync {
            device,
            queue,
            style: Some(style),
            program_offset: arenas.program_offset(style.program()).ok_or_else(limit)?,
            parameter_buffer: parameters,
            parameter_offset: arenas.parameter_offset(style).ok_or_else(limit)?,
            parameters_preloaded: true,
            property_offsets: offsets,
            attribute_layouts: layouts,
            property_end_offsets: end_offsets,
            property_alphas: alphas,
            time_seconds,
            entity_count: row_count,
            result_count: row_count,
            base: VisualBase::rgba8_opacity(color, opacity),
        })
    }

    pub(super) fn entries<'a>(
        &'a self,
        properties: &'a D::Buffer,
        arenas: &'a PagedRelationVisualArenas<D>,
    ) -> Option<VisualCullEntries<'a, D>> {
        let (instructions, parameters) = arenas.buffers()?;
        self.slot.cull_entries(instructions, parameters, properties)
    }

    pub(super) const fn binding_revision(&self) -> u64 {
        self.slot.binding_revision()
    }
}

fn limit() -> pdviewx_gpu::GpuError {
    pdviewx_gpu::GpuError::LimitExceeded {
        resource: "paged relation visual arena",
        limit: (MAX_VISUAL_PARAMETERS * 16) as u64,
    }
}
