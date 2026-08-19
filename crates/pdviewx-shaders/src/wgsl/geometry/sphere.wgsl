// Specialized analytic sphere impostors into the shared gbuffer.
//
// Opaque and transparent vertex paths avoid evaluating pass-specific data.
// Clipped and unclipped fragment paths are separate so the common path pays
// no clipping branch or clip-cap material work.
//
// One visible atom expands to a six-vertex triangle-list quad.
//
// Contract:
//   visible_atoms contains only drawable atoms with radius > 0.
//
// This file holds the pipeline stages; the payloads, intersection and output
// they call live beside it under include/sphere/.

//!include "include/camera.wgsl"
//!include "include/quad.wgsl"
//!include "include/atom.wgsl"
//!include "include/intersect.wgsl"
//!include "include/material_lighting.wgsl"
//!include "include/oit_input.wgsl"
//!include "include/representation.wgsl"
//!include "include/motion.wgsl"
//!include "include/sphere/types.wgsl"
//!include "include/sphere/intersect.wgsl"
//!include "include/sphere/output.wgsl"

@vertex
fn vs_sphere_opaque(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> SphereVsOut {
    let atom =
        atoms[
            visible_atoms[instance]
        ];

    let geometry =
        sphere_geometry(
            atom,
            vertex,
        );

    var out =
        sphere_vertex_output(
            geometry,
        );

    if sphere_flat_source(vertex) {
        out.center_radius =
            geometry.center_radius;

        out.color =
            atom_color(atom.color);

        out.previous_softness =
            vec4f(
                previous_atom_position(atom.entity_id) -
                    geometry.world_center,
                0.0,
            );

        out.material =
            sphere_flat_material(atom);

        out.entity_id =
            atom.entity_id;
    }

    return out;
}

@vertex
fn vs_sphere_transparent(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> SphereVsOut {
    let atom =
        atoms[
            visible_atoms[instance]
        ];

    let geometry =
        sphere_geometry(
            atom,
            vertex,
        );

    var out =
        sphere_vertex_output(
            geometry,
        );

    if sphere_flat_source(vertex) {
        out.center_radius =
            geometry.center_radius;

        out.color =
            atom_color(atom.color);

        out.previous_softness =
            vec4f(
                0.0,
                0.0,
                0.0,
                atom_softness_pixels(atom.semantic),
            );

        out.material =
            sphere_flat_material(atom);

        out.entity_id =
            atom.entity_id;
    }

    return out;
}

// -----------------------------------------------------------------------------
// Opaque / unclipped
// -----------------------------------------------------------------------------

@fragment
fn fs_sphere(
    in: SphereVsOut,
) -> SphereFsOut {
    let surface =
        sphere_surface_unclipped(in);

    if !surface.valid {
        discard;
    }

    return sphere_opaque_output(
        in,
        surface,
        SphereMaterial(
            vec4f(
                in.color.rgb,
                in.material.z,
            ),
            in.material.x,
        ),
    );
}

// -----------------------------------------------------------------------------
// Opaque / clipped
// -----------------------------------------------------------------------------

@fragment
fn fs_sphere_clipped(
    in: SphereVsOut,
) -> SphereFsOut {
    let surface =
        sphere_surface_clipped(in);

    if !surface.valid {
        discard;
    }

    return sphere_opaque_output(
        in,
        surface,
        sphere_clipped_material(
            in,
            surface.cap,
        ),
    );
}

// -----------------------------------------------------------------------------
// Transparent / unclipped
// -----------------------------------------------------------------------------

@fragment
fn fs_sphere_transparent(
    in: SphereVsOut,
) -> OitOutput {
    let surface =
        sphere_surface_unclipped(in);

    if !surface.valid {
        discard;
    }

    return sphere_transparent_output(
        in,
        surface,
        SphereMaterial(
            vec4f(
                in.color.rgb,
                in.material.z,
            ),
            in.material.x,
        ),
    );
}

// -----------------------------------------------------------------------------
// Transparent / clipped
// -----------------------------------------------------------------------------

@fragment
fn fs_sphere_transparent_clipped(
    in: SphereVsOut,
) -> OitOutput {
    let surface =
        sphere_surface_clipped(in);

    if !surface.valid {
        discard;
    }

    return sphere_transparent_output(
        in,
        surface,
        sphere_clipped_material(
            in,
            surface.cap,
        ),
    );
}
