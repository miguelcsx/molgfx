// Trilinear sampling of the caller's scalar grid.
//
// The value and its gradient share one eight-texel cell load, replacing the
// forty-eight texture loads six independent trilinear samples would cost. The
// grid is caller data and is never resampled or smoothed on the way in.

// -----------------------------------------------------------------------------
// Trilinear density
// -----------------------------------------------------------------------------

fn density_cell(
    coordinate: vec3f,
) -> DensityCell {
    let size =
        volume.dimensions.xyz;

    let upper =
        size - vec3u(1u);

    let point =
        clamp(
            coordinate,
            vec3f(0.0),
            vec3f(upper),
        );

    let lower =
        min(
            vec3u(point),
            size - vec3u(2u),
        );

    return DensityCell(
        lower,
        point - vec3f(lower),
    );
}

fn density_corners(
    cell: DensityCell,
) -> DensityCorners {
    // Texel addressing is signed and vec3i has no mixed-component
    // constructor, so convert once rather than at each corner below.
    let low =
        vec3i(cell.lower);

    let high =
        low + vec3i(1);

    return DensityCorners(
        textureLoad(density_texture, vec3i(low), 0).x,
        textureLoad(density_texture, vec3i(high.x, low.y, low.z), 0).x,
        textureLoad(density_texture, vec3i(low.x, high.y, low.z), 0).x,
        textureLoad(density_texture, vec3i(high.x, high.y, low.z), 0).x,
        textureLoad(density_texture, vec3i(low.x, low.y, high.z), 0).x,
        textureLoad(density_texture, vec3i(high.x, low.y, high.z), 0).x,
        textureLoad(density_texture, vec3i(low.x, high.y, high.z), 0).x,
        textureLoad(density_texture, vec3i(high), 0).x,
    );
}

fn density_interpolate(
    c: DensityCorners,
    f: vec3f,
) -> f32 {
    let x00 = mix(c.c000, c.c100, f.x);
    let x10 = mix(c.c010, c.c110, f.x);
    let x01 = mix(c.c001, c.c101, f.x);
    let x11 = mix(c.c011, c.c111, f.x);

    return mix(
        mix(x00, x10, f.y),
        mix(x01, x11, f.y),
        f.z,
    );
}

fn density_at(
    coordinate: vec3f,
) -> f32 {
    let cell =
        density_cell(coordinate);

    return density_interpolate(
        density_corners(cell),
        cell.fraction,
    );
}

/// Exact derivative of the manually trilinear interpolated field.
///
/// One eight-texel load replaces six density_at() calls = 48 texel loads.
fn gradient_normal(
    coordinate: vec3f,
) -> vec3f {
    let cell =
        density_cell(coordinate);

    let c =
        density_corners(cell);

    let f =
        cell.fraction;

    let dx =
        mix(
            mix(
                c.c100 - c.c000,
                c.c110 - c.c010,
                f.y,
            ),
            mix(
                c.c101 - c.c001,
                c.c111 - c.c011,
                f.y,
            ),
            f.z,
        );

    let dy =
        mix(
            mix(
                c.c010 - c.c000,
                c.c110 - c.c100,
                f.x,
            ),
            mix(
                c.c011 - c.c001,
                c.c111 - c.c101,
                f.x,
            ),
            f.z,
        );

    let dz =
        mix(
            mix(
                c.c001 - c.c000,
                c.c101 - c.c100,
                f.x,
            ),
            mix(
                c.c011 - c.c010,
                c.c111 - c.c110,
                f.x,
            ),
            f.y,
        );

    let world_normal =
        (
            transpose(volume.world_to_voxel) *
            vec4f(
                -vec3f(dx, dy, dz),
                0.0,
            )
        ).xyz;

    let view_normal =
        volume_transform_direction(
            frame.view,
            world_normal,
        );

    let length_sq =
        dot(
            view_normal,
            view_normal,
        );

    if length_sq <= 1.0e-10 {
        return vec3f(0.0, 0.0, 1.0);
    }

    return view_normal *
        inverseSqrt(length_sq);
}
