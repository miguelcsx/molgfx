//! Native gap coverage, identity, and resident trajectory deformation.
#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{
    AtomSelection, EntityKind, GapStyle, LogicalRow, RepresentationConfig, RepresentationKind,
    Scene, TrajectoryFrame, TrajectorySegment,
};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{Engine, EngineConfig, ImageConfig, PickEntity};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

#[test]
#[ignore = "requires a native GPU adapter"]
fn gap_dashes_preserve_physical_coverage_and_atom_identity_through_trajectory_growth() {
    let cif = b"data_gap\nloop_\n_atom_site.group_PDB\n_atom_site.id\n\
_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_alt_id\n\
_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_entity_id\n\
_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n\
_atom_site.occupancy\n_atom_site.B_iso_or_equiv\n_atom_site.auth_seq_id\n\
_atom_site.auth_asym_id\n_atom_site.pdbx_PDB_model_num\n\
ATOM 1 C CA . GLY A 1 1 0 0 0 1 10 1 A 1\n\
ATOM 2 C CA . GLY A 1 5 4 0 0 1 10 5 A 1\n";
    let (source, _) =
        molframe::read_bytes(cif.to_vec(), Some("gap.cif"), &molframe::ReadOptions::new())
            .expect("gap fixture parses");
    let mut scene = Scene::from_structure(&source).expect("source binds");
    let owner = scene.structures().next().expect("placement exists").0;
    let selection = scene.add_selection(AtomSelection::All);
    scene
        .represent(
            selection,
            RepresentationConfig::new(RepresentationKind::Cartoon).gaps(GapStyle::Dashed),
        )
        .expect("dashed cartoon attaches");
    let camera = Camera {
        eye: Vec3::new(4.0, 0.0, 10.0),
        target: Vec3::new(4.0, 0.0, 0.0),
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 10.0,
            aspect: 1.0,
            near: 0.1,
            far: 30.0,
        },
    };
    let mut engine =
        Engine::<WgpuDevice>::new(&EngineConfig::default(), None).expect("native GPU opens");
    let extent = ImageConfig {
        width: 128,
        height: 128,
    };
    engine
        .render_image(&scene, &camera, extent)
        .expect("gaps render");
    let pick = engine
        .pick(16, 64)
        .expect("dash pick succeeds")
        .expect("dash is covered");
    assert!(matches!(pick.entity, PickEntity::Structure(identity)
        if identity.kind() == EntityKind::Atom && identity.row() == LogicalRow::new(0)));
    assert!(
        engine
            .pick(22, 64)
            .expect("spacing pick succeeds")
            .is_none()
    );
    assert!(
        engine
            .pick(93, 64)
            .expect("empty region pick succeeds")
            .is_none()
    );
    let start = TrajectoryFrame::new(0, 0.0, Arc::from([[0.0; 3], [4.0, 0.0, 0.0]]), "start")
        .expect("start validates");
    let end = TrajectoryFrame::new(1, 1.0, Arc::from([[0.0; 3], [8.0, 0.0, 0.0]]), "end")
        .expect("end validates");
    scene
        .set_trajectory_segment(
            owner,
            TrajectorySegment::new(start, end, 0.0).expect("interval validates"),
        )
        .expect("trajectory attaches");
    engine
        .render_image(&scene, &camera, extent)
        .expect("start renders");
    assert!(
        engine
            .pick(93, 64)
            .expect("reserved region pick succeeds")
            .is_none()
    );
    scene
        .set_trajectory_time(owner, 1.0)
        .expect("sample advances");
    engine
        .render_image(&scene, &camera, extent)
        .expect("expanded gaps render");
    assert!(
        engine
            .pick(93, 64)
            .expect("new dash pick succeeds")
            .is_some()
    );
    assert!(
        engine
            .pick(99, 64)
            .expect("new spacing pick succeeds")
            .is_none()
    );
}
