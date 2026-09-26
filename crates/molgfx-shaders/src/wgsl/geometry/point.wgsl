// Pixel-stable circular atom points for dense semantic zoom.
//
// Opaque and transparent paths use dedicated payloads.
// Point clipping is specialized at pipeline creation and evaluated per vertex,
// never per covered fragment.
//
// Draw contract:
//   topology    = triangle-list
//   vertexCount = 6
//
// Input contract:
//   visible_atoms contains drawable atoms.
//   Optimized camera.wgsl, atom.wgsl, quad.wgsl and motion.wgsl are used.

//!include "include/camera.wgsl"
//!include "include/atom.wgsl"
//!include "include/quad.wgsl"
//!include "include/material_lighting.wgsl"
//!include "include/oit_input.wgsl"
//!include "include/representation.wgsl"
//!include "include/motion.wgsl"
//!include "include/visual/fragment.wgsl"

const POINT_NORMAL: vec3f = vec3f(0.0, 0.0, 1.0);
const POINT_COVERAGE_EPSILON: f32 = 1.0e-6;

// Compile two pipeline variants when representation clipping is optional.
override POINT_CLIPPING_ENABLED: bool = false;

struct PointGeometry {
    clip: vec4f,
    position: vec4f,
    corner: vec2f,
    view_position: vec3f,
}

struct PointOpaqueVsOut {
    @builtin(position) position: vec4f,

    @location(0) @interpolate(linear) corner: vec2f,

    @location(1) @interpolate(flat, either) color: vec4f,
    @location(2) @interpolate(flat, either) motion: vec2f,
    @location(3) @interpolate(flat, either) entity_id: u32,
    // Palette indices for the colour scheme, resolved per fragment.
    @location(4) @interpolate(flat, either) semantic: u32,
}

struct PointTransparentVsOut {
    @builtin(position) position: vec4f,

    @location(0) @interpolate(linear) corner: vec2f,

    @location(1) @interpolate(flat, either) view_position: vec3f,
    @location(2) @interpolate(flat, either) color: vec4f,
    @location(3) @interpolate(flat, either) softness_pixels: f32,
    @location(4) @interpolate(flat, either) entity_id: u32,
    // Palette indices for the colour scheme, resolved per fragment.
    @location(5) @interpolate(flat, either) semantic: u32,
}

struct PointFsOut {
    @location(0) albedo_material: vec4f,
    @location(1) normal_roughness: vec4f,
    @location(2) entity_id: u32,
    @location(3) resident_page: u32,
    @location(4) motion: vec2f,
    @builtin(frag_depth) depth: f32,
}

/// Returns the provoking vertices of the two independent triangles.
fn point_flat_source(vertex: u32) -> bool {
    return vertex == 0u || vertex == 3u;
}

/// Builds the pixel-stable impostor geometry.
fn point_geometry(
    world_position: vec3f,
    vertex: u32,
) -> PointGeometry {
    let view_position =
        camera_view_position(world_position);

    let clip =
        camera_view_clip(view_position);

    let corner =
        quad_corner(vertex);

    let offset =
        corner *
        representation.visual.x *
        frame.viewport.zw *
        clip.w;

    return PointGeometry(
        clip,
        vec4f(
            clip.xy + offset,
            clip.zw,
        ),
        corner,
        view_position,
    );
}

/// Moves a rejected point outside clip space.
fn point_rejected_position() -> vec4f {
    return vec4f(
        0.0,
        0.0,
        -1.0,
        1.0,
    );
}

// -----------------------------------------------------------------------------
// Opaque
// -----------------------------------------------------------------------------

@vertex
fn vs_point(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> PointOpaqueVsOut {
    let atom =
        atoms[
            visible_atoms[instance]
        ];

    let flat =
        point_flat_source(vertex);

    var world_position: vec3f;
    var previous_world = vec3f(0.0);

    // Previous coordinates are fetched only by the two provoking vertices.
    if flat {
        let positions =
            atom_motion_positions(
                atom.entity_id
            );

        world_position =
            positions.current;

        previous_world =
            positions.previous;
    } else {
        world_position =
            atom_position(
                atom.entity_id
            );
    }

    let geometry =
        point_geometry(
            world_position,
            vertex,
        );

    var out: PointOpaqueVsOut;

    out.position =
        geometry.position;

    out.corner =
        geometry.corner;

    out.color =
        vec4f(0.0);

    out.motion =
        vec2f(0.0);

    out.entity_id =
        0u;

    out.semantic =
        0u;

    if POINT_CLIPPING_ENABLED &&
        !representation_visible(world_position) {
        out.position =
            point_rejected_position();

        return out;
    }

    if flat {
        out.color =
            atom_color(atom.color);

        // Current clip XYW already exists; do not project current_world again.
        out.motion =
            screen_motion_from_clip(
                geometry.clip.xyw,

                clip_xyw(
                    frame.previous_view_proj,
                    previous_world,
                ),
            );

        out.entity_id =
            atom.entity_id;

        out.semantic =
            atom.semantic;
    }

    return out;
}

@fragment
fn fs_point(
    in: PointOpaqueVsOut,
) -> PointFsOut {
    if dot(in.corner, in.corner) > 1.0 {
        discard;
    }

    let world_position = atom_position(in.entity_id);
    let visual = visual_fragment(
        in.entity_id,
        point_scheme_color(in.semantic, in.entity_id, in.color),
        visual_local_position(world_position),
        world_position,
        visual_world_normal(POINT_NORMAL),
    );
    if !visual.visible {
        discard;
    }

    var out: PointFsOut;

    out.albedo_material =
        vec4f(
            visual.color.rgb + visual.emission,
            visual_gbuffer_payload(visual),
        );

    out.normal_roughness =
        vec4f(
            encode_shading_frame(
                POINT_NORMAL,
                canonical_tangent(
                    POINT_NORMAL
                ),
            ),
                    visual.roughness,
        );

    out.entity_id =
        pick_local_row(in.entity_id);

    out.resident_page =
        model_pick_page(in.entity_id);

    out.motion =
        in.motion;

    // Fragment position Z is already the rasterized device depth.
    out.depth =
        in.position.z;

    return out;
}

// -----------------------------------------------------------------------------
// Transparent
// -----------------------------------------------------------------------------

@vertex
fn vs_point_transparent(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> PointTransparentVsOut {
    let atom =
        atoms[
            visible_atoms[instance]
        ];

    let world_position =
        atom_position(
            atom.entity_id
        );

    let geometry =
        point_geometry(
            world_position,
            vertex,
        );

    var out: PointTransparentVsOut;

    out.position =
        geometry.position;

    out.corner =
        geometry.corner;

    out.view_position =
        vec3f(0.0);

    out.color =
        vec4f(0.0);

    out.softness_pixels =
        0.0;

    out.entity_id =
        0u;

    out.semantic =
        0u;

    if POINT_CLIPPING_ENABLED &&
        !representation_visible(world_position) {
        out.position =
            point_rejected_position();

        return out;
    }

    if point_flat_source(vertex) {
        out.view_position =
            geometry.view_position;

        out.color =
            atom_record_color(
                atom
            );

        out.softness_pixels =
            atom_softness_pixels(
                atom.semantic
            );

        out.entity_id =
            atom.entity_id;

        out.semantic =
            atom.semantic;
    }

    return out;
}

@fragment
fn fs_point_transparent(
    in: PointTransparentVsOut,
) -> OitOutput {
    let radius_sq =
        dot(
            in.corner,
            in.corner,
        );

    if radius_sq > 1.0 {
        discard;
    }

    let world_position = atom_position(in.entity_id);
    let visual = visual_fragment(
        in.entity_id,
        point_scheme_color(in.semantic, in.entity_id, in.color),
        visual_local_position(world_position),
        world_position,
        visual_world_normal(POINT_NORMAL),
    );
    if !visual.visible {
        discard;
    }

    let radius =
        sqrt(radius_sq);

    let transition =
        max(
            fwidth(radius) *
                max(
                    max(in.softness_pixels, visual.softness_pixels),
                    1.0,
                ),
            POINT_COVERAGE_EPSILON,
        );

    let coverage =
        smoothstep(
            0.0,
            transition,
            1.0 - radius,
        );

    if coverage <= 0.0 {
        discard;
    }

    let lit =
        shade_molecule(
            visual.color.rgb,
            POINT_NORMAL,
            visual.roughness,
            visual_material_payload(visual),
            in.view_position,
            oit_occlusion(
                in.position
            ),
        ) + visual.emission;

    return weighted_transparency(
        lit,
        visual.color.a * coverage,
        in.position.z,
    );
}

/// A point's colour under the representation's scheme and overlay.
///
/// The vertex stage forwards the record's element colour and palette indices;
/// the scheme is applied here, where the overlay's class column is visible.
fn point_scheme_color(semantic: u32, entity_id: u32, element: vec4f) -> vec4f {
    let color = atom_scheme_color(semantic, atom_source_index(entity_id), vec4f(element.rgb, 1.0));
    return vec4f(color.rgb, element.a);
}
