//! Semantic points that labels, measurements and interactions attach to.

use crate::id::StructureId;
use crate::representation::Selection;
use serde::{Deserialize, Serialize};

/// A semantic point used by labels, measurements, and explicit interactions.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Anchor {
    /// Fixed world-space point.
    World {
        /// Position in the scene's world coordinate system.
        position: [f32; 3],
    },
    /// Centroid of a molecular query on one structure.
    Selection {
        /// Molecular source containing the query target.
        structure: StructureId,
        /// Query whose centroid defines the anchor.
        selection: Selection,
    },
}
