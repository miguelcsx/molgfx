//! Contiguous-array Python adapters for generic scene tables.

mod arrays;
mod attributes;
mod batches;
mod model;
mod registration;
mod relations;
mod template;

pub(crate) use model::{PyRelationPattern, PyRowDomain};
pub(crate) use registration::register;
pub(crate) use template::PyAnalyticTemplate;
