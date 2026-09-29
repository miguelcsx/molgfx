// Cartoon fragment path: visual filtering, lighting, and gbuffer output.
// The vertex module owns transport-frame deformation and interpolation.

struct CartoonFsIn {
    @builtin(position) position: vec4f,

    // xyz = view normal, w = curve parameter.
    @location(0) normal_curve: vec4f,

    @location(1) color: vec4f,
    @location(2) view_position: vec3f,
    @location(3) motion: vec2f,
    @location(4) @interpolate(flat, either) entity_id: u32,

    @location(5) clip: vec4f,
}

struct CartoonFsOut {
    @location(0) albedo_material: vec4f,
    @location(1) normal_roughness: vec4f,
    @location(2) entity_id: u32,
    @location(3) resident_page: u32,
    @location(4) motion: vec2f,
}
/// True when a fragment lies outside any active clip plane. Inactive planes
/// hold +1, so their lanes never fail the vector comparison.
fn cartoon_clipped(
    clip: vec4f,
) -> bool {
    return any(
        clip < vec4f(0.0),
    );
}

/// Resolves the representation-specific visual state for one cartoon fragment.
///
/// Keeping world/model reconstruction here gives both opaque and transparent
/// paths exactly one implementation while leaving tangent construction outside:
/// tangent work is needed only after the visual survives filtering.
fn cartoon_fragment_visual(
    entity_id: u32,
    color: vec4f,
    view_position: vec3f,
    view_normal: vec3f,
) -> VisualFragmentResult {
    let world_position =
        transform_point(
            frame.inv_view,
            view_position,
        );

    let model_position =
        transform_point(
            model.world_to_model,
            world_position,
        );

    return ribbon_visual(
        entity_id,
        color,
        model_position,
        world_position,
        cartoon_world_normal(
            view_normal,
        ),
    );
}
@fragment
fn fs_cartoon(
    in: CartoonFsIn,
) -> CartoonFsOut {
    if cartoon_clipped(in.clip) {
        discard;
    }

    let normal =
        normalize(
            in.normal_curve.xyz,
        );

    let visual =
        cartoon_fragment_visual(
            in.entity_id,
            in.color,
            in.view_position,
            normal,
        );

    if !visual.visible {
        discard;
    }

    // Tangent construction is deliberately after visual filtering. Hidden or
    // filtered fragments therefore pay none of the curve-frame work.
    let tangent =
        curve_tangent(
            in.view_position,
            in.normal_curve.w,
            normal,
        );

    var out: CartoonFsOut;

    out.albedo_material =
        vec4f(
            visual.color.rgb
                + visual.emission,
            ribbon_visual_gbuffer_material(
                visual,
            ),
        );

    out.normal_roughness =
        vec4f(
            encode_shading_frame(
                normal,
                tangent,
            ),
            visual.roughness,
        );

    out.entity_id =
        cartoon_pick_local_row(
            in.entity_id,
        );

    out.resident_page =
        cartoon_pick_page(
            in.entity_id,
        );

    out.motion =
        in.motion;

    return out;
}

@fragment
fn fs_cartoon_transparent(
    in: CartoonFsIn,
) -> OitOutput {
    if cartoon_clipped(in.clip) {
        discard;
    }

    let normal =
        normalize(
            in.normal_curve.xyz,
        );

    let visual =
        cartoon_fragment_visual(
            in.entity_id,
            in.color,
            in.view_position,
            normal,
        );

    if !visual.visible {
        discard;
    }

    // As in the opaque path, do not construct a curve tangent for a fragment
    // that visual filtering will throw away.
    let tangent =
        curve_tangent(
            in.view_position,
            in.normal_curve.w,
            normal,
        );

    let lit =
        shade_ribbon(
            visual.color.rgb,
            normal,
            tangent,
            visual.roughness,
            ribbon_visual_material(
                visual,
            ),
            in.view_position,
            oit_occlusion(
                in.position,
            ),
        ) + visual.emission;

    return weighted_transparency(
        lit,
        visual.color.a,
        in.position.z,
    );
}
