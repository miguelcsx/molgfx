// HDR deferred molecular lighting.
//
// A broad key, cool fill and restrained rim expose cavities and silhouettes
// without inventing texture. Material response remains reversible and tied
// to the packed gbuffer properties.
//
// This file holds the pass; the gbuffer decode, illustration cues and shadow
// filtering it composes live beside it under include/deferred/.

//!include "include/camera.wgsl"
//!include "include/fullscreen.wgsl"
//!include "include/material_lighting.wgsl"
//!include "include/deferred/gbuffer.wgsl"
//!include "include/deferred/illustration.wgsl"
//!include "include/deferred/shadow.wgsl"

@fragment
fn fs_lighting(
    in: FullscreenOut,
) -> @location(0) vec4f {
    let dimensions =
        vec2i(
            textureDimensions(
                depth_texture,
            ),
        );

    let pixel = clamp(
        vec2i(
            in.position.xy,
        ),
        vec2i(0),
        dimensions - 1,
    );

    let depth =
        textureLoad(
            depth_texture,
            pixel,
            0,
        );

    if depth <= 0.0 {
        return vec4f(
            background(
                in.uv,
            ),
            1.0,
        );
    }

    let albedo_material =
        textureLoad(
            albedo_texture,
            pixel,
            0,
        );

    let normal_roughness =
        textureLoad(
            normal_texture,
            pixel,
            0,
        );

    let shading_frame =
        decode_shading_frame(
            normal_roughness.xyz,
        );

    let position =
        view_position(
            pixel,
            depth,
            dimensions,
        );

    let occlusion =
        textureLoad(
            ao_texture,
            pixel,
            0,
        ).rg;

    let visibility =
        direct_visibility(
            position,
            shading_frame.normal,
            occlusion.g,
        );

    let lit =
        shade_ribbon(
            albedo_material.rgb,
            shading_frame.normal,
            shading_frame.tangent,
            clamp(
                normal_roughness.w,
                0.05,
                0.9,
            ),
            albedo_material.a,
            position,
            vec2f(
                occlusion.r,
                visibility,
            ),
        );

    return vec4f(
        apply_illustration(
            lit,
            pixel,
            position,
            shading_frame.normal,
            dimensions,
            in.uv,
        ),
        1.0,
    );
}
