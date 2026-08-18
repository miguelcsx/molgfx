use super::*;
use pdviewx_core::{AtomSelection, TrajectoryFrame, TrajectorySegment};
use pdviewx_math::Projection;
use std::sync::Arc;

#[test]
fn selection_focus_resolves_the_world_centroid_on_the_camera_axis() {
    let structure = structure();
    let mut scene = Scene::new();
    let placed = match scene.add_structure(&structure) {
        Ok(handle) => handle,
        Err(error) => panic!("fixture is renderable: {error}"),
    };
    let selection = match scene.add_structure_selection(placed, AtomSelection::Range(0..2)) {
        Ok(handle) => handle,
        Err(error) => panic!("selection is valid: {error}"),
    };
    let distance =
        match FocusTracker::default().resolve(FocusTarget::Selection(selection), &scene, &camera())
        {
            Ok(distance) => distance,
            Err(error) => panic!("focus resolves: {error}"),
        };
    assert!((distance - 8.0).abs() < 1.0e-5);
}

#[test]
fn trajectory_focus_interpolates_cached_endpoint_centroids() {
    let structure = structure();
    let mut scene = Scene::new();
    let placed = match scene.add_structure(&structure) {
        Ok(handle) => handle,
        Err(error) => panic!("fixture is renderable: {error}"),
    };
    let selection = match scene.add_structure_selection(placed, AtomSelection::All) {
        Ok(handle) => handle,
        Err(error) => panic!("selection is valid: {error}"),
    };
    let start = frame(0, 0.0, [[0.0, 0.0, 0.0], [0.0, 0.0, 4.0]]);
    let end = frame(1, 1.0, [[0.0, 0.0, 4.0], [0.0, 0.0, 8.0]]);
    let segment = match TrajectorySegment::new(start, end, 0.5) {
        Ok(segment) => segment,
        Err(error) => panic!("trajectory is valid: {error}"),
    };
    if let Err(error) = scene.set_trajectory_segment(placed, segment) {
        panic!("trajectory attaches: {error}");
    }
    let mut tracker = FocusTracker::default();
    let midpoint = match tracker.resolve(FocusTarget::Selection(selection), &scene, &camera()) {
        Ok(distance) => distance,
        Err(error) => panic!("midpoint focus resolves: {error}"),
    };
    let endpoints = tracker
        .structures
        .first()
        .map(|entry| (entry.start, entry.end));
    if let Err(error) = scene.set_trajectory_time(placed, 1.0) {
        panic!("trajectory advances: {error}");
    }
    let end = match tracker.resolve(FocusTarget::Selection(selection), &scene, &camera()) {
        Ok(distance) => distance,
        Err(error) => panic!("end focus resolves: {error}"),
    };
    assert!((midpoint - 6.0).abs() < 1.0e-5);
    assert!((end - 4.0).abs() < 1.0e-5);
    assert_eq!(
        tracker
            .structures
            .first()
            .map(|entry| (entry.start, entry.end)),
        endpoints
    );
}

#[test]
fn stale_or_empty_selection_focus_is_a_typed_error() {
    let scene = Scene::new();
    let structure = structure();
    let mut scene_with_selection = Scene::new();
    let placed = match scene_with_selection.add_structure(&structure) {
        Ok(handle) => handle,
        Err(error) => panic!("fixture is renderable: {error}"),
    };
    let selection = match scene_with_selection.add_structure_selection(placed, AtomSelection::Empty)
    {
        Ok(handle) => handle,
        Err(error) => panic!("empty selection can be stored: {error}"),
    };
    let result =
        FocusTracker::default().resolve(FocusTarget::Selection(selection), &scene, &camera());
    assert!(matches!(
        result,
        Err(RenderError::InvalidFocusTarget { .. })
    ));
}

fn frame<const N: usize>(index: u64, time: f32, positions: [[f32; 3]; N]) -> TrajectoryFrame {
    match TrajectoryFrame::new(index, time, Arc::from(positions), "focus-test") {
        Ok(frame) => frame,
        Err(error) => panic!("frame is valid: {error}"),
    }
}

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, 10.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Perspective {
            fov_y: 0.8,
            aspect: 1.0,
            near: 0.1,
            far: 100.0,
        },
    }
}

fn structure() -> pdbiox::Structure {
    let cif = "\
data_focus
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
ATOM 1 C C1 . LIG A 1 1 0.0 0.0 0.0 1.00 10.0 1 A 1
ATOM 2 C C2 . LIG A 1 1 0.0 0.0 4.0 1.00 10.0 1 A 1
";
    match pdbiox::read_bytes(
        cif.as_bytes().to_vec(),
        Some("focus.cif"),
        &pdbiox::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("fixture parses: {diagnostics:?}"),
    }
}
