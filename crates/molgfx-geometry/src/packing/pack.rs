//! Instance-record packing.
//!
//! Packs columnar per-atom state into contiguous GPU instance records in one
//! `O(selected)` pass into a reused caller-owned scratch vector.

use super::ColorContext;
use super::PackingError;
use molgfx_core::{
    AtomGpu, AtomProperty, AtomSelection, AtomTable, ColorOverlay, ColorScheme, EntityId,
    EntityKind, PropertyAppearance, Representation, RepresentationKind, SurfaceKind, SurfaceStyle,
};
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
    pack_atoms_inner(table, representation, selection, out)
}

/// Compact representation state used while recolouring generated splines.
#[derive(Clone, Copy, Debug)]
pub struct RibbonColoring {
    /// Colour mapping.
    pub color: ColorScheme,
    /// Selection-scoped schemes overriding `color`.
    pub overlay: Option<ColorOverlay>,
    /// Optional physical appearance mapping.
    pub appearance: Option<PropertyAppearance>,
    /// Base representation opacity.
    pub opacity: u8,
}

fn pack_atoms_inner(
    table: &AtomTable,
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
            // The record carries the element colour. The scheme is applied on
            // the GPU from property columns, so changing it is a uniform write
            // rather than a repack.
            color: *color,
            element: *element,
            flags: *flag,
            entity_id,
            semantic: *semantic,
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

/// Applies colour and per-guide opacity to generated spline vertices.
pub fn recolor_ribbon(
    vertices: &mut [crate::RibbonVertex],
    table: &AtomTable,
    context: ColorContext<'_>,
    style: RibbonColoring,
) {
    let element_colors = table.color().values();
    let appearance_column = style
        .appearance
        .and_then(|mapping| context.column(mapping.property));
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
        let scheme = context.scheme(style.color, style.overlay, index);
        let mut color = context.color(scheme, element, index);
        color.a = atom_appearance(style.appearance, appearance_column, index)
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
