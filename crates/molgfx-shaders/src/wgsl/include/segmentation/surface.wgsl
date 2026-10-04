// Indexed boundaries share exact styles and picking attachments with optical cells.
struct BoundaryVertex {
    position: vec3f,
    label: u32,
    normal: vec3f,
    padding: u32,
}

@group(3) @binding(0) var<storage, read> boundary_vertices: array<BoundaryVertex>;
@group(3) @binding(1) var<storage, read> boundary_indices: array<u32>;

struct BoundaryVsOut {
    @builtin(position) position: vec4f,
    @location(0) world_position: vec3f,
    @location(1) world_normal: vec3f,
    @location(2) @interpolate(flat) label: u32,
}

@vertex
fn vs_segmentation_surface(@builtin(vertex_index) index: u32) -> BoundaryVsOut {
    let vertex = boundary_vertices[boundary_indices[index]];
    let style = sample_style(vertex.label);
    var position = frame.view_proj * vec4f(vertex.position, 1.0);
    if !style.found || style.opacity <= SEGMENT_OPACITY_EPSILON {
        position = vec4f(2.0, 2.0, 2.0, 1.0);
    }
    return BoundaryVsOut(position, vertex.position, vertex.normal, vertex.label);
}

fn sample_segmentation_surface(in: BoundaryVsOut, front: bool) -> SegmentationOutput {
    let face = normalize(cross(dpdx(in.world_position), dpdy(in.world_position)));
    let normal = normalize(select(select(-face, face, front), in.world_normal,
        dot(in.world_normal, in.world_normal) > 0.0));
    if SEGMENTATION_CLIPPING_ENABLED {
        for (var index = 0u; index < min(volume.clip_meta.x, 4u); index++) {
            let plane = volume.clip_planes[index];
            if dot(plane.xyz, in.world_position) + plane.w < 0.0 { discard; }
        }
    }
    if volume.dimensions.w != 0u {
        let voxel = segment_transform_point(volume.world_to_voxel, in.world_position);
        if any(voxel < vec3f(volume.crop_minimum.xyz)) ||
            any(voxel > vec3f(volume.crop_maximum.xyz - vec3u(1u))) { discard; }
    }
    let style = sample_style(in.label);
    let opacity = clamp(style.opacity * volume.sampling.x, 0.0, 1.0);
    if !style.found || opacity <= SEGMENT_OPACITY_EPSILON { discard; }
    let view_position = segment_transform_point(frame.view, in.world_position);
    let color = shade_molecule(style.color,
        segment_transform_direction(frame.view, normal), volume.material.x,
        material_payload(volume.material), view_position, oit_occlusion(in.position));
    return output_at(color, opacity, in.position.z, in.label);
}

@fragment
fn fs_segmentation_surface(in: BoundaryVsOut,
    @builtin(front_facing) front: bool) -> SegmentationOutput {
    return sample_segmentation_surface(in, front);
}
