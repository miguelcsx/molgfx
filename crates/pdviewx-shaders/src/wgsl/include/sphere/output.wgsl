// Sphere gbuffer and transparency output.
//
// Opaque and transparent output stay separate functions: the opaque path
// never evaluates coverage weighting, and the transparent path never writes
// the gbuffer attachments it cannot use.

/// Writes one opaque sphere surface into the shared gbuffer.
fn sphere_opaque_output(
    in: SphereVsOut,
    surface: SphereSurface,
    material: SphereMaterial,
) -> SphereFsOut {
    let world_hit =
        sphere_world_position(
            surface.hit,
        );

    var out: SphereFsOut;

    out.albedo_material =
        material.albedo_material;

    out.normal_roughness =
        vec4f(
            encode_shading_frame(
                surface.normal,
                canonical_tangent(
                    surface.normal,
                ),
            ),
            material.roughness,
        );

    out.entity_id =
        in.entity_id;

    out.structure_id =
        model.structure_id;

    out.motion =
        screen_motion(
            world_hit,
            world_hit +
                in.previous_softness.xyz,
        );

    out.depth =
        sphere_view_depth(
            surface.hit,
        );

    return out;
}

/// Computes transparent edge coverage from quadratic data already produced
/// during intersection.
fn sphere_transparent_coverage(
    surface: SphereSurface,
    radius: f32,
    softness_pixels: f32,
) -> f32 {
    if softness_pixels <= 0.0 {
        return 1.0;
    }

    let closest =
        sqrt(
            surface.perpendicular_sq
        );

    let inward =
        max(
            radius - closest,
            0.0,
        );

    let transition =
        max(
            fwidth(closest) *
                softness_pixels,
            SPHERE_SOFTNESS_EPSILON,
        );

    return smoothstep(
        0.0,
        transition,
        inward,
    );
}

/// Shades one transparent sphere surface.
fn sphere_transparent_output(
    in: SphereVsOut,
    surface: SphereSurface,
    material: SphereMaterial,
) -> OitOutput {
    let coverage =
        sphere_transparent_coverage(
            surface,
            in.center_radius.w,
            in.previous_softness.w,
        );

    if coverage <= 0.0 {
        discard;
    }

    let depth =
        sphere_view_depth(
            surface.hit,
        );

    let lit =
        shade_molecule(
            material.albedo_material.rgb,
            surface.normal,
            material.roughness,
            material.albedo_material.a,
            surface.hit,
            oit_occlusion(
                in.position,
            ),
        );

    return weighted_transparency(
        lit,
        in.color.a * coverage,
        depth,
    );
}
