//! Typed immutable representation specifications.

mod builders;
pub(crate) mod form;
mod item;
#[cfg(test)]
mod tests;

pub use crate::selection::Selection;
pub use builders::{
    BallAndStick, BasePairs, Bases, Cartoon, Glycan, Licorice, Lines, NucleicAcid,
    PointRepresentation, Spacefill, Surface, ball_and_stick, base_pairs, bases, cartoon, glycan,
    licorice, lines, nucleic_acid, points, spacefill, surface,
};
pub use form::RepresentationSpec;
pub use item::SceneItem;

pub(crate) use item::private;

/// Cartoon geometry recipe.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CartoonStyle {
    /// Smooth secondary-structure ribbon.
    Ribbon,
    /// Discrete helix and strand solids.
    Rocket,
    /// Nucleic-acid backbone and base-aware ribbon.
    NucleicAcid,
    /// Glycosidic tree ribbon.
    Glycan,
}

/// Molecular-surface definition.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKind {
    /// Van der Waals boundary.
    VanDerWaals,
    /// Solvent-accessible boundary.
    SolventAccessible,
    /// Solvent-excluded boundary.
    SolventExcluded,
    /// Gaussian density boundary.
    Gaussian,
}

/// Molecular-surface presentation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceStyle {
    /// Filled boundary.
    Solid,
    /// Contour lines.
    Contour,
    /// Dot lattice.
    Dots,
    /// Filled boundary with contours.
    FilledContour,
    /// Wire lattice.
    Mesh,
}
