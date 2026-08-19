//! Representation lookup, visibility and revision state.

use super::{RepresentationHandle, Scene, StoredRepresentation};
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
        let mut value = representation.bind(target);
        if let RepresentationTarget::Volume(handle) = target {
            let range = self
                .volumes
                .get(handle.0)
                .map(|stored| stored.value.range())
                .ok_or(CoreError::StaleHandle)?;
            if value.volume == crate::VolumeStyle::default() {
                value.volume.transfer = crate::VolumeTransferFunction::linear(
                    range,
                    pdviewx_math::Rgba8::opaque(68, 1, 84),
                    pdviewx_math::Rgba8::opaque(253, 231, 37),
                );
                value.params.isolevel = (range[0] + range[1]) * 0.5;
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

    /// Revision key for representation membership and parameters.
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
