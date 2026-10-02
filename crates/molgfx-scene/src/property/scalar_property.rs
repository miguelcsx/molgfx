//! The handle a visual program names to read a bound property.

use crate::StructureId;
use serde::{Deserialize, Serialize};

/// Stable reference shared by color and visual expressions.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct ScalarProperty {
    structure: StructureId,
    name: Box<str>,
}

impl ScalarProperty {
    /// The reference to `name` owned by `structure`.
    pub(super) const fn new(structure: StructureId, name: Box<str>) -> Self {
        Self { structure, name }
    }

    /// Owning molecular structure.
    #[must_use]
    pub const fn structure(&self) -> StructureId {
        self.structure
    }

    /// Unique scene-level property name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}
