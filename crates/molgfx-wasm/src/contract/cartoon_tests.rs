use super::WebSceneSpec;

#[wasm_bindgen_test::wasm_bindgen_test]
fn cartoon_shape_controls_round_trip_through_the_browser_contract() {
    let source = molgfx::source::structure_from_payload(
        b"ATOM      1  CA  ALA A   1      11.104   6.134  -6.504  1.00  0.00           C\nEND\n",
        "cartoon.pdb",
    )
    .expect("molecular payload");
    let mut scene = molgfx::Scene::from_structure(&source).expect("source binds");
    scene
        .add(
            molgfx::rep::cartoon("all")
                .aspect_ratio(3.0)
                .arrow_factor(2.0),
        )
        .expect("cartoon attaches");
    let authored = scene.to_spec().to_json().expect("portable scene");
    let browser = WebSceneSpec::new(&authored).expect("browser contract accepts shape");
    assert_eq!(
        browser.to_json().expect("browser serializes shape"),
        authored
    );
}
