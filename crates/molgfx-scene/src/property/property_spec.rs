//! The portable description of a bound property.

use crate::{DataSource, StructureId};
use serde::{Deserialize, Serialize};

/// Serializable property metadata; values remain in a runtime binding.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct PropertySpec {
    /// Structure whose atom rows own the values.
    pub structure: StructureId,
    /// Portable bulk-data source.
    pub source: DataSource,
    /// Finite display domain in physical units.
    pub domain: [f32; 2],
    /// Optional non-empty unit symbol.
    pub units: Option<Box<str>>,
}
