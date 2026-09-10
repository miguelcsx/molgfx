use super::tests::{camera, engine, structure};
use pdviewx_core::{
    AtomSelection, RepresentationKind, Scene, SurfaceComponentPolicy, SurfaceKind, SurfaceStyle,
};

#[test]
fn solid_mesh_and_dots_share_the_incremental_component_filter() {
    let mut dispatch_count = None;
    for style in [SurfaceStyle::Solid, SurfaceStyle::Mesh, SurfaceStyle::Dots] {
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
        representation.params.surface_kind = SurfaceKind::SolventExcluded;
        representation.params.surface_style = style;
        representation.params.surface_components = SurfaceComponentPolicy::minimum_voxels(2)
            .unwrap_or_else(|error| panic!("policy validates: {error}"));

        let mut renderer = engine();
        renderer
            .render(&scene, &camera())
            .unwrap_or_else(|error| panic!("filtered frame renders: {error}"));
        let first = renderer
            .device
            .log
            .dispatches
            .lock()
            .unwrap_or_else(|error| panic!("dispatch log locks: {error}"))
            .len();
        assert!(
            first > 6,
            "field, union-find, filtering and normals execute"
        );
        assert_eq!(dispatch_count.get_or_insert(first), &first);
        let allocations = renderer
            .device
            .log
            .buffers
            .lock()
            .unwrap_or_else(|error| panic!("buffer log locks: {error}"))
            .len();
        renderer
            .render(&scene, &camera())
            .unwrap_or_else(|error| panic!("stable filtered frame renders: {error}"));
        assert_eq!(
            renderer
                .device
                .log
                .dispatches
                .lock()
                .unwrap_or_else(|error| panic!("dispatch log locks: {error}"))
                .len(),
            first,
            "unchanged frames do no component work"
        );
        assert_eq!(
            renderer
                .device
                .log
                .buffers
                .lock()
                .unwrap_or_else(|error| panic!("buffer log locks: {error}"))
                .len(),
            allocations,
            "unchanged frames allocate no component scratch"
        );
    }
}
