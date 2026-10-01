//! Deterministic benchmark measurements and acceptance summaries.

#![forbid(unsafe_code)]

pub mod fixtures;
mod metrics;
pub mod process;
mod profile_metadata;
pub mod reader;
pub mod resources;
pub mod synthetic;

pub use metrics::{
    CumulativeTelemetry, FrameSample, FrameSummary, HeapMeasurement, MetricsError, measure_heap,
    summarize,
};
pub use profile_metadata::profile_metadata_json;
