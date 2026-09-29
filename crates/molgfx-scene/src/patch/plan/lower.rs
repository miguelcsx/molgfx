//! Validation and physical lowering of a local patch, before any mutation.

use super::{Change, LocalPatchPlan, PatchInputs};
use crate::error::Error;
use crate::id::RepresentationId;
use crate::representation::form::RepresentationSpec;
use molgfx_core::Representation;
use std::collections::BTreeMap;

impl LocalPatchPlan {
    /// Validates every staged value and computes the physical records.
    ///
    /// Queries a retarget or an appearance rule needs are evaluated here,
    /// through the scene's evaluated-rows cache, so commit never evaluates.
    pub(super) fn validate_and_lower(&mut self, inputs: PatchInputs<'_>) -> Result<(), Error> {
        for selection in self.interactions.selections() {
            let _ = selection.fingerprint()?;
        }
        if let Change::Set(camera) = &self.camera {
            crate::spec::validate_camera(*camera)?;
        }
        let structure_handles = inputs.structure_handles();
        self.targets
            .prepare(&self.representations, inputs, &structure_handles)?;
        self.appearance.prepare(inputs, &structure_handles)?;
        let channels = self.interactions.channel_names(inputs.spec);
        self.physical.reserve(self.representations.len());
        for (id, spec) in &self.representations {
            let handle = inputs.handles.get(id).copied().ok_or(Error::MissingId)?;
            let mut representation = inputs
                .scene
                .representation(handle)
                .cloned()
                .ok_or(Error::MissingId)?;
            let lowering = crate::spec::lowering::Lowering {
                properties: inputs.properties,
                channels: &channels,
            };
            if self.visual_replacements.contains(id) {
                let appearance = spec.prepare_appearance(lowering)?;
                let resolved = appearance.apply(&mut representation);
                let _ = self.visual_updates.insert(*id, resolved);
            } else {
                representation.material.opacity = spec.common.opacity;
                if self.color_updates.contains(id) {
                    representation.color = spec.native_color(lowering)?;
                }
                self.apply_parameters(*id, spec, inputs.visuals, &mut representation)?;
            }
            representation.visible = spec.common.visible;
            self.physical.push((*id, handle, representation));
        }
        Ok(())
    }

    fn apply_parameters(
        &self,
        id: RepresentationId,
        spec: &RepresentationSpec,
        visuals: &BTreeMap<RepresentationId, crate::visual::ResolvedVisual>,
        representation: &mut Representation,
    ) -> Result<(), Error> {
        let Some(names) = self.parameter_updates.get(&id) else {
            return Ok(());
        };
        let source =
            spec.common.visual.as_ref().ok_or_else(|| {
                Error::InvalidSpec("visual parameters require a visual style".into())
            })?;
        let resolved = visuals
            .get(&id)
            .filter(|value| value.matches(source))
            .ok_or_else(|| {
                Error::InvalidSpec("resolved visual metadata does not match its source".into())
            })?;
        let style = representation.visual.as_mut().ok_or_else(|| {
            Error::InvalidSpec("resolved representation has no visual style".into())
        })?;
        for name in names {
            let binding = resolved.parameter(name).ok_or_else(|| {
                Error::InvalidSpec(format!("visual parameter '{name}' is not declared"))
            })?;
            let mut value = &binding.default;
            if let Some(replacement) = spec.common.parameters.get(name) {
                value = replacement;
            }
            apply_parameter(style, binding.slot, value)?;
        }
        Ok(())
    }
}

fn apply_parameter(
    style: &mut molgfx_core::VisualStyle,
    slot: usize,
    value: &crate::ParameterValue,
) -> Result<(), Error> {
    let invalid = || Error::InvalidSpec(format!("visual parameter slot {slot} has the wrong type"));
    match value {
        crate::ParameterValue::Scalar(value) => {
            let parameter = style.program().scalar_parameter(slot).ok_or_else(invalid)?;
            style
                .set_scalar(parameter, *value)
                .map_err(molgfx_core::CoreError::from)?;
        }
        crate::ParameterValue::Color(value) => {
            let parameter = style.program().color_parameter(slot).ok_or_else(invalid)?;
            style
                .set_color(parameter, value.to_linear_f32())
                .map_err(molgfx_core::CoreError::from)?;
        }
        crate::ParameterValue::Vector(value) => {
            let parameter = style.program().vector_parameter(slot).ok_or_else(invalid)?;
            style
                .set_vector(parameter, *value)
                .map_err(molgfx_core::CoreError::from)?;
        }
    }
    Ok(())
}
