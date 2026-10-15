//! Shared visual-program lowering for generic homogeneous row domains.

use crate::error::RenderError;
use crate::scene_gpu::slot_types::SlotShading;
use crate::scene_gpu::visual::{VisualBase, VisualCullEntries, VisualSlot, VisualSync};
use crate::scene_gpu::visual_parameters::VisualParameterTable;
use crate::scene_gpu::visual_programs::VisualProgramTable;
use crate::scene_gpu::visual_properties::VisualPropertyTable;
use molgfx_core::{RowDomain, Scene, VisualOutput};
use molgfx_gpu::Device;
use molgfx_math::Rgba8;

pub(in crate::scene_gpu) struct GenericVisualResources<'a, D: Device> {
    pub(in crate::scene_gpu) programs: &'a VisualProgramTable<D>,
    pub(in crate::scene_gpu) parameters: &'a VisualParameterTable<D>,
    pub(in crate::scene_gpu) properties: &'a VisualPropertyTable<D>,
    pub(in crate::scene_gpu) fallback: VisualCullEntries<'a, D>,
    pub(in crate::scene_gpu) parameter_slot_base: usize,
    pub(in crate::scene_gpu) time_seconds: f32,
}

pub(in crate::scene_gpu) struct GenericVisualTarget<'a> {
    pub(in crate::scene_gpu) scene: &'a Scene,
    pub(in crate::scene_gpu) domain: RowDomain,
    pub(in crate::scene_gpu) color: Rgba8,
    pub(in crate::scene_gpu) opacity: f32,
    pub(in crate::scene_gpu) row_count: usize,
}

impl<D: Device> Copy for GenericVisualResources<'_, D> {}

impl<D: Device> Clone for GenericVisualResources<'_, D> {
    fn clone(&self) -> Self {
        *self
    }
}

#[derive(Debug)]
pub(in crate::scene_gpu) struct GenericVisualState<D: Device> {
    slot: Option<VisualSlot<D>>,
    shading: SlotShading,
    translucent: bool,
}

impl<D: Device> GenericVisualState<D> {
    pub(in crate::scene_gpu) fn new() -> Self {
        Self {
            slot: None,
            shading: SlotShading::default(),
            translucent: false,
        }
    }

    pub(in crate::scene_gpu) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        target: &GenericVisualTarget<'_>,
        resources: GenericVisualResources<'_, D>,
    ) -> Result<bool, RenderError> {
        let Some(descriptor) = target.scene.domain_visual(target.domain) else {
            return self.sync_uniform(
                device,
                queue,
                target.color,
                target.opacity,
                target.row_count,
                resources,
            );
        };
        let style = descriptor.style();
        let program_offset = resources
            .programs
            .offset(style.program())
            .ok_or_else(missing_visual_buffer)?;
        let parameter_buffer = resources
            .parameters
            .buffer()
            .ok_or_else(missing_visual_buffer)?;
        let descriptor_index = target
            .scene
            .domain_visuals()
            .position(|(candidate, _)| candidate == target.domain)
            .ok_or_else(missing_visual_buffer)?;
        let parameter_slot = resources
            .parameter_slot_base
            .checked_add(descriptor_index)
            .ok_or_else(missing_visual_buffer)?;
        let attributes =
            resources
                .properties
                .offsets_for_domain(target.scene, target.domain, Some(style));
        let slot = self.slot.get_or_insert_with(VisualSlot::new);
        let changed = slot.sync(&VisualSync {
            device,
            queue,
            style: Some(style),
            program_offset,
            parameter_buffer,
            parameter_offset: VisualParameterTable::<D>::offset(parameter_slot),
            parameters_preloaded: false,
            property_offsets: attributes.offsets,
            attribute_layouts: attributes.layouts,
            property_end_offsets: [0; 4],
            property_alphas: [0.0; 4],
            state_offset: 1,
            time_seconds: resources.time_seconds,
            track: molgfx_core::TrackClock::default(),
            entity_count: target.row_count,
            result_count: target.row_count,
            base: VisualBase::rgba8_opacity(target.color, target.opacity),
        })?;
        self.shading = SlotShading::default()
            .with_visual(true)
            .with_fragment_visual(style.program().fragment_instruction_count() != 0);
        self.translucent = target.color.a < u8::MAX
            || style
                .program()
                .output_register(VisualOutput::Opacity)
                .is_some();
        Ok(changed)
    }

    pub(in crate::scene_gpu) fn sync_uniform(
        &mut self,
        device: &D,
        queue: &D::Queue,
        color: Rgba8,
        opacity: f32,
        row_count: usize,
        resources: GenericVisualResources<'_, D>,
    ) -> Result<bool, RenderError> {
        let parameter_buffer = resources
            .parameters
            .buffer()
            .ok_or_else(missing_visual_buffer)?;
        let slot = self.slot.get_or_insert_with(VisualSlot::new);
        let changed = slot.sync(&VisualSync {
            device,
            queue,
            style: None,
            program_offset: 0,
            parameter_buffer,
            parameter_offset: 0,
            parameters_preloaded: false,
            property_offsets: [0; 4],
            attribute_layouts: [0; 4],
            property_end_offsets: [0; 4],
            property_alphas: [0.0; 4],
            state_offset: 1,
            time_seconds: resources.time_seconds,
            track: molgfx_core::TrackClock::default(),
            entity_count: row_count,
            result_count: row_count,
            base: VisualBase::rgba8_opacity(color, opacity),
        })?;
        self.shading = SlotShading::default();
        self.translucent = color.a < u8::MAX;
        Ok(changed)
    }

    pub(in crate::scene_gpu) fn entries<'a>(
        &'a self,
        resources: GenericVisualResources<'a, D>,
    ) -> VisualCullEntries<'a, D> {
        let Some(slot) = &self.slot else {
            return resources.fallback;
        };
        let (Some(instructions), Some(parameters), Some(properties)) = (
            resources.programs.buffer(),
            resources.parameters.buffer(),
            resources.properties.buffer(),
        ) else {
            return resources.fallback;
        };
        crate::fallback(
            slot.cull_entries(instructions, parameters, properties),
            resources.fallback,
        )
    }

    pub(in crate::scene_gpu) const fn shading(&self) -> SlotShading {
        self.shading
    }

    pub(in crate::scene_gpu) const fn is_translucent(&self) -> bool {
        self.translucent
    }

    pub(in crate::scene_gpu) fn has_cull_results(&self) -> bool {
        self.slot.as_ref().is_some_and(VisualSlot::has_cull_results)
    }

    pub(in crate::scene_gpu) fn has_shading_results(&self) -> bool {
        self.slot
            .as_ref()
            .is_some_and(VisualSlot::has_shading_results)
    }

    pub(in crate::scene_gpu) fn binding_revision(&self) -> u64 {
        self.slot.as_ref().map_or(0, VisualSlot::binding_revision)
    }
}

impl<D: Device> GenericVisualResources<'_, D> {
    pub(in crate::scene_gpu) const fn binding_revision(self) -> [u64; 3] {
        [
            self.programs.binding_revision(),
            self.parameters.binding_revision(),
            self.properties.binding_revision(),
        ]
    }
}

fn missing_visual_buffer() -> molgfx_gpu::GpuError {
    molgfx_gpu::GpuError::LimitExceeded {
        resource: "generic visual arena",
        limit: 0,
    }
}
