use super::*;
use molgfx_math::Rgba8;

fn handle() -> AtomPropertyHandle {
    let mut scene = crate::Scene::new();
    let source = crate::MolecularSource::from_molframe(&crate::fixture::structure());
    let Ok(structure) = scene.add_source(source) else {
        panic!("fixture structure binds")
    };
    let count = match scene.structure(structure) {
        Some(placed) => placed.atoms.len(),
        None => panic!("structure is placed"),
    };
    let values: std::sync::Arc<[f32]> = vec![0.0; count as usize].into();
    let property = match crate::AtomProperty::new(
        structure,
        std::sync::Arc::<str>::from("classes"),
        values,
        crate::AtomPropertyMeaning::Generic,
        crate::ScalarFieldSemantics::UncalibratedRank,
    ) {
        Ok(property) => property,
        Err(error) => panic!("property builds: {error}"),
    };
    match scene.add_atom_property(property) {
        Ok(handle) => handle,
        Err(error) => panic!("property binds: {error}"),
    }
}

#[test]
fn a_class_selects_its_scheme_and_zero_keeps_the_base() {
    let red = ColorScheme::Uniform(Rgba8::opaque(255, 0, 0));
    let Ok(overlay) = ColorOverlay::new(handle(), &[red, ColorScheme::ByChain]) else {
        panic!("overlay builds")
    };
    assert_eq!(overlay.scheme_for(0.0), None);
    assert_eq!(overlay.scheme_for(1.0), Some(red));
    assert_eq!(overlay.scheme_for(2.0), Some(ColorScheme::ByChain));
    assert_eq!(overlay.scheme_for(3.0), None);
    assert_eq!(overlay.scheme_for(f32::NAN), None);
    assert_eq!(overlay.scheme_for(1.5), None);
    assert_eq!(overlay.schemes().len(), 2);
}

#[test]
fn an_overlay_is_bounded_and_rejects_property_schemes() {
    let classes = handle();
    assert!(ColorOverlay::new(classes, &[]).is_err());
    let many = [ColorScheme::ByChain; MAX_COLOR_OVERLAY_CLASSES + 1];
    assert!(ColorOverlay::new(classes, &many).is_err());
    assert!(ColorOverlay::new(classes, &many[..MAX_COLOR_OVERLAY_CLASSES]).is_ok());
}
