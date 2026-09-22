//! Atomic mutation and inspection operations for a live scene.

use super::{Resolution, Scene};
use crate::ScenePatch;
use crate::SceneTransaction;
use crate::error::{Error, PatchError};
use crate::patch::plan::{PatchInputs, PatchPlan};
use crate::scene::runtime::{canonical_selection_count, next_representation_id};

impl Scene {
    /// Applies all operations or leaves the scene unchanged.
    ///
    /// # Errors
    ///
    /// Returns a revision conflict, missing-ID, validation, or renderer error.
    pub fn apply(&mut self, patch: &ScenePatch) -> Result<(), Error> {
        if patch.base_revision != self.spec.revision {
            return Err(PatchError::RevisionConflict {
                expected: patch.base_revision,
                actual: self.spec.revision,
            }
            .into());
        }
        let plan = PatchPlan::prepare(
            PatchInputs {
                spec: &self.spec,
                scene: &self.resolved,
                handles: &self.representations,
                visuals: &self.visuals,
                properties: &self.properties,
                structures: &self.structures,
                property_bindings: &self.property_bindings,
            },
            patch,
        )?;
        match plan {
            PatchPlan::Structural { spec, resolution } => {
                let Resolution {
                    scene,
                    representations,
                    selections,
                    visuals,
                    properties,
                } = *resolution;
                self.spec = *spec;
                self.resolved = scene;
                self.representations = representations;
                self.selections = selections;
                self.visuals = visuals;
                self.properties = properties;
                self.next_representation = next_representation_id(&self.spec)?;
            }
            PatchPlan::Local(plan) => {
                plan.commit(&mut self.spec, &mut self.resolved, &mut self.visuals)?;
            }
        }
        Ok(())
    }

    /// Collects several semantic changes into one revision and patch.
    ///
    /// # Errors
    ///
    /// Returns the edit callback error or an atomic patch application error.
    pub fn transaction<F>(&mut self, edit: F) -> Result<ScenePatch, Error>
    where
        F: FnOnce(&mut SceneTransaction) -> Result<(), Error>,
    {
        let mut transaction = SceneTransaction {
            patch: ScenePatch::empty(self.revision()),
        };
        edit(&mut transaction)?;
        let patch = transaction.patch;
        self.apply(&patch)?;
        Ok(patch)
    }

    /// Deterministic semantic plan summary.
    #[must_use]
    pub fn explain(&self) -> String {
        let visible = self
            .spec
            .representations
            .values()
            .filter(|representation| representation.common.visible)
            .count();
        format!(
            "SceneSpec\nrevision: {}\nstructures: {}\nrepresentations: {} ({} visible)\ncanonical selections: {}",
            self.spec.revision,
            self.spec.structures.len(),
            self.spec.representations.len(),
            visible,
            canonical_selection_count(&self.spec)
        )
    }

    pub(crate) const fn resolved(&self) -> &molgfx_core::Scene {
        &self.resolved
    }

    /// Camera framing the focused subject, or the whole scene when none is set.
    ///
    /// An explicit camera always wins; only its aspect ratio is adapted to the
    /// render target.
    #[must_use]
    pub fn framing_camera(&self, aspect: f32) -> molgfx_math::Camera {
        let Some(mut camera) = self.spec.camera else {
            let bounds = match self.focus_bounds() {
                Some(bounds) => bounds,
                None => self.resolved.world_aabb(),
            };
            return molgfx_math::Camera::framing_aabb(&bounds, aspect);
        };
        camera.projection.set_aspect(aspect);
        camera
    }

    /// World-space bounds of the focused subject, when a focus is set.
    #[must_use]
    pub fn focus_bounds(&self) -> Option<molgfx_math::Aabb> {
        crate::scene::interaction::focus_bounds(&self.resolved)
    }
}
