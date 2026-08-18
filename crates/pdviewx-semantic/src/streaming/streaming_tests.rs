use super::*;
use crate::LodScene;
use pdviewx_core::{Primitive, Scene, StructureHandle};
use pdviewx_math::{Camera, Projection, Vec3};

fn structure() -> pdbiox::Structure {
    let cif = "data_lod\n\
loop_\n\
_atom_site.group_PDB\n\
_atom_site.id\n\
_atom_site.type_symbol\n\
_atom_site.label_atom_id\n\
_atom_site.label_alt_id\n\
_atom_site.label_comp_id\n\
_atom_site.label_asym_id\n\
_atom_site.label_entity_id\n\
_atom_site.label_seq_id\n\
_atom_site.Cartn_x\n\
_atom_site.Cartn_y\n\
_atom_site.Cartn_z\n\
_atom_site.occupancy\n\
_atom_site.B_iso_or_equiv\n\
_atom_site.auth_seq_id\n\
_atom_site.auth_asym_id\n\
_atom_site.pdbx_PDB_model_num\n\
ATOM 1 C CA . GLY A 1 1 0 0 0 1 10 1 A 1\n";
    match pdbiox::read_bytes(
        cif.as_bytes().to_vec(),
        Some("lod.cif"),
        &pdbiox::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("LOD fixture parses: {diagnostics:?}"),
    }
}

fn handle() -> StructureHandle {
    let mut scene = Scene::new();
    match scene.add_structure(&structure()) {
        Ok(handle) => handle,
        Err(error) => panic!("LOD fixture enters a scene: {error}"),
    }
}

#[test]
fn stream_planner_retains_high_priority_chunks_under_a_byte_cap() {
    let mut planner = StreamPlanner::new(StreamingBudget {
        max_resident_bytes: 10,
        max_requests_per_frame: 4,
    });
    let requests = [
        ChunkRequest {
            key: ChunkKey {
                structure: handle(),
                level: LodLevel::Residue,
                index: 2,
            },
            priority: 1.0,
            bytes: 6,
        },
        ChunkRequest {
            key: ChunkKey {
                structure: handle(),
                level: LodLevel::Residue,
                index: 1,
            },
            priority: 2.0,
            bytes: 4,
        },
    ];
    let plan = planner.plan(&requests);
    assert_eq!(plan.retain.len(), 2);
    assert_eq!(plan.retain[0].key.index, 1);
    assert_eq!(plan.resident_bytes, 10);
}

#[test]
fn planner_reports_evictions_when_a_visible_set_changes() {
    let structure = handle();
    let mut planner = StreamPlanner::new(StreamingBudget {
        max_resident_bytes: 32,
        max_requests_per_frame: 4,
    });
    let first = ChunkRequest {
        key: ChunkKey {
            structure,
            level: LodLevel::Domain,
            index: 0,
        },
        priority: 1.0,
        bytes: 8,
    };
    assert!(planner.plan(&[first]).evict.is_empty());
    let second = ChunkRequest {
        key: ChunkKey {
            index: 1,
            ..first.key
        },
        ..first
    };
    let plan = planner.plan(&[second]);
    assert_eq!(plan.evict, vec![first.key]);
}

#[test]
fn planner_caps_new_requests_but_keeps_resident_visible_chunks() {
    let structure = handle();
    let mut planner = StreamPlanner::new(StreamingBudget {
        max_resident_bytes: 32,
        max_requests_per_frame: 1,
    });
    let first = ChunkRequest {
        key: ChunkKey {
            structure,
            level: LodLevel::Residue,
            index: 0,
        },
        priority: 1.0,
        bytes: 8,
    };
    let second = ChunkRequest {
        key: ChunkKey {
            index: 1,
            ..first.key
        },
        priority: 2.0,
        bytes: 8,
    };
    assert_eq!(planner.plan(&[first, second]).retain, vec![second]);
    let plan = planner.plan(&[first, second]);
    assert_eq!(plan.retain, vec![second, first]);
}

#[test]
fn lod_scene_reuses_a_coarse_representation_across_frames() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("LOD scene fixture enters a scene: {error}"),
    };
    let Some((structure, _)) = scene.structures().next() else {
        panic!("LOD scene has one structure")
    };
    let index = LodIndex::from_scene(&scene);
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 10_000.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Perspective {
            fov_y: 0.8,
            aspect: 1.0,
            near: 0.1,
            far: 20_000.0,
        },
    };
    let frame = index.select(&camera, [800, 800], LodPolicy::default(), None);
    assert!(
        frame
            .level(structure)
            .is_some_and(|level| level != LodLevel::Atom)
    );
    let mut lod_scene = LodScene::default();
    assert!(lod_scene.apply(&mut scene, &index, &frame).is_ok());
    assert_eq!(lod_scene.primitive_count(), 1);
    assert!(lod_scene.apply(&mut scene, &index, &frame).is_ok());
    assert_eq!(lod_scene.primitive_count(), 1);
    assert_eq!(scene.primitives().count(), 1);
}

#[test]
fn lod_scene_crossfades_coarse_records_without_reallocating_them() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("LOD transition fixture enters a scene: {error}"),
    };
    let index = LodIndex::from_scene(&scene);
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 10_000.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Perspective {
            fov_y: 0.8,
            aspect: 1.0,
            near: 0.1,
            far: 20_000.0,
        },
    };
    let coarse = index.select(&camera, [800, 800], LodPolicy::default(), None);
    let mut lod_scene = LodScene::default();
    assert!(
        lod_scene
            .apply_transition(&mut scene, &index, &LodFrame::default(), &coarse, 0.5)
            .is_ok()
    );
    let opacity = scene
        .primitives()
        .find_map(|(_, primitive)| match primitive {
            Primitive::Particle(value) => Some(value.opacity),
            _ => None,
        });
    assert_eq!(opacity, Some(0.41));
    assert_eq!(lod_scene.primitive_count(), 1);
    assert!(
        lod_scene
            .apply_transition(&mut scene, &index, &LodFrame::default(), &coarse, 1.0)
            .is_ok()
    );
    assert_eq!(lod_scene.primitive_count(), 1);
}
