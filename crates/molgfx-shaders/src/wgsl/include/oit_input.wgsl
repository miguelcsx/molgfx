// Opaque-scene visibility used to shade transparent molecular geometry.
//
// Contract: oit_ao_texture matches the current render-target dimensions.

@group(1) @binding(0) var oit_ao_texture: texture_2d<f32>;
@group(1) @binding(1) var oit_depth_texture: texture_2d<f32>;

/// Loads ambient and direct-light visibility for the current fragment.
fn oit_occlusion(
    fragment_position: vec4f,
) -> vec2f {
    return textureLoad(
        oit_ao_texture,
        vec2i(fragment_position.xy),
        0,
    ).rg;
}
