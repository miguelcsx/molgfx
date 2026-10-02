//! Caller-resolved molecular interactions.

use super::Anchor;
use serde::{Deserialize, Serialize};

/// Overlay interaction classification.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionKind {
    /// Directional donor-acceptor hydrogen bond.
    HydrogenBond,
    /// Oppositely charged residue contact.
    SaltBridge,
    /// Aromatic ring stacking.
    PiStacking,
    /// Cation-aromatic contact. `molframe` has no cation-π class, so this
    /// authoring kind lowers onto [`Self::PiStacking`], the aromatic-ring
    /// interaction it specializes.
    CationPi,
    /// Hydrophobic contact.
    Hydrophobic,
    /// Metal-ligand coordination.
    MetalCoordination,
    /// Unclassified spatial contact. `molframe` has no unclassified class, so
    /// this authoring kind lowers onto [`Self::Hydrophobic`], its non-polar
    /// contact.
    Contact,
}

impl InteractionKind {
    /// Nearest `molframe` interaction class this authoring kind lowers onto.
    ///
    /// Two authoring kinds have no exact upstream counterpart: `CationPi`
    /// lowers onto [`molgfx_core::InteractionKind::PiStacking`] and `Contact`
    /// onto [`molgfx_core::InteractionKind::Hydrophobic`]. A covalent
    /// disulfide bridge is not a contact interaction at all, so it is not an
    /// authorable kind rather than being mapped onto a class it contradicts.
    #[must_use]
    pub const fn core_kind(self) -> molgfx_core::InteractionKind {
        match self {
            Self::HydrogenBond => molgfx_core::InteractionKind::HydrogenBond,
            Self::SaltBridge => molgfx_core::InteractionKind::SaltBridge,
            Self::PiStacking | Self::CationPi => molgfx_core::InteractionKind::PiStacking,
            Self::Hydrophobic | Self::Contact => molgfx_core::InteractionKind::Hydrophobic,
            Self::MetalCoordination => molgfx_core::InteractionKind::MetalCoordination,
        }
    }
}

/// Caller-supplied overlay interaction specification.
///
/// Detection is molecular analysis and belongs to `molframe`; this crate stores
/// and presents interactions the caller has already resolved.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum InteractionSpec {
    /// Caller-provided interaction endpoints.
    Explicit {
        /// Overlay interaction class.
        kind: InteractionKind,
        /// Ordered endpoints.
        endpoints: [Anchor; 2],
    },
}
