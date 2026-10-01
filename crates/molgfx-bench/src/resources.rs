//! Scene heap measurements. Retained allocator bytes are not process RSS.
//! Fixture construction and output storage stay outside measured regions.

use crate::{HeapMeasurement, measure_heap, synthetic};
use molgfx::{Scene, rep, sel};
use serde::Serialize;
use std::error::Error;
use std::io;

/// Edits performed by a warm edit case.
pub const EDIT_COUNT: usize = 64;
const OPACITY_RAMP: [f32; 16] = [
    0.10, 0.15, 0.20, 0.25, 0.30, 0.35, 0.40, 0.45, 0.50, 0.55, 0.60, 0.65, 0.70, 0.75, 0.80, 0.85,
];
/// Residue counts spanning a ligand, a domain and a small complex.
pub const SIZES: [usize; 3] = [64, 1_024, 8_192];
/// Cases understood by the resource consumer.
pub const CASES: [&str; 9] = [
    "scene_only",
    "add_one_representation",
    "add_eight_representations",
    "one_representation",
    "eight_representations",
    "shared_selection",
    "distinct_selections",
    "opacity_edits",
    "visibility_edits",
];

/// One scene operation's measured allocator activity.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ResourceRecord {
    /// Validated case name.
    pub case: &'static str,
    /// Source atoms in the scene.
    pub atoms: u64,
    /// Rust system allocator activity, excluding fixture construction.
    pub heap: HeapMeasurement,
    /// Signed retained bytes per atom, scaled by one hundred.
    pub retained_bytes_per_atom_hundredths: i128,
}

impl ResourceRecord {
    /// Measures an operation after its fixtures have been constructed.
    pub fn measure<T>(case: &'static str, atoms: u64, operation: impl FnOnce() -> T) -> (Self, T) {
        let (heap, value) = measure_heap(operation);
        let record = Self {
            case,
            atoms,
            heap,
            retained_bytes_per_atom_hundredths: if atoms == 0 {
                0
            } else {
                heap.retained_bytes * 100 / i128::from(atoms)
            },
        };
        (record, value)
    }
}

/// Returns the canonical case name or an error instead of an empty result.
///
/// # Errors
/// Unknown case names are rejected.
pub fn case_name(selected: &str) -> Result<&'static str, io::Error> {
    CASES
        .iter()
        .copied()
        .find(|case| *case == selected)
        .ok_or_else(|| io::Error::other(format!("unknown resource case: {selected}")))
}

/// Runs a selected case, or all cases, with output storage reserved up front.
///
/// # Errors
/// Unknown cases and scene-authoring failures are returned to the caller.
pub fn run(selected: &str) -> Result<Vec<ResourceRecord>, Box<dyn Error>> {
    if selected != "all" {
        case_name(selected)?;
    }
    let count = if selected == "all" { CASES.len() } else { 1 };
    let mut records = Vec::with_capacity(count * SIZES.len());
    for case in CASES
        .into_iter()
        .filter(|case| selected == "all" || *case == selected)
    {
        for residues in SIZES {
            records.push(run_one(case, residues)?);
        }
    }
    Ok(records)
}

/// Runs exactly one case/size, suitable for an isolated child process.
///
/// # Errors
/// Unknown cases, invalid sizes, and authoring failures are returned.
pub fn run_one(selected: &str, residues: usize) -> Result<ResourceRecord, Box<dyn Error>> {
    let case = case_name(selected)?;
    if !SIZES.contains(&residues) {
        return Err(io::Error::other("resource residue count must be 64, 1024 or 8192").into());
    }
    let structure = synthetic::structure(residues);
    let atoms = u64::from(structure.atom_count());
    match case {
        "scene_only" => {
            let (record, built) =
                ResourceRecord::measure(case, atoms, || Scene::from_structure(&structure));
            built?;
            Ok(record)
        }
        "add_one_representation" | "add_eight_representations" => {
            let mut built = Scene::from_structure(&structure)?;
            let count = if case == "add_one_representation" {
                1
            } else {
                8
            };
            let (record, result) =
                ResourceRecord::measure(case, atoms, || -> Result<(), Box<dyn Error>> {
                    for _ in 0..count {
                        built.add(rep::spacefill(sel::all()))?;
                    }
                    Ok(())
                });
            result?;
            Ok(record)
        }
        "opacity_edits" | "visibility_edits" => {
            let mut built = Scene::from_structure(&structure)?;
            let id = built.add(rep::spacefill(sel::all()))?;
            let (record, result) =
                ResourceRecord::measure(case, atoms, || -> Result<(), Box<dyn Error>> {
                    for step in 0..EDIT_COUNT {
                        if case == "opacity_edits" {
                            built.set_opacity(id, OPACITY_RAMP[step % OPACITY_RAMP.len()])?;
                        } else {
                            built.set_visible(id, step % 2 == 0)?;
                        }
                    }
                    Ok(())
                });
            result?;
            Ok(record)
        }
        _ => {
            let (record, result) =
                ResourceRecord::measure(case, atoms, || -> Result<Scene, Box<dyn Error>> {
                    let mut built = Scene::from_structure(&structure)?;
                    match case {
                        "one_representation" => {
                            built.add(rep::spacefill(sel::all()))?;
                        }
                        "eight_representations" => {
                            for _ in 0..8 {
                                built.add(rep::spacefill(sel::all()))?;
                            }
                        }
                        "shared_selection" | "distinct_selections" => {
                            built.add(rep::cartoon(sel::all()))?;
                            let target = if case == "shared_selection" {
                                sel::all()
                            } else {
                                sel::backbone()
                            };
                            built.add(rep::spacefill(target))?;
                        }
                        _ => return Err(io::Error::other("unhandled resource case").into()),
                    }
                    Ok(built)
                });
            result?;
            Ok(record)
        }
    }
}
