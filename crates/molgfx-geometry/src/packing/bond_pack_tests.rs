use super::*;
use crate::{pack_atoms, packing::pack::tests::fixture_structure};
use molgfx_core::{
    AtomSelection, BondTopologyFrame, BondTopologySegment, EntityKind, RepresentationKind, Scene,
    TopologyBond,
};
use molgfx_math::Vec3;
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
fn a_multiple_bond_packs_an_in_plane_reference_the_shader_can_reconstruct() {
    // A double bond must be drawn in the plane its substituents define, not
    // about a direction chosen from the bond axis alone. The pack records the
    // third atom's direction as a turn about the axis; reconstructing it the way
    // the shader does must land back on an in-plane, axis-perpendicular vector.
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
    let multiple = bonds
        .iter()
        .find(|bond| bond.order() > 1)
        .copied()
        .unwrap_or_else(|| panic!("the fixture has a double bond"));
    let turns = multiple
        .plane_turns()
        .unwrap_or_else(|| panic!("the double bond carries a plane reference"));

    // Reconstruct exactly as `bond_separation_direction` does. The fixture
    // double bond is CA=O (source atoms 1-2) and its plane is defined by the
    // backbone nitrogen (source atom 0) bonded to CA.
    let coordinates = placed.source.coordinates();
    let carbon = coordinates[1];
    let oxygen = coordinates[2];
    let nitrogen = coordinates[0];
    let axis = (Vec3::from(oxygen) - Vec3::from(carbon)).normalize();
    let reference = perpendicular_to(axis).unwrap_or_else(|| panic!("axis has a perpendicular"));
    let angle = f32::from(turns) / 65_536.0 * std::f32::consts::TAU;
    let basis_y = axis.cross(reference);
    let direction = reference * angle.cos() + basis_y * angle.sin();

    assert!(
        direction.dot(axis).abs() < 1.0e-4,
        "the separation direction is perpendicular to the bond: {}",
        direction.dot(axis)
    );
    let length = direction.length();
    assert!(
        (length - 1.0).abs() < 1.0e-4,
        "the separation direction is unit length: {length}"
    );
    // The substituent that defines the plane must lie along the separation, so
    // the two strands of the double bond sit either side of it.
    let offset = Vec3::from(nitrogen) - Vec3::from(carbon);
    let in_plane = (offset - axis * offset.dot(axis)).normalize();
    assert!(
        in_plane.dot(direction).abs() > 0.999,
        "the packed plane follows the substituent: {}",
        in_plane.dot(direction)
    );
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
    assert_eq!(bonds.len(), 2);
    assert!(
        bonds
            .iter()
            .all(|bond| (bond.atom_a, bond.atom_b) == (0, 1))
    );
    assert!(bonds.iter().all(|bond| bond.is_aromatic()));
    assert_eq!(bonds[0].order(), 2);
    assert_eq!(bonds[1].order(), 2);
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
    assert_eq!(bonds.len(), 3);
    assert_eq!(bonds[0].order(), 1);
    assert_eq!(bonds[1].order(), 2);
    assert_eq!(bonds[2].order(), 2);
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
fn a_multi_bond_packs_one_strand_per_order_with_sequential_variants() {
    let structure = fixture_structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let selection = scene.add_selection(AtomSelection::All);
    let handle = scene
        .represent(selection, RepresentationKind::BallAndStick)
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

    // The fixture carries one SING and one AROM edge, so the single bond has
    // one strand and the aromatic has two, each with its own variant index.
    assert_eq!(bonds[0].order(), 1);
    assert_eq!(bonds[0].variant(), 0);
    assert_eq!(bonds[1].order(), 2);
    assert_eq!(bonds[2].order(), 2);
    assert_eq!(
        [bonds[1].variant(), bonds[2].variant()],
        [0, 1],
        "the two strands of one double bond carry distinct variants"
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

#[test]
fn neighbour_index_matches_a_scan_of_every_bond() {
    use molgfx_core::SourceBond;
    let bonds: Vec<SourceBond> = [[0, 1], [1, 2], [2, 3], [1, 3], [4, 1], [3, 0]]
        .into_iter()
        .map(|atoms| SourceBond {
            atoms,
            ..SourceBond::default()
        })
        .collect();
    let index = BondNeighbours::build(&bonds);
    for atom in 0..6u32 {
        for excluded in [[0, 1], [1, 2], [1, 3], [9, 9]] {
            let scan = bonds.iter().find_map(|bond| {
                let [first, second] = bond.atoms;
                if first == atom && !excluded.contains(&second) {
                    Some(second)
                } else if second == atom && !excluded.contains(&first) {
                    Some(first)
                } else {
                    None
                }
            });
            assert_eq!(
                index.first_other(atom, excluded),
                scan,
                "{atom} {excluded:?}"
            );
        }
    }
}
