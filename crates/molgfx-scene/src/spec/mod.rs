//! Canonical scene specifications and atomic patches.

mod form;
pub(crate) mod lowering;

pub(crate) use crate::patch::{PatchOperation, ScenePatch};

mod camera_validation;
mod interaction_channel;
mod runtime_sources;
mod scene_spec;
mod stable_hash;
mod structure_source;
mod timeline_spec;

pub(crate) use camera_validation::validate_camera;
pub use interaction_channel::InteractionChannel;
pub use scene_spec::SceneSpec;
pub(crate) use stable_hash::stable_json_hash;
pub use structure_source::StructureSource;
pub use timeline_spec::{TimelineSpec, TrackPace, TrackSpec};
