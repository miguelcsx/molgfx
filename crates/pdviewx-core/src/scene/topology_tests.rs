use super::*;
use crate::{BondTopologyFrame, EntityKind, EntityRef, ProvenanceDetail, TopologyBond};
use std::sync::Arc;

fn segment(atom_count: u32) -> BondTopologySegment {
    let start_bond = TopologyBond::new(0, 1, false).unwrap_or_else(|error| panic!("{error}"));
    let end_bond = TopologyBond::new(1, 2, false).unwrap_or_else(|error| panic!("{error}"));
    let start = BondTopologyFrame::new(0, 0.0, atom_count, Arc::from([start_bond]), "start")
        .unwrap_or_else(|error| panic!("{error}"));
    let end = BondTopologyFrame::new(1, 1.0, atom_count, Arc::from([end_bond]), "end")
        .unwrap_or_else(|error| panic!("{error}"));
    BondTopologySegment::new(start, end, 0.0).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn scene_validates_samples_and_restores_static_connectivity() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::new();
    let handle = scene
        .add_structure(&structure)
        .unwrap_or_else(|error| panic!("{error}"));
    let atom_count = scene
        .structure(handle)
        .map_or(0, |placed| placed.atoms.len());
    scene
        .set_bond_topology_segment(handle, segment(atom_count))
        .unwrap_or_else(|error| panic!("{error}"));
    let revision = scene
        .structure(handle)
        .map_or(0, crate::PlacedStructure::bond_topology_revision);
    scene
        .set_bond_topology_time(handle, 0.5)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(
        scene
            .structure(handle)
            .is_some_and(|placed| placed.bond_topology_revision() > revision)
    );
    let provenance = scene.provenance(EntityRef {
        structure: handle,
        kind: EntityKind::DynamicBond,
        index: 0,
    });
    assert!(matches!(
        provenance.map(|value| value.detail),
        Some(ProvenanceDetail::DynamicBond(_))
    ));
    assert!(matches!(scene.clear_bond_topology(handle), Ok(true)));
    assert!(
        scene
            .structure(handle)
            .is_some_and(|placed| placed.bond_topology().is_none())
    );
}

#[test]
fn scene_rejects_topology_for_a_different_atom_table() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::new();
    let handle = scene
        .add_structure(&structure)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(scene.set_bond_topology_segment(handle, segment(3)).is_err());
}
