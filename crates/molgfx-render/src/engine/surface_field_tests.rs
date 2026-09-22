//! Sharing of implicit surface fields across representations.

use super::tests::{camera, engine, structure};
use crate::testing::MockDevice;
use molgfx_core::{AtomSelection, RepresentationKind, Scene, SurfaceKind, SurfaceStyle};

/// A scene with `count` surfaces over one selection, identical but for colour.
///
/// The colour differs through `surface_style`, which is a presentation input,
/// so every one of these surfaces needs the same field.
fn surface_scene(count: usize) -> Scene {
    let source = structure();
    let mut scene = Scene::new();
    if let Err(error) = scene.add_structure(&source) {
        panic!("fixture structure places: {error}")
    }
    let selection = scene.add_selection(AtomSelection::Sparse(vec![0, 2]));
    for order in 0..count {
        let Ok(handle) = scene.represent(selection, RepresentationKind::Surface) else {
            panic!("surface applies")
        };
        let Some(representation) = scene.representation_mut(handle) else {
            panic!("representation resolves")
        };
        representation.order = u16::try_from(order).unwrap_or(0);
        representation.params.surface_kind = SurfaceKind::SolventExcluded;
        representation.params.probe_radius = 1.4;
        // Presentation-only difference: the same field, drawn differently.
        representation.params.surface_style = if order % 2 == 0 {
            SurfaceStyle::Solid
        } else {
            SurfaceStyle::Dots
        };
        representation.color = molgfx_core::ColorScheme::ByElement;
    }
    scene
}

/// The surface field labels the device created, in creation order.
fn field_labels(device: &MockDevice) -> Vec<&'static str> {
    let labels = device
        .log
        .textures
        .lock()
        .unwrap_or_else(|error| panic!("texture log locks: {error}"));
    labels
        .iter()
        .filter_map(|label| {
            [
                "probe-inflated surface field",
                "solvent-excluded surface field",
                "continuous surface normals",
            ]
            .iter()
            .find(|known| label.contains(*known))
            .copied()
        })
        .collect()
}

#[test]
fn surfaces_differing_only_in_appearance_allocate_one_field() {
    // Field textures are the largest single GPU allocation in the engine, so
    // surfaces over the same geometry and sampling policy must generate one
    // field between them rather than one each.
    let mut labels = Vec::new();
    for count in [1_usize, 4] {
        let scene = surface_scene(count);
        let mut engine = engine();
        if let Err(error) = engine.render(&scene, &camera()) {
            panic!("frame renders: {error}")
        }
        labels.push(field_labels(&engine.device));
    }
    let (Some(one), Some(four)) = (labels.first(), labels.get(1)) else {
        panic!("both scene sizes were measured")
    };
    assert!(!one.is_empty(), "one surface allocates its field");
    assert_eq!(
        one.len(),
        four.len(),
        "four surfaces differing only in colour share one field: {one:?} vs {four:?}"
    );
}

#[test]
fn a_second_surface_over_one_key_dispatches_no_second_field() {
    // The field is generated once, by whichever surface reaches its key first.
    // A sibling over the same key dispatches nothing, so the frame's field work
    // does not grow with the number of representations.
    let mut passes = Vec::new();
    for count in [1_usize, 4] {
        let scene = surface_scene(count);
        let mut engine = engine();
        if let Err(error) = engine.render(&scene, &camera()) {
            panic!("frame renders: {error}")
        }
        let Ok(log) = engine.device.log.compute_passes.lock() else {
            panic!("compute pass log lock")
        };
        passes.push(
            log.iter()
                .filter(|label| {
                    label.contains("surface field")
                        || label.contains("surface erosion")
                        || label.contains("surface field normals")
                })
                .count(),
        );
    }
    let (Some(one), Some(four)) = (passes.first(), passes.get(1)) else {
        panic!("both scene sizes were measured")
    };
    assert!(*one > 0, "one surface generates and erodes its field");
    assert_eq!(
        one, four,
        "field generation does not scale with representation count"
    );
}
