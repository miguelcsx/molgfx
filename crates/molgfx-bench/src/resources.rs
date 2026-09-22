//! The measured cases: what a representative scene costs in resident bytes.
//!
//! Each case reports allocations, reallocations, bytes requested and bytes
//! still live. `bytes_per_atom` is the figure to watch: it says whether a scene
//! is dense or whether something is holding a second copy of the molecule.
//!
//! Sizes span a ligand, a domain and a small complex, so a per-atom figure that
//! drifts with size is a copy that scales, and a flat one is a constant.

use crate::synthetic;
use molgfx::{Scene, rep, sel};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, Stats};

/// Edits each edit case performs, so a per-edit allocation is visible.
pub const EDIT_COUNT: u32 = 64;

/// Edits run once, for a case that measures a cold edit against a warm one.
pub const SINGLE_EDIT: u32 = 1;

/// Residue counts spanning a ligand, a domain and a small complex.
pub const SIZES: [usize; 3] = [64, 1_024, 8_192];

/// The cases this instrument measures.
pub const CASES: [&str; 8] = [
    "scene_only",
    "add_one_representation",
    "one_representation",
    "eight_representations",
    "shared_selection",
    "distinct_selections",
    "opacity_edits",
    "visibility_edits",
];

/// One measured case: what it allocated in total and what it still holds.
#[derive(Clone, Copy, Debug)]
pub struct ResourceRecord {
    /// The case's name, as selected on the command line.
    pub case: &'static str,
    /// Atoms in the scene the case measured.
    pub atoms: u64,
    /// Allocation calls the case made.
    pub allocations: usize,
    /// Reallocation calls, which say whether a buffer was grown blindly.
    pub reallocations: usize,
    /// Bytes requested in total, including anything later freed.
    pub allocated_bytes: usize,
    /// Bytes still live after the case, which is what a scene retains.
    pub live_bytes: isize,
    /// Live bytes per atom; a figure that drifts with size is a scaling copy.
    pub bytes_per_atom: f64,
}

impl ResourceRecord {
    /// Measures one operation, returning what it allocated and retained.
    ///
    /// The region opens after any fixture exists, so a fixture's own cost is not
    /// attributed to the operation under measurement.
    pub fn measure<T>(case: &'static str, atoms: u64, operation: impl FnOnce() -> T) -> (Self, T) {
        let region = Region::new(&INSTRUMENTED_SYSTEM);
        let value = operation();
        let Stats {
            allocations,
            reallocations,
            bytes_allocated,
            bytes_deallocated,
            ..
        } = region.change();
        let live_bytes = signed(bytes_allocated) - signed(bytes_deallocated);
        let record = Self {
            case,
            atoms,
            allocations,
            reallocations,
            allocated_bytes: bytes_allocated,
            live_bytes,
            bytes_per_atom: ratio(live_bytes, atoms),
        };
        (record, value)
    }

    /// One machine-readable line, so two revisions compare without prose.
    #[must_use]
    pub fn line(self) -> String {
        format!(
            "{{\"case\":\"{}\",\"atoms\":{},\"allocations\":{},\"reallocations\":{},\
             \"allocated_bytes\":{},\"live_bytes\":{},\"bytes_per_atom\":{:.2}}}",
            self.case,
            self.atoms,
            self.allocations,
            self.reallocations,
            self.allocated_bytes,
            self.live_bytes,
            self.bytes_per_atom
        )
    }
}

/// Live bytes per atom, the figure that says whether a scene is dense.
///
/// The quotient is a reported figure, never an argument to anything, so the
/// widening conversions are exact in every range a scene can reach.
#[allow(clippy::cast_precision_loss)]
fn ratio(live_bytes: isize, atoms: u64) -> f64 {
    if atoms == 0 {
        return 0.0;
    }
    live_bytes as f64 / atoms as f64
}

fn signed(value: usize) -> isize {
    isize::try_from(value).unwrap_or(isize::MAX)
}

/// Runs the selected case, or every case when `selected` is `all`.
#[must_use]
pub fn run(selected: &str) -> Vec<ResourceRecord> {
    let cases: Vec<&str> = if selected == "all" {
        CASES.to_vec()
    } else {
        CASES
            .iter()
            .copied()
            .filter(|case| *case == selected)
            .collect()
    };
    let mut records = Vec::new();
    for case in cases {
        for residues in SIZES {
            records.push(run_one(case, residues));
        }
    }
    records
}

fn run_one(case: &'static str, residues: usize) -> ResourceRecord {
    // The fixture is built before measurement, so its cost is not attributed to
    // the scene under test.
    let structure = synthetic::structure(residues);
    let atoms = u64::from(structure.atom_count());
    let scene = || Scene::from_structure(&structure).ok();
    let represented = |target| {
        let mut built = scene()?;
        let _ = built.add(rep::spacefill(target));
        Some(built)
    };

    match case {
        "scene_only" => ResourceRecord::measure(case, atoms, scene).0,
        "one_representation" => ResourceRecord::measure(case, atoms, || represented(sel::all())).0,
        "add_one_representation" => {
            // The scene is built before measurement, so this is the cost of one
            // representation attaching to a scene that already exists.
            let Some(mut built) = scene() else {
                panic!("scene builds");
            };
            ResourceRecord::measure(case, atoms, || {
                let _ = built.add(rep::spacefill(sel::all()));
            })
            .0
        }
        "eight_representations" => {
            // Eight representations over one selection share one packed record
            // set, so the resident bytes must not grow with the count.
            ResourceRecord::measure(case, atoms, || {
                let mut built = scene()?;
                for _ in 0..8 {
                    let _ = built.add(rep::spacefill(sel::all()));
                }
                Some(built)
            })
            .0
        }
        "shared_selection" => {
            ResourceRecord::measure(case, atoms, || {
                let mut built = scene()?;
                let _ = built.add(rep::cartoon(sel::all()));
                let _ = built.add(rep::spacefill(sel::all()));
                Some(built)
            })
            .0
        }
        "distinct_selections" => {
            // Two different queries over one structure must not alias, so this
            // case is the control for the sharing above.
            ResourceRecord::measure(case, atoms, || {
                let mut built = scene()?;
                let _ = built.add(rep::cartoon(sel::all()));
                let _ = built.add(rep::spacefill(sel::backbone()));
                Some(built)
            })
            .0
        }
        "opacity_edits" => {
            let Some(mut built) = scene() else {
                panic!("scene builds");
            };
            let id = match built.add(rep::spacefill(sel::all())) {
                Ok(id) => id,
                Err(error) => panic!("representation adds: {error}"),
            };
            // An appearance edit writes one uniform block, so it must not
            // allocate a scene's worth of state per edit.
            ResourceRecord::measure(case, atoms, || {
                for step in 0..EDIT_COUNT {
                    let opacity = 0.1 + f32::from(u16::try_from(step).unwrap_or(0)) * 0.01;
                    let _ = built.set_opacity(id, opacity);
                }
            })
            .0
        }
        "visibility_edits" => {
            let Some(mut built) = scene() else {
                panic!("scene builds");
            };
            let id = match built.add(rep::spacefill(sel::all())) {
                Ok(id) => id,
                Err(error) => panic!("representation adds: {error}"),
            };
            // Hiding retains resources, so toggling must not allocate either.
            ResourceRecord::measure(case, atoms, || {
                for step in 0..EDIT_COUNT {
                    let _ = built.set_visible(id, step % 2 == 0);
                }
            })
            .0
        }
        other => panic!("unknown resource case: {other}"),
    }
}
