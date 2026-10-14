//! Loads the protein and pairs each trajectory's atoms with the structure atoms
//! the video draws.

use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use molframe::trajectory::{PsfTopology, parse_psf};
use molframe::{AnalysisPolicy, Structure};

use crate::track::{self, Feed};

/// A residue number, residue name and atom name, spelled the way both files agree on.
type Key = (i32, String, String);

/// What the video shows: two structures, each with the file that moves it.
pub(crate) struct Layout {
    /// Every protein atom (left) and the heavy protein atoms (right).
    pub(crate) structures: [Structure; 2],
    /// The XTC drives the left structure and the DCD drives the right one.
    pub(crate) feeds: [Feed; 2],
}

impl Layout {
    /// Each track's caption name, with the file that drives it.
    pub(crate) fn titles(&self) -> [String; 2] {
        let names = ["All protein atoms", "Heavy protein atoms"];
        std::array::from_fn(|track| {
            let file = self.feeds[track]
                .path
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
            format!("{} ({file})", names[track])
        })
    }
}

/// Loads `structure` and pairs each trajectory with the atoms it moves.
///
/// The left track is every protein atom, read from `xtc` at its place in the whole
/// system: the XTC records every atom of the PDB, water and ions included. The right
/// track is the heavy protein atoms, read from `dcd` through the PSF, whose records
/// the DCD follows; each heavy atom is matched to its PSF record by name.
///
/// # Errors
/// Returns an error when a file cannot be read, when an atom count disagrees, or
/// when a heavy atom has no PSF record of its own.
pub(crate) fn load(
    structure: &Path,
    psf: &Path,
    xtc: &Path,
    dcd: &Path,
) -> Result<Layout, Box<dyn Error>> {
    // The PDB has no element column, so each atom's element comes from its name; the
    // `hydrogen` selection below depends on it.
    let (full, notes) = molgfx_bench::reader::read_structure_with(
        structure,
        molframe::MissingElementPolicy::InferFromAtomName,
    )?;
    if let Some(notes) = notes {
        let head: String = notes.chars().take(160).collect();
        eprintln!("structure: reader recovered past diagnostics, first: {head}...");
    }
    let policy = AnalysisPolicy::default();
    let protein = without(&full, "not protein", &policy)?;
    let heavy = without(&protein, "hydrogen", &policy)?;

    let protein_atoms = usize::try_from(protein.atom_count())?;
    let heavy_atoms = usize::try_from(heavy.atom_count())?;
    let full_atoms = usize::try_from(full.atom_count())?;
    let xtc_atoms = track::file_atoms(xtc)?;
    if xtc_atoms != full_atoms {
        return Err(format!(
            "{} holds {xtc_atoms} atoms but the structure has {full_atoms}; the XTC must cover the whole system in structure order",
            xtc.display()
        )
        .into());
    }
    let dcd_atoms = track::file_atoms(dcd)?;
    let topology = parse_psf(&std::fs::read_to_string(psf)?)?;
    if dcd_atoms != topology.atoms.len() {
        return Err(format!(
            "{} holds {dcd_atoms} atoms but {} lists {}",
            dcd.display(),
            psf.display(),
            topology.atoms.len()
        )
        .into());
    }

    let left = Feed {
        path: xtc.to_path_buf(),
        map: Some(protein_map(&full, &protein)?),
        file_atoms: xtc_atoms,
        atoms: protein_atoms,
    };
    let right = Feed {
        path: dcd.to_path_buf(),
        map: Some(heavy_map(&heavy, &topology)?),
        file_atoms: dcd_atoms,
        atoms: heavy_atoms,
    };
    Ok(Layout {
        structures: [protein, heavy],
        feeds: [left, right],
    })
}

/// `structure` with the atoms that `selection` covers removed.
///
/// Domain extensions (assemblies, symmetry, NCS, model quality) refer to atoms the
/// edit may remove, so molframe refuses the edit while they are attached. The video
/// draws none of them, so they are dropped first.
fn without(
    structure: &Structure,
    selection: &str,
    policy: &AnalysisPolicy,
) -> Result<Structure, String> {
    let covered = structure
        .select(selection, policy)
        .map_err(|findings| findings.to_string())?;
    let mut editor = structure.edit();
    editor.clear_extensions();
    editor
        .delete(&covered)
        .map_err(|diagnostic| diagnostic.to_string())?;
    editor.finish().map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    })
}

/// The whole-system index of each protein atom, in the protein's order.
///
/// Deleting atoms keeps the rest in order, so the protein is an increasing subsequence
/// of the whole system. Each PDB serial number names one atom, so serials find the
/// place of each protein atom. A missing or repeated serial, or an order that
/// deletion could not have produced, is an error.
fn protein_map(full: &Structure, protein: &Structure) -> Result<Box<[usize]>, String> {
    let mut position: HashMap<u32, usize> = HashMap::new();
    for (index, atom) in full.atoms().iter().enumerate() {
        let serial = atom
            .atom_site_id()
            .ok_or_else(|| format!("whole-system atom {index} has no serial number"))?;
        if let Some(previous) = position.insert(serial, index) {
            return Err(format!(
                "serial {serial} names whole-system atoms {previous} and {index}"
            ));
        }
    }

    let mut map = Vec::new();
    for atom in protein.atoms() {
        let serial = atom
            .atom_site_id()
            .ok_or("a protein atom has no serial number")?;
        let index = *position
            .get(&serial)
            .ok_or_else(|| format!("protein serial {serial} is not in the whole system"))?;
        map.push(index);
    }
    if map.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("protein atoms are not in whole-system order".to_owned());
    }
    Ok(map.into_boxed_slice())
}

/// The PSF index of each heavy atom, in the structure's order.
///
/// The PSF and the PDB spell some names differently. CHARMM's terminal hydrogens and
/// oxygens are HT* and OT* where the PDB has H* and O*, and the histidine tautomers
/// HSD, HSE and HSP are HIS in the PDB. Both sides are normalized the same way. A heavy
/// atom with no PSF record, or two heavy atoms on one record, is an error.
fn heavy_map(heavy: &Structure, topology: &PsfTopology) -> Result<Box<[usize]>, String> {
    let mut records: HashMap<Key, usize> = HashMap::with_capacity(topology.atoms.len());
    for (index, atom) in topology.atoms.iter().enumerate() {
        let number = atom
            .residue_id
            .parse::<i32>()
            .map_err(|_| format!("PSF residue id {} is not a number", atom.residue_id))?;
        let key = record_key(number, &atom.residue_name, &atom.atom_name);
        if let Some(previous) = records.insert(key.clone(), index) {
            return Err(format!(
                "the PSF names {key:?} twice, at records {previous} and {index}"
            ));
        }
    }

    let mut used = vec![false; topology.atoms.len()];
    let mut map = Vec::new();
    for atom in heavy.atoms() {
        let residue = atom.residue().ok_or("a heavy atom belongs to no residue")?;
        let number = residue
            .auth_seq_id()
            .ok_or("a heavy atom's residue has no number")?;
        let key = record_key(
            number,
            atom.component_name()
                .ok_or("a heavy atom has no residue name")?,
            atom.name().ok_or("a heavy atom has no name")?,
        );
        let index = *records
            .get(&key)
            .ok_or_else(|| format!("no PSF record for heavy atom {key:?}"))?;
        if std::mem::replace(&mut used[index], true) {
            return Err(format!(
                "two heavy atoms claim PSF record {index}; the second is {key:?}"
            ));
        }
        map.push(index);
    }
    Ok(map.into_boxed_slice())
}

/// A key with the spellings both files share.
fn record_key(number: i32, residue: &str, atom: &str) -> Key {
    (
        number,
        canonical_residue(residue).to_owned(),
        canonical_atom(atom),
    )
}

fn canonical_residue(residue: &str) -> &str {
    match residue {
        "HSD" | "HSE" | "HSP" => "HIS",
        other => other,
    }
}

fn canonical_atom(atom: &str) -> String {
    match (atom.get(..1), atom.get(2..)) {
        (Some(first), Some(rest)) if atom.starts_with("HT") || atom.starts_with("OT") => {
            format!("{first}{rest}")
        }
        _ => atom.to_owned(),
    }
}
