use super::tests::{camera, engine, structure};
use molgfx_core::{AtomSelection, RepresentationKind, Scene, SurfaceKind};

#[test]
fn van_der_waals_surface_allocates_no_voxel_grid() {
    let source = structure();
    let mut scene = Scene::from_structure(&source)
        .unwrap_or_else(|error| panic!("fixture scene builds: {error}"));
    let selection = scene.add_selection(AtomSelection::All);
    let handle = scene
        .represent(selection, RepresentationKind::Surface)
        .unwrap_or_else(|error| panic!("surface applies: {error}"));
    let Some(representation) = scene.representation_mut(handle) else {
        panic!("surface resolves")
    };
    representation.params.surface_kind = SurfaceKind::VanDerWaals;

    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("frame renders: {error}"));
    let textures = engine
        .device
        .log
        .textures
        .lock()
        .unwrap_or_else(|error| panic!("texture log locks: {error}"));
    assert!(textures.iter().all(|label| {
        !matches!(
            *label,
            "probe-inflated surface field"
                | "probe-inflated surface provenance"
                | "solvent-excluded surface field"
                | "solvent-excluded surface provenance"
        )
    }));
}
