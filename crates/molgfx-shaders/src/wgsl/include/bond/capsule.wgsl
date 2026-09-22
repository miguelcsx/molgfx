// The analytic bond capsule pipeline.
//
// A capsule is one instance of a quad intersected analytically in the
// fragment stage, so the silhouette is exact at any zoom and a bond costs a
// point of instance data rather than a cylinder's vertex ring. Cap shading is
// separated from the barrel so the common path pays no clipping-cap work.

@vertex
fn vs_bond_capsule(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> BondCapsuleVsOut {
    let bond =
        bonds[visible_bonds[instance_index]];

    var radius =
        abs(bond.radius);

    var out: BondCapsuleVsOut;

    if radius <= 0.0 {
        out.position =
            vec4f(
                0.0,
                0.0,
                -1.0,
                1.0,
            );

        return out;
    }

    let atom_a =
        atoms[bond.atom_a];

    let atom_b =
        atoms[bond.atom_b];

    if visual_counts.visual_enabled != 0u {
        let scale = 0.5 * (
            atom_visual_geometry(atom_a.entity_id).z
                + atom_visual_geometry(atom_b.entity_id).z
        ) * 4.0;
        radius *= scale;
    }

    let world_a =
        atom_position(atom_a.entity_id);

    let world_b =
        atom_position(atom_b.entity_id);

    let endpoint_a =
        bond_view_position(world_a);

    let endpoint_b =
        bond_view_position(world_b);

    let ndc_a =
        bond_project_ndc(endpoint_a);

    let ndc_b =
        bond_project_ndc(endpoint_b);

    let ndc =
        bond_quad_ndc(
            ndc_a,
            ndc_b,
            bond_capsule_extent(
                endpoint_a,
                endpoint_b,
                radius,
            ),
            vertex_index,
        );

    out.position =
        vec4f(
            ndc,
            0.0,
            1.0,
        );

    out.ray_xy =
        bond_ray_xy(ndc);

    if bond_flat_source(vertex_index) {
        let axis =
            endpoint_b -
            endpoint_a;

        let color_a =
            atom_color(atom_a.color);

        let color_b =
            atom_color(atom_b.color);

        let motion_a =
            screen_motion(
                world_a,
                previous_atom_position(atom_a.entity_id),
            );

        let motion_b =
            screen_motion(
                world_b,
                previous_atom_position(atom_b.entity_id),
            );

        out.endpoint_a_radius =
            vec4f(
                endpoint_a,
                radius,
            );

        out.endpoint_b_inv_axis_sq =
            vec4f(
                endpoint_b,

                1.0 / max(
                    dot(axis, axis),
                    BOND_AXIS_EPSILON_SQ,
                ),
            );

        out.color_a =
            color_a;

        out.color_delta =
            color_b - color_a;

        out.motion_a_delta =
            vec4f(
                motion_a,
                motion_b - motion_a,
            );

        out.aux =
            vec4f(
                varied_roughness(
                    bond.entity_id,
                    representation.material.x,
                ),

                varied_roughness(
                    bond.entity_id,
                    BOND_CAP_ROUGHNESS,
                ),

                1.0 / radius,

                material_payload(
                    representation.material,
                ),
            );

        out.entity_id =
            bond.entity_id;

        out.atom_entities =
            vec2u(atom_a.entity_id, atom_b.entity_id);
    }

    return out;
}

fn bond_capsule_miss() -> BondCapsuleHit {
    return BondCapsuleHit(
        vec3f(0.0),
        vec3f(0.0),
        0.0,
        false,
        false,
    );
}

/// Resolves the nearest capsule intersection and optional clipping.
fn bond_capsule_resolve(
    in: BondCapsuleVsOut,
    ray_origin: vec3f,
    ray_direction: vec3f,
) -> RepresentationPrimitiveHit {
    let interval =
        ray_capsule_interval(
            ray_direction,
            in.endpoint_a_radius.xyz - ray_origin,
            in.endpoint_b_inv_axis_sq.xyz - ray_origin,
            in.endpoint_a_radius.w,
        );

    if representation.clip_meta.x != 0u {
        return representation_primitive_hit(
            ray_origin,
            ray_direction,
            interval,
            frame.inv_view,
        );
    }

    let t =
        nearest_positive_interval(
            interval,
        );

    return RepresentationPrimitiveHit(
        t,
        NO_CLIP_PLANE,
        false,
        t > 0.0,
    );
}

fn bond_capsule_hit(
    in: BondCapsuleVsOut,
) -> BondCapsuleHit {
    var ray_origin =
        vec3f(0.0);

    var ray_direction =
        vec3f(
            in.ray_xy,
            -1.0,
        );

    if frame.projection_kind.x > 0.5 {
        ray_origin =
            vec3f(in.ray_xy, 0.0);

        ray_direction =
            vec3f(0.0, 0.0, -1.0);
    }

    let resolved =
        bond_capsule_resolve(
            in,
            ray_origin,
            ray_direction,
        );

    if !resolved.valid {
        return bond_capsule_miss();
    }

    let position =
        ray_origin + resolved.t * ray_direction;

    let endpoint_a =
        in.endpoint_a_radius.xyz;

    let axis =
        in.endpoint_b_inv_axis_sq.xyz -
        endpoint_a;

    let along = clamp(
        dot(
            position - endpoint_a,
            axis,
        ) * in.endpoint_b_inv_axis_sq.w,
        0.0,
        1.0,
    );

    let nearest =
        endpoint_a +
        axis * along;

    var normal: vec3f;

    if resolved.cap {
        normal =
            primitive_view_normal(
                resolved,
                vec3f(0.0),
                frame.view,
            );
    } else {
        // Analytic capsule hits are radius units from the nearest axis point.
        normal =
            (position - nearest) *
            in.aux.z;
    }

    return BondCapsuleHit(
        position,
        normal,
        along,
        resolved.cap,
        true,
    );
}

fn bond_capsule_material(
    color: vec3f,
    in: BondCapsuleVsOut,
    cap: bool,
) -> BondMaterialData {
    if cap {
        return BondMaterialData(
            color * BOND_CAP_TINT,
            in.aux.y,
            BOND_CAP_MATERIAL,
        );
    }

    return BondMaterialData(
        color,
        in.aux.x,
        in.aux.w,
    );
}

@fragment
fn fs_bond_capsule(
    in: BondCapsuleVsOut,
) -> BondFsOut {
    let hit =
        bond_capsule_hit(in);

    if !hit.valid {
        discard;
    }

    let color =
        bond_color(
            in.color_a,
            in.color_delta,
            hit.along,
        );

    let material =
        bond_capsule_material(
            color.rgb,
            in,
            hit.cap,
        );

    let visual = bond_visual(
        in.atom_entities,
        hit.along,
        vec4f(material.base, color.a),
        hit.position,
        hit.normal,
    );
    if !visual.visible {
        discard;
    }

    var out: BondFsOut;

    out.albedo_material =
        vec4f(
            visual.color.rgb + visual.emission,
            visual_gbuffer_payload(visual),
        );

    out.normal_roughness =
        vec4f(
            encode_shading_frame(
                hit.normal,
                canonical_tangent(
                    hit.normal,
                ),
            ),
            visual.roughness,
        );

    out.entity_id =
        pick_local_row(in.entity_id);

    out.resident_page =
        model_pick_page(in.entity_id);

    out.motion =
        bond_motion(
            in.motion_a_delta,
            hit.along,
        );

    out.depth =
        bond_view_depth(
            hit.position,
        );

    return out;
}

@fragment
fn fs_bond_capsule_transparent(
    in: BondCapsuleVsOut,
) -> OitOutput {
    let hit =
        bond_capsule_hit(in);

    if !hit.valid {
        discard;
    }

    let color =
        bond_color(
            in.color_a,
            in.color_delta,
            hit.along,
        );

    let material =
        bond_capsule_material(
            color.rgb,
            in,
            hit.cap,
        );

    let visual = bond_visual(
        in.atom_entities,
        hit.along,
        vec4f(material.base, color.a),
        hit.position,
        hit.normal,
    );
    if !visual.visible {
        discard;
    }

    let depth =
        bond_view_depth(
            hit.position,
        );

    let lit =
        shade_molecule(
            visual.color.rgb,
            hit.normal,
            visual.roughness,
            visual_material_payload(visual),
            hit.position,
            oit_occlusion(in.position),
        ) + visual.emission;

    return weighted_transparency(
        lit,
        visual.color.a,
        depth,
    );
}
