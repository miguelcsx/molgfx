//! Per-atom categories the structure defines for itself.
//!
//! A category is a whole number per atom — which chain it is in, which entity,
//! what kind of molecule, which residue kind, which secondary-structure class —
//! that a palette turns into a colour. Deriving one is a single pass over the
//! atoms, `O(atoms)`, and the column is bound like any other scalar property, so
//! a categorical scheme has no path of its own on the GPU or in geometry.
//!
//! An atom the category does not describe reads as missing (`NaN`), and the
//! renderer keeps that atom's element colour. That is how a scheme colours only
//! some atoms: colouring only the carbons by chain is the chain category with
//! every other element blanked.

use crate::Error;
use molgfx_core::{MolecularSource, SourceTopology};
use num_traits::ToPrimitive as _;
use serde::{Deserialize, Serialize};

/// What an atom is categorised by.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AtomCategory {
    /// The chain the atom belongs to, numbered in file order.
    Chain,
    /// The entity (the distinct molecular species) of the atom's chain.
    Entity,
    /// Water, ion, protein, RNA, DNA, saccharide or other.
    MoleculeType,
    /// The standard residue kind: an amino acid or a nucleotide.
    ResidueName,
    /// The residue's position in the structure, spread so neighbours differ.
    Residue,
    /// The residue's secondary-structure class.
    SecondaryStructure,
}

/// Residue kinds a residue-name palette distinguishes, in palette order.
const RESIDUE_KINDS: [&str; 25] = [
    "ALA", "ARG", "ASN", "ASP", "CYS", "GLN", "GLU", "GLY", "HIS", "ILE", "LEU", "LYS", "MET",
    "PHE", "PRO", "SER", "THR", "TRP", "TYR", "VAL", "A", "C", "G", "T", "U",
];

/// Step between the categories of neighbouring residues, coprime with every
/// palette length, so a short palette does not colour a run of residues alike.
const RESIDUE_STEP: u32 = 5;

/// Categories stay exactly representable as `f32` below this bound.
const EXACT_LIMIT: u32 = 1 << 24;

impl AtomCategory {
    /// Every category, in a stable order.
    pub const ALL: [Self; 6] = [
        Self::Chain,
        Self::Entity,
        Self::MoleculeType,
        Self::ResidueName,
        Self::Residue,
        Self::SecondaryStructure,
    ];

    /// The name used in properties, commands and serialized scenes.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Chain => "chain",
            Self::Entity => "entity",
            Self::MoleculeType => "molecule_type",
            Self::ResidueName => "residue_name",
            Self::Residue => "residue",
            Self::SecondaryStructure => "secondary_structure",
        }
    }

    /// The category a name refers to.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|category| category.name() == name)
    }

    /// The palette that reads this category best.
    #[must_use]
    pub const fn default_palette(self) -> molgfx_core::CategoryPalette {
        use molgfx_core::CategoryPalette;
        match self {
            Self::Chain => CategoryPalette::Kelly,
            Self::Entity => CategoryPalette::Dark2,
            Self::MoleculeType => CategoryPalette::MoleculeType,
            Self::ResidueName => CategoryPalette::ResidueName,
            Self::Residue => CategoryPalette::CvdSafe,
            Self::SecondaryStructure => CategoryPalette::SecondaryStructure,
        }
    }

    /// Computes the column for `source`, one value per atom.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when the category needs the
    /// structure's own model (entity, molecule type, residue name) and the
    /// source carries none.
    pub fn column(self, source: &MolecularSource) -> Result<Vec<f32>, Error> {
        let topology = source.topology();
        let atoms = topology.atoms.len();
        let mut values = vec![f32::NAN; atoms];
        match self {
            Self::Chain => fill_chains(topology, &mut values),
            Self::Residue => fill_by_residue(topology, &mut values, |residue| {
                residue
                    .checked_mul(RESIDUE_STEP)
                    .map(|step| step % EXACT_LIMIT)
            }),
            Self::SecondaryStructure => fill_by_residue(topology, &mut values, |residue| {
                let index = usize::try_from(residue).ok()?;
                topology
                    .secondary_structure
                    .get(index)
                    .map(|class| class_of(*class))
            }),
            Self::Entity | Self::MoleculeType | Self::ResidueName => {
                let structure = source.molframe().ok_or_else(|| {
                    Error::InvalidSpec(format!(
                        "the '{}' category needs a MolFrame-backed structure",
                        self.name()
                    ))
                })?;
                fill_from_structure(self, structure, &mut values);
            }
        }
        Ok(values)
    }
}

/// The secondary-structure class of a state, matching the palette order.
const fn class_of(state: molframe::SecondaryStructure) -> u32 {
    match state {
        molframe::SecondaryStructure::Unknown => 0,
        molframe::SecondaryStructure::Coil => 1,
        molframe::SecondaryStructure::Helix => 2,
        molframe::SecondaryStructure::Strand => 3,
        molframe::SecondaryStructure::Turn => 4,
    }
}

/// A category as the scalar a column stores; exact below [`EXACT_LIMIT`].
fn exact(value: u32) -> f32 {
    let Some(category) = (value % EXACT_LIMIT).to_f32() else {
        return f32::NAN;
    };
    category
}

/// Sets every atom of residue `residue` to `value`.
fn set_residue(topology: &SourceTopology, values: &mut [f32], residue: usize, value: f32) {
    let (Some(&start), Some(&end)) = (
        topology.residue_atom_start.get(residue),
        topology.residue_atom_start.get(residue + 1),
    ) else {
        return;
    };
    let (Ok(start), Ok(end)) = (usize::try_from(start), usize::try_from(end)) else {
        return;
    };
    if let Some(slice) = values.get_mut(start..end) {
        slice.fill(value);
    }
}

fn fill_chains(topology: &SourceTopology, values: &mut [f32]) {
    let starts = &topology.chain_residue_start;
    for (chain, pair) in starts.windows(2).enumerate() {
        let (Ok(first), Ok(last)) = (usize::try_from(pair[0]), usize::try_from(pair[1])) else {
            continue;
        };
        let Ok(chain) = u32::try_from(chain) else {
            continue;
        };
        let category = exact(chain);
        for residue in first..last {
            set_residue(topology, values, residue, category);
        }
    }
}

fn fill_by_residue(
    topology: &SourceTopology,
    values: &mut [f32],
    category: impl Fn(u32) -> Option<u32>,
) {
    let residues = topology.residue_atom_start.len().saturating_sub(1);
    for residue in 0..residues {
        let Ok(row) = u32::try_from(residue) else {
            continue;
        };
        if let Some(value) = category(row) {
            set_residue(topology, values, residue, exact(value));
        }
    }
}

fn fill_from_structure(
    category: AtomCategory,
    structure: &molframe::Structure,
    values: &mut [f32],
) {
    for chain in structure.chains() {
        let entity_kind = chain
            .entity()
            .and_then(|entity| structure.engine().data().topology.entities.kind(entity));
        for residue in chain.residues() {
            let Some(value) = (match category {
                AtomCategory::Entity => chain
                    .entity()
                    .and_then(|entity| u32::try_from(entity.as_usize()).ok()),
                AtomCategory::MoleculeType => Some(molecule_type(
                    chain.polymer_kind(),
                    entity_kind,
                    residue.name(),
                    residue.atoms().count(),
                )),
                AtomCategory::ResidueName => residue.name().and_then(residue_kind),
                _ => None,
            }) else {
                continue;
            };
            let category_value = exact(value);
            for atom in residue.atoms() {
                if let Some(slot) = values.get_mut(atom.index().as_usize()) {
                    *slot = category_value;
                }
            }
        }
    }
}

fn residue_kind(name: &str) -> Option<u32> {
    RESIDUE_KINDS
        .iter()
        .position(|known| *known == name)
        .and_then(|index| u32::try_from(index).ok())
}

/// The molecule type of one residue, as a [`molgfx_core::MoleculeType`] index.
///
/// The chain's declared polymer kind decides first, then the entity's declared
/// kind. Only when a file declares neither, as a bare PDB does not, are
/// standard component names read, and only by this theme: selections stay
/// strict about what a file declares.
fn molecule_type(
    polymer: molframe::PolymerKind,
    entity: Option<molframe::EntityKind>,
    name: Option<&str>,
    atoms: usize,
) -> u32 {
    use molgfx_core::MoleculeType;
    let deoxy = name.is_some_and(|name| name.starts_with('D') && name.len() == 2);
    let kind = match (polymer, entity) {
        (molframe::PolymerKind::Protein, _) => MoleculeType::Protein,
        (molframe::PolymerKind::Dna, _) => MoleculeType::Dna,
        (molframe::PolymerKind::NucleicHybrid, _) if deoxy => MoleculeType::Dna,
        (molframe::PolymerKind::Rna | molframe::PolymerKind::NucleicHybrid, _) => MoleculeType::Rna,
        (molframe::PolymerKind::Saccharide, _) | (_, Some(molframe::EntityKind::Branched)) => {
            MoleculeType::Saccharide
        }
        (_, Some(molframe::EntityKind::Water)) => MoleculeType::Water,
        _ => match name {
            Some(name) if is_named_water(name) => MoleculeType::Water,
            Some(name) if is_named_amino_acid(name) => MoleculeType::Protein,
            Some(name) if is_named_nucleotide(name) && deoxy => MoleculeType::Dna,
            Some(name) if is_named_nucleotide(name) => MoleculeType::Rna,
            _ if atoms == 1 => MoleculeType::Ion,
            _ => MoleculeType::Other,
        },
    };
    u32::from(kind as u8)
}

fn is_named_water(name: &str) -> bool {
    matches!(name, "HOH" | "WAT" | "DOD" | "H2O" | "SOL" | "TIP3" | "TIP")
}

fn is_named_amino_acid(name: &str) -> bool {
    RESIDUE_KINDS[..20].contains(&name)
}

fn is_named_nucleotide(name: &str) -> bool {
    matches!(
        name,
        "A" | "C" | "G" | "U" | "I" | "DA" | "DC" | "DG" | "DT" | "DU" | "DI"
    )
}

#[cfg(test)]
#[path = "category_tests.rs"]
mod tests;
