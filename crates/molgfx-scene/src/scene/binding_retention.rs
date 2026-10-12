//! Hosts release obsolete immutable assets after their undo, redo and captures stop using them.

use crate::{DataSource, Scene};

impl Scene {
    /// Releases assets referenced by neither the current document nor `retained`.
    /// Pass identities from reachable undo/redo operations and saved captures.
    /// This changes no authored revision and leaves current rendering intact.
    /// Staged inputs for future edits must also be included explicitly.
    pub fn prune_runtime_bindings<'a>(
        &mut self,
        retained: impl IntoIterator<Item = &'a DataSource>,
    ) {
        let sources = self
            .spec
            .runtime_sources()
            .map(|source| source.content_hash.clone())
            .chain(
                retained
                    .into_iter()
                    .map(|source| source.content_hash.clone()),
            )
            .collect();
        self.overlay_bindings.retain_sources(&sources);
    }
}
