//! Transported analytical results use the same atomic binding boundary as native providers.
use crate::{Error, MolecularSurfaceBinding, MolecularSurfaceData, RepresentationId, Scene};

impl Scene {
    /// Whether an analytical representation has geometry for the displayed molecular sample.
    /// Stale assets stay retained and become drawable again when their exact sample returns.
    ///
    /// # Errors
    /// Refuses an unknown representation or a recipe that is not an analytical surface.
    pub fn molecular_surface_is_current(&self, identity: RepresentationId) -> Result<bool, Error> {
        let spec = self
            .spec
            .representations
            .get(&identity)
            .ok_or(Error::MissingId)?;
        let crate::representation::form::RepresentationFormSpec::MolecularSurface {
            source, ..
        } = &spec.form
        else {
            return Err(Error::InvalidSpec(
                "representation is not an analytical surface".to_owned(),
            ));
        };
        let structure = spec.structure_id().ok_or(Error::MissingId)?;
        let placed = self
            .structure_handle(structure)
            .and_then(|handle| self.resolved.structure(handle))
            .ok_or(Error::MissingId)?;
        Ok(self
            .overlay_bindings
            .molecular_surface(&source.content_hash, structure)
            .is_some_and(|binding| binding.geometry.validate_pose(placed).is_ok()))
    }
    pub(super) fn validate_surface_pose(
        &self,
        binding: &MolecularSurfaceBinding,
    ) -> Result<(), Error> {
        let placed = self
            .structure_handle(binding.structure)
            .and_then(|handle| self.resolved.structure(handle))
            .ok_or(Error::MissingId)?;
        binding.geometry.validate_pose(placed)?;
        Ok(())
    }
    /// Captures the current molecular coordinate sample in O(1), retaining shared storage.
    ///
    /// # Errors
    /// Refuses an unbound molecular structure.
    pub fn molecular_pose(
        &self,
        structure: crate::StructureId,
    ) -> Result<crate::source::MolecularPose, Error> {
        let placed = self
            .structure_handle(structure)
            .and_then(|handle| self.resolved.structure(handle))
            .ok_or(Error::MissingId)?;
        Ok(crate::source::MolecularPose::capture(placed))
    }
    /// Installs scientific geometry and applies its authored patch as one scene revision.
    /// Failed validation consumes neither the binding identity nor the previous rendering.
    ///
    /// # Errors
    /// Rejects stale poses, duplicate sources, conflicting revisions or invalid resulting recipes.
    pub fn apply_with_molecular_surface(
        &mut self,
        patch: &crate::ScenePatch,
        binding: MolecularSurfaceBinding,
    ) -> Result<(), Error> {
        let molecule = self
            .structures
            .get(&binding.structure)
            .ok_or(Error::MissingId)?;
        binding.validate_molecule(molecule)?;
        self.validate_surface_pose(&binding)?;
        let mut bindings = self.overlay_bindings.clone();
        bindings.insert_molecular_surface(binding)?;
        self.apply_with_overlay_bindings(patch, Some(bindings))
    }

    /// Binds scientific output to a pending Surf or MSMS representation.
    /// The caller supplies the actual provider metadata; it is checked against the recipe.
    ///
    /// # Errors
    /// Rejects unknown representations, non-surface recipes, malformed geometry,
    /// absent atom owners or contradictory scientific controls without changing the scene.
    pub fn bind_molecular_surface_data(
        &mut self,
        identity: RepresentationId,
        data: MolecularSurfaceData,
    ) -> Result<(), Error> {
        self.bind_surface_data(identity, data, None)
    }

    /// Installs transported scientific output at the exact captured calculation sample.
    ///
    /// # Errors
    /// Rejects stale or foreign poses and contradictory provider metadata atomically.
    pub fn bind_molecular_surface_data_at_pose(
        &mut self,
        identity: RepresentationId,
        data: MolecularSurfaceData,
        pose: &crate::source::MolecularPose,
    ) -> Result<(), Error> {
        self.bind_surface_data(identity, data, Some(pose))
    }

    fn bind_surface_data(
        &mut self,
        identity: RepresentationId,
        data: MolecularSurfaceData,
        pose: Option<&crate::source::MolecularPose>,
    ) -> Result<(), Error> {
        let spec = self
            .spec
            .representations
            .get(&identity)
            .ok_or_else(|| Error::InvalidSpec("unknown representation identity".to_owned()))?;
        let crate::representation::form::RepresentationFormSpec::MolecularSurface {
            source, ..
        } = &spec.form
        else {
            return Err(Error::InvalidSpec(
                "representation is not an analytical surface".to_owned(),
            ));
        };
        let structure = spec
            .structure_id()
            .ok_or_else(|| Error::InvalidSpec("surface has no molecular structure".to_owned()))?;
        let molecule = self
            .structures
            .get(&structure)
            .ok_or_else(|| Error::InvalidSpec("surface structure is not bound".to_owned()))?;
        let geometry = data.mesh.into_geometry()?;
        let binding = MolecularSurfaceBinding::new(
            source.clone(),
            structure,
            molecule,
            data.method,
            data.controls,
            geometry,
        )?;
        let binding = match pose {
            Some(pose) => binding.at_pose(pose)?,
            None => binding,
        };
        self.bind_molecular_surface(binding)
    }
}
