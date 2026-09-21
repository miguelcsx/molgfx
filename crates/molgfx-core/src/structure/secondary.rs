//! Caller- or molframe-supplied secondary-structure state.

/// Reversible per-residue cartoon classification. The renderer never infers
/// this state; callers may apply records produced by `molframe`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SecondaryStructure {
    /// Missing or not yet evaluated assignment.
    #[default]
    Unknown,
    /// No helix or sheet assignment.
    Coil,
    /// Alpha helix.
    Helix,
    /// Beta strand.
    Strand,
    /// Hydrogen-bonded turn.
    Turn,
}
