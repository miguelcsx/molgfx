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

    let visual =
        visual_fragment(
            in.entity_id,
            vec4f(atom_fragment_color(in.color, in.semantic, in.entity_id).rgb, in.color.a),
            visual_local_position(world_hit),
            world_hit,
            visual_world_normal(surface.normal),
        );

    if !visual.visible {
        discard;
    }

    var out: SphereFsOut;

    out.albedo_material =
        vec4f(
            visual.color.rgb + visual.emission,
            visual_gbuffer_payload(visual),
        );

    out.normal_roughness =
        vec4f(
            encode_shading_frame(
                surface.normal,
                canonical_tangent(
                    surface.normal,
                ),
            ),
            visual.roughness,
        );

    out.entity_id =
        pick_local_row(in.entity_id);

    out.resident_page =
        model_pick_page(in.entity_id);

    out.motion =
        screen_motion(
            world_hit,
            world_hit +
                in.previous_softness.xyz,
        );

    out.depth =
        stable_entity_depth(
            sphere_view_depth(
                surface.hit,
            ),
            in.entity_id,
        );

    return out;
}

/// Computes transparent edge coverage from quadratic data already produced
/// during intersection.
@diagnostic(off, derivative_uniformity)
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
    let world_hit =
        sphere_world_position(
            surface.hit,
        );

    let visual =
        visual_fragment(
            in.entity_id,
            vec4f(atom_fragment_color(in.color, in.semantic, in.entity_id).rgb, in.color.a),
            visual_local_position(world_hit),
            world_hit,
            visual_world_normal(surface.normal),
        );

    if !visual.visible {
        discard;
    }

    let coverage =
        sphere_transparent_coverage(
            surface,
            in.center_radius.w,
            max(in.previous_softness.w, visual.softness_pixels),
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
            visual.color.rgb,
            surface.normal,
            visual.roughness,
            visual_material_payload(visual),
            surface.hit,
            oit_occlusion(
                in.position,
            ),
        ) + visual.emission;

    return weighted_transparency(
        lit,
        visual.color.a * coverage,
        depth,
    );
}
