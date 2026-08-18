//! The mutable renderable scene and its domain-specific editing operations.

pub(crate) mod ensembles;
pub(crate) mod guides;
pub(crate) mod interactions;
pub(crate) mod labels;
mod mesh_instances;
mod meshes;
mod overlays;
pub(crate) mod primitives;
pub(crate) mod properties;
pub(crate) mod query;
pub(crate) mod segmentation;
pub(crate) mod selection;
pub(crate) mod state;
pub(crate) mod trajectory;

#[cfg(test)]
#[path = "overlay_instance_tests.rs"]
mod overlay_instance_tests;
#[cfg(test)]
#[path = "primitive_tests.rs"]
mod primitive_tests;

pub use state::Scene;
pub(crate) use state::{
    StoredAtomProperty, StoredRepresentation, StoredSegmentation, StoredSelection, StoredVolume,
};
