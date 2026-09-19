//! Declarative GS-001..GS-010 acceptance contracts and evidence records.

mod artifact;
mod cli;
mod report;
mod spec;

#[cfg(test)]
#[path = "acceptance_tests.rs"]
mod tests;

pub(crate) use artifact::{dimension_aspect, write_png};
pub(crate) use cli::arguments;
/// Re-exported so the acceptance records and the binary share one reader.
pub(crate) use molgfx_bench::reader::read_structure;
pub(crate) use report::{
    AcceptanceReport, AdapterEvidence, Availability, CandidateEvidence, MetricSet, SceneEvidence,
    SceneOutcome,
};
pub(crate) use spec::{GoldenSceneId, GoldenSceneSpec, SceneRecipe, golden_scene_specs};
