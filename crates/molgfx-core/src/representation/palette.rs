//! Categorical colour palettes.
//!
//! A categorical scheme colours an atom by an integer category (its chain, its
//! entity, its residue name, its secondary-structure class) looked up in a
//! palette. The palette is data owned here once: the CPU colour path, the
//! geometry that bakes colours into meshes, and the block uploaded to the GPU
//! all read the same table, so they cannot disagree about which colour a
//! category gets.
//!
//! A category is reduced modulo the palette length, so a palette shorter than
//! the number of categories cycles instead of failing.

use molgfx_math::Rgba8;
use num_traits::ToPrimitive as _;

const fn hex(rgb: u32) -> Rgba8 {
    let [_, red, green, blue] = rgb.to_be_bytes();
    Rgba8::opaque(red, green, blue)
}

/// Longest palette, so a colour block holds any number of them in fixed space.
pub const MAX_PALETTE_COLORS: usize = 32;

/// A built-in categorical palette.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum CategoryPalette {
    /// Eight hues that stay apart under the common colour-vision deficiencies
    /// and clear a 2.9:1 luminance contrast against the default backdrop. The
    /// lighter members of the qualitative sets they derive from vanish on a
    /// lit ground, so darker siblings replace them.
    #[default]
    CvdSafe,
    /// Twenty colours of maximum contrast, for many categories at once.
    Kelly,
    /// Eight saturated, distinct hues.
    Dark2,
    /// Nine strong primary-like hues.
    Set1,
    /// Eight soft hues.
    Set2,
    /// Twelve pastel hues.
    Set3,
    /// Twelve light-and-dark pairs.
    Paired,
    /// Fixed roles by molecule type; see [`MoleculeType`].
    MoleculeType,
    /// Fixed roles by secondary structure; see [`SecondaryStructureClass`].
    SecondaryStructure,
    /// The twenty standard amino acids and five nucleotides, coloured by
    /// chemistry (acidic, basic, polar, hydrophobic, aromatic), in the order
    /// alanine … valine, then A, C, G, T, U.
    ResidueName,
}

/// The molecule type a category index means under
/// [`CategoryPalette::MoleculeType`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MoleculeType {
    /// Anything not classified below.
    Other = 0,
    /// Solvent water.
    Water = 1,
    /// A monoatomic ion.
    Ion = 2,
    /// An amino-acid polymer.
    Protein = 3,
    /// A ribonucleotide polymer.
    Rna = 4,
    /// A deoxyribonucleotide polymer.
    Dna = 5,
    /// A peptide nucleic acid polymer.
    Pna = 6,
    /// A carbohydrate.
    Saccharide = 7,
}

/// The secondary-structure class a category index means under
/// [`CategoryPalette::SecondaryStructure`], in `SecondaryStructure` order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SecondaryStructureClass {
    /// Not assigned.
    Unknown = 0,
    /// Loop or coil.
    Coil = 1,
    /// Helix.
    Helix = 2,
    /// Strand.
    Strand = 3,
    /// Turn.
    Turn = 4,
}

const CVD_SAFE: [Rgba8; 8] = [
    hex(0x33_22_88),
    hex(0xb2_4a_5c),
    hex(0x11_77_33),
    hex(0x85_7c_28),
    hex(0x18_74_6a),
    hex(0x88_22_55),
    hex(0x3b_6e_a3),
    hex(0x96_48_a0),
];

const KELLY: [Rgba8; 20] = [
    hex(0xf3_c3_00),
    hex(0x87_56_92),
    hex(0xf3_84_00),
    hex(0xa1_ca_f1),
    hex(0x00be_0032),
    hex(0xc2_b2_80),
    hex(0x84_84_82),
    hex(0x00_88_56),
    hex(0xe6_8f_ac),
    hex(0x00_67_a5),
    hex(0xf9_93_79),
    hex(0x60_4e_97),
    hex(0xf6_a6_00),
    hex(0xb3_44_6c),
    hex(0xdc_d3_00),
    hex(0x88_2d_17),
    hex(0x8d_b6_00),
    hex(0x65_45_22),
    hex(0xe2_58_22),
    hex(0x2b_3d_26),
];

const DARK2: [Rgba8; 8] = [
    hex(0x1b_9e_77),
    hex(0xd9_5f_02),
    hex(0x75_70_b3),
    hex(0xe7_29_8a),
    hex(0x66_a6_1e),
    hex(0xe6_ab_02),
    hex(0xa6_76_1d),
    hex(0x66_66_66),
];

const SET1: [Rgba8; 9] = [
    hex(0xe4_1a_1c),
    hex(0x37_7e_b8),
    hex(0x4d_af_4a),
    hex(0x98_4e_a3),
    hex(0xff_7f_00),
    hex(0xff_ff_33),
    hex(0xa6_56_28),
    hex(0xf7_81_bf),
    hex(0x99_99_99),
];

const SET2: [Rgba8; 8] = [
    hex(0x66_c2_a5),
    hex(0xfc_8d_62),
    hex(0x8d_a0_cb),
    hex(0xe7_8a_c3),
    hex(0xa6_d8_54),
    hex(0xff_d9_2f),
    hex(0xe5_c4_94),
    hex(0xb3_b3_b3),
];

const SET3: [Rgba8; 12] = [
    hex(0x8d_d3_c7),
    hex(0xff_ff_b3),
    hex(0xbe_ba_da),
    hex(0xfb_80_72),
    hex(0x80_b1_d3),
    hex(0xfd_b4_62),
    hex(0xb3_de_69),
    hex(0xfc_cd_e5),
    hex(0xd9_d9_d9),
    hex(0xbc_80_bd),
    hex(0xcc_eb_c5),
    hex(0xff_ed_6f),
];

const PAIRED: [Rgba8; 12] = [
    hex(0xa6_ce_e3),
    hex(0x1f_78_b4),
    hex(0xb2_df_8a),
    hex(0x33_a0_2c),
    hex(0xfb_9a_99),
    hex(0xe3_1a_1c),
    hex(0xfd_bf_6f),
    hex(0xff_7f_00),
    hex(0xca_b2_d6),
    hex(0x6a_3d_9a),
    hex(0xff_ff_99),
    hex(0xb1_59_28),
];

/// In [`MoleculeType`] order.
const MOLECULE_TYPE: [Rgba8; 8] = [
    hex(0xff_ff_99),
    hex(0x38_6c_b0),
    hex(0xf0_02_7f),
    hex(0xbe_ae_d4),
    hex(0xfd_c0_86),
    hex(0xbf_5b_17),
    hex(0x42_a4_9a),
    hex(0x7f_c9_7f),
];

/// Amino acids by chemistry, then nucleotides, in residue-kind order.
const RESIDUE_NAME: [Rgba8; 25] = [
    hex(0xc8_c8_c8),
    hex(0x14_5a_ff),
    hex(0x00_dc_dc),
    hex(0xe6_0a_0a),
    hex(0xe6_e6_00),
    hex(0x00_dc_dc),
    hex(0xe6_0a_0a),
    hex(0xeb_eb_eb),
    hex(0x82_82_d2),
    hex(0x0f_82_0f),
    hex(0x0f_82_0f),
    hex(0x14_5a_ff),
    hex(0xe6_e6_00),
    hex(0x32_32_aa),
    hex(0xdc_96_82),
    hex(0xfa_96_00),
    hex(0xfa_96_00),
    hex(0xb4_5a_b4),
    hex(0x32_32_aa),
    hex(0x0f_82_0f),
    hex(0xa0_a0_ff),
    hex(0xff_8c_4b),
    hex(0xff_70_80),
    hex(0xa0_ff_a0),
    hex(0xa0_ff_a0),
];

/// In [`SecondaryStructureClass`] order.
const SECONDARY_STRUCTURE: [Rgba8; 5] = [
    hex(0x80_80_80),
    hex(0x3c_78_aa),
    hex(0xaa_44_99),
    hex(0xbe_6e_00),
    hex(0x00_80_5e),
];

impl CategoryPalette {
    /// Every palette, in a stable order.
    pub const ALL: [Self; 10] = [
        Self::CvdSafe,
        Self::Kelly,
        Self::Dark2,
        Self::Set1,
        Self::Set2,
        Self::Set3,
        Self::Paired,
        Self::MoleculeType,
        Self::SecondaryStructure,
        Self::ResidueName,
    ];

    /// The palette's colours, category zero first.
    #[must_use]
    pub const fn colors(self) -> &'static [Rgba8] {
        match self {
            Self::CvdSafe => &CVD_SAFE,
            Self::Kelly => &KELLY,
            Self::Dark2 => &DARK2,
            Self::Set1 => &SET1,
            Self::Set2 => &SET2,
            Self::Set3 => &SET3,
            Self::Paired => &PAIRED,
            Self::MoleculeType => &MOLECULE_TYPE,
            Self::SecondaryStructure => &SECONDARY_STRUCTURE,
            Self::ResidueName => &RESIDUE_NAME,
        }
    }

    /// Number of colours.
    #[must_use]
    pub const fn len(self) -> usize {
        self.colors().len()
    }

    /// A palette always holds colours.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        false
    }

    /// The colour of `category`, cycling when it exceeds the palette.
    ///
    /// A category that is not a non-negative whole number has no colour.
    #[must_use]
    pub fn color(self, category: f32) -> Option<Rgba8> {
        if !category.is_finite() || category < 0.0 || category.fract() != 0.0 {
            return None;
        }
        // The category is a whole number below 2^24 in every column this
        // engine derives, so truncation is exact.
        let index = category.to_usize()?;
        self.colors().get(index % self.len()).copied()
    }

    /// The stable name used in serialized scenes and commands.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CvdSafe => "cvd_safe",
            Self::Kelly => "kelly",
            Self::Dark2 => "dark2",
            Self::Set1 => "set1",
            Self::Set2 => "set2",
            Self::Set3 => "set3",
            Self::Paired => "paired",
            Self::MoleculeType => "molecule_type",
            Self::SecondaryStructure => "secondary_structure",
            Self::ResidueName => "residue_name",
        }
    }

    /// The palette a name refers to.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|palette| palette.name() == name)
    }
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;
