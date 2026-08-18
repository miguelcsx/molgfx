//! Caller- or pdbiox-supplied secondary-structure state.

/// Reversible per-residue cartoon classification. The renderer never infers
/// this state; callers may apply records produced by `pdbiox`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SecondaryStructure {
    /// No helix or sheet assignment.
    #[default]
    Coil,
    /// Alpha helix.
    Helix,
    /// Beta strand.
    Strand,
    /// Hydrogen-bonded turn.
    Turn,
}
