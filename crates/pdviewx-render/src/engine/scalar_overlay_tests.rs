use super::tests::{camera, engine, structure};
use pdviewx_core::{
    AtomSelection, DensityVolume, RepresentationKind, ScalarContours, ScalarRamp, Scene,
    SurfaceScalarOverlay,
};
use pdviewx_math::Vec3;
use std::sync::Arc;

#[test]
fn a_surface_overlay_reuses_one_resident_caller_grid_across_frames() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let volume = match DensityVolume::from_spacing(
        [4, 4, 4],
        Vec3::splat(-1.0),
        Vec3::ONE,
        Arc::from(vec![0.0; 64]),
    ) {
        Ok(volume) => volume,
        Err(error) => panic!("overlay volume builds: {error}"),
    };
    let volume = scene.add_volume(volume);
    let selection = scene.add_selection(AtomSelection::All);
    let surface = match scene.represent(selection, RepresentationKind::Surface) {
        Ok(surface) => surface,
        Err(error) => panic!("surface applies: {error}"),
    };
    let contours = match ScalarContours::new(0.25, 1.0) {
        Ok(contours) => contours,
        Err(error) => panic!("contours validate: {error}"),
    };
    let Some(representation) = scene.representation_mut(surface) else {
        panic!("surface resolves")
    };
    representation.surface_scalar = Some(SurfaceScalarOverlay {
        contours: Some(contours),
        ..SurfaceScalarOverlay::new(volume, ScalarRamp::diverging(1.0))
    });
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("overlay frame renders: {error}")
    }
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("unchanged overlay frame renders: {error}")
    }
    let Ok(uploads) = engine.device.log.texture_writes.lock() else {
        panic!("texture log locks")
    };
    assert_eq!(uploads.len(), 3, "the caller field remains resident");
    assert_eq!(uploads[0].0, "caller density volume");
}
