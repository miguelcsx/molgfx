// Empty-space skipping over the precomputed min/max grid.
//
// A cell whose whole range maps to zero opacity is crossed in one step
// instead of sampled, so cost tracks occupied space rather than grid size.
// Skipping is exact: it never steps past a cell that could contribute.

// -----------------------------------------------------------------------------
// Empty-space skipping
// -----------------------------------------------------------------------------

fn empty_space_cell(
    coordinate: vec3f,
    ray: VolumeRay,
) -> EmptySpaceCell {
    let brick_size =
        max(
            f32(volume.dimensions.w),
            1.0,
        );

    let grid_size =
        volume.empty_space_dimensions.xyz;

    let cell =
        min(
            vec3u(
                max(
                    coordinate,
                    vec3f(0.0),
                ) /
                brick_size
            ),
            grid_size - vec3u(1u),
        );

    let lower =
        vec3f(cell) *
        brick_size;

    let upper =
        min(
            lower +
                vec3f(brick_size),
            vec3f(volume.dimensions.xyz),
        );

    let interval =
        volume_ray_box(
            ray.voxel_origin,
            ray.voxel_inverse_direction,
            lower,
            upper,
        );

    let bounds = textureLoad(empty_space_bounds_texture, vec3i(cell), 0).xy;
    return EmptySpaceCell(bounds.x, bounds.y, interval.y);
}

fn empty_space_can_skip(
    cell: EmptySpaceCell,
    level: f32,
    isosurface: bool,
    count: u32,
) -> bool {
    if isosurface {
        return level < cell.minimum ||
            level > cell.maximum;
    }

    return transfer_opacity_maximum(
        cell.minimum,
        cell.maximum,
        count,
    ) <= 1.0e-5;
}

fn skip_to_cell_exit(
    distance: f32,
    exit_distance: f32,
    step_size: f32,
    limit: f32,
) -> f32 {
    return min(
        max(
            distance + step_size,
            exit_distance + 1.0e-3,
        ),
        limit,
    );
}
