//! GPU visibility compaction and indirect argument generation.

mod paged;
mod pass;
mod pipeline;
mod relations;
mod structure_pipelines;
mod timeline;

pub(crate) use pass::{CullLayouts, CullPass};
