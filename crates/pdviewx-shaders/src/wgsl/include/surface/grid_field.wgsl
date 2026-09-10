// Trilinear sampling of the persistent SES field.
//
// The value, its analytic gradient and the provenance lookup all read the
// same eight-texel cell. Taking the gradient analytically from that one cell
// replaces the four extra field evaluations a finite difference would cost,
// which is the difference between one and five cell loads per shaded pixel.

fn grid_coordinate(point: vec3f) -> GridCoordinate {
    let size =
        representation.grid_size.xyz;

    let coordinate =
        clamp(
            (
                point -
                representation.grid_min.xyz
            ) /
            representation.grid_cell.xyz,
            vec3f(0.0),
            vec3f(size - vec3u(1u)),
        );

    let lower =
        min(
            vec3u(floor(coordinate)),
            size - vec3u(2u),
        );

    return GridCoordinate(
        lower,
        clamp(
            coordinate - vec3f(lower),
            vec3f(0.0),
            vec3f(1.0),
        ),
    );
}

/// Loads one complete trilinear cell exactly once.
fn grid_corners(
    coordinate: GridCoordinate,
) -> GridCorners {
    let lower =
        vec3i(coordinate.lower);

    let upper =
        lower + vec3i(1);

    return GridCorners(
        textureLoad(
            surface_grid,
            vec3i(lower),
            0,
        ).x,

        textureLoad(
            surface_grid,
            vec3i(upper.x, lower.y, lower.z),
            0,
        ).x,

        textureLoad(
            surface_grid,
            vec3i(lower.x, upper.y, lower.z),
            0,
        ).x,

        textureLoad(
            surface_grid,
            vec3i(upper.x, upper.y, lower.z),
            0,
        ).x,

        textureLoad(
            surface_grid,
            vec3i(lower.x, lower.y, upper.z),
            0,
        ).x,

        textureLoad(
            surface_grid,
            vec3i(upper.x, lower.y, upper.z),
            0,
        ).x,

        textureLoad(
            surface_grid,
            vec3i(lower.x, upper.y, upper.z),
            0,
        ).x,

        textureLoad(
            surface_grid,
            vec3i(upper),
            0,
        ).x,
    );
}

fn trilinear_value(
    corners: GridCorners,
    fraction: vec3f,
) -> f32 {
    let x00 =
        mix(
            corners.c000,
            corners.c100,
            fraction.x,
        );

    let x10 =
        mix(
            corners.c010,
            corners.c110,
            fraction.x,
        );

    let x01 =
        mix(
            corners.c001,
            corners.c101,
            fraction.x,
        );

    let x11 =
        mix(
            corners.c011,
            corners.c111,
            fraction.x,
        );

    return mix(
        mix(x00, x10, fraction.y),
        mix(x01, x11, fraction.y),
        fraction.z,
    );
}

fn grid_surface_field(point: vec3f) -> f32 {
    let coordinate =
        grid_coordinate(point);

    return trilinear_value(
        grid_corners(coordinate),
        coordinate.fraction,
    );
}

/// Converts both field conventions to an outside-positive level value.
///
/// This remains affine in the stored scalar. A trilinear cell therefore stays
/// a cubic polynomial along a ray and can be solved without approximate
/// distance marching. Gaussian density decreases outwards, so its sign is the
/// reverse of the signed molecular-distance grids.
fn grid_level_value(field: f32) -> f32 {
    return select(
        field - representation.surface.y,
        representation.surface.y - field,
        representation.options.x == SURFACE_KIND_GAUSSIAN,
    );
}

/// Interpolates central-difference normals generated at grid vertices.
///
/// Interpolating one shared value at every vertex makes the normal continuous
/// across cell boundaries. The old derivative of each independent trilinear
/// scalar cell jumped at those boundaries and exposed the grid in lighting.
fn grid_surface_normal_sample(
    coordinate: GridCoordinate,
) -> vec3f {
    let lower = vec3i(coordinate.lower);
    let upper = lower + vec3i(1);
    let x00 = mix(
        textureLoad(surface_normals, lower, 0).xyz,
        textureLoad(surface_normals, vec3i(upper.x, lower.y, lower.z), 0).xyz,
        coordinate.fraction.x,
    );
    let x10 = mix(
        textureLoad(surface_normals, vec3i(lower.x, upper.y, lower.z), 0).xyz,
        textureLoad(surface_normals, vec3i(upper.x, upper.y, lower.z), 0).xyz,
        coordinate.fraction.x,
    );
    let x01 = mix(
        textureLoad(surface_normals, vec3i(lower.x, lower.y, upper.z), 0).xyz,
        textureLoad(surface_normals, vec3i(upper.x, lower.y, upper.z), 0).xyz,
        coordinate.fraction.x,
    );
    let x11 = mix(
        textureLoad(surface_normals, vec3i(lower.x, upper.y, upper.z), 0).xyz,
        textureLoad(surface_normals, upper, 0).xyz,
        coordinate.fraction.x,
    );
    return mix(
        mix(x00, x10, coordinate.fraction.y),
        mix(x01, x11, coordinate.fraction.y),
        coordinate.fraction.z,
    );
}

fn grid_surface_normal(
    point: vec3f,
    nearest: u32,
    coordinate: GridCoordinate,
) -> vec3f {
    if nearest == EMPTY_COMPACT_INDEX {
        return vec3f(0.0, 0.0, 1.0);
    }

    let normal = grid_surface_normal_sample(coordinate);

    if dot(normal, normal) >=
        SURFACE_NORMAL_EPSILON_SQ {
        return normalize(normal);
    }

    let atom =
        atoms[nearest];

    let source_index =
        atom.entity_id &
        0x1FFFFFFFu;

    let base =
        source_index * 3u;

    let center =
        vec3f(
            coords[base],
            coords[base + 1u],
            coords[base + 2u],
        );

    return normalize(
        point - center
    );
}
