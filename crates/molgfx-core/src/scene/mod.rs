//! The mutable renderable scene and its domain-specific editing operations.

mod attributes;
mod difference;
mod domain_visuals;
mod ensembles;
mod generic_batches;
mod generic_timeline;
mod interaction_state;
pub use difference::{AtomCorrespondence, DifferenceScene, DifferenceStyle, DifferenceView};
pub use generic_timeline::{InstanceFramePair, PointFramePair};
pub use interaction_state::InteractionState;
pub(crate) mod guides;
pub(crate) mod interactions;
pub(crate) mod labels;
pub(crate) mod ligand_pose_batches;
mod mesh_instances;
mod meshes;
mod occupancy;
mod overlays;
mod presentation;
pub(crate) mod primitives;
pub(crate) mod properties;
pub(crate) mod query;
mod representation_input;
mod rows;
pub(crate) mod segmentation;
pub(crate) mod selection;
pub(crate) mod state;
mod timeline;
pub(crate) mod topology;
pub(crate) mod trajectory;

#[cfg(test)]
#[path = "overlay_instance_tests.rs"]
mod overlay_instance_tests;
#[cfg(test)]
#[path = "primitive_tests.rs"]
mod primitive_tests;

pub use branch_graph::{TrajectoryBranch, TrajectoryStateGraph};
pub use representation_input::RepresentationInput;
pub use rows::{RowDomain, RowEntityRef, SourceNamespace, SourceRows, TemplatePartPick};
pub use state::Scene;
pub(crate) use state::{
    BoundOccupancy, StoredAtomProperty, StoredAttribute, StoredRepresentation, StoredSegmentation,
    StoredSelection, StoredVolume, TemporalAttribute, TemporalInstances, TemporalPoints,
};
pub use timeline::{PlaybackMode, TimeWarp, Timeline};
mod branch_graph;
