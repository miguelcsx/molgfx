//! Typed immutable representation specifications.

mod applicability;
mod distance_applicability;
mod review;
pub use review::RepresentationReview;
mod molecular_surface;
pub use molecular_surface::{
    MolecularSurfaceBinding, MolecularSurfaceControls, MolecularSurfaceData,
    MolecularSurfaceGeometry, MolecularSurfaceMethod, MolecularSurfaceSource,
};
mod validation;
pub use applicability::{RepresentationApplicability, RepresentationRequirement};
mod builders;
pub(crate) mod form;
mod item;
#[cfg(test)]
mod tests;

pub use crate::selection::Selection;
pub use builders::{
    Backbone, BallAndStick, BasePairs, Bases, Beads, Bonds, Cartoon, Dots, DynamicBonds, Glycan,
    Licorice, Lines, MolecularSurface, NewRibbons, NucleicAcid, PaperChain, PointRepresentation,
    Polyhedra, Putty, QuickSurf, Ribbons, Solvent, Spacefill, Surface, Trace, Tube, backbone,
    ball_and_stick, base_pairs, bases, beads, bonds, cartoon, dots, dynamic_bonds, glycan,
    licorice, lines, msms, new_cartoon, new_ribbons, nucleic_acid, paper_chain, points, polyhedra,
    putty, quick_surf, ribbons, solvent, spacefill, surf, surface, trace, tube,
};
pub use form::RepresentationSpec;
pub use item::SceneItem;

pub(crate) use item::private;

mod styles;

pub use molgfx_core::{CartoonProfile, GapStyle};
pub use styles::{CartoonStyle, SurfaceKind, SurfaceStyle};
