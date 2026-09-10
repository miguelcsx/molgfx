// Compact reusable-topology ligand candidates expanded in the vertex stage.

//!include "include/camera.wgsl"
//!include "include/quad.wgsl"
//!include "include/intersect.wgsl"
//!include "include/quaternion.wgsl"
//!include "include/material_lighting.wgsl"
//!include "include/oit_input.wgsl"
//!include "include/motion.wgsl"
//!include "include/primitive/types.wgsl"
//!include "include/primitive/box.wgsl"
//!include "include/ligand_pose/types.wgsl"
//!include "include/ligand_pose/transform.wgsl"
//!include "include/particle.wgsl"
//!include "include/primitive/output.wgsl"

@vertex
fn vs_ligand_pose(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> PrimitiveVsOut {
    let value =
        ligand_pose_instance(
            vertex,
            instance,
        );
    let world_center =
        value.center_radius.xyz;
    let center =
        transform_point(
            frame.view,
            world_center,
        );
    var half_size = vec2f(value.center_radius.w);

    if frame.projection_kind.x < 0.5 {
        half_size = sphere_quad_half_extent(center, value.center_radius.w);
    }
    let local_vertex =
        vertex % 6u;
    let view_position =
        center
        + vec3f(
            primitive_corner(local_vertex)
                * half_size,
            0.0,
        );

    var out: PrimitiveVsOut;
    out.position =
        frame.proj
        * vec4f(view_position, 1.0);
    out.view_position = view_position;
    out.world_center = vec3f(0.0);
    out.radius = 0.0;
    out.orientation = vec4f(0.0);
    out.size = vec3f(0.0);
    out.inverse_primary = vec4f(0.0);
    out.inverse_cross = vec4f(0.0);
    out.color = vec4f(0.0);
    out.metadata = vec4u(0u);
    out.previous_world_center = world_center;
    if primitive_flat_source(local_vertex) {
        out.world_center = world_center;
        out.radius = value.center_radius.w;
        out.orientation = value.orientation;
        out.size = value.size;
        out.color = value.color;
        out.metadata = value.metadata;
    }
    return out;
}

@fragment
fn fs_ligand_pose(
    in: PrimitiveVsOut,
) -> PrimitiveFsOut {
    let ray =
        primitive_ray(in.view_position);
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
fn fs_ligand_pose_transparent(
    in: PrimitiveVsOut,
) -> OitOutput {
    let ray =
        primitive_ray(in.view_position);
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
