// Shared surface shading, patterning and gbuffer output.
//
// Both pipelines converge here, so a hit is shaded by one implementation
// regardless of how it was found. Opaque and transparent output are separate
// functions: the opaque path never evaluates coverage weighting, and the
// transparent path never writes the gbuffer attachments it cannot use.

fn surface_frame(
    hit: SurfaceHit,
) -> SurfaceFrame {
    let world_position =
        transform_point(
            model.model_to_world,
            hit.local_position,
        );

    let world_normal =
        normalize(
            (
                transpose(
                    model.world_to_model
                ) *
                vec4f(
                    hit.local_normal,
                    0.0,
                )
            ).xyz
        );

    // A camera view transform is rigid, so it preserves the normalized
    // normal's length; no second normalize() is required.
    let view_normal =
        transform_direction(
            frame.view,
            world_normal,
        );

    return SurfaceFrame(
        world_position,

        transform_point(
            frame.view,
            world_position,
        ),

        world_normal,

        view_normal,
    );
}

fn surface_view_depth(
    position: vec3f,
) -> f32 {
    let zw =
        frame.proj[0].zw * position.x
        + frame.proj[1].zw * position.y
        + frame.proj[2].zw * position.z
        + frame.proj[3].zw;

    return zw.x / zw.y;
}

fn surface_pattern_weight(
    hit: SurfaceHit,
    view_position: vec3f,
) -> f32 {
    let mode =
        representation.options.w;

    if mode == 0u || mode == 5u || hit.cap {
        return 1.0;
    }

    let normal =
        abs(hit.local_normal);

    var coordinates =
        hit.local_position.xy;

    if normal.x >= normal.y
        && normal.x >= normal.z {
        coordinates =
            hit.local_position.yz;
    } else if normal.y >= normal.z {
        coordinates =
            hit.local_position.xz;
    }

    let spacing =
        representation.visual.y;

    let inverse_spacing =
        1.0 / spacing;

    let cell =
        abs(
            fract(
                coordinates *
                    inverse_spacing +
                vec2f(0.5)
            ) -
            vec2f(0.5)
        );

    let world_per_pixel =
        2.0 *
        max(
            -view_position.z,
            1.0e-3,
        ) /
        max(
            abs(frame.proj[1][1]) *
                frame.viewport.y,
            1.0e-3,
        );

    let width =
        clamp(
            world_per_pixel *
                representation.visual.z *
                inverse_spacing,
            0.002,
            0.24,
        );

    if mode == 2u {
        let radius =
            min(
                width * 1.8,
                0.32,
            );

        return 1.0 -
            smoothstep(
                radius,
                radius * 1.45,
                length(cell),
            );
    }

    var edge =
        min(cell.x, cell.y);

    if mode == 4u {
        let diagonal =
            abs(
                fract(
                    (coordinates.x + coordinates.y) *
                        inverse_spacing +
                    0.5
                ) -
                0.5
            );

        edge =
            min(edge, diagonal);
    }

    return 1.0 -
        smoothstep(
            width,
            width * 1.65,
            edge,
        );
}

fn surface_presented_color(
    base: vec3f,
    pattern: f32,
) -> vec3f {
    if representation.options.w != 3u {
        return base;
    }

    return base *
        mix(
            0.76,
            1.08,
            pattern,
        );
}

struct SurfaceFsOut {
    @location(0) albedo_material: vec4f,
    @location(1) normal_roughness: vec4f,
    @location(2) entity_id: u32,
    @location(3) resident_page: u32,
    @location(4) motion: vec2f,
    @builtin(frag_depth) depth: f32,
}

fn surface_opaque_output(
    hit: SurfaceHit,
) -> SurfaceFsOut {
    let atom =
        atoms[hit.compact_index];

    let color =
        atom_visual_color(atom.entity_id, atom.color);

    let mapped =
        scalar_overlay_color(
            hit.local_position,
            hit.local_normal,
            color.rgb,
        );

    var base =
        mapped;

    if hit.cap {
        base *= SURFACE_CAP_TINT;
    }

    let geometry =
        surface_frame(hit);

    let pattern =
        surface_pattern_weight(
            hit,
            geometry.view_position,
        );

    if representation.options.w != 0u
        && representation.options.w != 5u
        && representation.options.w != 3u
        && pattern <= 0.01 {
        discard;
    }

    let presented =
        surface_presented_color(
            base,
            pattern,
        );

    let visual =
        visual_fragment(
            atom.entity_id,
            vec4f(presented, color.a),
            hit.local_position,
            geometry.world_position,
            geometry.world_normal,
        );

    if !visual.visible {
        discard;
    }

    let source_index =
        atom.entity_id &
        0x1FFFFFFFu;

    let coordinate =
        source_index * 3u;

    let current_center =
        vec3f(
            coords[coordinate],
            coords[coordinate + 1u],
            coords[coordinate + 2u],
        );

    let previous_center =
        vec3f(
            previous_coords[coordinate],
            previous_coords[coordinate + 1u],
            previous_coords[coordinate + 2u],
        );

    let previous_local =
        hit.local_position +
        previous_center -
        current_center;

    let previous_world =
        transform_point(
            model.previous_model_to_world,
            previous_local,
        );

    var out: SurfaceFsOut;

    out.albedo_material =
        vec4f(
            visual.color.rgb + visual.emission,
            visual_gbuffer_payload(visual),
        );

    out.normal_roughness =
        vec4f(
            encode_shading_frame(
                geometry.view_normal,

                canonical_tangent(
                    geometry.view_normal
                ),
            ),
            visual.roughness,
        );

    out.entity_id =
        pick_local_row(atom.entity_id);

    out.resident_page =
        model_pick_page(atom.entity_id);

    out.motion =
        screen_motion(
            geometry.world_position,
            previous_world,
        );

    out.depth =
        surface_view_depth(
            geometry.view_position,
        );

    return out;
}

fn surface_transparent_output(
    in: SurfaceVsOut,
    hit: SurfaceHit,
) -> OitOutput {
    let atom =
        atoms[hit.compact_index];

    let color =
        atom_visual_color(atom.entity_id, atom.color);

    let mapped =
        scalar_overlay_color(
            hit.local_position,
            hit.local_normal,
            color.rgb,
        );

    var base =
        mapped;

    if hit.cap {
        base *= SURFACE_CAP_TINT;
    }

    let geometry =
        surface_frame(hit);

    let pattern =
        surface_pattern_weight(
            hit,
            geometry.view_position,
        );

    if representation.options.w != 0u
        && representation.options.w != 5u
        && representation.options.w != 3u
        && pattern <= 0.01 {
        discard;
    }

    let presented =
        surface_presented_color(
            base,
            pattern,
        );

    let visual =
        visual_fragment(
            atom.entity_id,
            vec4f(presented, color.a),
            hit.local_position,
            geometry.world_position,
            geometry.world_normal,
        );

    if !visual.visible {
        discard;
    }

    let depth =
        surface_view_depth(
            geometry.view_position,
        );

    let lit =
        shade_molecule(
            visual.color.rgb,
            geometry.view_normal,
            visual.roughness,
            visual_material_payload(visual),
            geometry.view_position,
            oit_occlusion(in.position),
        ) + visual.emission;

    var opacity =
        visual.color.a;

    if !hit.cap {
        if representation.options.w == 3u {
            opacity *=
                mix(
                    0.20,
                    1.0,
                    pattern,
                );
        } else if representation.options.w != 0u
            && representation.options.w != 5u {
            opacity *= pattern;
        }
    }

    return weighted_transparency(
        lit,
        opacity,
        depth,
    );
}
