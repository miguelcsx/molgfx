//! Persistent GPU state for one bounded typed visual program.
//!
//! Immutable instruction streams and scalar property columns are scene-wide
//! and deduplicated. Each representation retains only its small mutable
//! parameter block, result table and configuration, so parameter edits never
//! duplicate program storage or reallocate entity-scale buffers.

use super::buffers::ensure_upload_buffer;
use crate::error::RenderError;
use molgfx_core::{Representation, VisualInstructionGpu, VisualOutput, VisualStyle};
use molgfx_gpu::{BufferDesc, BufferUsage, Device, Queue};

#[path = "visual_helpers.rs"]
mod helpers;
use helpers::{representation_base_color, result_word_count, saturating_u32};
#[path = "visual/fragment_program.rs"]
mod fragment_program;
use fragment_program::{FRAGMENT_PARAMETER_OFFSET, VisualFragmentProgram};

#[cfg(test)]
#[path = "visual_tests.rs"]
mod tests;

const MISSING_REGISTER: u32 = u32::MAX;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct VisualConfig {
    pub(super) counts: [u32; 4],
    pub(super) outputs0: [u32; 4],
    pub(super) outputs1: [u32; 4],
    pub(super) outputs2: [u32; 4],
    pub(super) base_color: [f32; 4],
    pub(super) material: [f32; 4],
    pub(super) uniform_emission: [f32; 4],
    pub(super) uniform_geometry: [f32; 4],
    pub(super) uniform_offset: [f32; 4],
    pub(super) presentation: [f32; 4],
    pub(super) property_offsets: [u32; 4],
    pub(super) attribute_layouts: [u32; 4],
    pub(super) property_end_offsets: [u32; 4],
    pub(super) property_alphas: [f32; 4],
    pub(super) arena_offsets: [u32; 4],
    pub(super) result_layout: [u32; 4],
}

impl Default for VisualConfig {
    fn default() -> Self {
        Self {
            counts: [0; 4],
            outputs0: [MISSING_REGISTER; 4],
            outputs1: [MISSING_REGISTER; 4],
            outputs2: [MISSING_REGISTER; 4],
            base_color: [1.0; 4],
            material: [1.0, 0.34, 0.5, 0.0],
            uniform_emission: [0.0; 4],
            uniform_geometry: [1.0, 0.0, 0.25, 0.25],
            uniform_offset: [0.0; 4],
            presentation: [0.0; 4],
            property_offsets: [0; 4],
            attribute_layouts: [0; 4],
            property_end_offsets: [0; 4],
            property_alphas: [0.0; 4],
            arena_offsets: [0; 4],
            result_layout: [0; 4],
        }
    }
}

#[derive(Debug)]
pub(super) struct VisualSlot<D: Device> {
    results: Option<D::Buffer>,
    config: Option<D::Buffer>,
    fragment_program: Option<D::Buffer>,
    result_capacity: u64,
    program_fingerprint: u64,
    parameter_fingerprint: u64,
    entity_count: usize,
    config_value: VisualConfig,
    instruction_scratch: Vec<VisualInstructionGpu>,
    binding_revision: u64,
}

pub(super) struct VisualCullEntries<'a, D: Device> {
    pub(super) instructions: &'a D::Buffer,
    pub(super) parameters: &'a D::Buffer,
    pub(super) properties: &'a D::Buffer,
    pub(super) results: &'a D::Buffer,
    pub(super) config: &'a D::Buffer,
    pub(super) fragment_program: &'a D::Buffer,
}

impl<D: Device> Copy for VisualCullEntries<'_, D> {}

impl<D: Device> Clone for VisualCullEntries<'_, D> {
    fn clone(&self) -> Self {
        *self
    }
}

/// One scene-wide disabled binding set for drawable families without a style.
/// The specialized built-in pipeline never reads it; sharing it avoids five
/// placeholder allocations per caller mesh.
#[derive(Debug)]
pub(super) struct VisualFallback<D: Device> {
    instructions: D::Buffer,
    parameters: D::Buffer,
    properties: D::Buffer,
    results: D::Buffer,
    config: D::Buffer,
    fragment_program: D::Buffer,
    initialized: bool,
}

impl<D: Device> VisualFallback<D> {
    pub(super) fn new(device: &D) -> Result<Self, RenderError> {
        let storage = |label, size| {
            device.create_buffer(&BufferDesc {
                label,
                size,
                usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
            })
        };
        Ok(Self {
            instructions: storage("disabled visual instructions", 32)?,
            parameters: storage("disabled visual parameters", 16)?,
            properties: storage("disabled visual properties", 4)?,
            results: storage("disabled visual results", 32)?,
            config: device.create_buffer(&BufferDesc {
                label: "disabled visual configuration",
                size: std::mem::size_of::<VisualConfig>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?,
            fragment_program: device.create_buffer(&BufferDesc {
                label: "disabled fragment visual program",
                size: std::mem::size_of::<VisualFragmentProgram>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?,
            initialized: false,
        })
    }

    pub(super) fn sync(&mut self, queue: &D::Queue) -> bool {
        if self.initialized {
            return false;
        }
        queue.write_buffer(&self.instructions, 0, &[0; 32]);
        queue.write_buffer(&self.parameters, 0, &[0; 16]);
        queue.write_buffer(&self.properties, 0, &[0; 4]);
        queue.write_buffer(&self.results, 0, &[0; 32]);
        queue.write_buffer(
            &self.config,
            0,
            bytemuck::bytes_of(&VisualConfig::default()),
        );
        queue.write_buffer(
            &self.fragment_program,
            0,
            bytemuck::bytes_of(&VisualFragmentProgram::default()),
        );
        self.initialized = true;
        true
    }

    pub(super) const fn entries(&self) -> VisualCullEntries<'_, D> {
        VisualCullEntries {
            instructions: &self.instructions,
            parameters: &self.parameters,
            properties: &self.properties,
            results: &self.results,
            config: &self.config,
            fragment_program: &self.fragment_program,
        }
    }
}

pub(super) struct VisualSync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) style: Option<&'a VisualStyle>,
    pub(super) program_offset: u32,
    pub(super) parameter_buffer: &'a D::Buffer,
    pub(super) parameter_offset: u32,
    pub(super) parameters_preloaded: bool,
    pub(super) property_offsets: [u32; 4],
    pub(super) attribute_layouts: [u32; 4],
    pub(super) property_end_offsets: [u32; 4],
    pub(super) property_alphas: [f32; 4],
    pub(super) time_seconds: f32,
    pub(super) entity_count: usize,
    pub(super) result_count: usize,
    pub(super) base: VisualBase,
}

/// Drawable-specific fallback inputs kept outside the generic program.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct VisualBase {
    color: [f32; 4],
    material: [f32; 4],
}

impl VisualBase {
    pub(super) fn representation(value: &Representation) -> Self {
        let lanes = value.material.model_lanes();
        Self {
            color: representation_base_color(value),
            material: [
                value.material.opacity.clamp(0.0, 1.0),
                value.material.perceptual_roughness(),
                value.material.specular_strength(),
                lanes[1],
            ],
        }
    }

    pub(super) fn rgba8_opacity(color: molgfx_math::Rgba8, opacity: f32) -> Self {
        let scale = 1.0 / 255.0;
        Self {
            color: [
                f32::from(color.r) * scale,
                f32::from(color.g) * scale,
                f32::from(color.b) * scale,
                f32::from(color.a) * scale,
            ],
            material: [opacity.clamp(0.0, 1.0), 0.55, 0.5, 0.0],
        }
    }
}

struct VisualConfigInput<'a> {
    program: &'a molgfx_core::VisualProgram,
    style: &'a VisualStyle,
    base: VisualBase,
    entity_count: usize,
    result_count: usize,
    property_offsets: [u32; 4],
    attribute_layouts: [u32; 4],
    property_end_offsets: [u32; 4],
    property_alphas: [f32; 4],
    time_seconds: f32,
    program_offset: u32,
    parameter_offset: u32,
}

impl<D: Device> VisualSlot<D> {
    pub(super) fn new() -> Self {
        Self {
            results: None,
            config: None,
            fragment_program: None,
            result_capacity: 0,
            program_fingerprint: 0,
            parameter_fingerprint: 0,
            entity_count: 0,
            config_value: VisualConfig::default(),
            instruction_scratch: Vec::new(),
            binding_revision: 0,
        }
    }

    pub(super) fn entity_count(&self) -> usize {
        self.entity_count
    }

    pub(super) const fn has_cull_results(&self) -> bool {
        self.config_value.result_layout[0] & (RESULT_GEOMETRY | RESULT_OFFSET) != 0
    }

    pub(super) const fn has_shading_results(&self) -> bool {
        self.config_value.result_layout[0] & (RESULT_COLOR | RESULT_EMISSION | RESULT_RESPONSE) != 0
    }

    pub(super) const fn binding_revision(&self) -> u64 {
        self.binding_revision
    }

    pub(super) fn cull_entries<'a>(
        &'a self,
        instructions: &'a D::Buffer,
        parameters: &'a D::Buffer,
        properties: &'a D::Buffer,
    ) -> Option<VisualCullEntries<'a, D>> {
        Some(VisualCullEntries {
            instructions,
            parameters,
            properties,
            results: self.results.as_ref()?,
            config: self.config.as_ref()?,
            fragment_program: self.fragment_program.as_ref()?,
        })
    }

    pub(super) fn sync(&mut self, input: &VisualSync<'_, D>) -> Result<bool, RenderError> {
        let Some(style) = input.style else {
            let changed = self.ensure_fallback(input.device, input.queue)?;
            self.entity_count = input.entity_count;
            return Ok(changed);
        };

        let program = style.program();
        let fragment_program_changed =
            self.sync_parameters(input.device, input.queue, input, style, program)?;
        let result_layout = result_layout(program, input.result_count);
        let results_changed = self.sync_results(input.device, result_word_count(result_layout))?;
        let config_changed = self.sync_config(input, program)?;
        self.entity_count = input.entity_count;
        if fragment_program_changed || results_changed {
            self.binding_revision = self.binding_revision.wrapping_add(1);
        }
        Ok(fragment_program_changed || results_changed || config_changed)
    }

    fn sync_parameters(
        &mut self,
        device: &D,
        queue: &D::Queue,
        input: &VisualSync<'_, D>,
        style: &VisualStyle,
        program: &molgfx_core::VisualProgram,
    ) -> Result<bool, RenderError> {
        let program_changed = self.program_fingerprint != program.fingerprint();
        let parameters_changed = self.parameter_fingerprint != style.parameter_fingerprint();
        let fragment_program_missing = self.fragment_program.is_none();
        if program_changed || parameters_changed || fragment_program_missing {
            if !input.parameters_preloaded
                && program.entity_instruction_count() != 0
                && !style.parameters().is_empty()
            {
                queue.write_buffer(
                    input.parameter_buffer,
                    u64::from(input.parameter_offset)
                        .saturating_mul(std::mem::size_of::<[f32; 4]>() as u64),
                    bytemuck::cast_slice(style.parameters()),
                );
            }
            if fragment_program_missing {
                let buffer = device.create_buffer(&BufferDesc {
                    label: "fragment visual program",
                    size: std::mem::size_of::<VisualFragmentProgram>() as u64,
                    usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
                })?;
                let mut packed = VisualFragmentProgram::default();
                program.write_gpu_instructions(&mut self.instruction_scratch);
                packed.instructions[..self.instruction_scratch.len()]
                    .copy_from_slice(&self.instruction_scratch);
                packed.parameters[..style.parameters().len()].copy_from_slice(style.parameters());
                queue.write_buffer(&buffer, 0, bytemuck::bytes_of(&packed));
                self.fragment_program = Some(buffer);
            } else if let Some(buffer) = &self.fragment_program {
                if program_changed {
                    program.write_gpu_instructions(&mut self.instruction_scratch);
                    queue.write_buffer(buffer, 0, bytemuck::cast_slice(&self.instruction_scratch));
                }
                if parameters_changed
                    && program.fragment_instruction_count() != 0
                    && !style.parameters().is_empty()
                {
                    queue.write_buffer(
                        buffer,
                        FRAGMENT_PARAMETER_OFFSET,
                        bytemuck::cast_slice(style.parameters()),
                    );
                }
            }
            self.parameter_fingerprint = style.parameter_fingerprint();
        }
        self.program_fingerprint = program.fingerprint();
        Ok(fragment_program_missing)
    }

    fn sync_results(&mut self, device: &D, result_words: u32) -> Result<bool, RenderError> {
        let result_bytes = u64::from(result_words.max(1)) * 4;
        if self.results.is_some() && result_bytes <= self.result_capacity {
            return Ok(false);
        }
        ensure_upload_buffer(
            device,
            "visual entity results",
            result_bytes,
            &mut self.results,
            &mut self.result_capacity,
        )?;
        Ok(true)
    }

    fn sync_config(
        &mut self,
        input: &VisualSync<'_, D>,
        program: &molgfx_core::VisualProgram,
    ) -> Result<bool, RenderError> {
        let Some(style) = input.style else {
            return Ok(false);
        };
        let config = styled_config(&VisualConfigInput {
            program,
            style,
            base: input.base,
            entity_count: input.entity_count,
            result_count: input.result_count,
            property_offsets: input.property_offsets,
            attribute_layouts: input.attribute_layouts,
            property_end_offsets: input.property_end_offsets,
            property_alphas: input.property_alphas,
            time_seconds: input.time_seconds,
            program_offset: input.program_offset,
            parameter_offset: input.parameter_offset,
        });
        let mut changed = false;
        if self.config.is_none() {
            self.config = Some(input.device.create_buffer(&BufferDesc {
                label: "visual configuration",
                size: std::mem::size_of::<VisualConfig>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?);
            self.binding_revision = self.binding_revision.wrapping_add(1);
            changed = true;
        }
        if config != self.config_value || changed {
            if let Some(buffer) = &self.config {
                input
                    .queue
                    .write_buffer(buffer, 0, bytemuck::bytes_of(&config));
            }
            self.config_value = config;
            changed = true;
        }
        Ok(changed)
    }

    fn ensure_fallback(&mut self, device: &D, queue: &D::Queue) -> Result<bool, RenderError> {
        let mut changed = false;
        // The disabled path is intentionally allocation-only after its first
        // frame.  None of these buffers is read while `visual_enabled == 0`,
        // so rewriting zero records on every unrelated timeline/clip update
        // would turn a no-op style into steady-state queue traffic.
        let result_bytes = 4_u64;
        if self.results.is_none() || result_bytes > self.result_capacity {
            ensure_upload_buffer(
                device,
                "visual entity results",
                result_bytes,
                &mut self.results,
                &mut self.result_capacity,
            )?;
            changed = true;
        }
        let config = VisualConfig::default();
        let config_created = self.config.is_none();
        if config_created {
            self.config = Some(device.create_buffer(&BufferDesc {
                label: "visual configuration",
                size: std::mem::size_of::<VisualConfig>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?);
            changed = true;
        }
        if self.fragment_program.is_none() {
            let buffer = device.create_buffer(&BufferDesc {
                label: "disabled fragment visual program",
                size: std::mem::size_of::<VisualFragmentProgram>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?;
            queue.write_buffer(
                &buffer,
                0,
                bytemuck::bytes_of(&VisualFragmentProgram::default()),
            );
            self.fragment_program = Some(buffer);
            changed = true;
        }
        if config_created || self.config_value != config {
            if let Some(buffer) = &self.config {
                queue.write_buffer(buffer, 0, bytemuck::bytes_of(&config));
            }
            self.config_value = config;
            changed = true;
        }
        if changed {
            self.binding_revision = self.binding_revision.wrapping_add(1);
        }
        Ok(changed)
    }
}

include!("visual_config.rs");
