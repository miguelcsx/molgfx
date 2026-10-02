//! Typed immutable representation specifications.

mod builders;
pub(crate) mod form;
mod item;
#[cfg(test)]
mod tests;

pub use crate::selection::Selection;
pub use builders::{
    Backbone, BallAndStick, BasePairs, Bases, Beads, Cartoon, Dots, Glycan, Licorice, Lines,
    NucleicAcid, PointRepresentation, Putty, Spacefill, Surface, Trace, Tube, backbone,
    ball_and_stick, base_pairs, bases, beads, cartoon, dots, glycan, licorice, lines, nucleic_acid,
    points, putty, spacefill, surface, trace, tube,
};
pub use form::RepresentationSpec;
pub use item::SceneItem;

pub(crate) use item::private;

mod styles;

pub use styles::{CartoonStyle, SurfaceKind, SurfaceStyle};
