// Depth-only compact ligand-pose spheres and capsules.

//!include "include/camera.wgsl"
//!include "include/quad.wgsl"
//!include "include/intersect.wgsl"
//!include "include/quaternion.wgsl"
//!include "include/shadow/types.wgsl"
//!include "include/shadow/primitive.wgsl"
//!include "include/ligand_pose/types.wgsl"
//!include "include/ligand_pose/transform.wgsl"

@vertex
fn vs_shadow_ligand_pose(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> ShadowPrimitiveVsOut {
    let value =
        ligand_pose_instance(
            vertex,
            instance,
        );
    let center =
        shadow_view_position(
            value.center_radius.xyz
        );
    let local_vertex =
        vertex % 6u;
    let light_xy =
        center.xy
        + quad_corner(local_vertex)
            * value.center_radius.w;

    var out: ShadowPrimitiveVsOut;
    out.position =
        shadow_clip(
            vec3f(light_xy, center.z)
        );
    out.light_xy = light_xy;
    out.world_center = vec3f(0.0);
    out.orientation = vec4f(0.0);
    out.size = vec3f(0.0);
    out.inverse_primary = vec4f(0.0);
    out.inverse_cross = vec4f(0.0);
    out.shape = 0u;
    if shadow_flat_source(local_vertex) {
        out.world_center =
            value.center_radius.xyz;
        out.size = value.size;
        if LIGAND_POSE_SHAPE !=
            LIGAND_POSE_SPHERE {
            out.orientation =
                value.orientation;
        }
    }
    return out;
}
