use super::WebScene;
use molgfx::{Color, Scene, density, schema::DataSource};

#[wasm_bindgen_test::wasm_bindgen_test]
fn a_bound_volume_survives_browser_scene_resolution() {
    let mut authored = Scene::empty();
    let id = authored
        .add(
            density::volume(DataSource::new("grid"), [2, 2, 2]).isosurface(
                0.5,
                Color::rgb(49, 104, 142),
                1.0,
            ),
        )
        .expect("authored volume");
    let json = authored.to_spec().to_json().expect("portable scene");
    let mut browser = WebScene::new(&json).expect("browser scene");
    browser.resolve().expect("first resolution");
    assert_eq!(
        browser
            .resolved
            .as_ref()
            .expect("resolved")
            .unresolved_overlays()
            .len(),
        1
    );
    browser
        .bind_volume(id.get(), vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0])
        .expect("bound scalar grid");
    browser.resolve().expect("re-resolution");
    assert!(
        browser
            .resolved
            .as_ref()
            .expect("resolved")
            .unresolved_overlays()
            .is_empty()
    );
}
