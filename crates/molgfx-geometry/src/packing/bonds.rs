//! Static and caller-streamed bond instance packing.

use super::PackingError;
use molgfx_core::{
    AtomSelection, BondGpu, EntityId, EntityKind, PlacedStructure, Representation,
    RepresentationKind,
};

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
    if !matches!(
        representation.kind,
        RepresentationKind::BallAndStick | RepresentationKind::Licorice | RepresentationKind::Lines
    ) {
        return Ok(());
    }
    if let Some(topology) = placed.bond_topology() {
        return pack_dynamic(topology, representation, compaction, out);
    }
    for (source_index, bond) in placed.structure.data().bonds.iter().enumerate() {
        let (Some(&atom_a), Some(&atom_b)) = (
            compaction.get(bond.atom_a.as_usize()),
            compaction.get(bond.atom_b.as_usize()),
        ) else {
            continue;
        };
        if atom_a == u32::MAX || atom_b == u32::MAX {
            continue;
        }
        out.push(BondGpu::new(
            atom_a,
            atom_b,
            representation.params.bond_radius,
            bond.order == molframe::BondOrder::Aromatic,
            bond_entity(EntityKind::Bond, source_index)?,
        ));
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
