//! Typed immutable representation specifications.

mod applicability;
pub use applicability::{RepresentationApplicability, RepresentationRequirement};
mod builders;
pub(crate) mod form;
mod item;
#[cfg(test)]
mod tests;

pub use crate::selection::Selection;
pub use builders::{
    Backbone, BallAndStick, BasePairs, Bases, Beads, Bonds, Cartoon, Dots, Glycan, Licorice, Lines,
    NucleicAcid, PaperChain, PointRepresentation, Putty, Spacefill, Surface, Trace, Tube, backbone,
    ball_and_stick, base_pairs, bases, beads, bonds, cartoon, dots, glycan, licorice, lines,
    nucleic_acid, paper_chain, points, putty, spacefill, surface, trace, tube,
};
pub use form::RepresentationSpec;
pub use item::SceneItem;

pub(crate) use item::private;

mod styles;

pub use molgfx_core::{CartoonProfile, GapStyle};
pub use styles::{CartoonStyle, SurfaceKind, SurfaceStyle};
