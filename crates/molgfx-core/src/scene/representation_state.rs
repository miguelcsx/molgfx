//! Representation lookup, visibility and revision state.

use super::{RepresentationHandle, Scene, StoredRepresentation, StoredVolume};
use crate::{
    CoreError, Representation, RepresentationConfig, RepresentationInput, RepresentationKind,
    RepresentationTarget, VolumeHandle,
};

impl Scene {
    /// Adds one declarative representation over a stored or inline selection.
    ///
    /// # Errors
    ///
    /// Fails on an invalid query, stale handle, or unsupported target kind.
    pub fn represent(
        &mut self,
        target: impl Into<RepresentationInput>,
        representation: impl Into<RepresentationConfig>,
    ) -> Result<RepresentationHandle, CoreError> {
        let target = match target.into() {
            RepresentationInput::Stored(handle) => RepresentationTarget::Selection(handle),
            RepresentationInput::Query(query) => {
                RepresentationTarget::Selection(self.select(query)?)
            }
            RepresentationInput::Source(source) => {
                RepresentationTarget::Selection(self.select_str(&source)?)
            }
            RepresentationInput::Volume(handle) => RepresentationTarget::Volume(handle),
            RepresentationInput::Segmentation(handle) => {
                RepresentationTarget::SegmentedVolume(handle)
            }
        };
        let representation = representation.into();
        let kind = representation.kind;
        self.validate_representation_target(target, kind)?;
        if let Some(visual) = representation.visual_style() {
            visual
                .program()
                .validate_compatibility(crate::VisualCompatibility::for_representation(kind))
                .map_err(|error| CoreError::InvalidVisual {
                    summary: error.to_string(),
                })?;
            if !visual_attributes_are_live(self, visual.program()) {
                return Err(CoreError::StaleHandle);
            }
        }
        let mut value = representation.bind(target);
        if let RepresentationTarget::Volume(handle) = target {
            let range = self
                .volumes
                .get(handle.0)
                .map(StoredVolume::range)
                .ok_or(CoreError::StaleHandle)?;
            if value.volume == crate::VolumeStyle::default() {
                value.volume.transfer = crate::VolumeTransferFunction::linear(
                    range,
                    molgfx_math::Rgba8::opaque(68, 1, 84),
                    molgfx_math::Rgba8::opaque(253, 231, 37),
                );
                value.params.isolevel = range[0].midpoint(range[1]);
            }
        }
        self.representation_revision = self.representation_revision.wrapping_add(1);
        let handle = self
            .representations
            .insert(StoredRepresentation { value, revision: 0 });
        Ok(RepresentationHandle(handle))
    }

    fn validate_representation_target(
        &self,
        target: RepresentationTarget,
        kind: RepresentationKind,
    ) -> Result<(), CoreError> {
        match target {
            RepresentationTarget::Selection(handle) => {
                if self.selections.get(handle.0).is_none() {
                    return Err(CoreError::StaleHandle);
                }
                if matches!(
                    kind,
                    RepresentationKind::Volume | RepresentationKind::Segmentation
                ) {
                    return Err(CoreError::Unsupported { kind });
                }
            }
            RepresentationTarget::Volume(handle) => {
                if self.volumes.get(handle.0).is_none() {
                    return Err(CoreError::StaleHandle);
                }
                if kind != RepresentationKind::Volume {
                    return Err(CoreError::Unsupported { kind });
                }
            }
            RepresentationTarget::SegmentedVolume(handle) => {
                if self.segmentations.get(handle.0).is_none() {
                    return Err(CoreError::StaleHandle);
                }
                if kind != RepresentationKind::Segmentation {
                    return Err(CoreError::Unsupported { kind });
                }
            }
        }
        Ok(())
    }

    /// Resolves a representation handle.
    #[must_use]
    pub fn representation(&self, handle: RepresentationHandle) -> Option<&Representation> {
        self.representations
            .get(handle.0)
            .map(|stored| &stored.value)
    }

    /// Resolves a representation mutably and advances its revision.
    pub fn representation_mut(
        &mut self,
        handle: RepresentationHandle,
    ) -> Option<&mut Representation> {
        let stored = self.representations.get_mut(handle.0)?;
        self.representation_revision = self.representation_revision.wrapping_add(1);
        stored.revision = stored.revision.wrapping_add(1);
        Some(&mut stored.value)
    }

    /// Replaces the safe visual style attached to one representation.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an unknown representation or a
    /// style that reads a property the scene no longer holds, and
    /// [`CoreError::InvalidVisual`] when the style writes a channel this
    /// representation cannot honour.
    pub fn set_representation_visual(
        &mut self,
        handle: RepresentationHandle,
        visual: Option<crate::VisualStyle>,
    ) -> Result<(), CoreError> {
        if let Some(style) = &visual {
            let kind = self
                .representation(handle)
                .ok_or(CoreError::StaleHandle)?
                .kind;
            style
                .program()
                .validate_compatibility(crate::VisualCompatibility::for_representation(kind))
                .map_err(|error| CoreError::InvalidVisual {
                    summary: error.to_string(),
                })?;
            if !visual_attributes_are_live(self, style.program()) {
                return Err(CoreError::StaleHandle);
            }
        }
        let stored = self
            .representations
            .get_mut(handle.0)
            .ok_or(CoreError::StaleHandle)?;
        stored.value.visual = visual;
        stored.revision = stored.revision.wrapping_add(1);
        Ok(())
    }

    /// Updates one scalar visual parameter without recompiling the program.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an unknown representation or one
    /// carrying no visual style, and [`CoreError::InvalidVisual`] when the
    /// index or the value is not one the program accepts.
    pub fn set_visual_scalar_parameter(
        &mut self,
        handle: RepresentationHandle,
        index: usize,
        value: f32,
    ) -> Result<(), CoreError> {
        let (visual, revision) = self.visual_style_mut(handle)?;
        let parameter =
            visual
                .program()
                .scalar_parameter(index)
                .ok_or_else(|| CoreError::InvalidVisual {
                    summary: "visual scalar parameter index is invalid".to_owned(),
                })?;
        visual
            .set_scalar(parameter, value)
            .map_err(|error| CoreError::InvalidVisual {
                summary: error.to_string(),
            })?;
        *revision = revision.wrapping_add(1);
        Ok(())
    }

    /// Updates one color visual parameter without recompiling the program.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an unknown representation or one
    /// carrying no visual style, and [`CoreError::InvalidVisual`] when the
    /// index or a component is not one the program accepts.
    pub fn set_visual_color_parameter(
        &mut self,
        handle: RepresentationHandle,
        index: usize,
        value: [f32; 4],
    ) -> Result<(), CoreError> {
        let (visual, revision) = self.visual_style_mut(handle)?;
        let parameter =
            visual
                .program()
                .color_parameter(index)
                .ok_or_else(|| CoreError::InvalidVisual {
                    summary: "visual color parameter index is invalid".to_owned(),
                })?;
        visual
            .set_color(parameter, value)
            .map_err(|error| CoreError::InvalidVisual {
                summary: error.to_string(),
            })?;
        *revision = revision.wrapping_add(1);
        Ok(())
    }

    /// Updates one vector visual parameter without recompiling the program.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an unknown representation or one
    /// carrying no visual style, and [`CoreError::InvalidVisual`] when the
    /// index or a component is not one the program accepts.
    pub fn set_visual_vector_parameter(
        &mut self,
        handle: RepresentationHandle,
        index: usize,
        value: [f32; 3],
    ) -> Result<(), CoreError> {
        let (visual, revision) = self.visual_style_mut(handle)?;
        let parameter =
            visual
                .program()
                .vector_parameter(index)
                .ok_or_else(|| CoreError::InvalidVisual {
                    summary: "visual vector parameter index is invalid".to_owned(),
                })?;
        visual
            .set_vector(parameter, value)
            .map_err(|error| CoreError::InvalidVisual {
                summary: error.to_string(),
            })?;
        *revision = revision.wrapping_add(1);
        Ok(())
    }

    fn visual_style_mut(
        &mut self,
        handle: RepresentationHandle,
    ) -> Result<(&mut crate::VisualStyle, &mut u64), CoreError> {
        let stored = self
            .representations
            .get_mut(handle.0)
            .ok_or(CoreError::StaleHandle)?;
        let visual = stored
            .value
            .visual
            .as_mut()
            .ok_or_else(|| CoreError::InvalidVisual {
                summary: "representation has no visual style".to_owned(),
            })?;
        Ok((visual, &mut stored.revision))
    }

    /// Hides a representation without removing it.
    pub fn hide(&mut self, handle: RepresentationHandle) {
        if let Some(rep) = self.representations.get_mut(handle.0)
            && rep.value.visible
        {
            rep.value.visible = false;
            rep.revision = rep.revision.wrapping_add(1);
            self.representation_revision = self.representation_revision.wrapping_add(1);
        }
    }

    /// Shows a hidden representation.
    pub fn show(&mut self, handle: RepresentationHandle) {
        if let Some(rep) = self.representations.get_mut(handle.0)
            && !rep.value.visible
        {
            rep.value.visible = true;
            rep.revision = rep.revision.wrapping_add(1);
            self.representation_revision = self.representation_revision.wrapping_add(1);
        }
    }

    /// Removes a representation.
    pub fn remove_representation(&mut self, handle: RepresentationHandle) {
        if self.representations.remove(handle.0).is_some() {
            self.representation_revision = self.representation_revision.wrapping_add(1);
        }
    }

    /// Iterates representations in stable slot order.
    pub fn representations(
        &self,
    ) -> impl Iterator<Item = (RepresentationHandle, &Representation)> + '_ {
        self.representations
            .iter()
            .map(|(handle, value)| (RepresentationHandle(handle), &value.value))
    }

    /// Number of live representations.
    #[must_use]
    pub fn representation_count(&self) -> usize {
        self.representations.len()
    }

    /// Revision key for representation membership and topology-affecting edits.
    #[must_use]
    pub fn representation_revision(&self) -> u64 {
        self.representation_revision
    }

    /// Revision of one representation's parameters.
    #[must_use]
    pub fn representation_content_revision(&self, handle: RepresentationHandle) -> Option<u64> {
        self.representations
            .get(handle.0)
            .map(|stored| stored.revision)
    }

    /// Revision key for placed structures.
    #[must_use]
    pub fn structure_revision(&self) -> u64 {
        self.structure_revision
    }

    /// Revision key for density textures.
    #[must_use]
    pub const fn volume_revision(&self) -> u64 {
        self.volume_revision
    }

    /// Revision of one density grid's contents.
    #[must_use]
    pub fn volume_content_revision(&self, handle: VolumeHandle) -> Option<u64> {
        self.volumes.get(handle.0).map(|stored| stored.revision)
    }
}

fn visual_attributes_are_live(scene: &Scene, program: &crate::VisualProgram) -> bool {
    program
        .attributes()
        .iter()
        .all(|reference| match *reference {
            crate::VisualAttributeRef::Attribute { handle, kind } => scene
                .attribute(handle)
                .is_some_and(|attribute| attribute.kind() == kind),
            crate::VisualAttributeRef::LegacyScalar(handle) => {
                scene.atom_property(handle).is_some()
            }
            crate::VisualAttributeRef::Column { .. } => false,
        })
}
