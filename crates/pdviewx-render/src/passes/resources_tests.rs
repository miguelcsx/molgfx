use super::*;

#[test]
fn gbuffer_target_formats_follow_the_shared_shader_location_order() {
    let targets = gbuffer_targets();
    assert_eq!(targets[0].format, GBUFFER_ALBEDO_FORMAT);
    assert_eq!(targets[1].format, GBUFFER_NORMAL_FORMAT);
    assert_eq!(targets[2].format, TextureFormat::R32Uint);
    assert_eq!(targets[3].format, TextureFormat::R32Uint);
    assert_eq!(targets[4].format, GBUFFER_MOTION_FORMAT);
}

#[test]
fn categorical_oit_uses_two_weighted_and_two_integer_targets() {
    let targets = segmentation_targets();
    assert_eq!(targets.len(), 4);
    assert_eq!(targets[0].format, TextureFormat::Rgba16Float);
    assert_eq!(targets[0].blend, BlendMode::Additive);
    assert_eq!(targets[1].format, TextureFormat::R8Unorm);
    assert_eq!(targets[1].blend, BlendMode::ReverseMultiply);
    assert_eq!(targets[2].format, TextureFormat::R32Uint);
    assert_eq!(targets[2].blend, BlendMode::Replace);
    assert_eq!(targets[3].format, TextureFormat::R32Uint);
    assert_eq!(targets[3].blend, BlendMode::Replace);
}
