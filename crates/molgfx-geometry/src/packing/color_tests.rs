use super::ColorContext;
use molgfx_core::{
    AtomProperty, AtomPropertyMeaning, CategoryPalette, ColorOverlay, ColorScheme,
    ScalarFieldSemantics, Scene,
};
use molgfx_math::Rgba8;
use std::sync::Arc;

fn scene_with(
    values: &[f32],
) -> (
    Scene,
    molgfx_core::StructureHandle,
    molgfx_core::AtomPropertyHandle,
) {
    let structure = match molframe::read_bytes(
        b"ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00  0.00           N\n\
ATOM      2  CA  ALA A   1       1.400   0.000   0.000  1.00  0.00           C\nEND\n"
            .to_vec(),
        Some("two.pdb"),
        &molframe::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(error) => panic!("fixture parses: {error:?}"),
    };
    let mut scene = match Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(error) => panic!("scene builds: {error}"),
    };
    let Some((handle, _)) = scene.structures().next() else {
        panic!("one structure")
    };
    let column = match AtomProperty::new(
        handle,
        Arc::<str>::from("categories"),
        values.to_vec().into(),
        AtomPropertyMeaning::Generic,
        ScalarFieldSemantics::UncalibratedRank,
    ) {
        Ok(column) => column,
        Err(error) => panic!("column builds: {error}"),
    };
    let Ok(property) = scene.add_atom_property(column) else {
        panic!("column binds")
    };
    (scene, handle, property)
}

const ELEMENT: Rgba8 = Rgba8::opaque(200, 200, 200);

#[test]
fn a_category_takes_its_palette_colour_and_a_missing_one_keeps_the_element() {
    let (scene, structure, property) = scene_with(&[1.0, f32::NAN]);
    let context = ColorContext::new(&scene, structure);
    let scheme = ColorScheme::category(property, CategoryPalette::Dark2);
    assert_eq!(
        context.color(scheme, ELEMENT, 0),
        CategoryPalette::Dark2.color(1.0).unwrap_or(ELEMENT)
    );
    assert_eq!(context.color(scheme, ELEMENT, 1), ELEMENT);
}

#[test]
fn an_overlay_class_overrides_the_base_scheme_for_its_atoms_only() {
    let (scene, structure, classes) = scene_with(&[0.0, 1.0]);
    let context = ColorContext::new(&scene, structure);
    let red = Rgba8::opaque(255, 0, 0);
    let Ok(overlay) = ColorOverlay::new(classes, &[ColorScheme::Uniform(red)]) else {
        panic!("overlay builds")
    };
    let base = ColorScheme::ByElement;
    assert_eq!(context.scheme(base, Some(overlay), 0), base);
    assert_eq!(
        context.scheme(base, Some(overlay), 1),
        ColorScheme::Uniform(red)
    );
    assert_eq!(context.scheme(base, None, 1), base);
}
