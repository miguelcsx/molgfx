//! Source-backed molecular structure data and caller-supplied fields.

pub(crate) mod atoms;
pub(crate) mod coord;
pub(crate) mod density;
pub(crate) mod ensemble;
pub(crate) mod hierarchy;
pub(crate) mod particle;
pub(crate) mod placed;
pub(crate) mod planar;
pub(crate) mod primitive;
pub(crate) mod provenance;
pub(crate) mod secondary;
pub(crate) mod segmentation;
pub(crate) mod trajectory;
pub(crate) mod validation;

pub use atoms::AtomTable;
pub use coord::CoordRef;
pub use density::DensityVolume;
pub use ensemble::Ensemble;
pub use hierarchy::Hierarchy;
pub use particle::{Particle, ParticleBoundary, ParticleMotion, ParticleShape};
pub use placed::PlacedStructure;
pub use planar::PlanarRegion;
pub use primitive::{
    AnisotropicEllipsoid, CarbohydrateShape, CarbohydrateSymbol, CrystalCell, Primitive,
    SymmetryInstance,
};
pub use provenance::{EntityProvenance, ProvenanceDetail};
pub use secondary::SecondaryStructure;
pub use segmentation::{SegmentStyle, SegmentStyleTable, SegmentationStyle, SegmentedVolume};
pub use trajectory::{TrajectoryFrame, TrajectorySegment};
pub use validation::{ValidationKind, ValidationMarker};
