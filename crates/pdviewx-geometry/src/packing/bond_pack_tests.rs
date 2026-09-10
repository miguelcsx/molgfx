use super::*;
use crate::{pack_atoms, packing::pack::tests::fixture_structure};
use pdviewx_core::{
    AtomSelection, BondTopologyFrame, BondTopologySegment, EntityKind, RepresentationKind, Scene,
    TopologyBond,
};
use std::sync::Arc;

#[test]
fn compaction_maps_source_rows_to_packed_indices() {
    let structure = fixture_structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let selection = scene.add_selection(AtomSelection::Sparse(vec![0, 2]));
    let handle = scene
        .represent(selection, RepresentationKind::Spacefill)
        .unwrap_or_else(|error| panic!("{error}"));
    let table = scene.first_atoms().unwrap_or_else(|| panic!("atoms"));
    let representation = scene
        .representation(handle)
        .unwrap_or_else(|| panic!("representation"));
    let mut atoms = Vec::new();
    pack_atoms(
        table,
        representation,
        &AtomSelection::Sparse(vec![0, 2]),
        &mut atoms,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let mut map = Vec::new();
    build_compaction_map(&atoms, table.len(), &mut map).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(map, vec![0, u32::MAX, 1]);
}

#[test]
fn bond_packing_remaps_endpoints_and_keeps_only_selected_edges() {
    let structure = fixture_structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let selection = scene.add_selection(AtomSelection::Sparse(vec![1, 2]));
    let handle = scene
        .represent(selection, RepresentationKind::BallAndStick)
        .unwrap_or_else(|error| panic!("{error}"));
    let table = scene.first_atoms().unwrap_or_else(|| panic!("atoms"));
    let representation = scene
        .representation(handle)
        .unwrap_or_else(|| panic!("representation"));
    let mut atoms = Vec::new();
    pack_atoms(
        table,
        representation,
        &AtomSelection::Sparse(vec![1, 2]),
        &mut atoms,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let mut map = Vec::new();
    build_compaction_map(&atoms, table.len(), &mut map).unwrap_or_else(|error| panic!("{error}"));
    let placed = scene
        .structures()
        .next()
        .map_or_else(|| panic!("structure"), |(_, placed)| placed);
    let mut bonds = Vec::new();
    pack_bonds(placed, representation, &map, &mut bonds).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(bonds.len(), 1);
    assert_eq!((bonds[0].atom_a, bonds[0].atom_b), (0, 1));
    assert!(bonds[0].is_aromatic());
}

#[test]
fn lines_reuse_the_indexed_bond_path() {
    let structure = fixture_structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let selection = scene.add_selection(AtomSelection::All);
    let handle = scene
        .represent(selection, RepresentationKind::Lines)
        .unwrap_or_else(|error| panic!("{error}"));
    let table = scene.first_atoms().unwrap_or_else(|| panic!("atoms"));
    let representation = scene
        .representation(handle)
        .unwrap_or_else(|| panic!("representation"));
    let mut atoms = Vec::new();
    pack_atoms(table, representation, &AtomSelection::All, &mut atoms)
        .unwrap_or_else(|error| panic!("{error}"));
    let mut map = Vec::new();
    build_compaction_map(&atoms, table.len(), &mut map).unwrap_or_else(|error| panic!("{error}"));
    let placed = scene
        .structures()
        .next()
        .map_or_else(|| panic!("structure"), |(_, placed)| placed);
    let mut bonds = Vec::new();
    pack_bonds(placed, representation, &map, &mut bonds).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(bonds.len(), 2);
}

#[test]
fn dynamic_connectivity_packs_one_weighted_union_with_distinct_identity() {
    let structure = fixture_structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let selection = scene.add_selection(AtomSelection::All);
    let handle = scene
        .represent(selection, RepresentationKind::BallAndStick)
        .unwrap_or_else(|error| panic!("{error}"));
    let owner = scene
        .structures()
        .next()
        .map_or_else(|| panic!("structure"), |(handle, _)| handle);
    let start_bond = TopologyBond::new(0, 1, false).unwrap_or_else(|error| panic!("{error}"));
    let end_bond = TopologyBond::new(1, 2, true).unwrap_or_else(|error| panic!("{error}"));
    let start = BondTopologyFrame::new(0, 0.0, 3, Arc::from([start_bond]), "start")
        .unwrap_or_else(|error| panic!("{error}"));
    let end = BondTopologyFrame::new(1, 1.0, 3, Arc::from([end_bond]), "end")
        .unwrap_or_else(|error| panic!("{error}"));
    let segment =
        BondTopologySegment::new(start, end, 0.25).unwrap_or_else(|error| panic!("{error}"));
    scene
        .set_bond_topology_segment(owner, segment)
        .unwrap_or_else(|error| panic!("{error}"));
    let table = scene.first_atoms().unwrap_or_else(|| panic!("atoms"));
    let representation = scene
        .representation(handle)
        .unwrap_or_else(|| panic!("representation"));
    let mut atoms = Vec::new();
    pack_atoms(table, representation, &AtomSelection::All, &mut atoms)
        .unwrap_or_else(|error| panic!("{error}"));
    let mut map = Vec::new();
    build_compaction_map(&atoms, table.len(), &mut map).unwrap_or_else(|error| panic!("{error}"));
    let placed = scene
        .structures()
        .next()
        .map_or_else(|| panic!("structure"), |(_, placed)| placed);
    let mut bonds = Vec::new();
    pack_bonds(placed, representation, &map, &mut bonds).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(bonds.len(), 2);
    assert_eq!(
        bonds[0].draw_radius().to_bits(),
        (representation.params.bond_radius * 0.75).to_bits()
    );
    assert_eq!(
        bonds[1].draw_radius().to_bits(),
        (representation.params.bond_radius * 0.25).to_bits()
    );
    assert_eq!(
        bonds[0].entity_id.unpack(),
        Some((EntityKind::DynamicBond, 0))
    );
    assert_eq!(
        bonds[1].entity_id.unpack(),
        Some((EntityKind::DynamicBond, 1))
    );
}

#[test]
fn bond_identity_rejects_rows_above_the_chunk_local_limit() {
    let above_limit = usize::try_from(u64::from(EntityId::MAX_INDEX) + 1)
        .unwrap_or_else(|error| panic!("{error}"));
    let error = bond_entity(EntityKind::Bond, above_limit)
        .expect_err("an unencodable source row must fail");
    assert!(matches!(error, PackingError::Entity(_)));

    if let Ok(above_u32) = usize::try_from(u64::from(u32::MAX) + 1) {
        let error = gpu_index("test compaction", above_u32)
            .expect_err("a compacted row wider than u32 must fail");
        assert!(matches!(error, PackingError::IndexOverflow { .. }));
    }
}
