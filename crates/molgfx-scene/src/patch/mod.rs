//! Atomic revision-checked semantic scene edits.

pub(crate) mod appearance_ops;
mod inverse;
#[cfg(test)]
mod inverse_tests;
pub(crate) mod overlay_ops;
pub(crate) mod plan;
#[cfg(test)]
mod tests;

mod operation;
mod scene_patch;

pub use operation::PatchOperation;
pub use scene_patch::ScenePatch;
