//! Detail selection and property mappings.

pub(crate) mod lod;
pub(crate) mod mapping;

pub use lod::{LodLevel, LodPolicy};
pub use mapping::{MappingError, PropertyMapping};
