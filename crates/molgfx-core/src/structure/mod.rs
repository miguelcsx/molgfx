//! Source-backed molecular structure data and caller-supplied fields.

mod analytic_instance;
pub(crate) mod atoms;
pub(crate) mod coord;
pub(crate) mod density;
mod ensemble;
pub(crate) mod hierarchy;
pub(crate) mod occupancy;
pub(crate) mod particle;
pub(crate) mod placed;
pub(crate) mod planar;
mod point_batch;
mod pose_batch;
pub(crate) mod primitive;
mod primitive_declaration;
pub(crate) mod provenance;
pub(crate) mod secondary;
pub(crate) mod segmentation;
pub(crate) mod topology;
pub(crate) mod trajectory;
pub(crate) mod validation;

pub use analytic_instance::{
    AnalyticCapsule, AnalyticSphere, AnalyticTemplate, InstanceBatch, InstanceStyle, RigidInstance,
};
pub use atoms::AtomTable;
pub use coord::CoordRef;
pub use density::ScalarVolume;
pub use ensemble::Ensemble;
pub use hierarchy::Hierarchy;
pub use occupancy::OccupancyStream;
pub use particle::{Particle, ParticleBoundary, ParticleMotion, ParticleShape};
pub use placed::PlacedStructure;
pub use planar::PlanarRegion;
pub use point_batch::{PointBatch, PointGlyph, PointStyle};
pub use pose_batch::{LicoriceTemplate, LigandPose, LigandPoseBatch};
pub use primitive::{
    AnisotropicEllipsoid, CarbohydrateShape, CarbohydrateSymbol, CrystalCell, Primitive,
    SymmetryInstance,
};
pub use provenance::{EntityProvenance, ProvenanceDetail};
pub use secondary::SecondaryStructure;
pub use segmentation::{SegmentStyle, SegmentStyleTable, SegmentationStyle, SegmentedVolume};
pub use topology::{ActiveTopologyBond, BondTopologyFrame, BondTopologySegment, TopologyBond};
pub use trajectory::{TrajectoryFrame, TrajectorySegment};
pub use validation::{ValidationKind, ValidationMarker};
