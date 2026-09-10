//! Shared visual-program lowering for generic homogeneous row domains.

use super::slot_types::SlotShading;
use super::visual::{VisualBase, VisualCullEntries, VisualSlot, VisualSync};
use super::visual_parameters::VisualParameterTable;
use super::visual_programs::VisualProgramTable;
use super::visual_properties::VisualPropertyTable;
use crate::error::RenderError;
use pdviewx_core::{RowDomain, Scene, VisualOutput};
use pdviewx_gpu::Device;
use pdviewx_math::Rgba8;

pub(super) struct GenericVisualResources<'a, D: Device> {
    pub(super) programs: &'a VisualProgramTable<D>,
    pub(super) parameters: &'a VisualParameterTable<D>,
    pub(super) properties: &'a VisualPropertyTable<D>,
    pub(super) fallback: VisualCullEntries<'a, D>,
    pub(super) parameter_slot_base: usize,
    pub(super) time_seconds: f32,
}

pub(super) struct GenericVisualTarget<'a> {
    pub(super) scene: &'a Scene,
    pub(super) domain: RowDomain,
    pub(super) color: Rgba8,
    pub(super) opacity: f32,
    pub(super) row_count: usize,
}

impl<D: Device> Copy for GenericVisualResources<'_, D> {}

impl<D: Device> Clone for GenericVisualResources<'_, D> {
    fn clone(&self) -> Self {
        *self
    }
}

#[derive(Debug)]
pub(super) struct GenericVisualState<D: Device> {
    slot: Option<VisualSlot<D>>,
    shading: SlotShading,
    translucent: bool,
}

impl<D: Device> GenericVisualState<D> {
    pub(super) fn new() -> Self {
        Self {
            slot: None,
            shading: SlotShading::default(),
            translucent: false,
        }
    }

    pub(super) fn sync(
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
            time_seconds: resources.time_seconds,
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

    pub(super) fn sync_uniform(
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
            time_seconds: resources.time_seconds,
            entity_count: row_count,
            result_count: row_count,
            base: VisualBase::rgba8_opacity(color, opacity),
        })?;
        self.shading = SlotShading::default();
        self.translucent = color.a < u8::MAX;
        Ok(changed)
    }

    pub(super) fn entries<'a>(
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
        slot.cull_entries(instructions, parameters, properties)
            .map_or(resources.fallback, |entries| entries)
    }

    pub(super) const fn shading(&self) -> SlotShading {
        self.shading
    }

    pub(super) const fn is_translucent(&self) -> bool {
        self.translucent
    }

    pub(super) fn has_cull_results(&self) -> bool {
        self.slot.as_ref().is_some_and(VisualSlot::has_cull_results)
    }

    pub(super) fn has_shading_results(&self) -> bool {
        self.slot
            .as_ref()
            .is_some_and(VisualSlot::has_shading_results)
    }

    pub(super) fn binding_revision(&self) -> u64 {
        self.slot.as_ref().map_or(0, VisualSlot::binding_revision)
    }
}

impl<D: Device> GenericVisualResources<'_, D> {
    pub(super) const fn binding_revision(self) -> [u64; 3] {
        [
            self.programs.binding_revision(),
            self.parameters.binding_revision(),
            self.properties.binding_revision(),
        ]
    }
}

fn missing_visual_buffer() -> pdviewx_gpu::GpuError {
    pdviewx_gpu::GpuError::LimitExceeded {
        resource: "generic visual arena",
        limit: 0,
    }
}
