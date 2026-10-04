//! Named snapshots use the same atomic command and history boundary as text.

use super::{Outcome, Session};
use crate::{Command, CommandErrors, Name};
use molgfx_scene::Scene;

impl Session {
    /// Captures the live scene under a name, replacing an earlier capture.
    ///
    /// # Errors
    ///
    /// Returns an error if the authored scene cannot be captured.
    pub fn snapshot_save(
        &mut self,
        scene: &mut Scene,
        name: Name,
    ) -> Result<Outcome, CommandErrors> {
        self.run(scene, Command::SnapshotSave { name })
    }

    /// Restores a named capture atomically, recording its inverse for undo.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown name, invalid capture or missing assets.
    pub fn snapshot_restore(
        &mut self,
        scene: &mut Scene,
        name: Name,
    ) -> Result<Outcome, CommandErrors> {
        self.run(scene, Command::SnapshotRestore { name })
    }

    /// Removes a named capture without changing the live scene.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot name does not exist.
    pub fn snapshot_remove(
        &mut self,
        scene: &mut Scene,
        name: Name,
    ) -> Result<Outcome, CommandErrors> {
        self.run(scene, Command::SnapshotRemove { name })
    }
}
