use super::*;

fn resolved(result: Result<Aabb, RenderError>) -> Aabb {
    result.unwrap_or_else(|error| panic!("{error}"))
}

fn rendered_bounds(placed: &PlacedStructure) -> Aabb {
    placed
        .render_bvh()
        .unwrap_or_else(|error| panic!("{error}"))
        .bounds()
}
use molgfx_core::{Scene, StructureHandle, TrajectoryFrame, TrajectorySegment};
use std::sync::Arc;

fn trajectory_scene() -> (Scene, StructureHandle) {
    let source = "data_bounds\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\nATOM 1 C CA ALA A 1 0 0 0\nATOM 2 C CA GLY A 2 100 0 0\n";
    let parsed = molframe::read_bytes(
        source.as_bytes().to_vec(),
        Some("trajectory-bounds.cif"),
        &molframe::ReadOptions::new(),
    );
    let (structure, _) = match parsed {
        Ok(value) => value,
        Err(diagnostics) => panic!("trajectory bounds fixture parses: {diagnostics:?}"),
    };
    let mut scene = Scene::new();
    let handle = match scene.add_structure(&structure) {
        Ok(handle) => handle,
        Err(error) => panic!("trajectory bounds fixture is placed: {error}"),
    };
    (scene, handle)
}

fn trajectory_segment(start_x: f32, end_x: f32) -> TrajectorySegment {
    let start_positions: Arc<[[f32; 3]]> = Arc::from([[start_x, 0.0, 0.0], [100.0, 0.0, 0.0]]);
    let end_positions: Arc<[[f32; 3]]> = Arc::from([[end_x, 0.0, 0.0], [100.0, 0.0, 0.0]]);
    let start = match TrajectoryFrame::new(1, 0.0, start_positions, "bounds:start") {
        Ok(frame) => frame,
        Err(error) => panic!("start frame is valid: {error}"),
    };
    let end = match TrajectoryFrame::new(2, 1.0, end_positions, "bounds:end") {
        Ok(frame) => frame,
        Err(error) => panic!("end frame is valid: {error}"),
    };
    match TrajectorySegment::new(start, end, 0.25) {
        Ok(segment) => segment,
        Err(error) => panic!("trajectory segment is valid: {error}"),
    }
}

#[test]
fn a_sparse_surface_selection_uses_only_its_atom_bounds() {
    let source = "data_bounds\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\nATOM 1 C CA ALA A 1 0 0 0\nATOM 2 C CA GLY A 2 100 0 0\n";
    let parsed = molframe::read_bytes(
        source.as_bytes().to_vec(),
        Some("bounds.cif"),
        &molframe::ReadOptions::new(),
    );
    let (structure, _) = match parsed {
        Ok(value) => value,
        Err(diagnostics) => panic!("bounds fixture parses: {diagnostics:?}"),
    };
    let Some(placed) = PlacedStructure::new(&structure) else {
        panic!("bounds fixture has coordinates")
    };
    let selected = resolved(selected_atom_bounds(
        &placed,
        &AtomSelection::Sparse(vec![0]),
        1.0,
    ));
    assert!(selected.max.x < 10.0);
    assert!(rendered_bounds(&placed).max.x > 90.0);
}

#[test]
fn selected_bounds_include_the_rendered_radius_scale() {
    let source = "data_bounds\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\nATOM 1 C CA ALA A 1 0 0 0\n";
    let parsed = molframe::read_bytes(
        source.as_bytes().to_vec(),
        Some("scaled-bounds.cif"),
        &molframe::ReadOptions::new(),
    );
    let (structure, _) = match parsed {
        Ok(value) => value,
        Err(diagnostics) => panic!("scaled bounds fixture parses: {diagnostics:?}"),
    };
    let Some(placed) = PlacedStructure::new(&structure) else {
        panic!("scaled bounds fixture has coordinates")
    };
    let radius = placed.atoms.radius().values()[0];

    let selected = resolved(selected_atom_bounds(&placed, &AtomSelection::All, 3.0));

    assert!((selected.max.x - radius * 3.0).abs() < f32::EPSILON);
    assert!((selected.min.x + radius * 3.0).abs() < f32::EPSILON);
}

#[test]
fn trajectory_bounds_include_both_resident_frame_endpoints() {
    let (mut scene, handle) = trajectory_scene();
    if let Err(error) = scene.set_trajectory_segment(handle, trajectory_segment(-20.0, 30.0)) {
        panic!("trajectory segment is installed: {error}");
    }
    let Some(placed) = scene.structure(handle) else {
        panic!("placed structure remains resident")
    };
    let radius = placed.atoms.radius().values()[0] * 2.0;

    let selected = resolved(selected_atom_bounds(
        placed,
        &AtomSelection::Sparse(vec![0]),
        2.0,
    ));

    assert!((selected.min.x - (-20.0 - radius)).abs() < f32::EPSILON);
    assert!((selected.max.x - (30.0 + radius)).abs() < f32::EPSILON);
    assert!(selected.max.x < 100.0);
}

#[test]
fn a_time_only_trajectory_update_reuses_the_surface_bounds_state() {
    let (mut scene, handle) = trajectory_scene();
    if let Err(error) = scene.set_trajectory_segment(handle, trajectory_segment(-20.0, 30.0)) {
        panic!("trajectory segment is installed: {error}");
    }
    let selection = scene.add_selection(AtomSelection::All);
    let representation = Representation::new(
        RepresentationTarget::Selection(selection),
        RepresentationKind::Surface,
    );
    let mut cache = SelectionBoundsCache::new();
    let Some(placed) = scene.structure(handle) else {
        panic!("placed structure remains resident")
    };
    let revision = placed.trajectory_revision();
    let before_state = SelectionBoundsState::new(placed, &representation);
    let before_bounds = resolved(cache.resolve(placed, &representation, &AtomSelection::All));

    if let Err(error) = scene.set_trajectory_time(handle, 0.75) {
        panic!("trajectory time advances: {error}");
    }
    let Some(placed) = scene.structure(handle) else {
        panic!("placed structure remains resident")
    };
    let after_state = SelectionBoundsState::new(placed, &representation);
    let after_bounds = resolved(cache.resolve(placed, &representation, &AtomSelection::All));

    assert!(placed.trajectory_revision() > revision);
    assert_eq!(after_state, before_state);
    assert_eq!(after_bounds, before_bounds);
}

#[test]
fn visual_displacement_expands_cached_culling_bounds() {
    let (mut scene, handle) = trajectory_scene();
    let selection = AtomSelection::All;
    let mut builder = molgfx_core::VisualProgramBuilder::new();
    let offset = match builder.vector([2.0, 0.0, 0.0]) {
        Ok(value) => value,
        Err(error) => panic!("offset is finite: {error}"),
    };
    if let Err(error) = builder.set_position_offset(offset, 2.0) {
        panic!("offset bound is valid: {error}");
    }
    let program = match builder.finish() {
        Ok(value) => value,
        Err(error) => panic!("program is valid: {error}"),
    };
    let mut representation = Representation::new(
        RepresentationTarget::Selection(scene.add_selection(selection.clone())),
        RepresentationKind::Spacefill,
    );
    representation.visual = Some(molgfx_core::VisualStyle::new(program));
    let Some(placed) = scene.structure(handle) else {
        panic!("structure remains resident")
    };
    let base = rendered_bounds(placed);
    let expanded =
        resolved(SelectionBoundsCache::new().resolve(placed, &representation, &selection));
    assert_eq!(expanded.min, base.min - Vec3::splat(2.0));
    assert_eq!(expanded.max, base.max + Vec3::splat(2.0));
}
