// BVH-bounded vdW/SAS and persistent-grid SES surfaces.
//
// Pipelines:
//   union -> vdW / solvent-accessible
//   grid  -> solvent-excluded
//
// Ray unprojection is performed on the four impostor vertices and linearly
// interpolated. Fragment work starts directly in model-local ray space.
//
// The SES normal is the analytic gradient of the same trilinear grid field,
// requiring one eight-texel cell load instead of four field evaluations.
//
// This file holds the two pipeline stages only; the tracing, sampling and
// shading each pipeline calls live beside it under include/surface/.

//!include "include/camera.wgsl"
//!include "include/atom.wgsl"
//!include "include/surface_field.wgsl"
//!include "include/material_lighting.wgsl"
//!include "include/oit_input.wgsl"
//!include "include/scalar_overlay.wgsl"
//!include "include/motion.wgsl"
//!include "include/surface/types.wgsl"
//!include "include/surface/ray.wgsl"
//!include "include/surface/grid_field.wgsl"
//!include "include/surface/union_trace.wgsl"
//!include "include/surface/grid_trace.wgsl"
//!include "include/surface/shading.wgsl"

@vertex
fn vs_surface(
    @builtin(vertex_index) vertex: u32,
) -> SurfaceVsOut {
    let bounds =
        surface_bounds();

    let ndc =
        mix(
            bounds.low,
            bounds.high,
            surface_quad_uv(vertex),
        );

    let ray =
        surface_ray_span(ndc);

    var out: SurfaceVsOut;

    out.position =
        vec4f(
            ndc,
            0.0,
            1.0,
        );

    out.ray_origin =
        ray[0];

    out.ray_vector =
        ray[1];

    return out;
}

// -----------------------------------------------------------------------------
// vdW / SAS
// -----------------------------------------------------------------------------

@fragment
fn fs_surface_union(
    in: SurfaceVsOut,
) -> SurfaceFsOut {
    let hit =
        intersect_union_surface(
            surface_ray(in)
        );

    if !hit.valid {
        discard;
    }

    return surface_opaque_output(hit);
}

@fragment
fn fs_surface_union_transparent(
    in: SurfaceVsOut,
) -> OitOutput {
    let hit =
        intersect_union_surface(
            surface_ray(in)
        );

    if !hit.valid {
        discard;
    }

    return surface_transparent_output(
        in,
        hit,
    );
}

// -----------------------------------------------------------------------------
// SES
// -----------------------------------------------------------------------------

@fragment
fn fs_surface_grid(
    in: SurfaceVsOut,
) -> SurfaceFsOut {
    let hit =
        intersect_grid_surface(
            surface_ray(in)
        );

    if !hit.valid {
        discard;
    }

    return surface_opaque_output(hit);
}

@fragment
fn fs_surface_grid_transparent(
    in: SurfaceVsOut,
) -> OitOutput {
    let hit =
        intersect_grid_surface(
            surface_ray(in)
        );

    if !hit.valid {
        discard;
    }

    return surface_transparent_output(
        in,
        hit,
    );
}
