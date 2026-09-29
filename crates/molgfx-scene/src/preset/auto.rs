//! Default representations chosen from structure size and chemical class.

use super::size::{AtomicDetail, StructureSize};
use crate::representation::{Selection, SurfaceKind, SurfaceStyle};
use crate::{Error, RepresentationSpec, StructureId, rep, sel};
use molgfx_core::MolecularSource;

/// Chooses default representations for `structure`.
///
/// - **Small** structures (no real polymer) draw every atom at atomic detail.
/// - **Medium** structures draw polymers as cartoons and everything that is
///   neither polymer nor solvent as ball-and-stick.
/// - **Large** structures draw only the polymer cartoons.
/// - **Huge** structures draw the polymer as one Gaussian surface.
///
/// Solvent is never drawn by default. A nucleic polymer uses the nucleic-acid
/// form so bases stay visible.
///
/// # Errors
///
/// Returns an error when a class selection cannot be evaluated.
pub fn auto_representations(
    source: &MolecularSource,
    structure: StructureId,
) -> Result<Vec<RepresentationSpec>, Error> {
    let atoms = widen(source.coordinates().len());
    let polymer = polymer_residues(source)?;
    let size = StructureSize::from_polymer_residues(polymer);
    let mut forms = Vec::new();
    match size {
        StructureSize::Small => forms.push(atomic(sel::all().into(), atoms, structure)),
        StructureSize::Medium => {
            polymer_forms(source, structure, &mut forms)?;
            forms.push(atomic(non_polymer_non_solvent(), atoms, structure));
        }
        StructureSize::Large => polymer_forms(source, structure, &mut forms)?,
        StructureSize::Huge => forms.push(
            rep::surface(sel::polymer())
                .kind(SurfaceKind::Gaussian)
                .style(SurfaceStyle::Solid)
                .structure(structure)
                .into(),
        ),
    }
    Ok(forms)
}

/// A length as a count wide enough for any structure.
fn widen(length: usize) -> u64 {
    let Ok(count) = u64::try_from(length) else {
        return u64::MAX;
    };
    count
}

/// The atom table length in the row type selections are relative to.
fn row_count(source: &MolecularSource) -> u32 {
    let Ok(rows) = u32::try_from(source.coordinates().len()) else {
        return u32::MAX;
    };
    rows
}

fn non_polymer_non_solvent() -> Selection {
    (!sel::polymer() & !sel::water()).into()
}

fn atomic(target: Selection, atoms: u64, structure: StructureId) -> RepresentationSpec {
    match AtomicDetail::for_atoms(atoms) {
        AtomicDetail::BallAndStick => rep::ball_and_stick(target).structure(structure).into(),
        AtomicDetail::Lines => rep::lines(target).structure(structure).into(),
        AtomicDetail::Points => rep::points(target).structure(structure).into(),
    }
}

/// Cartoons for protein polymers, nucleic-acid ribbons for nucleic polymers.
/// A class with no atoms adds nothing.
fn polymer_forms(
    source: &MolecularSource,
    structure: StructureId,
    forms: &mut Vec<RepresentationSpec>,
) -> Result<(), Error> {
    if !count(source, &sel::protein().into())?.eq(&0) {
        forms.push(rep::cartoon(sel::protein()).structure(structure).into());
    }
    if !count(source, &sel::nucleic().into())?.eq(&0) {
        forms.push(
            rep::nucleic_acid(sel::nucleic())
                .structure(structure)
                .into(),
        );
    }
    Ok(())
}

fn count(source: &MolecularSource, target: &Selection) -> Result<u64, Error> {
    let selected = source
        .select_compiled(&*target.compiled()?)
        .map_err(|error| Error::InvalidSpec(error.to_string()))?;
    Ok(selected.count(row_count(source)))
}

/// Residues that contain at least one polymer atom. Atoms are ordered by
/// residue, so a change of residue row between visited atoms is a new residue.
fn polymer_residues(source: &MolecularSource) -> Result<u64, Error> {
    let target: Selection = sel::polymer().into();
    let selected = source
        .select_compiled(&*target.compiled()?)
        .map_err(|error| Error::InvalidSpec(error.to_string()))?;
    let table = row_count(source);
    let atoms = &source.topology().atoms;
    let mut residues = 0_u64;
    let mut last = None;
    selected.for_each(table, |atom| {
        let Some(row) = atoms.get(atom as usize).map(|a| a.residue) else {
            return;
        };
        if last != Some(row) {
            residues += 1;
            last = Some(row);
        }
    });
    Ok(residues)
}
