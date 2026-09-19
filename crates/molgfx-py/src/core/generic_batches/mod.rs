//! Contiguous-array Python adapters for generic scene tables.

pub(crate) mod anchors;
pub(crate) mod arrays;
pub(crate) mod attributes;
pub(crate) mod instances;
pub(crate) mod model;
pub(crate) mod points;
pub(crate) mod primitives;
pub(crate) mod relations;
pub(crate) mod template;

pub(crate) use model::{PyRelationPattern, PyRowDomain};
pub(crate) use template::PyAnalyticTemplate;
