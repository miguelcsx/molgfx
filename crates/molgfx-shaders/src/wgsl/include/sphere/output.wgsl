// Sphere gbuffer and transparency output.
//
// Opaque and transparent output stay separate functions: the opaque path
// never evaluates coverage weighting, and the transparent path never writes
// the gbuffer attachments it cannot use.

// Derivatives here follow non-uniform intersection tests. The filter is a
// module-level directive, not a function attribute, because Safari does not
// accept @diagnostic on a function; composition hoists it above every
// declaration of the shader that includes this file.
diagnostic(off, derivative_uniformity);

/// Adds a thin analytic rim marker without changing sphere geometry.
///
/// Only translucent spheres need it: opaque fragments carry their marker in
/// the gbuffer and the lighting pass draws the edge for every form alike.
fn sphere_marker_visual(
    visual: VisualFragmentResult,
    surface: SphereSurface,
    entity_id: u32,
    radius: f32,
) -> VisualFragmentResult {
    let marker = visual.marker;
    if marker == MARKER_NONE {
        return visual;
    }

    let distance = sqrt(max(surface.perpendicular_sq, 0.0));
    let edge = smoothstep(radius * 0.72, radius * 0.98, distance);
    if edge <= 0.0 {
        return visual;
    }

    let tint = marker_tint(marker);

    return VisualFragmentResult(
        vec4f(mix(visual.color.rgb, tint, edge * 0.9), visual.color.a),
        visual.emission,
        visual.emission_enabled,
        visual.roughness,
        visual.specular,
        visual.material_strength,
        visual.visible,
        visual.softness_pixels,
        visual.marker,
    );
}

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

    let visual = visual_fragment(
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

    var visual = sphere_marker_visual(
        visual_fragment(
            in.entity_id,
            vec4f(atom_fragment_color(in.color, in.semantic, in.entity_id).rgb, in.color.a),
            visual_local_position(world_hit),
            world_hit,
            visual_world_normal(surface.normal),
        ),
        surface,
        in.entity_id,
        in.center_radius.w,
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
