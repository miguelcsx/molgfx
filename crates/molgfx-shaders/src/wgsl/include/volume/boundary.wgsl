// Indexed scalar boundaries retain the voxel-space lattice presentation.
//!include "include/field_boundary.wgsl"

struct ScalarBoundaryVsOut {
    @builtin(position) position: vec4f,
    @location(0) world_position: vec3f,
    @location(1) world_normal: vec3f,
}

@vertex
fn vs_volume_boundary(@builtin(vertex_index) index: u32) -> ScalarBoundaryVsOut {
    let vertex = boundary_vertices[boundary_indices[index]];
    return ScalarBoundaryVsOut(frame.view_proj * vec4f(vertex.position, 1.0),
        vertex.position, vertex.normal);
}

@fragment
fn fs_volume_boundary(in: ScalarBoundaryVsOut,
    @builtin(front_facing) front: bool) -> OitOutput {
    let face = cross(dpdx(in.world_position), dpdy(in.world_position));
    var normal = in.world_normal;
    if dot(normal, normal) == 0.0 { normal = select(-face, face, front); }
    if VOLUME_CLIPPING_ENABLED {
    for (var index = 0u; index < min(volume.clip_meta.x, 4u); index++) {
        let plane = volume.clip_planes[index];
        if dot(plane.xyz, in.world_position) + plane.w < 0.0 { discard; }
    }
    }
    let coordinate = volume_transform_point(volume.world_to_voxel, in.world_position);
    if any(coordinate < vec3f(volume.crop_minimum.xyz)) ||
        any(coordinate > vec3f(volume.crop_maximum.xyz - vec3u(1u))) { discard; }
    if !iso_style_visible(coordinate) { discard; }
    let view_position = volume_transform_point(frame.view, in.world_position);
    let view_normal = normalize(volume_transform_direction(frame.view, normal));
    let transfer = transfer_at(volume.scalar.z, transfer_count());
    let lit = shade_molecule(transfer.color, view_normal, volume.material.x,
        material_payload(volume.material), view_position, oit_occlusion(in.position));
    return weighted_transparency(lit, volume.scalar.w, in.position.z);
}
