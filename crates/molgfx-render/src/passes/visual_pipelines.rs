//! Pipeline specialization for optional typed visual programs.
//!
//! Built-in, entity-only and fragment-dependent styles are separate GPU
//! pipelines. The common built-in path therefore contains no interpreter
//! branch or visual storage access after override specialization.
//!
//! Each set additionally holds an optional fourth pipeline built from the
//! unit's specialized sibling, which replaces the interpreter with
//! straight-line code emitted from one exact program. A set without one is the
//! normal state for a pass whose style never specializes, and selection then
//! behaves exactly as it did before specialization existed.

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

    /// Selects the draw's pipeline, preferring generated code when the caller
    /// resolved a specialized pipeline and the shading asks for one.
    ///
    /// A specialized pipeline only ever replaces the fragment stage, because
    /// only that stage's unit carries the marker; every other shading keeps the
    /// unconditional selection so a caller cannot route an entity or built-in
    /// draw onto generated code by mistake.
    pub(super) fn select<'s>(
        &'s self,
        shading: SlotShading,
        specialized: Option<&'s D::Pipeline>,
    ) -> &'s D::Pipeline {
        match specialized {
            Some(pipeline) if shading.fragment_visual() => pipeline,
            _ => self.get(shading),
        }
    }
}

pub(super) const fn constants(fragment: bool) -> [(&'static str, f64); 2] {
    [
        (VISUAL_PROGRAM_CONSTANT, 1.0),
        (VISUAL_FRAGMENT_CONSTANT, if fragment { 1.0 } else { 0.0 }),
    ]
}
