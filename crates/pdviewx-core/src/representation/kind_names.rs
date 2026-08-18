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
            Self::Lines => "lines",
            Self::Cartoon => "cartoon",
            Self::Trace => "trace",
            Self::Tube => "tube",
            Self::Surface => "surface",
            Self::Volume => "volume",
            Self::Segmentation => "segmentation",
            Self::Beads => "beads",
            Self::Rocket => "rocket",
            Self::Twister => "twister",
            Self::PaperChain => "paper-chain",
            Self::Points => "points",
        }
    }

    /// Resolves a stable manifest name.
    #[must_use]
    pub fn from_stable_name(name: &str) -> Option<Self> {
        Some(match name {
            "spacefill" => Self::Spacefill,
            "ball_and_stick" => Self::BallAndStick,
            "licorice" => Self::Licorice,
            "lines" => Self::Lines,
            "cartoon" => Self::Cartoon,
            "trace" => Self::Trace,
            "tube" => Self::Tube,
            "surface" => Self::Surface,
            "volume" => Self::Volume,
            "segmentation" => Self::Segmentation,
            "beads" => Self::Beads,
            "rocket" => Self::Rocket,
            "twister" => Self::Twister,
            "paper-chain" => Self::PaperChain,
            "points" => Self::Points,
            _ => return None,
        })
    }
}
