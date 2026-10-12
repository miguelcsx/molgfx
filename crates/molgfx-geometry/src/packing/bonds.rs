//! Static and caller-streamed bond instance packing.

use super::PackingError;
use molgfx_core::{
    AtomSelection, BondGpu, BondStyle, EntityId, EntityKind, PlacedStructure, Representation,
    RepresentationKind,
};
use num_traits::ToPrimitive as _;

#[cfg(test)]
#[path = "bond_pack_tests.rs"]
mod tests;

/// Builds an original-row to compacted-instance map in caller-owned storage.
/// Missing rows carry `u32::MAX`. Work is `O(table_len + selected)` and runs
/// only when a representation changes.
///
/// # Errors
///
/// Returns [`PackingError`] when a compacted offset exceeds `u32`.
pub fn build_compaction_map(
    atoms: &[molgfx_core::AtomGpu],
    table_len: u32,
    out: &mut Vec<u32>,
) -> Result<(), PackingError> {
    out.clear();
    out.resize(table_len as usize, u32::MAX);
    for (compact, atom) in atoms.iter().enumerate() {
        let Some((EntityKind::Atom, source)) = atom.entity_id.unpack() else {
            continue;
        };
        let Some(slot) = out.get_mut(source as usize) else {
            continue;
        };
        *slot = gpu_index("atom compaction", compact)?;
    }
    Ok(())
}

/// Builds a source-row to selected-row map without packing atom records.
///
/// This is the dynamic-topology fast path: changing connectivity rebuilds one
/// reused `u32` scratch row but does not recolor or re-upload stable atoms.
///
/// # Errors
///
/// Returns [`PackingError`] when the compacted selection exceeds `u32`.
pub fn build_selection_compaction(
    selection: &AtomSelection,
    table_len: u32,
    out: &mut Vec<u32>,
) -> Result<(), PackingError> {
    out.clear();
    out.resize(table_len as usize, u32::MAX);
    let mut compact = 0u32;
    let mut overflow = None;
    selection.for_each(table_len, |source| {
        if overflow.is_some() {
            return;
        }
        if let Some(slot) = out.get_mut(source as usize) {
            *slot = compact;
            match compact.checked_add(1) {
                Some(next) => compact = next,
                None => {
                    overflow = Some(PackingError::IndexOverflow {
                        resource: "selection compaction",
                        index: u64::from(compact) + 1,
                    });
                }
            }
        }
    });
    match overflow {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Packs bonds whose endpoints both survived atom compaction. Endpoints index
/// compact atom records, so capsule shaders reuse atom addressing and colors.
///
/// # Errors
///
/// Returns [`PackingError`] when a compacted endpoint or bond row cannot fit
/// its GPU field.
pub fn pack_bonds(
    placed: &PlacedStructure,
    representation: &Representation,
    compaction: &[u32],
    out: &mut Vec<BondGpu>,
) -> Result<(), PackingError> {
    out.clear();
    if representation.kind == RepresentationKind::DynamicBonds {
        return super::distance::pack_distance_bonds(placed, representation, compaction, out);
    }
    if !matches!(
        representation.kind,
        RepresentationKind::BallAndStick
            | RepresentationKind::Licorice
            | RepresentationKind::Bonds
            | RepresentationKind::Lines
    ) {
        return Ok(());
    }
    if let Some(topology) = placed.bond_topology() {
        return pack_dynamic(topology, representation, compaction, out);
    }
    let coordinates = placed.source.coordinates();
    let bonds = &placed.source.topology().bonds;
    for (source_index, bond) in bonds.iter().enumerate() {
        let (Some(&atom_a), Some(&atom_b)) = (
            compaction.get(bond.atoms[0] as usize),
            compaction.get(bond.atoms[1] as usize),
        ) else {
            continue;
        };
        if atom_a == u32::MAX || atom_b == u32::MAX {
            continue;
        }
        let order = bond.gpu_order();
        let entity = bond_entity(EntityKind::Bond, source_index)?;
        // Only a multiple bond separates its strands, so the plane is computed
        // once for those and reused by every strand.
        let plane = (order > 1)
            .then(|| bond_plane_turns(coordinates, bonds, bond.atoms[0], bond.atoms[1]))
            .flatten();
        for variant in 0..order {
            let style = plane.map_or_else(
                || BondStyle::new(order, bond.aromatic, bond.metal, variant),
                |turns| {
                    BondStyle::new(order, bond.aromatic, bond.metal, variant)
                        .with_plane_turns(turns)
                },
            );
            out.push(BondGpu::with_style(
                atom_a,
                atom_b,
                representation.params.bond_radius,
                style,
                entity,
            ));
        }
    }
    Ok(())
}

fn pack_dynamic(
    topology: &molgfx_core::BondTopologySegment,
    representation: &Representation,
    compaction: &[u32],
    out: &mut Vec<BondGpu>,
) -> Result<(), PackingError> {
    for (source_index, active) in topology.bonds().enumerate() {
        if active.weight() <= 0.0 {
            continue;
        }
        let bond = active.bond();
        let [source_a, source_b] = bond.atoms();
        let (Some(&atom_a), Some(&atom_b)) = (
            compaction.get(source_a as usize),
            compaction.get(source_b as usize),
        ) else {
            continue;
        };
        if atom_a == u32::MAX || atom_b == u32::MAX {
            continue;
        }
        out.push(BondGpu::new(
            atom_a,
            atom_b,
            representation.params.bond_radius * active.weight(),
            bond.is_aromatic(),
            bond_entity(EntityKind::DynamicBond, source_index)?,
        ));
    }
    Ok(())
}

/// The in-plane reference direction a multiple bond's strands are displaced
/// along, packed as a turn about the bond axis.
///
/// The plane is fixed by a third atom bonded to one endpoint, exactly as the
/// reference engines do — a delocalised substituent for a carbonyl, a ring
/// neighbour for an aromatic bond. The packed value is the angle of the first
/// such neighbour's projection about the axis, measured from the same
/// deterministic reference `bond_perpendicular` derives, so the shader can
/// recover the direction from the axis alone and the value survives a rigid
/// placement transform unchanged. `None` when neither endpoint has a third
/// neighbour: a genuinely terminal bond has no plane to honour.
fn bond_plane_turns(
    coordinates: &[[f32; 3]],
    bonds: &[molgfx_core::SourceBond],
    atom_a: u32,
    atom_b: u32,
) -> Option<u16> {
    let axis = axis_between(coordinates, atom_a, atom_b)?;
    for endpoint in [atom_a, atom_b] {
        let neighbour = first_other_neighbour(bonds, endpoint, [atom_a, atom_b])?;
        let offset = neighbour_offset(coordinates, endpoint, neighbour)?;
        if let Some(turns) = plane_turn_of(axis, offset) {
            return Some(turns);
        }
    }
    None
}

/// The unit vector from `a` to `b`, or `None` when either is absent or they
/// coincide.
fn axis_between(coordinates: &[[f32; 3]], a: u32, b: u32) -> Option<[f32; 3]> {
    let first = coordinates.get(a as usize)?;
    let second = coordinates.get(b as usize)?;
    let delta = [
        second[0] - first[0],
        second[1] - first[1],
        second[2] - first[2],
    ];
    let length = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
    if !length.is_finite() || length < 1.0e-6 {
        return None;
    }
    Some([delta[0] / length, delta[1] / length, delta[2] / length])
}

/// The first bonded neighbour of `atom` that is neither endpoint of the bond
/// under consideration.
fn first_other_neighbour(
    bonds: &[molgfx_core::SourceBond],
    atom: u32,
    excluded: [u32; 2],
) -> Option<u32> {
    bonds.iter().find_map(|bond| {
        let [first, second] = bond.atoms;
        if first == atom && !excluded.contains(&second) {
            Some(second)
        } else if second == atom && !excluded.contains(&first) {
            Some(first)
        } else {
            None
        }
    })
}

/// The vector from `atom` to `neighbour`, or `None` on missing coordinates or
/// a coincident pair.
fn neighbour_offset(coordinates: &[[f32; 3]], atom: u32, neighbour: u32) -> Option<[f32; 3]> {
    let from = coordinates.get(atom as usize)?;
    let to = coordinates.get(neighbour as usize)?;
    let offset = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let length = (offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2]).sqrt();
    (length.is_finite() && length >= 1.0e-6).then_some(offset)
}

/// The angle of `offset`'s in-plane projection about `axis`, measured from the
/// reference `bond_perpendicular` derives, as a 16-bit fraction of a turn.
///
/// `None` when the projection is too short to define a direction, which means
/// the third atom is collinear with the bond and carries no plane information.
fn plane_turn_of(axis: [f32; 3], offset: [f32; 3]) -> Option<u16> {
    let reference = perpendicular_to(axis)?;
    // Project the third atom's direction onto the plane normal to the bond:
    // subtract the component along the axis, not along the reference (which is
    // already perpendicular, so its projection would remove nothing).
    let along = dot(offset, axis);
    let projection = sub(&offset, &scale(&axis, along));
    let length = (projection[0] * projection[0]
        + projection[1] * projection[1]
        + projection[2] * projection[2])
        .sqrt();
    if !length.is_finite() || length < 1.0e-6 {
        return None;
    }
    // `reference` and `basis_y` are an orthonormal basis of the plane, so the
    // angle is measured once, in the same sense the shader turns by.
    let basis_y = cross(axis, reference);
    let unit = [
        projection[0] / length,
        projection[1] / length,
        projection[2] / length,
    ];
    let angle = dot(basis_y, unit).atan2(dot(reference, unit));
    let turns = angle.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
    let quantized = (turns * 65_536.0).round().to_u32()?;
    // A turn of exactly one rounds up to the modulus, which wraps to zero.
    u16::try_from(quantized % 65_536).ok()
}

/// The deterministic unit perpendicular to `axis` the engine draws with, so the
/// CPU and the shader agree on the measurement origin.
fn perpendicular_to(axis: [f32; 3]) -> Option<[f32; 3]> {
    let absolute = [axis[0].abs(), axis[1].abs(), axis[2].abs()];
    let reference = if absolute[2] > absolute[0] && absolute[2] > absolute[1] {
        [1.0, 0.0, 0.0]
    } else if absolute[1] > absolute[0] {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let perpendicular = cross(axis, reference);
    let length = (perpendicular[0] * perpendicular[0]
        + perpendicular[1] * perpendicular[1]
        + perpendicular[2] * perpendicular[2])
        .sqrt();
    if !length.is_finite() || length < 1.0e-6 {
        return None;
    }
    Some([
        perpendicular[0] / length,
        perpendicular[1] / length,
        perpendicular[2] / length,
    ])
}

fn sub(a: &[f32; 3], b: &[f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: &[f32; 3], factor: f32) -> [f32; 3] {
    [a[0] * factor, a[1] * factor, a[2] * factor]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn gpu_index(resource: &'static str, index: usize) -> Result<u32, PackingError> {
    let diagnostic_index = u64::try_from(index)
        .into_iter()
        .fold(u64::MAX, |_, value| value);
    u32::try_from(index).map_err(|_| PackingError::IndexOverflow {
        resource,
        index: diagnostic_index,
    })
}

fn bond_entity(kind: EntityKind, index: usize) -> Result<EntityId, PackingError> {
    let index = u64::try_from(index).map_err(|_| PackingError::IndexOverflow {
        resource: "bond source",
        index: u64::MAX,
    })?;
    EntityId::pack(kind, index).map_err(Into::into)
}
