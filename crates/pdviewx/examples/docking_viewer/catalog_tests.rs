use super::*;
use pdviewx::{AtomSelection, Scene};

#[test]
fn cycle_visits_every_molecular_choice_and_wraps() {
    let mut index = 0;
    let mut names = Vec::with_capacity(CHOICES.len());
    for _ in 0..CHOICES.len() {
        let choice = CHOICES[index];
        names.push(choice.name());
        index = cycled_choice(index, CycleDirection::Next).index();
    }
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), CHOICES.len());
    assert_eq!(index, 0);
    assert_eq!(
        cycled_choice(0, CycleDirection::Previous),
        CHOICES[CHOICES.len() - 1]
    );
}

#[test]
fn putty_applies_a_b_factor_radius_mapping() {
    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let handle = match scene.represent(selection, RepresentationKind::Spacefill) {
        Ok(value) => value,
        Err(error) => panic!("representation builds: {error}"),
    };
    let Some(representation) = scene.representation_mut(handle) else {
        panic!("representation resolves")
    };
    RepresentationChoice::Putty.apply(representation, Some([10.0, 40.0]));
    assert_eq!(representation.kind, RepresentationKind::Tube);
    assert!(matches!(
        representation.params.tube_radius_mapping,
        TubeRadiusMapping::BFactor { .. }
    ));
}

#[test]
fn switching_kind_restores_its_tuned_geometry_defaults() {
    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let handle = match scene.represent(selection, RepresentationKind::Spacefill) {
        Ok(value) => value,
        Err(error) => panic!("representation builds: {error}"),
    };
    let Some(representation) = scene.representation_mut(handle) else {
        panic!("representation resolves")
    };

    RepresentationChoice::Kind(RepresentationKind::BallAndStick).apply(representation, None);
    assert_eq!(representation.params.radius_scale, 0.25);
    RepresentationChoice::Kind(RepresentationKind::Trace).apply(representation, None);
    assert_eq!(representation.params.tube_radius, 0.12);
    RepresentationChoice::Kind(RepresentationKind::PaperChain).apply(representation, None);
    assert_eq!(representation.params.ribbon_width, 0.0);
}

#[test]
fn switching_to_surface_restores_its_continuous_boundary_material() {
    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let handle = match scene.represent(selection, RepresentationKind::Spacefill) {
        Ok(value) => value,
        Err(error) => panic!("representation builds: {error}"),
    };
    let Some(representation) = scene.representation_mut(handle) else {
        panic!("representation resolves")
    };
    representation.material.roughness = 0.1;
    representation.material.specular = 0.9;

    RepresentationChoice::Kind(RepresentationKind::Surface).apply(representation, None);

    assert_eq!(
        representation.material.roughness.to_bits(),
        0.62_f32.to_bits()
    );
    assert_eq!(
        representation.material.specular.to_bits(),
        0.22_f32.to_bits()
    );
    assert_eq!(
        representation.params.surface_kind,
        pdviewx::SurfaceKind::SolventAccessible
    );
    assert_eq!(
        representation.params.surface_style,
        pdviewx::SurfaceStyle::SoftUnion
    );
}
