use super::engine_config;
use crate::profile::{self, AntiAliasing, Effect, EffectKind, RenderProfile};

#[test]
fn every_public_effect_reaches_the_resolved_render_plan() {
    let cue = profile::DepthCue::new(5.0, 20.0, 0.6).expect("valid cue");
    let bloom = profile::BloomStyle::restrained();
    let dof = profile::DepthOfField::macro_lens();
    let blur = profile::MotionBlur::restrained();
    let backdrop = profile::BackdropStyle::transparent();
    let lighting = profile::LightingEnvironment::soft_key();
    let cues = profile::ShapeCueStyle::restrained();
    let display = profile::DisplayTransform::filmic();
    let effects = [
        Effect::DepthCue(cue),
        Effect::AntiAliasing(AntiAliasing::Off),
        Effect::Bloom(bloom),
        Effect::DepthOfField(dof),
        Effect::MotionBlur(blur),
        Effect::Backdrop(backdrop),
        Effect::Lighting(lighting),
        Effect::ShapeCues(cues),
        Effect::Display(display),
    ];
    let mut profile = RenderProfile::default();
    for effect in effects {
        profile = profile.with_effect(effect).expect("valid effect");
        assert_eq!(profile.effect(effect.kind()), Some(effect));
    }
    let config = engine_config(profile, None);
    let engine = molgfx_render::Engine::<molgfx_wgpu::WgpuDevice>::new(&config, None)
        .expect("reference adapter");
    let plan = engine.resolved_render_plan();
    assert!((plan.depth_cue().strength - cue.strength()).abs() < f32::EPSILON);
    assert!(!plan.antialias().expect("explicit AA").edge_smoothing);
    assert_eq!(plan.bloom(), Some(bloom));
    assert_eq!(plan.depth_of_field(), Some(dof));
    assert_eq!(plan.motion_blur(), Some(blur));
    assert_eq!(plan.backdrop(), backdrop);
    assert!((plan.lighting().key_strength - lighting.key_strength).abs() < f32::EPSILON);
    assert!((plan.lighting().shadow_strength - lighting.shadow_strength).abs() < f32::EPSILON);
    assert!((plan.lighting().key_direction - lighting.key_direction.normalize()).length() < 1.0e-6);
    assert_eq!(plan.shape_cues(), cues);
    assert_eq!(plan.display(), display);
    assert_eq!(
        profile
            .without_effect(EffectKind::Bloom)
            .effect(EffectKind::Bloom),
        None
    );
}

#[test]
fn invalid_effects_do_not_change_the_profile() {
    let profile = RenderProfile::default();
    let invalid = Effect::Bloom(profile::BloomStyle {
        intensity: f32::NAN,
        ..profile::BloomStyle::restrained()
    });
    assert!(profile.with_effect(invalid).is_err());
    assert_eq!(profile.effect(EffectKind::Bloom), None);
}
