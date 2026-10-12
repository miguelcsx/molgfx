//! Typed external edits share command history and retain immutable scientific input for replay.

use super::{Outcome, Session, history::Entry, plan::scene_error};
use crate::error::CommandErrors;
use molgfx_scene::{HydrogenBondBinding, Scene, ScenePatch};

impl Session {
    /// Borrows both directions of retained undo and redo operations.
    /// Hosts can retain externally owned input and identity bookkeeping while
    /// any reachable history entry still references it.
    pub fn history_operations(&self) -> impl Iterator<Item = &molgfx_scene::PatchOperation> {
        self.history.operations()
    }

    /// Borrows named captures, including previous definitions reachable through history.
    pub fn retained_snapshots(
        &self,
    ) -> impl Iterator<Item = &molgfx_scene::interop::SceneSnapshot> {
        self.state
            .spec
            .snapshots
            .values()
            .map(|snapshot| &snapshot.scene)
            .chain(self.history.snapshots())
    }

    /// Publishes a typed scene patch through the same undo history as commands.
    /// Symbolic names are retained and removed scene identities are forgotten.
    /// # Errors
    /// Invalid patches leave the scene and session unchanged.
    pub fn apply_patch(
        &mut self,
        scene: &mut Scene,
        label: &str,
        patch: &ScenePatch,
    ) -> Result<Outcome, CommandErrors> {
        self.publish(scene, label, patch, None)
    }

    /// Publishes chemical input and its scene patch as one undoable operation.
    /// Immutable input remains available when redo restores its descriptor.
    /// # Errors
    /// Conflicting input or invalid patches leave the scene and history unchanged.
    pub fn apply_with_hydrogen_bonds(
        &mut self,
        scene: &mut Scene,
        label: &str,
        patch: &ScenePatch,
        binding: HydrogenBondBinding,
    ) -> Result<Outcome, CommandErrors> {
        self.publish(scene, label, patch, Some(binding))
    }

    fn publish(
        &mut self,
        scene: &mut Scene,
        label: &str,
        patch: &ScenePatch,
        binding: Option<HydrogenBondBinding>,
    ) -> Result<Outcome, CommandErrors> {
        let inverse = patch
            .inverse(scene.spec())
            .map_err(|error| scene_error(&error))?;
        let mut staged = self.clone();
        staged.sync(scene);
        let before = staged.state.spec.clone();
        match binding {
            Some(binding) => scene.apply_with_hydrogen_bonds(patch, binding),
            None => scene.apply(patch),
        }
        .map_err(|error| scene_error(&error))?;
        staged.state.sync(scene.spec());
        if !patch.operations.is_empty() {
            staged.history.record(Entry {
                label: label.to_owned(),
                forward: patch.operations.clone(),
                inverse: inverse.operations,
                before,
                after: staged.state.spec.clone(),
            });
        }
        staged.revision = Some(scene.revision());
        *self = staged;
        Ok(Outcome {
            patch: Some(patch.clone()),
            revision: scene.revision(),
            messages: Vec::new(),
        })
    }
}
