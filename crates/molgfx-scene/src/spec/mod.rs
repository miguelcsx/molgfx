//! Canonical scene specifications and atomic patches.

pub(crate) mod lowering;

pub(crate) use crate::patch::{PatchOperation, ScenePatch};

mod camera_validation;
mod interaction_channel;
mod scene_spec;
mod stable_hash;
mod structure_source;

pub(crate) use camera_validation::validate_camera;
pub use interaction_channel::InteractionChannel;
pub use scene_spec::SceneSpec;
pub(crate) use stable_hash::stable_json_hash;
pub use structure_source::StructureSource;
