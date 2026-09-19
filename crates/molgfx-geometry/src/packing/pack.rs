//! Instance-record packing.
//!
//! Packs columnar per-atom state into contiguous GPU instance records in one
//! `O(selected)` pass into a reused caller-owned scratch vector.

use super::PackingError;
use molgfx_core::{
    AtomGpu, AtomProperty, AtomSelection, AtomTable, ColorScheme, EntityId, EntityKind, Hierarchy,
    PropertyAppearance, Representation, RepresentationKind, SecondaryStructure, SurfaceKind,
    SurfaceStyle,
};
use molgfx_math::{Rgba8, Vec3};
use rayon::prelude::*;

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
    pack_atoms_inner(table, None, None, None, representation, selection, out)
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
    pack_atoms_inner(
        table,
        Some((hierarchy, secondary_structure)),
        properties.color,
        properties.appearance,
        representation,
        selection,
        out,
    )
}

fn pack_atoms_inner(
    table: &AtomTable,
    hierarchy: Option<(&Hierarchy, &[SecondaryStructure])>,
    color_property: Option<&AtomProperty>,
    appearance_property: Option<&AtomProperty>,
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
        let mut color = representation_color(
            representation.color,
            *color,
            hierarchy,
            color_property,
            residues.get(i).copied(),
            i,
        );
        color.a = representation.material.opacity_unorm8();
        let appearance = atom_appearance(representation.appearance, appearance_property, i);
        if let Some((appearance_opacity, _)) = appearance {
            color.a = multiply_unorm8(color.a, appearance_opacity);
        }
        let entity_id =
            EntityId::pack(EntityKind::Atom, u64::from(index)).map_err(PackingError::from)?;
        Ok(Some(AtomGpu {
            radius: if representation.kind == RepresentationKind::Licorice {
                representation.params.bond_radius
            } else {
                radius * scale + surface_inflation
            },
            color,
            element: *element,
            flags: *flag,
            entity_id,
            semantic: appearance.map_or(*semantic, |(_, softness)| {
                pack_softness(*semantic, softness)
            }),
        }))
    };

    if selection.count(table.len()) < MIN_PARALLEL_ATOMS {
        let mut packing_error = None;
        selection.for_each(table.len(), |index| {
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
        // The selected indices in ascending order; the parallel pack writes
        // them back in the same order, which the compacted layout assumes.
        let Ok(count) = usize::try_from(selection.count(table.len())) else {
            return Ok(());
        };
        let mut indices = Vec::with_capacity(count);
        selection.for_each(table.len(), |index| indices.push(index));
        let packed = indices
            .par_iter()
            .with_min_len(PARALLEL_BLOCK)
            .filter_map(|&index| pack_one(index).transpose())
            .collect::<Result<Vec<_>, _>>()?;
        out.extend(packed);
        Ok(())
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
    u8::try_from(product / 255).map_or(u8::MAX, |value| value)
}

fn pack_softness(semantic: u32, softness_pixels: f32) -> u32 {
    if softness_pixels <= 0.0 {
        return semantic & 0x00ff_ffff;
    }
    let quantized = molgfx_math::unorm8(softness_pixels / 8.0);
    (semantic & 0x00ff_ffff) | (u32::from(quantized) << 24)
}

fn chain_color(chain: usize) -> Rgba8 {
    // Colour-vision-deficiency-safe hues chosen against the bright default
    // ground: every entry clears a 2.9:1 luminance contrast at both stops of
    // the backdrop sweep. The lighter members of the qualitative sets these
    // derive from — pale cyan, sand, mid grey — vanish on a lit background, so
    // they are replaced by their darker siblings rather than kept for
    // tradition.
    const PALETTE: [Rgba8; 8] = [
        Rgba8::opaque(51, 34, 136),
        Rgba8::opaque(178, 74, 92),
        Rgba8::opaque(17, 119, 51),
        Rgba8::opaque(133, 124, 40),
        Rgba8::opaque(24, 116, 106),
        Rgba8::opaque(136, 34, 85),
        Rgba8::opaque(59, 110, 163),
        Rgba8::opaque(150, 72, 160),
    ];
    PALETTE[chain % PALETTE.len()]
}

fn categorical_color(index: usize) -> Rgba8 {
    chain_color(index.wrapping_mul(5))
}

fn secondary_color(value: SecondaryStructure) -> Rgba8 {
    match value {
        SecondaryStructure::Helix => Rgba8::opaque(170, 68, 153),
        SecondaryStructure::Strand => Rgba8::opaque(190, 110, 0),
        SecondaryStructure::Turn => Rgba8::opaque(0, 128, 94),
        SecondaryStructure::Coil => Rgba8::opaque(60, 120, 170),
    }
}

/// Packs one bead per residue: a sphere enclosing that residue's selected
/// atoms.
///
/// A bead answers "where is this residue and how big is it" without drawing a
/// thousand atoms, which is what makes it readable on a whole assembly. The
/// radius encloses the residue's own atoms including their van der Waals
/// extent, so a bead never claims less volume than the residue occupies. The
/// entity id is the residue's first selected atom, so picking a bead resolves
/// to real chemistry rather than to a synthetic id.
///
/// # Errors
///
/// Returns [`PackingError`] when a residue source row cannot be encoded.
pub fn pack_residue_beads(
    table: &AtomTable,
    hierarchy: &Hierarchy,
    secondary_structure: &[SecondaryStructure],
    property: Option<&AtomProperty>,
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

    // One pass gathers each residue's centroid; a second grows the radius to
    // enclose it. Residues arrive in table order, so a single sweep suffices.
    let mut current: Option<u32> = None;
    let mut members: Vec<usize> = Vec::new();
    let flush = |members: &mut Vec<usize>, out: &mut Vec<AtomGpu>| -> Result<(), PackingError> {
        let Some(&first) = members.first() else {
            return Ok(());
        };
        let (sum, count) = members
            .iter()
            .fold((Vec3::ZERO, 0.0f32), |(sum, n), index| {
                let point = coords
                    .get(*index)
                    .map_or(Vec3::ZERO, |p| Vec3::from_array(*p));
                (sum + point, n + 1.0)
            });
        let centre = sum / count.max(1.0);
        let mut radius = 0.0f32;
        for index in members.iter() {
            let point = coords.get(*index).map_or(centre, |p| Vec3::from_array(*p));
            let extent = radii.get(*index).copied().map_or(0.0, |value| value);
            radius = radius.max(centre.distance(point) + extent);
        }
        let mut color = representation_color(
            representation.color,
            colors
                .get(first)
                .copied()
                .map_or(Rgba8::opaque(255, 255, 255), |value| value),
            Some((hierarchy, secondary_structure)),
            property,
            residues.get(first).copied(),
            first,
        );
        color.a = representation.material.opacity_unorm8();
        let source = u64::try_from(first).map_err(|_| PackingError::IndexOverflow {
            resource: "residue bead source",
            index: u64::MAX,
        })?;
        out.push(AtomGpu {
            radius: radius * scale,
            color,
            element: elements.get(first).copied().map_or(0, |value| value),
            flags: flags
                .get(first)
                .copied()
                .map_or(molgfx_core::AtomFlags(0), |value| value),
            entity_id: EntityId::pack(EntityKind::Atom, source)?,
            semantic: semantics.get(first).copied().map_or(0, |value| value),
        });
        members.clear();
        Ok(())
    };

    let mut packing_error = None;
    selection.for_each(table.len(), |index| {
        if packing_error.is_some() {
            return;
        }
        let i = index as usize;
        let residue = residues.get(i).copied();
        if current != residue {
            if let Err(error) = flush(&mut members, out) {
                packing_error = Some(error);
                return;
            }
            current = residue;
        }
        members.push(i);
    });
    if let Some(error) = packing_error {
        out.clear();
        return Err(error);
    }
    if let Err(error) = flush(&mut members, out) {
        out.clear();
        return Err(error);
    }
    Ok(())
}
