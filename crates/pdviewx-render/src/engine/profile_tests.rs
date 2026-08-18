use super::{
    BackdropStyle, DepthOfField, DisplayTransform, EffectLayer, FocusTarget, IllustrationStyle,
    LightingEnvironment, MotionBlur, PresentationEffect, RenderProfile,
};
use crate::{DisplayGamut, TransferFunction};

fn approximately(left: f32, right: f32) -> bool {
    (left - right).abs() <= f32::EPSILON
}

#[test]
fn inspection_profile_resolves_to_neutral_presentation() {
    assert_eq!(
        RenderProfile::inspection().resolve().illustration(),
        IllustrationStyle::default()
    );
}

#[test]
fn illustrative_profile_resolves_the_publication_treatment() {
    assert_eq!(
        RenderProfile::illustrative().resolve().illustration(),
        IllustrationStyle::publication()
    );
}

#[test]
fn cinematic_profile_resolves_a_bounded_physical_lens() {
    let Some(lens) = RenderProfile::cinematic().resolve().depth_of_field() else {
        panic!("cinematic profile includes a lens")
    };
    assert_eq!(lens, DepthOfField::cinematic());
    assert_eq!(
        RenderProfile::cinematic().resolve().motion_blur(),
        Some(MotionBlur::cinematic())
    );
    let expected = [24.0, 50.0 / (8.0 * 36.0), 14.0, 7.0];
    assert!(
        lens.packed(24.0)
            .into_iter()
            .zip(expected)
            .all(|(left, right)| approximately(left, right))
    );
}

#[test]
fn cinematic_profile_does_not_select_a_backdrop_as_biological_context() {
    let resolved = RenderProfile::cinematic().resolve();
    assert_eq!(resolved.backdrop(), BackdropStyle::default());
    assert_eq!(resolved.display(), DisplayTransform::cinematic());
    assert!(
        resolved
            .packed_presentation(false)
            .into_iter()
            .flatten()
            .all(f32::is_finite)
    );
}

#[test]
fn backdrop_and_display_are_independently_composable() {
    let custom = BackdropStyle {
        top: pdviewx_math::Rgba8::opaque(12, 24, 36),
        bottom: pdviewx_math::Rgba8::opaque(40, 52, 64),
        glow_color: pdviewx_math::Rgba8::opaque(80, 90, 100),
        glow_strength: 0.0,
    };
    let resolved = RenderProfile::inspection()
        .with_effect(PresentationEffect::Backdrop(custom))
        .resolve();
    assert_eq!(resolved.backdrop(), custom);
    assert_eq!(resolved.display(), DisplayTransform::default());
}

#[test]
fn transparent_backdrops_pack_zero_coverage_without_changing_display_state() {
    let packed = crate::engine::backdrop::pack(
        BackdropStyle::transparent(),
        DisplayTransform::default(),
        false,
        [0.0; 4],
    );
    assert_eq!(packed[4][0].to_bits(), 0.0f32.to_bits());
    assert_eq!(packed[4][1].to_bits(), 0.0f32.to_bits());
    assert_eq!(packed[1][3].to_bits(), 1.0f32.to_bits());
}

#[test]
fn managed_display_state_packs_gamut_transfer_and_peak_luminance() {
    let display = DisplayTransform {
        gamut: DisplayGamut::Rec2020,
        transfer: TransferFunction::Pq,
        peak_luminance_nits: 1_000.0,
        ..DisplayTransform::default()
    };
    let packed = crate::engine::backdrop::pack(BackdropStyle::default(), display, false, [0.0; 4]);
    assert!(approximately(packed[4][3], 2.0 + 2.0 * 4.0));
    assert!(approximately(packed[5][3], 1_000.0));
}

#[test]
fn lighting_is_independent_from_backdrop_and_display() {
    let mut lighting = LightingEnvironment::documentary();
    lighting.key_strength = 2.25;
    let resolved = RenderProfile::inspection()
        .with_effect(PresentationEffect::Lighting(lighting))
        .resolve();
    assert!(approximately(
        resolved.lighting().key_strength,
        lighting.key_strength
    ));
    assert_eq!(resolved.lighting().key_color, lighting.key_color);
    assert!(resolved.lighting().key_direction.is_normalized());
    assert_eq!(resolved.backdrop(), BackdropStyle::default());
    assert_eq!(resolved.display(), DisplayTransform::default());
    assert!(
        resolved
            .packed_lighting()
            .into_iter()
            .flatten()
            .all(f32::is_finite)
    );
}

#[test]
fn malformed_lighting_resolves_to_finite_bounded_values() {
    let mut lighting = LightingEnvironment::neutral();
    lighting.key_direction = pdviewx_math::Vec3::splat(f32::NAN);
    lighting.fill_direction = pdviewx_math::Vec3::ZERO;
    lighting.diffuse_strength = f32::INFINITY;
    lighting.key_strength = -1.0;
    let resolved = RenderProfile::inspection()
        .with_effect(PresentationEffect::Lighting(lighting))
        .resolve()
        .lighting();
    assert!(resolved.key_direction.is_normalized());
    assert!(resolved.fill_direction.is_normalized());
    assert!(approximately(resolved.key_strength, 0.0));
    assert!(approximately(
        resolved.diffuse_strength,
        LightingEnvironment::neutral().diffuse_strength
    ));
}

#[test]
fn zero_weight_lens_layers_leave_graph_topology_neutral() {
    let profile = RenderProfile::inspection().with_layer(
        EffectLayer::new(PresentationEffect::DepthOfField(DepthOfField::cinematic()))
            .with_weight(0.0),
    );
    assert!(profile.resolve().depth_of_field().is_none());
}

#[test]
fn effect_layers_resolve_by_priority_and_blend_weight() {
    let strong = PresentationEffect::Illustration(IllustrationStyle {
        silhouette_strength: 1.0,
        cavity_strength: 0.0,
        depth_cue_strength: 0.0,
    });
    let soft = PresentationEffect::Illustration(IllustrationStyle {
        silhouette_strength: 0.2,
        cavity_strength: 0.4,
        depth_cue_strength: 0.6,
    });
    let resolved = RenderProfile::inspection()
        .with_layer(EffectLayer::new(soft).with_priority(20))
        .with_layer(EffectLayer::new(strong).with_priority(10))
        .with_layer(EffectLayer::new(strong).with_priority(30).with_weight(0.5))
        .resolve()
        .illustration();
    assert!(approximately(resolved.silhouette_strength, 0.6));
    assert!(approximately(resolved.cavity_strength, 0.2));
    assert!(approximately(resolved.depth_cue_strength, 0.3));
}

#[test]
fn malformed_layer_values_resolve_to_bounded_neutral_values() {
    let effect = PresentationEffect::Illustration(IllustrationStyle {
        silhouette_strength: f32::NAN,
        cavity_strength: -1.0,
        depth_cue_strength: 4.0,
    });
    let resolved = RenderProfile::inspection()
        .with_layer(EffectLayer::new(effect).with_weight(f32::INFINITY))
        .resolve()
        .illustration();
    assert_eq!(resolved, IllustrationStyle::default());
    assert_eq!(
        resolved.packed(f32::INFINITY).map(f32::to_bits),
        [0.0, 0.0, 0.0, 1.0].map(f32::to_bits)
    );
}

#[test]
fn malformed_focus_targets_resolve_to_the_camera_target() {
    let mut lens = DepthOfField::cinematic();
    lens.focus = FocusTarget::Distance(f32::NAN);
    assert_eq!(lens.sanitize().focus, FocusTarget::CameraTarget);
    lens.focus = FocusTarget::WorldPoint(pdviewx_math::Vec3::splat(f32::INFINITY));
    assert_eq!(lens.sanitize().focus, FocusTarget::CameraTarget);
}
