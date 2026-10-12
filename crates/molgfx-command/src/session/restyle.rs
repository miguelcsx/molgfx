//! Layer form changes retain identity, authored appearance and undo history.

use super::{Outcome, Session, history::Entry, plan::scene_error};
use crate::error::{CommandError, CommandErrors, ErrorKind};
use crate::ir::{Form, Look, Name};
use molgfx_scene::{PatchOperation, Scene, ScenePatch};

impl Session {
    /// Checks whether a requested form produces geometry on an existing layer.
    ///
    /// # Errors
    /// Returns an error for an unknown layer, invalid form or selection.
    pub fn layer_form_applicability(
        &self,
        scene: &Scene,
        name: &Name,
        form: Form,
    ) -> Result<molgfx_scene::rep::RepresentationApplicability, CommandErrors> {
        let requested = self.layer_form_specification(scene, name, form)?;
        Ok(scene
            .representation_applicability(requested)
            .map_err(|error| scene_error(&error))?)
    }

    /// Builds an exact form over an existing layer without extracting geometry.
    ///
    /// The returned immutable value can be reviewed against a shared molecular
    /// source on a worker, keeping expensive topology queries off a UI thread.
    ///
    /// # Errors
    /// Returns an error for an unknown layer or missing scene representation.
    pub fn layer_form_specification(
        &self,
        scene: &Scene,
        name: &Name,
        form: Form,
    ) -> Result<molgfx_scene::RepresentationSpec, CommandErrors> {
        let layer = self.state.spec.layers.get(name).ok_or_else(|| {
            CommandError::new(
                ErrorKind::UnknownSymbol,
                format!("there is no layer {name}"),
            )
        })?;
        let current = scene.spec().representations.get(&layer.id).ok_or_else(|| {
            CommandError::new(
                ErrorKind::UnknownSymbol,
                format!("there is no layer {name}"),
            )
        })?;
        Ok(form.specification(
            current.target().clone(),
            Look {
                structure: layer.structure,
                color: None,
                opacity: None,
            },
        ))
    }

    /// Changes a named layer's form atomically, preserving target and appearance.
    ///
    /// # Errors
    /// Returns an error for a missing layer, unsuitable target or invalid form.
    /// The existing layer remains intact when the requested form cannot draw it.
    pub fn restyle(
        &mut self,
        scene: &mut Scene,
        name: &Name,
        form: Form,
    ) -> Result<Outcome, CommandErrors> {
        self.restyle_bound(scene, name, form, None)
    }

    /// Changes a layer and installs its analytical result in the same undoable operation.
    ///
    /// # Errors
    /// Rejects conflicting scientific data or invalid forms without changing the scene or history.
    pub fn restyle_with_molecular_surface(
        &mut self,
        scene: &mut Scene,
        name: &Name,
        form: Form,
        binding: molgfx_scene::MolecularSurfaceBinding,
    ) -> Result<Outcome, CommandErrors> {
        self.restyle_bound(scene, name, form, Some(binding))
    }

    fn restyle_bound(
        &mut self,
        scene: &mut Scene,
        name: &Name,
        form: Form,
        binding: Option<molgfx_scene::MolecularSurfaceBinding>,
    ) -> Result<Outcome, CommandErrors> {
        self.sync(scene);
        let layer = self.state.spec.layers.get(name).ok_or_else(|| {
            CommandError::new(
                ErrorKind::UnknownSymbol,
                format!("there is no layer {name}"),
            )
        })?;
        let current = scene.spec().representations.get(&layer.id).ok_or_else(|| {
            CommandError::new(
                ErrorKind::UnknownSymbol,
                format!("there is no layer {name}"),
            )
        })?;
        let requested = form.specification(
            current.target().clone(),
            Look {
                structure: layer.structure,
                color: None,
                opacity: None,
            },
        );
        let representation = scene
            .prepare_representation_form(layer.id, requested)
            .map_err(|error| scene_error(&error))?;
        let patch = ScenePatch {
            base_revision: scene.revision(),
            operations: vec![PatchOperation::ReplaceRepresentation {
                id: layer.id,
                representation,
            }],
        };
        let inverse = patch
            .inverse(scene.spec())
            .map_err(|error| scene_error(&error))?;
        match binding {
            Some(binding) => scene.apply_with_molecular_surface(&patch, binding),
            None => scene.apply(&patch),
        }
        .map_err(|error| scene_error(&error))?;
        let before = self.state.spec.clone();
        if let Some(layer) = self.state.spec.layers.get_mut(name) {
            layer.form = form;
        }
        self.history.record(Entry {
            label: format!("restyle @{name}"),
            forward: patch.operations.clone(),
            inverse: inverse.operations,
            before,
            after: self.state.spec.clone(),
        });
        self.revision = Some(scene.revision());
        Ok(Outcome {
            patch: Some(patch),
            revision: scene.revision(),
            messages: Vec::new(),
        })
    }
}
