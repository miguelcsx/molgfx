//! Pipeline specialization for optional typed visual programs.
//!
//! Built-in, entity-only and fragment-dependent styles are separate GPU
//! pipelines. The common built-in path therefore contains no interpreter
//! branch or visual storage access after override specialization.

use crate::scene_gpu::SlotShading;
use molgfx_gpu::Device;

#[cfg(test)]
#[path = "visual_pipelines_tests.rs"]
mod tests;

pub(super) const VISUAL_PROGRAM_CONSTANT: &str = "VISUAL_PROGRAM_ENABLED";
pub(super) const VISUAL_FRAGMENT_CONSTANT: &str = "VISUAL_FRAGMENT_ENABLED";

#[derive(Debug)]
pub(super) struct VisualPipelineSet<D: Device> {
    built_in: D::Pipeline,
    entity: D::Pipeline,
    fragment: D::Pipeline,
}

impl<D: Device> VisualPipelineSet<D> {
    pub(super) fn new(built_in: D::Pipeline, entity: D::Pipeline, fragment: D::Pipeline) -> Self {
        Self {
            built_in,
            entity,
            fragment,
        }
    }

    pub(super) const fn get(&self, shading: SlotShading) -> &D::Pipeline {
        if shading.fragment_visual() {
            &self.fragment
        } else if shading.visual() {
            &self.entity
        } else {
            &self.built_in
        }
    }
}

pub(super) const fn constants(fragment: bool) -> [(&'static str, f64); 2] {
    [
        (VISUAL_PROGRAM_CONSTANT, 1.0),
        (VISUAL_FRAGMENT_CONSTANT, if fragment { 1.0 } else { 0.0 }),
    ]
}
