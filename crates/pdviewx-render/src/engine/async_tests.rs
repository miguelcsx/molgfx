use super::ImageConfig;
use super::tests::{camera, engine, structure};
use pdviewx_core::{AtomSelection, RepresentationKind, Scene};

#[test]
fn asynchronous_image_readback_returns_tightly_packed_rgba() {
    let mut engine = engine();
    let rendered = pollster::block_on(engine.render_image_async(
        &Scene::new(),
        &camera(),
        ImageConfig {
            width: 3,
            height: 2,
        },
    ));
    let image = match rendered {
        Ok(image) => image,
        Err(error) => panic!("asynchronous image renders: {error}"),
    };
    assert_eq!((image.width, image.height), (3, 2));
    assert_eq!(image.pixels.len(), 3 * 2 * 4);
}

#[test]
fn asynchronous_pick_resolves_the_same_integer_entity() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let selection = scene.add_selection(AtomSelection::All);
    if let Err(error) = scene.represent(selection, RepresentationKind::Spacefill) {
        panic!("spacefill applies: {error}")
    }
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame renders: {error}")
    }
    let picked = pollster::block_on(engine.pick_async(0, 0));
    let pick = match picked {
        Ok(Some(pick)) => pick,
        Ok(None) => panic!("mock readback resolves zero ids"),
        Err(error) => panic!("asynchronous pick resolves: {error}"),
    };
    let super::PickEntity::Structure(entity) = pick.entity else {
        panic!("molecular pick resolves to a structure entity")
    };
    assert_eq!(entity.index, 0);
}
