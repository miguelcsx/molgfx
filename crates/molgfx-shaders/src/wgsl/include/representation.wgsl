// Shared per-representation geometry, visual and world-space clipping state.
//
// Analytic clipping tightens a ray interval against at most four world-space
// half-spaces. Invalid intervals terminate immediately.
//
// Primitive resolution selects the retained front shell, optional clip cap,
// or retained rear shell without constructing intermediate hit records.

const SURFACE_KIND_GAUSSIAN: u32 = 3u;

struct RepresentationUniforms {
    surface: vec4f,
    grid_min: vec4f,
    grid_cell: vec4f,
    options: vec4u,
    grid_size: vec4u,
    visual: vec4f,
    clip_planes: array<vec4f, 4>,
    clip_meta: vec4u,
    overlay_world_to_voxel: mat4x4f,
    overlay_domain: vec4f,
    overlay_colors: array<vec4f, 3>,
    overlay_size: vec4u,
    overlay_visual: vec4f,
    material: vec4f,
    presentation: vec4f,
}

@group(2) @binding(9)
var<uniform> representation: RepresentationUniforms;

const NO_CLIP_PLANE: u32 = 0xFFFFFFFFu;
const REPRESENTATION_MAX_CLIP_PLANES: u32 = 4u;
const REPRESENTATION_CLIP_EPSILON: f32 = 1.0e-7;

struct RepresentationClipInterval {
    range: vec2f,
    entry_plane: u32,
}

struct RepresentationPrimitiveHit {
    t: f32,
    plane: u32,
    cap: bool,
    valid: bool,
}

fn representation_clip_count() -> u32 {
    return min(
        representation.clip_meta.x,
        REPRESENTATION_MAX_CLIP_PLANES,
    );
}

fn representation_interval_valid(
    interval: vec2f,
) -> bool {
    return interval.x <= interval.y
        && interval.y > 0.0;
}

fn representation_miss_hit() -> RepresentationPrimitiveHit {
    return RepresentationPrimitiveHit(
        -1.0,
        NO_CLIP_PLANE,
        false,
        false,
    );
}

fn representation_miss_interval() -> RepresentationClipInterval {
    return RepresentationClipInterval(
        vec2f(1.0, 0.0),
        NO_CLIP_PLANE,
    );
}

/// Transforms a direction without translation or homogeneous W work.
fn representation_transform_direction(
    transform: mat4x4f,
    direction: vec3f,
) -> vec3f {
    return transform[0].xyz * direction.x
        + transform[1].xyz * direction.y
        + transform[2].xyz * direction.z;
}

/// Transforms an affine point without computing homogeneous W.
fn representation_transform_point(
    transform: mat4x4f,
    point: vec3f,
) -> vec3f {
    return representation_transform_direction(
        transform,
        point,
    ) + transform[3].xyz;
}

/// Tests a world-space point against all active clipping half-spaces.
fn representation_visible(
    world_position: vec3f,
) -> bool {
    let count =
        representation_clip_count();

    for (
        var index = 0u;
        index < count;
        index++
    ) {
        let plane =
            representation.clip_planes[index];

        if dot(
            plane.xyz,
            world_position,
        ) + plane.w < 0.0 {
            return false;
        }
    }

    return true;
}

/// Clips a ray interval using an already-resolved active plane count.
///
/// Lower-bound crossings record the plane responsible for a potential cap.
/// Any empty interval terminates immediately.
fn representation_clip_interval_count(
    world_origin: vec3f,
    world_direction: vec3f,
    input: vec2f,
    count: u32,
) -> RepresentationClipInterval {
    if input.x > input.y {
        return representation_miss_interval();
    }

    var range =
        input;

    var entry_plane =
        NO_CLIP_PLANE;

    for (
        var index = 0u;
        index < count;
        index++
    ) {
        let plane =
            representation.clip_planes[index];

        let origin_distance =
            dot(
                plane.xyz,
                world_origin,
            ) + plane.w;

        let denominator =
            dot(
                plane.xyz,
                world_direction,
            );

        if denominator >
            REPRESENTATION_CLIP_EPSILON {
            let crossing =
                -origin_distance /
                denominator;

            if crossing > range.x {
                range.x =
                    crossing;

                entry_plane =
                    index;

                if range.x > range.y {
                    return representation_miss_interval();
                }
            }
        } else if denominator <
            -REPRESENTATION_CLIP_EPSILON {
            range.y =
                min(
                    range.y,
                    -origin_distance /
                        denominator,
                );

            if range.x > range.y {
                return representation_miss_interval();
            }
        } else if origin_distance < 0.0 {
            // Parallel ray entirely outside this half-space.
            return representation_miss_interval();
        }
    }

    return RepresentationClipInterval(
        range,
        entry_plane,
    );
}

/// Clips an arbitrary world-space ray interval.
fn representation_clip_interval(
    world_origin: vec3f,
    world_direction: vec3f,
    input: vec2f,
) -> RepresentationClipInterval {
    return representation_clip_interval_count(
        world_origin,
        world_direction,
        input,
        representation_clip_count(),
    );
}

/// Resolves clipping against a complete analytic primitive interval.
///
/// Open cuts expose the retained rear shell. Solid cuts return the geometric
/// entry-plane cap. With clipping disabled this reduces directly to the
/// nearest positive primitive endpoint.
fn representation_primitive_hit(
    view_origin: vec3f,
    view_direction: vec3f,
    primitive: vec2f,
    world_from_view: mat4x4f,
) -> RepresentationPrimitiveHit {
    if !representation_interval_valid(
        primitive
    ) {
        return representation_miss_hit();
    }

    let clip_count =
        representation_clip_count();

    // Common no-clipping path: avoid world-ray transformation entirely.
    if clip_count == 0u {
        return RepresentationPrimitiveHit(
            select(
                primitive.y,
                primitive.x,
                primitive.x > 0.0,
            ),
            NO_CLIP_PLANE,
            false,
            true,
        );
    }

    let world_origin =
        representation_transform_point(
            world_from_view,
            view_origin,
        );

    let world_direction =
        representation_transform_direction(
            world_from_view,
            view_direction,
        );

    let clipped =
        representation_clip_interval_count(
            world_origin,
            world_direction,
            primitive,
            clip_count,
        );

    if !representation_interval_valid(
        clipped.range
    ) {
        return representation_miss_hit();
    }

    // range.x begins at primitive.x and can only increase. Therefore this is
    // equivalent to testing whether the original front shell survived.
    if primitive.x > 0.0
        && clipped.range.x <= primitive.x {
        return RepresentationPrimitiveHit(
            primitive.x,
            NO_CLIP_PLANE,
            false,
            true,
        );
    }

    // A lower-bound clipping plane replaced the front shell.
    if representation.clip_meta.y != 0u
        && clipped.entry_plane != NO_CLIP_PLANE
        && clipped.range.x > 0.0 {
        return RepresentationPrimitiveHit(
            clipped.range.x,
            clipped.entry_plane,
            true,
            true,
        );
    }

    // range.y begins at primitive.y and can only decrease. Equality means the
    // original rear shell was retained after clipping.
    if primitive.y <= clipped.range.y {
        return RepresentationPrimitiveHit(
            primitive.y,
            NO_CLIP_PLANE,
            false,
            true,
        );
    }

    return representation_miss_hit();
}

/// Resolves the analytic or clip-plane view-space normal.
fn primitive_view_normal(
    hit: RepresentationPrimitiveHit,
    analytic_normal: vec3f,
    view_from_world: mat4x4f,
) -> vec3f {
    if !hit.cap {
        return analytic_normal;
    }

    return normalize(
        representation_transform_direction(
            view_from_world,
            -representation
                .clip_planes[hit.plane]
                .xyz,
        )
    );
}
