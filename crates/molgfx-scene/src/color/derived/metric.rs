//! Per-atom scalar columns derived from the structure itself.
//!
//! Occupancy, temperature factor, formal charge, residue hydrophobicity and
//! position along a chain are already in the structure, so a caller should not
//! have to extract them to colour by them. A [`AtomMetric`] names one of them,
//! carries the domain and ramp that read well by default, and computes the
//! column in one pass over the atoms. The column then binds through the same
//! scalar-property path as any caller-supplied one, so there is one colour
//! input rather than a second, metric-specific one.
//!
//! Cost is `O(atoms)` time and one `f32` per atom; atoms the metric does not
//! describe read as missing (`NaN`).

use crate::Error;
use molgfx_core::MolecularSource;
use num_traits::ToPrimitive as _;
use serde::{Deserialize, Serialize};

/// A per-atom value the structure defines for itself.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AtomMetric {
    /// Fraction of the site that is occupied, from zero to one.
    Occupancy,
    /// Crystallographic temperature factor, in square ångström.
    BFactor,
    /// Formal charge of the atom, in elementary charges.
    FormalCharge,
    /// Kyte–Doolittle hydropathy of the atom's residue.
    Hydrophobicity,
    /// Position of the atom's residue along its chain, zero at the first
    /// residue and one at the last.
    SequencePosition,
    /// Solvent-accessible area of the atom in isolation, in square ångström.
    ///
    /// Computed for the structure as it is now, so a moving structure must
    /// recompute rather than cache: this is the one metric whose value depends
    /// on the coordinates rather than on the deposited annotation.
    Sasa,
}

impl AtomMetric {
    /// Every metric, in a stable order.
    pub const ALL: [Self; 6] = [
        Self::Occupancy,
        Self::BFactor,
        Self::FormalCharge,
        Self::Hydrophobicity,
        Self::SequencePosition,
        Self::Sasa,
    ];

    /// Stable property name the derived column is bound under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Occupancy => "occupancy",
            Self::BFactor => "b_factor",
            Self::FormalCharge => "formal_charge",
            Self::Hydrophobicity => "hydrophobicity",
            Self::SequencePosition => "sequence_position",
            Self::Sasa => "sasa",
        }
    }

    /// Named colour ramp that reads well for this metric.
    #[must_use]
    pub const fn ramp(self) -> &'static str {
        match self {
            Self::Occupancy => "purples",
            Self::BFactor => "blue_white_red",
            Self::FormalCharge => "red_white_blue",
            Self::Hydrophobicity => "red_yellow_green",
            Self::SequencePosition => "rainbow",
            Self::Sasa => "blue_white_red",
        }
    }

    /// Default display domain.
    #[must_use]
    pub const fn domain(self) -> [f32; 2] {
        match self {
            Self::Occupancy | Self::SequencePosition => [0.0, 1.0],
            Self::BFactor => [0.0, 100.0],
            Self::FormalCharge => [-2.0, 2.0],
            Self::Hydrophobicity => [-4.5, 4.5],
            // A fully exposed atom in a protein is at most a few square
            // ångström per atom; this range covers an exposed-to-buried spread
            // and clamps the rare buried outlier.
            Self::Sasa => [0.0, 100.0],
        }
    }

    /// Physical unit symbol, when the metric has one.
    #[must_use]
    pub const fn units(self) -> Option<&'static str> {
        match self {
            Self::BFactor | Self::Sasa => Some("Å²"),
            Self::FormalCharge => Some("e"),
            _ => None,
        }
    }

    /// Computes the column for `source`, one value per atom.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when the source carries no
    /// native structure to read the metric from.
    pub fn column(self, source: &MolecularSource) -> Result<Vec<f32>, Error> {
        let structure = source.molframe().ok_or_else(|| {
            Error::InvalidSpec(format!(
                "metric '{}' needs a MolFrame-backed structure",
                self.name()
            ))
        })?;
        let mut values = vec![f32::NAN; structure.atom_count() as usize];
        match self {
            Self::Occupancy => fill_atoms(structure, &mut values, atom_occupancy),
            Self::BFactor => fill_atoms(structure, &mut values, atom_b_factor),
            Self::FormalCharge => fill_atoms(structure, &mut values, |atom| {
                atom.formal_charge().map(f32::from)
            }),
            Self::Hydrophobicity => fill_residues(structure, &mut values, |residue, _, _| {
                residue.name().and_then(kyte_doolittle)
            }),
            Self::SequencePosition => {
                fill_residues(structure, &mut values, |_, ordinal, count| {
                    Some(chain_position(ordinal, count))
                });
            }
            Self::Sasa => fill_sasa(structure, &mut values),
        }
        Ok(values)
    }
}

/// Where residue `ordinal` of `count` sits along its chain, from zero to one;
/// a one-residue chain sits in the middle.
fn chain_position(ordinal: usize, count: usize) -> f32 {
    match (ordinal.to_f32(), count.saturating_sub(1).to_f32()) {
        (Some(ordinal), Some(last)) if last > 0.0 => ordinal / last,
        _ => 0.5,
    }
}

/// Fills per-atom solvent-accessible area from the current coordinates.
///
/// The whole structure is sampled once: accessibility is a property of the
/// assembly, not of one atom, so a per-atom call would be both wrong and
/// quadratic. A structure without coordinates leaves every atom missing.
fn fill_sasa(structure: &molframe::Structure, values: &mut [f32]) {
    let positions: Vec<[f32; 3]> = structure
        .atoms()
        .into_iter()
        .filter_map(|atom| atom.position().map(|position| position.into()))
        .collect();
    if positions.len() != values.len() {
        return;
    }
    let radii: Vec<f32> = structure
        .atoms()
        .into_iter()
        .filter_map(|atom| {
            atom.element().and_then(|element| {
                molframe::chemistry::vdw_radius(element, molframe::chemistry::RadiusSet::Bondi)
            })
        })
        .collect();
    if radii.len() != values.len() {
        return;
    }
    let Ok(areas) = molframe::surface::shrake_rupley(
        &positions,
        &radii,
        SASA_PROBE,
        SASA_POINTS,
        &molframe::ExecutionContext::default(),
    ) else {
        return;
    };
    for (value, area) in values.iter_mut().zip(areas) {
        // A finite `f64` area always converts; a non-finite one has no
        // meaningful colour and reads as the missing value.
        *value = match area.to_f32() {
            Some(area) if area.is_finite() => area,
            _ => f32::NAN,
        };
    }
}

/// Water probe radius in ångström, the conventional value.
const SASA_PROBE: f32 = 1.4;
/// Sampling directions per atom; a compromise between accuracy and cost.
const SASA_POINTS: u16 = 92;

fn atom_occupancy(atom: molframe::AtomRef<'_>) -> Option<f32> {
    atom.occupancy()
}

fn atom_b_factor(atom: molframe::AtomRef<'_>) -> Option<f32> {
    atom.b_factor()
}

fn fill_atoms(
    structure: &molframe::Structure,
    values: &mut [f32],
    metric: impl Fn(molframe::AtomRef<'_>) -> Option<f32>,
) {
    for atom in structure.atoms() {
        if let (Some(slot), Some(value)) = (values.get_mut(atom.index().as_usize()), metric(atom)) {
            *slot = value;
        }
    }
}

/// Fills every atom of each residue with the residue's value: the residue, its
/// ordinal within its chain, and the chain's residue count.
fn fill_residues(
    structure: &molframe::Structure,
    values: &mut [f32],
    metric: impl Fn(molframe::ResidueRef<'_>, usize, usize) -> Option<f32>,
) {
    for chain in structure.chains() {
        let count = chain.residues().count();
        for (ordinal, residue) in chain.residues().enumerate() {
            let Some(value) = metric(residue, ordinal, count) else {
                continue;
            };
            for atom in residue.atoms() {
                if let Some(slot) = values.get_mut(atom.index().as_usize()) {
                    *slot = value;
                }
            }
        }
    }
}

/// Kyte–Doolittle hydropathy index of a standard residue.
fn kyte_doolittle(component: &str) -> Option<f32> {
    Some(match component {
        "ILE" => 4.5,
        "VAL" => 4.2,
        "LEU" => 3.8,
        "PHE" => 2.8,
        "CYS" => 2.5,
        "MET" => 1.9,
        "ALA" => 1.8,
        "GLY" => -0.4,
        "THR" => -0.7,
        "SER" => -0.8,
        "TRP" => -0.9,
        "TYR" => -1.3,
        "PRO" => -1.6,
        "HIS" => -3.2,
        "GLU" | "GLN" | "ASP" | "ASN" => -3.5,
        "LYS" => -3.9,
        "ARG" => -4.5,
        _ => return None,
    })
}

#[cfg(test)]
#[path = "metric_tests.rs"]
mod tests;
