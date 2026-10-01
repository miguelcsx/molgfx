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
}
