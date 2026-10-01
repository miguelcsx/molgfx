//! Scene-level entry points for the size- and focus-aware default compositions.

use super::Scene;
use crate::error::Error;
use crate::id::{RepresentationId, StructureId};
use crate::representation::Selection;

impl Scene {
    /// Adds the default representations for `structure`, chosen from its size.
    ///
    /// See [`crate::preset::auto_representations`] for the policy.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown structure or an invalid representation.
    pub fn add_auto(&mut self, structure: StructureId) -> Result<Vec<RepresentationId>, Error> {
        let Some(source) = self.structures.get(&structure) else {
            return Err(Error::InvalidSpec(format!(
                "structure {} is not part of this scene",
                structure.0
            )));
        };
        let forms = crate::preset::auto_representations(source, structure)?;
        forms.into_iter().map(|form| self.add(form)).collect()
    }

    /// Adds the pocket-and-pose composition around `focus` in one patch.
    ///
    /// See [`crate::preset::pocket_representations`] for the bands. The focus
    /// query is also made the scene's semantic focus, so the camera frames it.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown structure, an invalid style or a focus
    /// query that selects nothing.
    pub fn add_pocket(
        &mut self,
        structure: StructureId,
        focus: impl Into<Selection>,
        style: crate::preset::PocketStyle,
    ) -> Result<Vec<RepresentationId>, Error> {
        let focus = focus.into();
        let Some(source) = self.structures.get(&structure) else {
            return Err(Error::InvalidSpec(format!(
                "structure {} is not part of this scene",
                structure.0
            )));
        };
        let selected = source
            .select_compiled(&*focus.compiled()?)
            .map_err(|error| Error::InvalidSpec(error.to_string()))?;
        let Ok(rows) = u32::try_from(source.coordinates().len()) else {
            return Err(Error::InvalidSpec(
                "the structure has more atoms than a selection can address".to_owned(),
            ));
        };
        if selected.count(rows) == 0 {
            return Err(Error::InvalidSpec(
                "the pocket focus selects no atoms".to_owned(),
            ));
        }
        let forms = crate::preset::pocket_representations(&focus, structure, style)?;
        let ids = forms
            .into_iter()
            .map(|form| self.add(form))
            .collect::<Result<Vec<_>, _>>()?;
        self.focus(focus)?;
        Ok(ids)
    }

    /// Overlays several structures of this scene, the heaviest most opaque.
    ///
    /// See [`crate::preset::ensemble_representations`] for the opacity policy.
    /// Every member is checked before any representation is added, so a bad
    /// member leaves the scene unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown member structure or an invalid ensemble.
    pub fn add_ensemble(
        &mut self,
        members: &[crate::preset::EnsembleMember],
        style: crate::preset::EnsembleStyle,
    ) -> Result<Vec<RepresentationId>, Error> {
        for member in members {
            if !self.structures.contains_key(&member.structure) {
                return Err(Error::InvalidSpec(format!(
                    "structure {} is not part of this scene",
                    member.structure.0
                )));
            }
        }
        let forms = crate::preset::ensemble_representations(members, style)?;
        forms.into_iter().map(|form| self.add(form)).collect()
    }

    /// Draws `target` as a cartoon coloured by a bound property, fading small
    /// values to context.
    ///
    /// See [`crate::preset::difference_visual`] for the mapping.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown structure, an invalid style, or a
    /// property that is not bound to this scene.
    pub fn add_difference(
        &mut self,
        structure: StructureId,
        target: impl Into<Selection>,
        property: crate::ScalarProperty,
        style: &crate::preset::DifferenceStyle,
    ) -> Result<RepresentationId, Error> {
        if !self.structures.contains_key(&structure) {
            return Err(Error::InvalidSpec(format!(
                "structure {} is not part of this scene",
                structure.0
            )));
        }
        let visual = crate::preset::difference_visual(property, style)?;
        self.add(
            crate::rep::cartoon(target.into())
                .visual(visual)
                .structure(structure),
        )
    }
}
