// Shared primitive gbuffer and transparency output.
//
// Every shape converges here, so a hit is written by one implementation no
// matter which primitive produced it. Opaque and transparent output stay
// separate functions: neither pays for the attachments or the coverage
// weighting the other needs.

/// Resolves one valid opaque hit into the shared gbuffer.
fn primitive_opaque_output(
    in: PrimitiveVsOut,
    ray: PrimitiveRay,
    hit: PrimitiveHit,
) -> PrimitiveFsOut {
    let world_position =
        fma(
            ray.direction,
            vec3f(hit.t),
            ray.origin,
        );

    let view_position =
        transform_point(
            frame.view,
            world_position,
        );

    let view_normal =
        normalize(
            transform_direction(
                frame.view,
                hit.normal_world,
            )
        );

    let previous_world_position =
        in.previous_world_center +
        (world_position - in.world_center);

    var out: PrimitiveFsOut;

    out.albedo_material =
        vec4f(
            in.color.rgb,
            PRIMITIVE_MATERIAL,
        );

    out.normal_roughness =
        vec4f(
            encode_shading_frame(
                view_normal,
                canonical_tangent(
                    view_normal,
                ),
            ),

            varied_roughness(
                in.metadata.x,
                PRIMITIVE_ROUGHNESS,
            ),
        );

    out.entity_id =
        in.metadata.x & 0x0fffffffu;

    out.resident_page =
        in.metadata.y;

    out.motion =
        screen_motion(
            world_position,
            previous_world_position,
        );

    out.depth =
        primitive_view_depth(
            view_position,
        );

    return out;
}

/// Resolves one valid transparent hit.
fn primitive_transparent_output(
    in: PrimitiveVsOut,
    ray: PrimitiveRay,
    hit: PrimitiveHit,
) -> OitOutput {
    let opacity =
        in.color.a *
        hit.weight;

    if opacity <= 1.0e-4 {
        discard;
    }

    let world_position =
        fma(
            ray.direction,
            vec3f(hit.t),
            ray.origin,
        );

    let view_position =
        transform_point(
            frame.view,
            world_position,
        );

    let view_normal =
        normalize(
            transform_direction(
                frame.view,
                hit.normal_world,
            )
        );

    let depth =
        primitive_view_depth(
            view_position,
        );

    let lit =
        shade_molecule(
            in.color.rgb,
            view_normal,
            PRIMITIVE_ROUGHNESS,
            PRIMITIVE_MATERIAL,
            view_position,
            oit_occlusion(in.position),
        );

    return weighted_transparency(
        lit,
        opacity,
        depth,
    );
}
