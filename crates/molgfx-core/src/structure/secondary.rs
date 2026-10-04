//! Caller- or molframe-supplied secondary-structure state.

/// Reversible per-residue cartoon classification. The renderer never infers
/// this state; callers may apply records produced by `molframe`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[repr(u8)]
pub enum SecondaryStructure {
    /// Missing or not yet evaluated assignment.
    #[default]
    Unknown = 0,
    /// No helix or sheet assignment.
    Coil = 1,
    /// Alpha helix.
    AlphaHelix = 2,
    /// Beta strand in a ladder.
    Strand = 3,
    /// Hydrogen-bonded turn.
    Turn = 4,
    /// A three-ten helix.
    ThreeTenHelix = 5,
    /// A pi helix.
    PiHelix = 6,
    /// A source helix outside the named classes.
    OtherHelix = 7,
    /// An isolated beta bridge, not a strand in a ladder.
    BetaBridge = 8,
    /// A backbone bend.
    Bend = 9,
    /// A left-handed polyproline-II helix, regardless of residue sequence.
    PolyProline = 10,
}

impl SecondaryStructure {
    /// Every state in stable interchange and palette order.
    pub const ALL: [Self; 11] = [
        Self::Unknown,
        Self::Coil,
        Self::AlphaHelix,
        Self::Strand,
        Self::Turn,
        Self::ThreeTenHelix,
        Self::PiHelix,
        Self::OtherHelix,
        Self::BetaBridge,
        Self::Bend,
        Self::PolyProline,
    ];

    /// Stable code shared with `MolFrame` and categorical palettes.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Strictly decodes a code; unrecognised codes are not Unknown.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.get(usize::from(code)).copied()
    }

    /// Canonical exact-state label used by manifests and inspection records.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Coil => "coil",
            Self::AlphaHelix => "alpha_helix",
            Self::Strand => "strand",
            Self::Turn => "turn",
            Self::ThreeTenHelix => "three_ten_helix",
            Self::PiHelix => "pi_helix",
            Self::OtherHelix => "other_helix",
            Self::BetaBridge => "beta_bridge",
            Self::Bend => "bend",
            Self::PolyProline => "polyproline",
        }
    }

    /// Strictly decodes a canonical label without legacy aliases.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.name() == name)
    }

    /// Whether any helix profile applies, including polyproline-II.
    #[must_use]
    pub const fn is_helix(self) -> bool {
        matches!(
            self,
            Self::AlphaHelix
                | Self::ThreeTenHelix
                | Self::PiHelix
                | Self::OtherHelix
                | Self::PolyProline
        )
    }

    /// Whether the state belongs to a beta ladder rather than an isolated bridge.
    #[must_use]
    pub const fn is_strand(self) -> bool {
        matches!(self, Self::Strand)
    }

    /// Whether the state is a strand or an isolated beta bridge.
    #[must_use]
    pub const fn is_sheet_like(self) -> bool {
        matches!(self, Self::Strand | Self::BetaBridge)
    }
}

impl From<molframe::SecondaryStructure> for SecondaryStructure {
    fn from(state: molframe::SecondaryStructure) -> Self {
        match state {
            molframe::SecondaryStructure::Unknown => Self::Unknown,
            molframe::SecondaryStructure::Coil => Self::Coil,
            molframe::SecondaryStructure::AlphaHelix => Self::AlphaHelix,
            molframe::SecondaryStructure::Strand => Self::Strand,
            molframe::SecondaryStructure::Turn => Self::Turn,
            molframe::SecondaryStructure::ThreeTenHelix => Self::ThreeTenHelix,
            molframe::SecondaryStructure::PiHelix => Self::PiHelix,
            molframe::SecondaryStructure::OtherHelix => Self::OtherHelix,
            molframe::SecondaryStructure::BetaBridge => Self::BetaBridge,
            molframe::SecondaryStructure::Bend => Self::Bend,
            molframe::SecondaryStructure::PolyProline => Self::PolyProline,
        }
    }
}

#[cfg(test)]
#[path = "secondary_tests.rs"]
mod tests;
