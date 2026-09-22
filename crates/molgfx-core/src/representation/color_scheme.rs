//! Constructors and queries for representation colour schemes.

use super::ColorScheme;
use crate::AtomPropertyHandle;
use molgfx_math::Rgba8;

impl ColorScheme {
    /// Builds a sequential property colour scheme and its matching legend.
    #[must_use]
    pub fn property(property: AtomPropertyHandle, value: &crate::AtomProperty) -> Self {
        Self::ByProperty {
            property,
            ramp: crate::ScalarRamp::sequential(value.display_domain()),
            missing: Rgba8::opaque(112, 112, 112),
        }
    }

    /// Referenced property column, when this is a continuous encoding.
    #[must_use]
    pub const fn property_handle(self) -> Option<AtomPropertyHandle> {
        match self {
            Self::ByProperty { property, .. } => Some(property),
            _ => None,
        }
    }
}

/// The categorical colour palette, indexed by a reduced chain or residue row.
///
/// Colour-vision-deficiency-safe hues chosen against the bright default ground:
/// every entry clears a 2.9:1 luminance contrast at both stops of the backdrop
/// sweep. The lighter members of the qualitative sets these derive from — pale
/// cyan, sand, mid grey — vanish on a lit background, so they are replaced by
/// their darker siblings rather than kept for tradition.
///
/// Chains and residues share this table and differ only in how they reduce to an
/// index, so the CPU path and the GPU colour block cannot disagree about it.
pub const CATEGORICAL_COLORS: [molgfx_math::Rgba8; 8] = [
    molgfx_math::Rgba8::opaque(51, 34, 136),
    molgfx_math::Rgba8::opaque(178, 74, 92),
    molgfx_math::Rgba8::opaque(17, 119, 51),
    molgfx_math::Rgba8::opaque(133, 124, 40),
    molgfx_math::Rgba8::opaque(24, 116, 106),
    molgfx_math::Rgba8::opaque(136, 34, 85),
    molgfx_math::Rgba8::opaque(59, 110, 163),
    molgfx_math::Rgba8::opaque(150, 72, 160),
];

/// The five secondary-structure colours, in `SecondaryStructure` class order.
pub const SECONDARY_STRUCTURE_COLORS: [molgfx_math::Rgba8; 5] = [
    molgfx_math::Rgba8::opaque(128, 128, 128),
    molgfx_math::Rgba8::opaque(60, 120, 170),
    molgfx_math::Rgba8::opaque(170, 68, 153),
    molgfx_math::Rgba8::opaque(190, 110, 0),
    molgfx_math::Rgba8::opaque(0, 128, 94),
];
