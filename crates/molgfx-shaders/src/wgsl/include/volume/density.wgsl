// Trilinear sampling of the caller's scalar grid.
//
// The exact cell derivative is used for lattice orientation. Lighting uses
// central differences because cell derivatives jump at voxel boundaries.
// The caller's scalar field and its crossings remain unchanged.

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
        volume_scalar_texel(vec3i(low)),
        volume_scalar_texel(vec3i(high.x, low.y, low.z)),
        volume_scalar_texel(vec3i(low.x, high.y, low.z)),
        volume_scalar_texel(vec3i(high.x, high.y, low.z)),
        volume_scalar_texel(vec3i(low.x, low.y, high.z)),
        volume_scalar_texel(vec3i(high.x, low.y, high.z)),
        volume_scalar_texel(vec3i(low.x, high.y, high.z)),
        volume_scalar_texel(vec3i(high)),
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
fn density_gradient(
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

    return vec3f(dx, dy, dz);
}

fn density_shading_gradient(coordinate: vec3f) -> vec3f {
    // One voxel on either side gives a continuous shading gradient. At the
    // grid boundary use the actual one-sided span, preserving affine fields.
    let upper = vec3f(volume.dimensions.xyz - vec3u(1u));
    let point = clamp(coordinate, vec3f(0.0), upper);
    let low = max(point - vec3f(1.0), vec3f(0.0));
    let high = min(point + vec3f(1.0), upper);
    return vec3f(
        density_at(vec3f(high.x, point.y, point.z)) -
            density_at(vec3f(low.x, point.y, point.z)),
        density_at(vec3f(point.x, high.y, point.z)) -
            density_at(vec3f(point.x, low.y, point.z)),
        density_at(vec3f(point.x, point.y, high.z)) -
            density_at(vec3f(point.x, point.y, low.z)),
    ) / (high - low);
}

fn gradient_normal(coordinate: vec3f) -> vec3f {
    let gradient = density_shading_gradient(coordinate);
    let world_normal =
        (
            transpose(volume.world_to_voxel) *
            vec4f(
                -gradient,
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
