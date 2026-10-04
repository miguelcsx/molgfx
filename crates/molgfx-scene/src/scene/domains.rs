//! A source assembly selected for one structure.

use crate::StructureId;
use serde::{Deserialize, Serialize};

/// A crystallographic assembly selected for one source structure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssemblyChoice {
    /// Source structure for the selected assembly.
    pub structure: StructureId,
    /// Stable source assembly identifier.
    pub assembly_id: Box<str>,
}
