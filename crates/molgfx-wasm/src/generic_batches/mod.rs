//! Typed-array browser adapters for generic scene tables.

mod attributes;
mod batches;
mod handles;
mod relations;
mod template;
mod values;

pub use handles::{
    WebAttributeHandle, WebInstanceBatchHandle, WebPointBatchHandle, WebRelationBatchHandle,
    WebRowDomain,
};
pub use template::WebAnalyticTemplate;
pub(crate) use values::{core_error, vec3_rows};
