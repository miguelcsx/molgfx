// Shared indexed field vertices with exact categorical identities.
struct BoundaryVertex {
    position: vec3f,
    label: u32,
    normal: vec3f,
    padding: u32,
}

@group(3) @binding(0) var<storage, read> boundary_vertices: array<BoundaryVertex>;
@group(3) @binding(1) var<storage, read> boundary_indices: array<u32>;
