//! Registration hub for semantic policy adapters.

#[path = "semantic/dataset.rs"]
mod dataset;
#[path = "semantic/generated_registration.rs"]
mod generated_registration;
#[path = "semantic/generic/mod.rs"]
mod generic;
#[path = "semantic/residency.rs"]
mod residency;
#[path = "semantic/streaming.rs"]
mod streaming;

pub(crate) use dataset::{PyChunkId, PyDatasetCatalog, PyDatasetId};
pub(crate) use residency::{PyResidencyRequest, PyResidencyTicket};
pub(crate) use streaming::{PyLodClusterKey, PyLodLevel};

use pyo3::prelude::*;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    generated_registration::register_dataset(module)?;
    generic::register(module)?;
    generated_registration::register_residency(module)?;
    streaming::register(module)
}
