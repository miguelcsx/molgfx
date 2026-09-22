//! Instance-record packing.
//!
//! Packs columnar per-atom state into contiguous GPU instance records in one
//! `O(selected)` pass into a reused caller-owned scratch vector.

use super::PackingError;
use molgfx_core::{
    AtomGpu, AtomProperty, AtomSelection, AtomTable, CATEGORICAL_COLORS, ColorScheme, EntityId,
    EntityKind, Hierarchy, PropertyAppearance, Representation, RepresentationKind,
    SECONDARY_STRUCTURE_COLORS, SecondaryStructure, SemanticTag, SurfaceKind, SurfaceStyle,
};
use molgfx_math::Rgba8;
use rayon::prelude::*;

mod residue_beads;
pub use residue_beads::pack_residue_beads;

#[cfg(test)]
#[path = "pack_parallel_tests.rs"]
pub(super) mod parallel_tests;
#[cfg(test)]
#[path = "pack_tests.rs"]
pub(super) mod tests;

/// Packs the selected atoms of one table into instance records, appending
/// to `out` (cleared first). Radius scaling comes from the representation;
/// positions stay in the table's coordinate column, which the shader gathers
/// through the record's `entity_id`.
///
/// # Errors
///
/// Returns [`PackingError`] when a selected source row cannot be encoded.
pub fn pack_atoms(
    table: &AtomTable,
    representation: &Representation,
    selection: &AtomSelection,
    out: &mut Vec<AtomGpu>,
) -> Result<(), PackingError> {
    pack_atoms_inner(table, None, representation, selection, out)
}

/// Packs atoms with hierarchy-aware representation coloring.
///
/// # Errors
///
/// Returns [`PackingError`] when a selected source row cannot be encoded.
pub fn pack_atoms_with_hierarchy(
    table: &AtomTable,
    hierarchy: &Hierarchy,
    secondary_structure: &[SecondaryStructure],
    property: Option<&AtomProperty>,
    representation: &Representation,
    selection: &AtomSelection,
    out: &mut Vec<AtomGpu>,
) -> Result<(), PackingError> {
    pack_atoms_with_properties(
        table,
        hierarchy,
        secondary_structure,
        PropertyColumns {
            color: property,
            appearance: None,
        },
        representation,
        selection,
        out,
    )
}

/// Borrowed scientific columns independently driving colour and appearance.
#[derive(Clone, Copy, Debug, Default)]
pub struct PropertyColumns<'a> {
    /// Continuous colour source.
    pub color: Option<&'a AtomProperty>,
    /// Opacity and edge-softness source.
    pub appearance: Option<&'a AtomProperty>,
}

/// Compact representation state used while recolouring generated splines.
#[derive(Clone, Copy, Debug)]
pub struct RibbonColoring {
    /// Colour mapping.
    pub color: ColorScheme,
    /// Optional scientific appearance mapping.
    pub appearance: Option<PropertyAppearance>,
    /// Base representation opacity.
    pub opacity: u8,
}

/// Packs atoms with independent colour and scientific-appearance properties.
///
/// # Errors
///
/// Returns [`PackingError`] when a selected source row cannot be encoded.
pub fn pack_atoms_with_properties(
    table: &AtomTable,
    hierarchy: &Hierarchy,
    secondary_structure: &[SecondaryStructure],
    properties: PropertyColumns<'_>,
    representation: &Representation,
    selection: &AtomSelection,
    out: &mut Vec<AtomGpu>,
) -> Result<(), PackingError> {
    let _ = properties;
    pack_atoms_inner(
        table,
        Some((hierarchy, secondary_structure)),
        representation,
        selection,
        out,
    )
}

fn pack_atoms_inner(
    table: &AtomTable,
    hierarchy: Option<(&Hierarchy, &[SecondaryStructure])>,
    representation: &Representation,
    selection: &AtomSelection,
    out: &mut Vec<AtomGpu>,
) -> Result<(), PackingError> {
    out.clear();
    let coords = table.coords().slice();
    let radii = table.radius().values();
    let colors = table.color().values();
    let elements = table.element().values();
    let flags = table.flags().values();
    let semantics = table.semantic().values();
    let residues = table.residue().values();
    let scale = representation.params.radius_scale.max(0.0);
    let surface_inflation = surface_inflation_of(representation);

    // Both branches compute each record through the same pure closure, so the
    // parallel output is the serial output by construction; the deterministic
    // contract is pinned by `a_parallel_pack_matches_the_serial_pack`.
    let pack_one = |index: u32| -> Result<Option<AtomGpu>, PackingError> {
        let i = index as usize;
        let (Some(_), Some(radius), Some(color), Some(element), Some(flag), Some(semantic)) = (
            coords.get(i),
            radii.get(i),
            colors.get(i),
            elements.get(i),
            flags.get(i),
            semantics.get(i),
        ) else {
            return Ok(None);
        };
        let entity_id =
            EntityId::pack(EntityKind::Atom, u64::from(index)).map_err(PackingError::from)?;
        Ok(Some(AtomGpu {
            radius: if representation.kind == RepresentationKind::Licorice {
                representation.params.bond_radius
            } else {
                radius * scale + surface_inflation
            },
            // The record carries the element colour and the indices every
            // scheme resolves from. The scheme itself is applied on the GPU, so
            // changing it is a uniform write rather than a repack.
            color: *color,
            element: *element,
            flags: *flag,
            entity_id,
            semantic: color_indices(*semantic, hierarchy, residues.get(i).copied()),
        }))
    };

    pack_selection(table.len(), selection, out, &pack_one)
}

fn pack_selection<F>(
    atom_count: u32,
    selection: &AtomSelection,
    out: &mut Vec<AtomGpu>,
    pack_one: &F,
) -> Result<(), PackingError>
where
    F: Fn(u32) -> Result<Option<AtomGpu>, PackingError> + Sync,
{
    if selection.count(atom_count) < MIN_PARALLEL_ATOMS {
        let mut packing_error = None;
        selection.for_each(atom_count, |index| {
            if packing_error.is_some() {
                return;
            }
            match pack_one(index) {
                Ok(Some(record)) => out.push(record),
                Ok(None) => {}
                Err(error) => packing_error = Some(error),
            }
        });
        match packing_error {
            Some(error) => {
                out.clear();
                Err(error)
            }
            None => Ok(()),
        }
    } else {
        // Common contiguous encodings feed rayon directly. Irregular encodings
        // materialize indices once because rayon has no indexed iterator over
        // their compressed representation.
        //
        // Every worker feeds the same pure per-row closure into `par_extend`,
        // which appends partitions in source order, so records land in `out`
        // directly — no intermediate record vector, no final memcpy — and the
        // stream stays byte-identical to the serial one. A rejected row is
        // filtered out and remembered in a shared cell instead, because the
        // iterator handed to `par_extend` cannot carry a failure.
        let packing_error: std::sync::Mutex<Option<PackingError>> = std::sync::Mutex::new(None);
        let collect = |index: u32| -> Option<AtomGpu> {
            match pack_one(index) {
                Ok(record) => record,
                Err(error) => {
                    let Ok(mut recorded) = packing_error.lock() else {
                        return None;
                    };
                    if recorded.is_none() {
                        *recorded = Some(error);
                    }
                    None
                }
            }
        };
        match selection {
            AtomSelection::All => out.par_extend(
                (0..atom_count)
                    .into_par_iter()
                    .with_min_len(PARALLEL_BLOCK)
                    .filter_map(&collect),
            ),
            AtomSelection::Range(range) => out.par_extend(
                (range.start..range.end.min(atom_count))
                    .into_par_iter()
                    .with_min_len(PARALLEL_BLOCK)
                    .filter_map(&collect),
            ),
            AtomSelection::Sparse(indices) => out.par_extend(
                indices
                    .par_iter()
                    .with_min_len(PARALLEL_BLOCK)
                    .filter_map(|&index| collect(index)),
            ),
            _ => {
                let Ok(count) = usize::try_from(selection.count(atom_count)) else {
                    return Ok(());
                };
                let mut indices = Vec::with_capacity(count);
                selection.for_each(atom_count, |index| indices.push(index));
                out.par_extend(
                    indices
                        .par_iter()
                        .with_min_len(PARALLEL_BLOCK)
                        .filter_map(|&index| collect(index)),
                );
            }
        }
        // Nothing inside the lock can panic, so the cell is never poisoned; a
        // poisoned one would mean a worker already unwound the pack, and the
        // unwind is already reporting the failure.
        let recorded = match packing_error.lock() {
            Ok(error) => *error,
            Err(_) => None,
        };
        match recorded {
            // All-or-nothing, exactly as in the serial branch: a rejected row
            // discards every record packed for the selection.
            Some(error) => {
                out.clear();
                Err(error)
            }
            None => Ok(()),
        }
    }
}

/// Selected rows below which packing stays on the calling thread.
///
/// The threshold keeps a ligand from paying for a thread pool; `par_extend`'s
/// order preservation plus the shared per-row closure keep the parallel result
/// byte-identical to the serial one.
const MIN_PARALLEL_ATOMS: u64 = 8192;

/// Elements per parallel chunk, sized so scheduling overhead disappears
/// against the per-atom work.
const PARALLEL_BLOCK: usize = 8192;

fn surface_inflation_of(representation: &Representation) -> f32 {
    if representation.kind == RepresentationKind::Surface
        && representation.params.surface_kind == SurfaceKind::SolventAccessible
        && representation.params.surface_style == SurfaceStyle::Solid
    {
        representation.params.probe_radius.max(0.0)
    } else {
        0.0
    }
}

pub(crate) fn representation_color(
    scheme: ColorScheme,
    element: Rgba8,
    hierarchy: Option<(&Hierarchy, &[SecondaryStructure])>,
    property: Option<&AtomProperty>,
    residue: Option<u32>,
    atom: usize,
) -> Rgba8 {
    match scheme {
        ColorScheme::Uniform(color) => color,
        ColorScheme::ByChain => residue
            .and_then(|residue| hierarchy?.0.chain_of_residue(residue))
            .map_or(element, chain_color),
        ColorScheme::ByResidue => residue
            .and_then(|value| usize::try_from(value).ok())
            .map_or(element, categorical_color),
        ColorScheme::BySecondaryStructure => residue
            .and_then(|value| usize::try_from(value).ok())
            .and_then(|index| hierarchy?.1.get(index).copied())
            .map_or(element, secondary_color),
        ColorScheme::ByProperty { ramp, missing, .. } => property
            .and_then(|value| value.values().get(atom).copied())
            .map_or(missing, |value| ramp.sample(value, missing)),
        _ => element,
    }
}

/// Applies the same representation colour function to generated spline
/// vertices using their stable guide-atom provenance.
pub fn recolor_ribbon(
    vertices: &mut [crate::RibbonVertex],
    table: &AtomTable,
    hierarchy: &Hierarchy,
    secondary_structure: &[SecondaryStructure],
    property: Option<&AtomProperty>,
    scheme: ColorScheme,
    opacity: u8,
) {
    recolor_ribbon_with_appearance(
        vertices,
        table,
        hierarchy,
        secondary_structure,
        PropertyColumns {
            color: property,
            appearance: None,
        },
        RibbonColoring {
            color: scheme,
            appearance: None,
            opacity,
        },
    );
}

/// Applies colour and per-guide opacity to generated spline vertices.
pub fn recolor_ribbon_with_appearance(
    vertices: &mut [crate::RibbonVertex],
    table: &AtomTable,
    hierarchy: &Hierarchy,
    secondary_structure: &[SecondaryStructure],
    properties: PropertyColumns<'_>,
    style: RibbonColoring,
) {
    let element_colors = table.color().values();
    let residues = table.residue().values();
    for vertex in vertices {
        let Some((EntityKind::Atom, index)) = EntityId(vertex.entity_id).unpack() else {
            continue;
        };
        let Ok(index) = usize::try_from(index) else {
            continue;
        };
        let Some(&element) = element_colors.get(index) else {
            continue;
        };
        let mut color = representation_color(
            style.color,
            element,
            Some((hierarchy, secondary_structure)),
            properties.color,
            residues.get(index).copied(),
            index,
        );
        color.a = atom_appearance(style.appearance, properties.appearance, index)
            .map_or(style.opacity, |(appearance_opacity, _)| {
                multiply_unorm8(style.opacity, appearance_opacity)
            });
        vertex.color = color;
    }
}

fn atom_appearance(
    appearance: Option<PropertyAppearance>,
    property: Option<&AtomProperty>,
    atom: usize,
) -> Option<(u8, f32)> {
    let mapping = appearance?;
    let value = match property.and_then(|property| property.values().get(atom)) {
        Some(value) => *value,
        None => f32::NAN,
    };
    let sample = mapping.sample(value);
    Some((molgfx_math::unorm8(sample.opacity), sample.softness_pixels))
}

fn multiply_unorm8(left: u8, right: u8) -> u8 {
    let product = u16::from(left) * u16::from(right) + 127;
    u8::try_from(product / 255)
        .into_iter()
        .fold(u8::MAX, |_, value| value)
}

/// Packs the three palette indices the GPU colour schemes resolve from.
///
/// The chain and residue indices are the values the CPU colour functions
/// already reduce modulo the palette, and the class is the residue's
/// secondary-structure assignment, so the shader reproduces the CPU result
/// exactly rather than approximating it. A missing hierarchy leaves the fields
/// zero, which is the element-colour fallback.
fn color_indices(
    semantic: u32,
    hierarchy: Option<(&Hierarchy, &[SecondaryStructure])>,
    residue: Option<u32>,
) -> u32 {
    let (chain, residue_index, class) = match (hierarchy, residue) {
        (Some((hierarchy, styles)), Some(residue)) => {
            let chain = hierarchy.chain_of_residue(residue).map_or(0, |chain| {
                u32::try_from(chain).map_or(0, |chain| chain & SemanticTag::FIELD_MAX)
            });
            // The palette reduces the residue row modulo its length, so the
            // packed index is that reduction and nothing is lost.
            let residue_index = residue & SemanticTag::FIELD_MAX;
            let class = usize::try_from(residue)
                .ok()
                .and_then(|index| styles.get(index).copied())
                .map_or(0, secondary_class);
            (chain, residue_index, class)
        }
        _ => (0, 0, 0),
    };
    let indices = chain << SemanticTag::CHAIN_SHIFT
        | residue_index << SemanticTag::RESIDUE_SHIFT
        | class << SemanticTag::SECONDARY_SHIFT;
    semantic & (SemanticTag::TAG_MASK | SemanticTag::COLOR_MASK) | indices
}

/// The palette slot a secondary-structure class colours from.
///
/// The classes map to the same palette the CPU path uses, expressed as slots so
/// the shader needs no branch per class.
const fn secondary_class(value: SecondaryStructure) -> u32 {
    match value {
        SecondaryStructure::Unknown => 0,
        SecondaryStructure::Coil => 1,
        SecondaryStructure::Helix => 2,
        SecondaryStructure::Strand => 3,
        SecondaryStructure::Turn => 4,
    }
}

fn chain_color(chain: usize) -> Rgba8 {
    CATEGORICAL_COLORS[chain % CATEGORICAL_COLORS.len()]
}

fn categorical_color(index: usize) -> Rgba8 {
    chain_color(index.wrapping_mul(5))
}

fn secondary_color(value: SecondaryStructure) -> Rgba8 {
    SECONDARY_STRUCTURE_COLORS[secondary_class(value) as usize]
}
