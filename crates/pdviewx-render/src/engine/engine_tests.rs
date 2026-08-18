use super::*;
use crate::testing::MockDevice;
use pdviewx_core::{AtomSelection, ClipPlane, ClipSet, RepresentationKind, Scene, SurfaceKind};
use pdviewx_gpu::SurfaceError;
use pdviewx_math::{BoundingSphere, Camera, Vec3};

pub(super) fn camera() -> Camera {
    Camera::framing(
        &BoundingSphere {
            center: Vec3::ZERO,
            radius: 10.0,
        },
        1.0,
    )
}

pub(super) fn engine() -> Engine<MockDevice> {
    match Engine::new(&EngineConfig::default(), None) {
        Ok(engine) => engine,
        Err(e) => panic!("mock engine opens: {e}"),
    }
}

pub(super) fn structure() -> pdbiox::Structure {
    let cif = "\
data_test
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
ATOM 1 N N  . GLY A 1 1 0.0 0.0 0.0 1.00 10.0 1 A 1
ATOM 2 C CA . GLY A 1 1 1.5 0.0 0.0 1.00 10.0 1 A 1
ATOM 3 O O  . GLY A 1 1 3.0 1.0 0.0 1.00 10.0 1 A 1
loop_
_struct_conn.id
_struct_conn.conn_type_id
_struct_conn.ptnr1_label_asym_id
_struct_conn.ptnr1_label_seq_id
_struct_conn.ptnr1_label_comp_id
_struct_conn.ptnr1_label_atom_id
_struct_conn.ptnr2_label_asym_id
_struct_conn.ptnr2_label_seq_id
_struct_conn.ptnr2_label_comp_id
_struct_conn.ptnr2_label_atom_id
_struct_conn.pdbx_value_order
1 covale A 1 GLY N A 1 GLY CA SING
";
    match pdbiox::read_bytes(
        cif.as_bytes().to_vec(),
        Some("render-test.cif"),
        &pdbiox::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("fixture parses: {diagnostics:?}"),
    }
}

fn represented_scene(structure_count: usize, representation_count: usize) -> Scene {
    let source = structure();
    let mut scene = Scene::new();
    for _ in 0..structure_count {
        if let Err(error) = scene.add_structure(&source) {
            panic!("fixture structure places: {error}");
        }
    }
    let selection = scene.add_selection(AtomSelection::Sparse(vec![0, 2]));
    for order in (0..representation_count).rev() {
        let Ok(handle) = scene.represent(selection, RepresentationKind::Spacefill) else {
            panic!("spacefill applies")
        };
        let Some(representation) = scene.representation_mut(handle) else {
            panic!("representation resolves")
        };
        representation.order = u16::try_from(order).unwrap_or(u16::MAX);
    }
    scene
}

#[test]
fn a_healthy_frame_presents_with_exactly_one_submission() {
    let mut engine = engine();
    let scene = Scene::new();
    let outcome = match engine.render(&scene, &camera()) {
        Ok(outcome) => outcome,
        Err(e) => panic!("frame renders: {e}"),
    };
    assert_eq!(outcome, FrameOutcome::Presented);
    let Ok(submits) = engine.device.log.submits.lock() else {
        panic!("log lock")
    };
    let submits = *submits;
    assert_eq!(submits, 1, "one queue submission per frame");
}

#[test]
fn a_lost_surface_skips_the_frame_and_reconfigures_instead_of_panicking() {
    let mut engine = engine();
    if let Some(surface) = &mut engine.surface {
        surface.script = vec![Err(SurfaceError::Lost)];
    }
    let scene = Scene::new();
    let outcome = match engine.render(&scene, &camera()) {
        Ok(outcome) => outcome,
        Err(e) => panic!("surface loss is recoverable: {e}"),
    };
    assert_eq!(outcome, FrameOutcome::Skipped);
    // The next frame recovers.
    let outcome = match engine.render(&scene, &camera()) {
        Ok(outcome) => outcome,
        Err(e) => panic!("recovery frame renders: {e}"),
    };
    assert_eq!(outcome, FrameOutcome::Presented);
}

#[test]
fn an_outdated_surface_is_reconfigured_before_the_next_acquire() {
    let mut engine = engine();
    let configures_before = match &engine.surface {
        Some(surface) => surface.configures,
        None => panic!("mock opens with a surface"),
    };
    if let Some(surface) = &mut engine.surface {
        surface.script = vec![Err(SurfaceError::Outdated)];
    }
    let scene = Scene::new();
    let Ok(FrameOutcome::Skipped) = engine.render(&scene, &camera()) else {
        panic!("an outdated surface skips the frame")
    };
    let configures_after = match &engine.surface {
        Some(surface) => surface.configures,
        None => panic!("surface persists"),
    };
    assert!(configures_after > configures_before);
}

#[test]
fn frame_recording_issues_no_direct_scene_draws() {
    // The only direct draw allowed is the fullscreen pass's three vertices;
    // scene geometry must arrive via indirect draws.
    let mut engine = engine();
    let scene = Scene::new();
    let Ok(_) = engine.render(&scene, &camera()) else {
        panic!("frame renders")
    };
    let Ok(draws) = engine.device.log.draws.lock() else {
        panic!("log lock")
    };
    let draws = draws.clone();
    assert!(
        draws.iter().all(|(vertices, _)| vertices.end <= 3),
        "direct draws are fullscreen triangles only"
    );
}

#[test]
fn every_structure_representation_pair_has_one_indirect_draw() {
    let mut engine = engine();
    let scene = represented_scene(2, 2);
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame renders: {error}")
    }
    let first = match engine.device.log.indirect_draws.lock() {
        Ok(draws) => draws.clone(),
        Err(error) => panic!("log lock: {error}"),
    };
    assert_eq!(first.len(), 8);
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("second frame renders: {error}")
    }
    let Ok(draws) = engine.device.log.indirect_draws.lock() else {
        panic!("log lock")
    };
    assert_eq!(
        &draws[first.len()..],
        first.as_slice(),
        "draw ordering is stable"
    );
}

#[test]
fn opaque_and_translucent_representations_are_routed_to_disjoint_draws() {
    let mut scene = represented_scene(1, 2);
    let Some((transparent, _)) = scene.representations().next() else {
        panic!("fixture has a representation")
    };
    let Some(representation) = scene.representation_mut(transparent) else {
        panic!("representation resolves")
    };
    representation.material.opacity = 0.4;

    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame renders: {error}")
    }
    assert!(engine.scene_gpu.has_translucency());
    assert_eq!(engine.scene_gpu.atom_draws(false).count(), 1);
    assert_eq!(engine.scene_gpu.atom_draws(true).count(), 1);
}

#[test]
fn points_use_their_own_opaque_and_translucent_routes() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let selection = scene.add_selection(AtomSelection::All);
    let handle = match scene.represent(selection, RepresentationKind::Points) {
        Ok(handle) => handle,
        Err(error) => panic!("points apply: {error}"),
    };
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("opaque points render: {error}")
    }
    assert_eq!(engine.scene_gpu.point_draws(false).count(), 1);
    assert_eq!(engine.scene_gpu.atom_draws(false).count(), 0);
    let Some(representation) = scene.representation_mut(handle) else {
        panic!("points resolve")
    };
    representation.material.opacity = 0.4;
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("transparent points render: {error}")
    }
    assert_eq!(engine.scene_gpu.point_draws(false).count(), 0);
    assert_eq!(engine.scene_gpu.point_draws(true).count(), 1);
}

#[test]
fn an_unchanged_scene_uploads_only_the_frame_uniforms() {
    let mut engine = engine();
    let scene = represented_scene(1, 1);
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("first frame renders: {error}")
    }
    let before = match engine.device.log.writes.lock() {
        Ok(writes) => writes.len(),
        Err(error) => panic!("log lock: {error}"),
    };
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("second frame renders: {error}")
    }
    let Ok(writes) = engine.device.log.writes.lock() else {
        panic!("log lock")
    };
    assert_eq!(writes.len() - before, 1, "only group 0 changes");
    assert_eq!(
        writes[before].2,
        std::mem::size_of::<crate::scene_gpu::FrameUniforms>()
    );
}

#[test]
fn changing_a_clip_plane_updates_only_representation_state() {
    let source = structure();
    let coordinate_pointer = source.positions().as_ptr() as usize;
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let selection = scene.add_selection(AtomSelection::All);
    let representation = match scene.represent(selection, RepresentationKind::Surface) {
        Ok(handle) => handle,
        Err(error) => panic!("surface applies: {error}"),
    };
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("initial frame renders: {error}")
    }
    let before = match engine.device.log.writes.lock() {
        Ok(writes) => writes.len(),
        Err(error) => panic!("log lock: {error}"),
    };
    let plane = match ClipPlane::from_point_normal(Vec3::ZERO, Vec3::X) {
        Ok(plane) => plane,
        Err(error) => panic!("clip plane validates: {error}"),
    };
    let clipping = match ClipSet::new(&[plane]) {
        Ok(clipping) => clipping,
        Err(error) => panic!("clip set validates: {error}"),
    };
    let Some(representation) = scene.representation_mut(representation) else {
        panic!("surface resolves")
    };
    representation.clipping = clipping;
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("clipped frame renders: {error}")
    }
    let Ok(writes) = engine.device.log.writes.lock() else {
        panic!("log lock")
    };
    let changed = &writes[before..];
    assert_eq!(changed.len(), 2, "frame and representation uniforms change");
    assert!(
        changed.iter().all(|write| write.3 != coordinate_pointer),
        "clipping never re-uploads borrowed coordinates"
    );
}

#[test]
fn solvent_excluded_field_generates_once_and_not_on_an_unchanged_frame() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let selection = scene.add_selection(AtomSelection::All);
    let handle = match scene.represent(selection, RepresentationKind::Surface) {
        Ok(handle) => handle,
        Err(error) => panic!("surface applies: {error}"),
    };
    let Some(representation) = scene.representation_mut(handle) else {
        panic!("surface resolves")
    };
    representation.params.surface_kind = SurfaceKind::SolventExcluded;

    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("first frame renders: {error}")
    }
    let before = match engine.device.log.dispatches.lock() {
        Ok(dispatches) => dispatches.clone(),
        Err(error) => panic!("log lock: {error}"),
    };
    assert_eq!(
        before.len(),
        2,
        "SES has one field and one erosion dispatch"
    );
    assert!(before[0].0 > 1, "the field contains many workgroups");
    assert_eq!(before[0], before[1], "both stages cover the same grid");
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("unchanged frame renders: {error}")
    }
    let Ok(dispatches) = engine.device.log.dispatches.lock() else {
        panic!("log lock")
    };
    assert_eq!(dispatches.as_slice(), before.as_slice());
}

#[test]
fn gaussian_surface_generates_one_persistent_field_dispatch() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let selection = scene.add_selection(AtomSelection::All);
    let handle = match scene.represent(selection, RepresentationKind::Surface) {
        Ok(handle) => handle,
        Err(error) => panic!("surface applies: {error}"),
    };
    let Some(representation) = scene.representation_mut(handle) else {
        panic!("surface resolves")
    };
    representation.params.surface_kind = SurfaceKind::Gaussian;
    representation.params.gaussian_sigma = 0.8;
    representation.params.isolevel = 0.45;

    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("first Gaussian frame renders: {error}")
    }
    let before = match engine.device.log.dispatches.lock() {
        Ok(dispatches) => dispatches.clone(),
        Err(error) => panic!("log lock: {error}"),
    };
    assert_eq!(before.len(), 1, "Gaussian surfaces only generate one field");
    assert!(
        before[0].0 > 1,
        "the Gaussian field contains many workgroups"
    );
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("unchanged Gaussian frame renders: {error}")
    }
    let Ok(dispatches) = engine.device.log.dispatches.lock() else {
        panic!("log lock")
    };
    assert_eq!(dispatches.as_slice(), before.as_slice());
}

#[test]
fn off_screen_rendering_returns_tightly_packed_rgba_without_a_window_pass() {
    let mut engine = engine();
    let image = match engine.render_image(
        &Scene::new(),
        &camera(),
        ImageConfig {
            width: 3,
            height: 2,
        },
    ) {
        Ok(image) => image,
        Err(error) => panic!("image renders: {error}"),
    };
    assert_eq!((image.width, image.height), (3, 2));
    assert_eq!(image.pixels.len(), 3 * 2 * 4);
}

#[test]
fn zero_sized_off_screen_images_are_typed_errors() {
    let mut engine = engine();
    let Err(error) = engine.render_image(
        &Scene::new(),
        &camera(),
        ImageConfig {
            width: 0,
            height: 2,
        },
    ) else {
        panic!("zero width is invalid")
    };
    assert_eq!(error.code(), "PDVIEWX-E0072");
}

#[test]
fn profiling_without_timestamp_queries_is_a_typed_capability_error() {
    let mut engine = engine();
    let Err(error) = engine.profile_frame(
        &Scene::new(),
        &camera(),
        ImageConfig {
            width: 64,
            height: 64,
        },
    ) else {
        panic!("mock deliberately exposes no timestamp queries")
    };
    assert_eq!(error.code(), "PDVIEWX-E0010");
}

#[test]
fn picking_resolves_the_structure_and_source_atom_from_integer_attachments() {
    let mut engine = engine();
    let scene = represented_scene(2, 1);
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame renders: {error}")
    }
    let pick = match engine.pick(0, 0) {
        Ok(Some(pick)) => pick,
        Ok(None) => panic!("mock readback resolves zero ids"),
        Err(error) => panic!("pick resolves: {error}"),
    };
    let Some((expected, _)) = scene.structures().next() else {
        panic!("scene has structures")
    };
    let super::PickEntity::Structure(entity) = pick.entity else {
        panic!("molecular pick resolves to a structure entity")
    };
    assert_eq!(entity.structure, expected);
    assert_eq!(entity.kind, pdviewx_core::EntityKind::Atom);
    assert_eq!(entity.index, 0);
    assert!(pick.selection.contains(0));
    assert_eq!(pick.selection.count(3), 1);
    let outside = engine.width;
    assert!(matches!(engine.pick(outside, 0), Ok(None)));
}
