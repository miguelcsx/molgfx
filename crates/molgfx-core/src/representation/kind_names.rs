//! Stable serialized names for drawable representation kinds.

use super::RepresentationKind;

impl RepresentationKind {
    /// Stable manifest name, independent of Rust's debug formatting.
    #[must_use]
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::Spacefill => "spacefill",
            Self::BallAndStick => "ball_and_stick",
            Self::Licorice => "licorice",
            Self::Bonds => "bonds",
            Self::DynamicBonds => "dynamic_bonds",
            Self::Lines => "lines",
            Self::Cartoon => "cartoon",
            Self::NewRibbons => "new_ribbons",
            Self::Ribbons => "ribbons",
            Self::Polyhedra => "polyhedra",
            Self::Trace => "trace",
            Self::Tube => "tube",
            Self::Surface => "surface",
            Self::MolecularSurface => "molecular_surface",
            Self::Volume => "volume",
            Self::Segmentation => "segmentation",
            Self::Beads => "beads",
            Self::Rocket => "rocket",
            Self::Twister => "twister",
            Self::PaperChain => "paper-chain",
            Self::Points => "points",
            Self::Dots => "dots",
            Self::Solvent => "solvent",
            Self::Bases => "bases",
            Self::BasePairs => "base_pairs",
        }
    }

    /// Resolves a stable manifest name.
    #[must_use]
    pub fn from_stable_name(name: &str) -> Option<Self> {
        Some(match name {
            "spacefill" => Self::Spacefill,
            "ball_and_stick" => Self::BallAndStick,
            "licorice" => Self::Licorice,
            "bonds" => Self::Bonds,
            "dynamic_bonds" => Self::DynamicBonds,
            "lines" => Self::Lines,
            "cartoon" => Self::Cartoon,
            "new_ribbons" => Self::NewRibbons,
            "ribbons" => Self::Ribbons,
            "polyhedra" => Self::Polyhedra,
            "trace" => Self::Trace,
            "tube" => Self::Tube,
            "surface" => Self::Surface,
            "molecular_surface" => Self::MolecularSurface,
            "volume" => Self::Volume,
            "segmentation" => Self::Segmentation,
            "beads" => Self::Beads,
            "rocket" => Self::Rocket,
            "twister" => Self::Twister,
            "paper-chain" => Self::PaperChain,
            "points" => Self::Points,
            "dots" => Self::Dots,
            "solvent" => Self::Solvent,
            "bases" => Self::Bases,
            "base_pairs" => Self::BasePairs,
            _ => return None,
        })
    }
}
