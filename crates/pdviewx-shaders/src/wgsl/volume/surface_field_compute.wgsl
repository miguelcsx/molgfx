// Generates persistent molecular surface fields.
//
// Native 3D dispatch maps one invocation directly to one grid voxel. Separate
// entry points remove the union/Gaussian branch from every invocation.

//!include "include/atom.wgsl"
//!include "include/surface_field.wgsl"

@group(1) @binding(0)
var output_field:
    texture_storage_3d<r32float, write>;

fn surface_field_point(
    coordinate: vec3u,
) -> vec3f {
    return representation.grid_min.xyz +
        vec3f(coordinate) *
        representation.grid_cell.xyz;
}

fn store_surface_sample(
    coordinate: vec3u,
    sample: DistanceSample,
) {
    textureStore(
        output_field,
        vec3i(coordinate),
        vec4f(
            clamp(
                sample.distance,
                -64.0,
                64.0,
            ),
            0.0,
            0.0,
            0.0,
        ),
    );

}

@compute @workgroup_size(4, 4, 4)
fn cs_surface_field_union(
    @builtin(global_invocation_id)
    coordinate: vec3u,
) {
    if any(
        coordinate >=
        representation.grid_size.xyz
    ) {
        return;
    }

    store_surface_sample(
        coordinate,
        union_sample(
            surface_field_point(
                coordinate
            ),
            representation.surface.x,
        ),
    );
}

@compute @workgroup_size(4, 4, 4)
fn cs_surface_field_gaussian(
    @builtin(global_invocation_id)
    coordinate: vec3u,
) {
    if any(
        coordinate >=
        representation.grid_size.xyz
    ) {
        return;
    }

    store_surface_sample(
        coordinate,
        gaussian_sample(
            surface_field_point(
                coordinate
            )
        ),
    );
}
