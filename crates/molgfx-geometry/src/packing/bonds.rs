//! Static and caller-streamed bond instance packing.

use super::PackingError;
use molgfx_core::{
    AtomSelection, BondGpu, BondStyle, EntityId, EntityKind, PlacedStructure, Representation,
    RepresentationKind,
};
use molgfx_math::Vec3;
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
    // Built once and only when a multiple bond needs a plane, so the per-bond
    // neighbour lookup is O(degree) rather than a scan of every bond.
    let neighbours = bonds
        .iter()
        .any(|bond| bond.gpu_order() > 1)
        .then(|| BondNeighbours::build(bonds));
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
        let plane = neighbours
            .as_ref()
            .filter(|_| order > 1)
            .and_then(|index| bond_plane_turns(coordinates, index, bond.atoms[0], bond.atoms[1]));
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
    neighbours: &BondNeighbours,
    atom_a: u32,
    atom_b: u32,
) -> Option<u16> {
    let axis = unit_checked(separation(coordinates, atom_a, atom_b)?)?;
    for endpoint in [atom_a, atom_b] {
        let neighbour = neighbours.first_other(endpoint, [atom_a, atom_b])?;
        let offset = separation(coordinates, endpoint, neighbour)?;
        if let Some(turns) = plane_turn_of(axis, offset) {
            return Some(turns);
        }
    }
    None
}

/// The vector from atom `a` to atom `b`, or `None` on missing coordinates or a
/// coincident pair.
fn separation(coordinates: &[[f32; 3]], a: u32, b: u32) -> Option<Vec3> {
    let from = Vec3::from(*coordinates.get(a as usize)?);
    let to = Vec3::from(*coordinates.get(b as usize)?);
    let delta = to - from;
    direction_length(delta).map(|_| delta)
}

/// The length of `vector`, or `None` when it is non-finite or too short to
/// define a direction.
fn direction_length(vector: Vec3) -> Option<f32> {
    let length = vector.length();
    (length.is_finite() && length >= 1.0e-6).then_some(length)
}

/// `vector` divided by its length, or `None` when it has no direction.
fn unit_checked(vector: Vec3) -> Option<Vec3> {
    direction_length(vector).map(|length| vector / length)
}

/// Bonded neighbours of every atom in bond order, as one flat table.
#[derive(Debug)]
struct BondNeighbours {
    offsets: Vec<usize>,
    neighbours: Vec<u32>,
}

impl BondNeighbours {
    fn build(bonds: &[molgfx_core::SourceBond]) -> Self {
        let atoms = bonds
            .iter()
            .flat_map(|bond| bond.atoms)
            .max()
            .map_or(0, |highest| highest as usize + 1);
        let mut offsets = vec![0usize; atoms + 1];
        for bond in bonds {
            for atom in bond.atoms {
                offsets[atom as usize + 1] += 1;
            }
        }
        for index in 0..atoms {
            offsets[index + 1] += offsets[index];
        }
        let mut cursor = offsets.clone();
        let mut neighbours = vec![0u32; offsets[atoms]];
        for bond in bonds {
            let [first, second] = bond.atoms;
            neighbours[cursor[first as usize]] = second;
            cursor[first as usize] += 1;
            neighbours[cursor[second as usize]] = first;
            cursor[second as usize] += 1;
        }
        Self {
            offsets,
            neighbours,
        }
    }

    /// The first bonded neighbour of `atom` that is neither endpoint of the
    /// bond under consideration.
    fn first_other(&self, atom: u32, excluded: [u32; 2]) -> Option<u32> {
        let atom = atom as usize;
        let range = *self.offsets.get(atom)?..*self.offsets.get(atom + 1)?;
        self.neighbours
            .get(range)?
            .iter()
            .copied()
            .find(|neighbour| !excluded.contains(neighbour))
    }
}

/// The angle of `offset`'s in-plane projection about `axis`, measured from the
/// reference `bond_perpendicular` derives, as a 16-bit fraction of a turn.
///
/// `None` when the projection is too short to define a direction, which means
/// the third atom is collinear with the bond and carries no plane information.
fn plane_turn_of(axis: Vec3, offset: Vec3) -> Option<u16> {
    let reference = perpendicular_to(axis)?;
    // Project the third atom's direction onto the plane normal to the bond:
    // subtract the component along the axis, not along the reference (which is
    // already perpendicular, so its projection would remove nothing).
    let unit = unit_checked(offset - axis * offset.dot(axis))?;
    // `reference` and `basis_y` are an orthonormal basis of the plane, so the
    // angle is measured once, in the same sense the shader turns by.
    let basis_y = axis.cross(reference);
    let angle = basis_y.dot(unit).atan2(reference.dot(unit));
    let turns = angle.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
    let quantized = (turns * 65_536.0).round().to_u32()?;
    // A turn of exactly one rounds up to the modulus, which wraps to zero.
    u16::try_from(quantized % 65_536).ok()
}

/// The deterministic unit perpendicular to `axis` the engine draws with, so the
/// CPU and the shader agree on the measurement origin.
fn perpendicular_to(axis: Vec3) -> Option<Vec3> {
    let absolute = axis.abs();
    let reference = if absolute.z > absolute.x && absolute.z > absolute.y {
        Vec3::X
    } else if absolute.y > absolute.x {
        Vec3::Y
    } else {
        Vec3::Z
    };
    unit_checked(axis.cross(reference))
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
