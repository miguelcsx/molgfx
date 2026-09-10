use super::Engine;
use super::tests::{camera, engine, structure};
use crate::testing::MockDevice;
use pdviewx_core::{
    AnisotropicEllipsoid, AtomSelection, CarbohydrateShape, CarbohydrateSymbol, Material, Mesh,
    MeshInstance, MeshVertex, OverlayAnchor, OverlayContent, Particle, ParticleBoundary,
    ParticleMotion, ParticleShape, PlanarRegion, Primitive, RepresentationKind, Scene,
    ScreenOverlay,
};
use pdviewx_math::{Aabb, Mat4, Quat, Rgba8, Vec3};

fn polymer_structure() -> pdbiox::Structure {
    let cif = "\
data_polymer
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_alt_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.occupancy
_atom_site.B_iso_or_equiv
_atom_site.auth_seq_id
_atom_site.auth_asym_id
_atom_site.pdbx_PDB_model_num
ATOM   1 N N   . GLY A 1 1  0.000 0.000 0.000 1.00 10.0 1 A 1
ATOM   2 C CA  . GLY A 1 1  1.200 0.000 0.000 1.00 10.0 1 A 1
ATOM   3 C C   . GLY A 1 1  2.400 0.000 0.000 1.00 10.0 1 A 1
ATOM   4 N N   . GLY A 1 2  3.800 0.400 0.000 1.00 10.0 2 A 1
ATOM   5 C CA  . GLY A 1 2  5.000 0.400 0.000 1.00 10.0 2 A 1
ATOM   6 C C   . GLY A 1 2  6.200 0.400 0.000 1.00 10.0 2 A 1
ATOM   7 N N   . GLY A 1 3  7.600 0.800 0.000 1.00 10.0 3 A 1
ATOM   8 C CA  . GLY A 1 3  8.800 0.800 0.000 1.00 10.0 3 A 1
ATOM   9 C C   . GLY A 1 3 10.000 0.800 0.000 1.00 10.0 3 A 1
ATOM  10 N N   . GLY A 1 4 11.400 1.200 0.000 1.00 10.0 4 A 1
ATOM  11 C CA  . GLY A 1 4 12.600 1.200 0.000 1.00 10.0 4 A 1
ATOM  12 C C   . GLY A 1 4 13.800 1.200 0.000 1.00 10.0 4 A 1
";
    match pdbiox::read_bytes(
        cif.as_bytes().to_vec(),
        Some("polymer-render-test.cif"),
        &pdbiox::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("polymer fixture parses: {diagnostics:?}"),
    }
}

#[test]
fn ball_and_stick_records_atom_and_bond_indirect_draws() {
    let scene = scene(RepresentationKind::BallAndStick);
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame renders: {error}")
    }
    let Ok(draws) = engine.device.log.indirect_draws.lock() else {
        panic!("log lock")
    };
    assert_eq!(
        draws.len(),
        4,
        "shadow and beauty passes each draw spheres and bonds"
    );
    let Ok(dispatches) = engine.device.log.dispatches.lock() else {
        panic!("log lock")
    };
    assert_eq!(
        dispatches.as_slice(),
        &[(1, 1, 1), (1, 1, 1), (1, 1, 1)],
        "reset, atom compaction and bond compaction run on the GPU"
    );
}

#[test]
fn lines_record_only_one_gpu_compacted_bond_draw() {
    let scene = scene(RepresentationKind::Lines);
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame renders: {error}")
    }
    let Ok(draws) = engine.device.log.indirect_draws.lock() else {
        panic!("log lock")
    };
    assert_eq!(
        draws.len(),
        2,
        "shadow and beauty passes draw bonds without atom junctions"
    );
    let Ok(dispatches) = engine.device.log.dispatches.lock() else {
        panic!("log lock")
    };
    assert_eq!(
        dispatches.as_slice(),
        &[(1, 1, 1), (1, 1, 1)],
        "reset and bond compaction run without unused atom compaction"
    );
}

#[test]
fn overlays_share_one_post_tonemap_indirect_draw() {
    let mut scene = Scene::new();
    let anchor = match OverlayAnchor::new([0.05, 0.05], [0.0; 2]) {
        Ok(value) => value,
        Err(error) => panic!("overlay anchor validates: {error}"),
    };
    for content in [
        OverlayContent::Text {
            text: "10 A".to_owned(),
            color: Rgba8::WHITE,
            size_pixels: 14.0,
        },
        OverlayContent::CoordinateTripod {
            size_pixels: 48.0,
            width_pixels: 2.0,
        },
    ] {
        let overlay = match ScreenOverlay::new(content, anchor) {
            Ok(value) => value,
            Err(error) => panic!("overlay validates: {error}"),
        };
        scene.add_overlay(overlay);
    }
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("overlay frame renders: {error}");
    }
    let Ok(draws) = engine.device.log.indirect_draws.lock() else {
        panic!("log lock")
    };
    // The overlay kinds — glyph, gradient, scale, axis — are drawn by four
    // specialized pipelines over one decluttered table, so there are four
    // indirect draws and all share the same arguments buffer.
    assert_eq!(draws.len(), 4, "one indirect draw per overlay kind");
    let batch = draws.first().map(|(args, _)| *args);
    assert!(
        draws.iter().all(|(args, _)| Some(*args) == batch),
        "every overlay kind draws from the one shared table"
    );
}

#[test]
fn mesh_instances_share_one_indirect_draw_per_pass() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(value) => value,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture owner exists")
    };
    let vertex = |position, color| MeshVertex {
        position,
        normal: Vec3::Z,
        color,
    };
    let mesh = match Mesh::new(
        owner,
        vec![
            vertex(Vec3::ZERO, Rgba8::WHITE),
            vertex(Vec3::X, Rgba8::WHITE),
            vertex(Vec3::Y, Rgba8::WHITE),
        ],
        vec![0, 1, 2],
        Material::default(),
    ) {
        Ok(value) => value,
        Err(error) => panic!("mesh validates: {error}"),
    };
    let mesh = match scene.add_mesh(mesh) {
        Ok(value) => value,
        Err(error) => panic!("mesh stores: {error}"),
    };
    for offset in [2.0, 4.0, 6.0] {
        let instance = match MeshInstance::new(mesh, Mat4::from_translation(Vec3::X * offset)) {
            Ok(value) => value,
            Err(error) => panic!("instance validates: {error}"),
        };
        if let Err(error) = scene.add_mesh_instance(instance) {
            panic!("instance stores: {error}")
        }
    }

    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("instanced mesh renders: {error}")
    }
    let Ok(draws) = engine.device.log.indirect_draws.lock() else {
        panic!("log lock")
    };
    assert_eq!(draws.len(), 2, "shadow and beauty each consume one batch");
}

#[test]
fn vertex_alpha_routes_a_mesh_only_through_transparency() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(value) => value,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture owner exists")
    };
    let vertex = |position| MeshVertex {
        position,
        normal: Vec3::Z,
        color: Rgba8::new(80, 160, 240, 96),
    };
    let mesh = match Mesh::new(
        owner,
        vec![vertex(Vec3::ZERO), vertex(Vec3::X), vertex(Vec3::Y)],
        vec![0, 1, 2],
        Material::default(),
    ) {
        Ok(value) => value,
        Err(error) => panic!("alpha mesh validates: {error}"),
    };
    if let Err(error) = scene.add_mesh(mesh) {
        panic!("alpha mesh stores: {error}")
    }
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("alpha mesh renders: {error}")
    }
    let Ok(draws) = engine.device.log.indirect_draws.lock() else {
        panic!("log lock")
    };
    assert_eq!(
        draws.len(),
        1,
        "vertex alpha skips opaque and shadow passes"
    );
}

#[test]
fn cartoon_ribbons_share_the_scene_fit_shadow_stream() {
    let source = polymer_structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("cartoon fixture scene builds: {error}"),
    };
    let selection = scene.add_selection(AtomSelection::All);
    if let Err(error) = scene.represent(selection, RepresentationKind::Cartoon) {
        panic!("cartoon representation applies: {error}")
    }
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame renders: {error}")
    }
    let Ok(draws) = engine.device.log.indirect_draws.lock() else {
        panic!("log lock")
    };
    assert_eq!(
        draws.len(),
        2,
        "ribbon shadow and beauty passes are indirect"
    );
}

#[test]
fn structure_scoped_selection_creates_no_draw_for_equal_rows_elsewhere() {
    let source = structure();
    let mut scene = Scene::new();
    let first = match scene.add_structure(&source) {
        Ok(handle) => handle,
        Err(error) => panic!("first structure places: {error}"),
    };
    if let Err(error) = scene.add_structure(&source) {
        panic!("second structure places: {error}")
    }
    let selection = match scene.add_structure_selection(first, AtomSelection::All) {
        Ok(selection) => selection,
        Err(error) => panic!("scoped selection stores: {error}"),
    };
    if let Err(error) = scene.represent(selection, RepresentationKind::Spacefill) {
        panic!("spacefill applies: {error}")
    }
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame renders: {error}")
    }
    let Ok(draws) = engine.device.log.indirect_draws.lock() else {
        panic!("log lock")
    };
    assert_eq!(
        draws.len(),
        2,
        "only the scoped structure receives shadow and beauty draws"
    );
}

#[test]
fn heterogeneous_primitives_use_exact_shadow_ranges_and_transparency_route() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture has a structure")
    };
    let ellipsoid = match AnisotropicEllipsoid::new(Vec3::ZERO, [4.0, 1.0, 1.0, 0.0, 0.0, 0.0]) {
        Ok(value) => value,
        Err(error) => panic!("ellipsoid validates: {error}"),
    };
    let ellipsoid = match Primitive::ellipsoid(owner, ellipsoid, Rgba8::WHITE, 1.0) {
        Ok(value) => value,
        Err(error) => panic!("ellipsoid declares: {error}"),
    };
    let symbol = match CarbohydrateSymbol::new(
        owner,
        Vec3::new(4.0, 0.0, 0.0),
        Quat::IDENTITY,
        Vec3::splat(2.0),
        CarbohydrateShape::Glc,
        Rgba8::opaque(20, 120, 220),
    ) {
        Ok(value) => value,
        Err(error) => panic!("symbol validates: {error}"),
    };
    let symbol = Primitive::carbohydrate(symbol);
    let plane = match PlanarRegion::new(
        owner,
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::Z,
        Vec3::X,
        [4.0, 2.0],
    ) {
        Ok(value) => value,
        Err(error) => panic!("plane validates: {error}"),
    };
    let plane = match Primitive::planar(plane, Rgba8::opaque(220, 80, 40), 0.5) {
        Ok(value) => value,
        Err(error) => panic!("plane declares: {error}"),
    };
    let gaussian = match Particle::new(
        owner,
        Vec3::new(-3.0, 0.0, 0.0),
        Quat::from_rotation_z(0.4),
        Vec3::new(3.0, 2.0, 1.5),
        ParticleShape::Gaussian,
        Rgba8::opaque(220, 180, 40),
        0.7,
    ) {
        Ok(value) => {
            let motion = match ParticleMotion::new(
                Vec3::new(0.4, -0.1, 0.2),
                Aabb::new(Vec3::splat(-6.0), Vec3::splat(6.0)),
                0.016,
                19,
                ParticleBoundary::Wrap,
            ) {
                Ok(motion) => motion.with_respawn_after_steps(64),
                Err(error) => panic!("Gaussian motion validates: {error}"),
            };
            value.with_motion(motion)
        }
        Err(error) => panic!("Gaussian validates: {error}"),
    };
    if let Err(error) =
        scene.add_primitives(&[ellipsoid, symbol, plane, Primitive::particle(gaussian)])
    {
        panic!("primitive batch stores: {error}");
    }

    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("primitives render: {error}");
    }
    assert!(engine.scene_gpu.primitive_shadow_draw(false).is_some());
    assert!(engine.scene_gpu.has_transparent_primitives());
    assert!(engine.scene_gpu.has_translucency());

    // Primitive shadows no longer reinterpret the whole heterogeneous table
    // through one ellipsoid pipeline or allocate indirect arguments.
    let Ok(indirect) = engine.device.log.indirect_draws.lock() else {
        panic!("log lock")
    };
    assert_eq!(
        indirect.len(),
        0,
        "heterogeneous primitive shadows use direct class ranges"
    );

    // Opaque ellipsoid and polygon rows appear once in the shadow pass and once
    // in the gbuffer. Translucent box and Gaussian rows appear only in OIT.
    assert_primitive_draws_cover_expected_rows(&engine);

    let Ok(dispatches) = engine.device.log.dispatches.lock() else {
        panic!("log lock")
    };
    assert_eq!(
        dispatches.as_slice(),
        &[(1, 1, 1)],
        "one fixed-step particle dispatch updates the shared primitive table"
    );
}

/// Asserts every packed primitive is drawn exactly once by a specialized
/// six-vertex impostor draw, and that the sorted classes tile the table rows.
fn assert_primitive_draws_cover_expected_rows(engine: &Engine<MockDevice>) {
    let Ok(direct) = engine.device.log.draws.lock() else {
        panic!("log lock")
    };
    let mut instances = direct
        .iter()
        .filter(|(vertices, _)| *vertices == (0..6))
        .map(|(_, instances)| instances.clone())
        .collect::<Vec<_>>();
    let mut covered = instances.drain(..).flatten().collect::<Vec<_>>();
    covered.sort_unstable();
    assert_eq!(
        covered,
        vec![0, 0, 1, 1, 2, 3],
        "opaque rows cast once and all rows remain visible exactly once"
    );
}

fn scene(kind: RepresentationKind) -> Scene {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let selection = scene.add_selection(AtomSelection::All);
    if let Err(error) = scene.represent(selection, kind) {
        panic!("representation applies: {error}")
    }
    scene
}
