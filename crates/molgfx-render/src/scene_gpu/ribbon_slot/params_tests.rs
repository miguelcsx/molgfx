use super::spline_params;
use molgfx_core::{AtomSelection, Representation, RepresentationKind, RepresentationTarget, Scene};

#[test]
fn classic_beta_depth_tracks_the_authored_aspect_without_flattening_the_helix() {
    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let mut representation = Representation::new(
        RepresentationTarget::Selection(selection),
        RepresentationKind::Rocket,
    );
    representation.params.ribbon_width = 2.0;
    representation.params.cartoon_helix_radius = 2.7;
    for aspect in [2.0, 5.0, 10.0] {
        representation.params.cartoon_aspect_ratio = aspect;
        let params = spline_params(&representation, 8);
        assert_eq!(params.width.to_bits(), 2.0f32.to_bits());
        assert_eq!(params.helix_radius.to_bits(), 2.7f32.to_bits());
        assert_eq!(params.thickness.to_bits(), (2.0f32 / aspect).to_bits());
        assert_eq!(params.aspect_ratio.to_bits(), aspect.to_bits());
    }
}
