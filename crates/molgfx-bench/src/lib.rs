//! Deterministic benchmark measurements and acceptance summaries.

#![forbid(unsafe_code)]

mod fallback;
pub mod fixtures;
pub mod gallery;
pub mod image_compare;
pub mod ladder;
mod metrics;
pub mod process;
mod profile_metadata;
pub mod reader;
pub mod resources;
pub mod synthetic;

pub use fallback::fallback;
pub use metrics::{
    CumulativeTelemetry, FrameSample, FrameSummary, HeapMeasurement, MetricsError, measure_heap,
    summarize,
};
pub use profile_metadata::profile_metadata_json;
