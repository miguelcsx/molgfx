//! Read access to a scene.

use super::Scene;
use crate::id::StructureId;
use crate::spec::SceneSpec;
use std::collections::BTreeMap;

impl Scene {
    /// The scene's molecular sources, keyed by structure.
    ///
    /// Read-only: the coordinates and topology behind these sources are what
    /// every representation draws, so a caller may inspect them to decide what
    /// to author but never replace them in place. Adding a source goes through
    /// [`Self::add_source`], which announces the change as a patch.
    #[must_use]
    pub const fn sources(&self) -> &BTreeMap<StructureId, molgfx_core::MolecularSource> {
        &self.structures
    }

    /// The specification as it is, including the revision patches must name.
    #[must_use]
    pub const fn spec(&self) -> &SceneSpec {
        &self.spec
    }

    /// Clones the small semantic description without copying molecular data.
    #[must_use]
    pub fn to_spec(&self) -> SceneSpec {
        self.spec.clone()
    }

    /// Current semantic revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.spec.revision
    }
}
