//! Named snapshot edits share the ordinary transaction and undo history.

use super::{Planner, scene_error};
use crate::error::{CommandError, ErrorKind};
use crate::ir::Name;
use molgfx_scene::PatchOperation;
use molgfx_scene::interop::SceneSnapshot;

impl Planner<'_> {
    pub(super) fn snapshot_save(&mut self, name: &Name) -> Result<(), CommandError> {
        let snapshot = SceneSnapshot::capture(self.transaction.spec());
        snapshot.verify().map_err(|error| scene_error(&error))?;
        let _ = self.state.spec.snapshots.insert(name.clone(), snapshot);
        Ok(())
    }

    pub(super) fn snapshot_restore(&mut self, name: &Name) -> Result<(), CommandError> {
        let snapshot = self
            .state
            .spec
            .snapshots
            .get(name)
            .ok_or_else(|| missing(name))?;
        self.transaction
            .stage(PatchOperation::RestoreSnapshot(Box::new(snapshot.clone())))
            .map_err(|error| scene_error(&error))?;
        self.state.sync(self.transaction.spec());
        Ok(())
    }

    pub(super) fn snapshot_remove(&mut self, name: &Name) -> Result<(), CommandError> {
        if self.state.spec.snapshots.remove(name).is_none() {
            return Err(missing(name));
        }
        Ok(())
    }
}

fn missing(name: &Name) -> CommandError {
    CommandError::new(
        ErrorKind::UnknownSymbol,
        format!("snapshot '{name}' does not exist"),
    )
}
