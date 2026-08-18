//! Instance-record packing.
//!
//! Packs the scene's columnar per-atom state into contiguous GPU instance
//! records, applying the representation's radius scaling. One `O(selected)`
//! pass into a caller-owned scratch vector, so the steady-state path
//! allocates nothing: the scratch grows once and is reused.

use pdviewx_core::{
    AtomGpu, AtomProperty, AtomSelection, AtomTable, BondGpu, ColorScheme, EntityId, EntityKind,
    Hierarchy, PropertyAppearance, Representation, RepresentationKind, SecondaryStructure,
};
use pdviewx_math::{Rgba8, Vec3};

#[cfg(test)]
#[path = "pack_tests.rs"]
mod tests;

/// Packs the selected atoms of one table into instance records, appending
/// to `out` (cleared first). Radius scaling comes from the representation;
/// positions are duplicated into the record for backends that cannot bind
/// the borrowed coordinate column, and gathered from that column otherwise.
pub fn pack_atoms(
    table: &AtomTable,
    representation: &Representation,
    selection: &AtomSelection,
    out: &mut Vec<AtomGpu>,
) {
    pack_atoms_inner(table, None, None, None, representation, selection, out);
}

/// Packs atoms with hierarchy-aware representation coloring.
pub fn pack_atoms_with_hierarchy(
    table: &AtomTable,
    hierarchy: &Hierarchy,
    secondary_structure: &[SecondaryStructure],
    property: Option<&AtomProperty>,
    representation: &Representation,
    selection: &AtomSelection,
    out: &mut Vec<AtomGpu>,
) {
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
    );
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
pub fn pack_atoms_with_properties(
    table: &AtomTable,
    hierarchy: &Hierarchy,
    secondary_structure: &[SecondaryStructure],
    properties: PropertyColumns<'_>,
    representation: &Representation,
    selection: &AtomSelection,
    out: &mut Vec<AtomGpu>,
) {
    pack_atoms_inner(
        table,
        Some((hierarchy, secondary_structure)),
        properties.color,
        properties.appearance,
        representation,
        selection,
        out,
    );
}

fn pack_atoms_inner(
    table: &AtomTable,
    hierarchy: Option<(&Hierarchy, &[SecondaryStructure])>,
    color_property: Option<&AtomProperty>,
    appearance_property: Option<&AtomProperty>,
    representation: &Representation,
    selection: &AtomSelection,
    out: &mut Vec<AtomGpu>,
) {
    out.clear();
    let coords = table.coords().slice();
    let radii = table.radius().values();
    let colors = table.color().values();
    let elements = table.element().values();
    let flags = table.flags().values();
    let semantics = table.semantic().values();
    let residues = table.residue().values();
    let scale = representation.params.radius_scale;

    selection.for_each(table.len(), |index| {
        let i = index as usize;
        let (
            Some(position),
            Some(radius),
            Some(color),
            Some(element),
            Some(flag),
            Some(semantic),
        ) = (
            coords.get(i),
            radii.get(i),
            colors.get(i),
            elements.get(i),
            flags.get(i),
            semantics.get(i),
        )
        else {
            return;
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
        let appearance = atom_appearance(
            representation.appearance,
            appearance_property,
            i,
        );
        if let Some((appearance_opacity, _)) = appearance {
            color.a = multiply_unorm8(color.a, appearance_opacity);
        }
        out.push(AtomGpu {
            position: *position,
            radius: if representation.kind == RepresentationKind::Licorice {
                representation.params.bond_radius
            } else {
                radius * scale
            },
            color,
            element: *element,
            flags: *flag,
            entity_id: EntityId::pack(EntityKind::Atom, index),
            semantic: appearance.map_or(*semantic, |(_, softness)| {
                pack_softness(*semantic, softness)
            }),
        });
    });
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
    Some((quantize_unit(sample.opacity), sample.softness_pixels))
}

fn multiply_unorm8(left: u8, right: u8) -> u8 {
    let product = u16::from(left) * u16::from(right) + 127;
    u8::try_from(product / 255).map_or(u8::MAX, |value| value)
}

fn pack_softness(semantic: u32, softness_pixels: f32) -> u32 {
    if softness_pixels <= 0.0 {
        return semantic & 0x00ff_ffff;
    }
    let quantized = quantize_unit((softness_pixels / 8.0).clamp(0.0, 1.0));
    (semantic & 0x00ff_ffff) | (u32::from(quantized) << 24)
}

fn quantize_unit(value: f32) -> u8 {
    let target = value.clamp(0.0, 1.0) * 255.0;
    let mut low = 0u16;
    let mut high = u16::from(u8::MAX);
    while low < high {
        let middle = (low + high).div_ceil(2);
        if f32::from(middle) <= target {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let upper = (low + 1).min(u16::from(u8::MAX));
    let selected = if target - f32::from(low) < f32::from(upper) - target {
        low
    } else {
        upper
    };
    u8::try_from(selected).map_or(u8::MAX, |value| value)
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

/// Builds an original-row to compacted-instance map in caller-owned storage.
/// Missing rows carry `u32::MAX`. Work is `O(table_len + selected)` and runs
/// only when a representation changes.
pub fn build_compaction_map(atoms: &[AtomGpu], table_len: u32, out: &mut Vec<u32>) {
    out.clear();
    out.resize(table_len as usize, u32::MAX);
    for (compact, atom) in atoms.iter().enumerate() {
        let Some((EntityKind::Atom, source)) = atom.entity_id.unpack() else {
            continue;
        };
        let Some(slot) = out.get_mut(source as usize) else {
            continue;
        };
        *slot = u32::try_from(compact).map_or(u32::MAX, |index| index);
    }
}

/// Packs bonds whose endpoints both survived atom compaction. Endpoints index
/// the compact atom records, so capsule shaders reuse atom addressing and
/// colors. Work is `O(bonds)` into reused storage.
pub fn pack_bonds(
    structure: &pdbiox::Structure,
    representation: &Representation,
    compaction: &[u32],
    out: &mut Vec<BondGpu>,
) {
    out.clear();
    if !matches!(
        representation.kind,
        RepresentationKind::BallAndStick | RepresentationKind::Licorice | RepresentationKind::Lines
    ) {
        return;
    }
    for (source_index, bond) in structure.data().bonds.iter().enumerate() {
        let (Some(&atom_a), Some(&atom_b)) = (
            compaction.get(bond.atom_a.as_usize()),
            compaction.get(bond.atom_b.as_usize()),
        ) else {
            continue;
        };
        if atom_a == u32::MAX || atom_b == u32::MAX {
            continue;
        }
        let source_index = u32::try_from(source_index).map_or(EntityId::MAX_INDEX, |value| value);
        out.push(BondGpu::new(
            atom_a,
            atom_b,
            representation.params.bond_radius,
            bond.order == pdbiox::BondOrder::Aromatic,
            EntityId::pack(EntityKind::Bond, source_index),
        ));
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
pub fn pack_residue_beads(
    table: &AtomTable,
    hierarchy: &Hierarchy,
    secondary_structure: &[SecondaryStructure],
    property: Option<&AtomProperty>,
    representation: &Representation,
    selection: &AtomSelection,
    out: &mut Vec<AtomGpu>,
) {
    out.clear();
    let coords = table.coords().slice();
    let radii = table.radius().values();
    let colors = table.color().values();
    let elements = table.element().values();
    let flags = table.flags().values();
    let semantics = table.semantic().values();
    let residues = table.residue().values();
    let scale = representation.params.radius_scale;

    // One pass gathers each residue's centroid; a second grows the radius to
    // enclose it. Residues arrive in table order, so a single sweep suffices.
    let mut current: Option<u32> = None;
    let mut members: Vec<usize> = Vec::new();
    let flush = |members: &mut Vec<usize>, out: &mut Vec<AtomGpu>| {
        let Some(&first) = members.first() else {
            return;
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
        out.push(AtomGpu {
            position: centre.to_array(),
            radius: radius * scale,
            color,
            element: elements.get(first).copied().map_or(0, |value| value),
            flags: flags
                .get(first)
                .copied()
                .map_or(pdviewx_core::AtomFlags(0), |value| value),
            entity_id: EntityId::pack(
                EntityKind::Atom,
                u32::try_from(first).map_or(u32::MAX, |value| value),
            ),
            semantic: semantics.get(first).copied().map_or(0, |value| value),
        });
        members.clear();
    };

    selection.for_each(table.len(), |index| {
        let i = index as usize;
        let residue = residues.get(i).copied();
        if current != residue {
            flush(&mut members, out);
            current = residue;
        }
        members.push(i);
    });
    flush(&mut members, out);
}
