// Shared primitive gbuffer and transparency output.
//
// Every shape converges here, so a hit is written by one implementation no
// matter which primitive produced it. Opaque and transparent output stay
// separate functions: neither pays for the attachments or the coverage
// weighting the other needs.

/// Surface colour of a hit: the primary colour, or the secondary colour on
/// the half of a crossed or divided SNFG symbol the split mode selects.
///
/// Only box and polygon records carry a split mode; the lane is a tensor term
/// for ellipsoids, so the family gate is not optional.
fn primitive_albedo(
    in: PrimitiveVsOut,
    world_position: vec3f,
) -> vec3f {
    let mode = in.inverse_primary.z;
    let carries_split = in.metadata.z == 1u || in.metadata.z == 2u;
    if !carries_split || mode < 0.5 {
        return in.color.rgb;
    }
    let local = rotate_vector(
        vec4f(-in.orientation.xyz, in.orientation.w),
        world_position - in.world_center,
    );
    var secondary = false;
    if mode < 1.5 {
        secondary = local.y > local.x;
    } else if mode < 2.5 {
        secondary = local.y > 0.0;
    } else {
        secondary = local.x < 0.0;
    }
    return select(in.color.rgb, in.inverse_cross.rgb, secondary);
}

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
            primitive_albedo(in, world_position),
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
            primitive_albedo(in, world_position),
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
