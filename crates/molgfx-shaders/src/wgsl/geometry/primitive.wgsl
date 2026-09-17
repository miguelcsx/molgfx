// Specialized caller-authored analytic primitives into the shared gbuffer.
//
// Pipelines are specialized by primitive type:
//   ellipsoid -> fs_primitive_ellipsoid[_transparent]
//   polygon   -> fs_primitive_polygon[_transparent]
//   box       -> fs_primitive_box[_transparent]
//   particle  -> fs_primitive_particle[_transparent]
//
// All types share vs_primitive and a six-vertex triangle-list impostor.
//
// PrimitiveGpu.orientation is expected to contain a normalized quaternion.
//
// This file holds the two pipeline stages only. Each analytic primitive owns
// its interval math and its hit routine under include/primitive/, so adding a
// shape touches one file and the shared ray and output code stays untouched.

//!include "include/camera.wgsl"
//!include "include/quad.wgsl"
//!include "include/intersect.wgsl"
//!include "include/quaternion.wgsl"
//!include "include/material_lighting.wgsl"
//!include "include/oit_input.wgsl"
//!include "include/motion.wgsl"
//!include "include/primitive/types.wgsl"
//!include "include/primitive/ellipsoid.wgsl"
//!include "include/primitive/box.wgsl"
//!include "include/primitive/polygon.wgsl"

// Particle integration retains the shared PrimitiveVsOut contract.
//!include "include/particle.wgsl"
//!include "include/primitive/output.wgsl"

@vertex
fn vs_primitive(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> PrimitiveVsOut {
    let center_radius =
        primitive[instance].center_radius;

    let world_center =
        center_radius.xyz;

    let center =
        transform_point(
            frame.view,
            world_center,
        );

    var half_size = vec2f(center_radius.w);

    if frame.projection_kind.x < 0.5 {
        half_size = sphere_quad_half_extent(center, center_radius.w);
    }

    let view_position =
        center +
        vec3f(
            primitive_corner(vertex) *
                half_size,
            0.0,
        );

    var out: PrimitiveVsOut;

    out.position =
        frame.proj *
        vec4f(view_position, 1.0);

    out.view_position =
        view_position;

    out.world_center = vec3f(0.0);
    out.radius = 0.0;
    out.orientation = vec4f(0.0);
    out.size = vec3f(0.0);
    out.inverse_primary = vec4f(0.0);
    out.inverse_cross = vec4f(0.0);
    out.color = vec4f(0.0);
    out.metadata = vec4u(0u);
    out.previous_world_center = world_center;

    // Flat interpolation reads vertices 0 and 3 for the two triangles.
    // Only those vertices pay for the remaining six storage-record fields and
    // the previous-frame position; the other half perform geometry work only.
    if primitive_flat_source(vertex) {
        out.world_center = world_center;
        out.radius = center_radius.w;
        out.orientation = primitive[instance].orientation;
        out.size = primitive[instance].size_opacity.xyz;
        out.inverse_primary = primitive[instance].inverse_primary;
        let inverse_cross = primitive[instance].inverse_cross;
        out.inverse_cross = inverse_cross;
        out.color = primitive[instance].color;
        out.metadata = primitive[instance].metadata;
        if inverse_cross.w > 0.5 {
            out.previous_world_center = primitive_previous[instance].xyz;
        }
    }

    return out;
}

// -----------------------------------------------------------------------------
// Ellipsoid
// -----------------------------------------------------------------------------

@fragment
fn fs_primitive_ellipsoid(
    in: PrimitiveVsOut,
) -> PrimitiveFsOut {
    if in.color.a < 0.999 {
        discard;
    }

    let ray =
        primitive_ray(
            in.view_position,
        );

    let hit =
        ellipsoid_hit(
            in,
            ray,
        );

    if !hit.valid {
        discard;
    }

    return primitive_opaque_output(
        in,
        ray,
        hit,
    );
}

@fragment
fn fs_primitive_ellipsoid_transparent(
    in: PrimitiveVsOut,
) -> OitOutput {
    if in.color.a >= 0.999 {
        discard;
    }

    let ray =
        primitive_ray(
            in.view_position,
        );

    let hit =
        ellipsoid_hit(
            in,
            ray,
        );

    if !hit.valid {
        discard;
    }

    return primitive_transparent_output(
        in,
        ray,
        hit,
    );
}

// -----------------------------------------------------------------------------
// Oriented box
// -----------------------------------------------------------------------------

@fragment
fn fs_primitive_box(
    in: PrimitiveVsOut,
) -> PrimitiveFsOut {
    if in.color.a < 0.999 {
        discard;
    }

    let ray =
        primitive_ray(
            in.view_position,
        );

    let hit =
        box_hit(
            in,
            ray,
        );

    if !hit.valid {
        discard;
    }

    return primitive_opaque_output(
        in,
        ray,
        hit,
    );
}

@fragment
fn fs_primitive_box_transparent(
    in: PrimitiveVsOut,
) -> OitOutput {
    if in.color.a >= 0.999 {
        discard;
    }

    let ray =
        primitive_ray(
            in.view_position,
        );

    let hit =
        box_hit(
            in,
            ray,
        );

    if !hit.valid {
        discard;
    }

    return primitive_transparent_output(
        in,
        ray,
        hit,
    );
}

// -----------------------------------------------------------------------------
// Polygon
// -----------------------------------------------------------------------------

@fragment
fn fs_primitive_polygon(
    in: PrimitiveVsOut,
) -> PrimitiveFsOut {
    if in.color.a < 0.999 {
        discard;
    }

    let ray =
        primitive_ray(
            in.view_position,
        );

    let hit =
        polygon_hit(
            in,
            ray,
        );

    if !hit.valid {
        discard;
    }

    return primitive_opaque_output(
        in,
        ray,
        hit,
    );
}

@fragment
fn fs_primitive_polygon_transparent(
    in: PrimitiveVsOut,
) -> OitOutput {
    if in.color.a >= 0.999 {
        discard;
    }

    let ray =
        primitive_ray(
            in.view_position,
        );

    let hit =
        polygon_hit(
            in,
            ray,
        );

    if !hit.valid {
        discard;
    }

    return primitive_transparent_output(
        in,
        ray,
        hit,
    );
}

// -----------------------------------------------------------------------------
// Particle
// -----------------------------------------------------------------------------

@fragment
fn fs_primitive_particle(
    in: PrimitiveVsOut,
) -> PrimitiveFsOut {
    if in.color.a < 0.999 {
        discard;
    }

    let ray =
        primitive_ray(
            in.view_position,
        );

    let hit =
        particle_hit(
            in,
            ray.origin,
            ray.direction,
        );

    if !hit.valid {
        discard;
    }

    return primitive_opaque_output(
        in,
        ray,
        hit,
    );
}

@fragment
fn fs_primitive_particle_transparent(
    in: PrimitiveVsOut,
) -> OitOutput {
    if in.color.a >= 0.999 {
        discard;
    }

    let ray =
        primitive_ray(
            in.view_position,
        );

    let hit =
        particle_hit(
            in,
            ray.origin,
            ray.direction,
        );

    if !hit.valid {
        discard;
    }

    return primitive_transparent_output(
        in,
        ray,
        hit,
    );
}
