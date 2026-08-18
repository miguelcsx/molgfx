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

/// Computes the exact derivative of the trilinearly interpolated field.
///
/// Eight texture loads replace the previous four field samples = 32 loads.
fn grid_surface_gradient(
    coordinate: GridCoordinate,
) -> vec3f {
    let c =
        grid_corners(coordinate);

    let f =
        coordinate.fraction;

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

    return vec3f(dx, dy, dz) /
        representation.grid_cell.xyz;
}

/// Selects the closest provenance atom among the cell's eight corners.
///
/// The provenance texture is assumed to contain either EMPTY_COMPACT_INDEX or
/// a valid compact atom index generated alongside the persistent surface grid.
fn surface_provenance_at(
    point: vec3f,
    coordinate: GridCoordinate,
) -> u32 {
    let lower =
        coordinate.lower;

    let upper =
        lower + vec3u(1u);

    var best_index =
        EMPTY_COMPACT_INDEX;

    var best_distance =
        SURFACE_INFINITY;

    for (var corner = 0u; corner < 8u; corner++) {
        let grid_point =
            vec3u(
                select(
                    lower.x,
                    upper.x,
                    (corner & 1u) != 0u,
                ),
                select(
                    lower.y,
                    upper.y,
                    (corner & 2u) != 0u,
                ),
                select(
                    lower.z,
                    upper.z,
                    (corner & 4u) != 0u,
                ),
            );

        let compact_index =
            textureLoad(
                surface_provenance,
                vec3i(grid_point),
                0,
            ).x;

        if compact_index ==
            EMPTY_COMPACT_INDEX {
            continue;
        }

        let atom =
            atoms[compact_index];

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

        let distance =
            length(point - center) -
            atom.radius;

        if distance < best_distance {
            best_distance =
                distance;

            best_index =
                compact_index;
        }
    }

    return best_index;
}

fn grid_surface_normal(
    point: vec3f,
    nearest: u32,
    coordinate: GridCoordinate,
) -> vec3f {
    if nearest == EMPTY_COMPACT_INDEX {
        return vec3f(0.0, 0.0, 1.0);
    }

    let gradient =
        grid_surface_gradient(
            coordinate
        );

    if dot(gradient, gradient) >=
        SURFACE_NORMAL_EPSILON_SQ {
        return normalize(gradient);
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
