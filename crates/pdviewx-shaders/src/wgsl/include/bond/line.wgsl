// The screen-space bond line pipeline.
//
// A line is a pixel-width screen rectangle spanning two projected endpoints,
// so it stays legible at any zoom without a geometry shader or a mesh. Flat
// per-bond attributes are authored by the provoking vertices only, which
// keeps the interpolator count down and the per-vertex work minimal.

@vertex
fn vs_bond_line(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> BondLineVsOut {
    let bond = bonds[visible_bonds[instance_index]];

    let atom_a = atoms[bond.atom_a];
    let atom_b = atoms[bond.atom_b];

    let world_a = atom_position(atom_a.entity_id);
    let world_b = atom_position(atom_b.entity_id);

    let endpoint_a = bond_view_position(world_a);
    let endpoint_b = bond_view_position(world_b);

    let ndc_a = bond_project_ndc(endpoint_a);
    let ndc_b = bond_project_ndc(endpoint_b);

    let line_width = representation.visual.w;

    let ndc = bond_quad_ndc(
        ndc_a,
        ndc_b,
        line_width * frame.viewport.zw,
        vertex_index,
    );

    var out: BondLineVsOut;

    out.position = vec4f(
        ndc,
        0.0,
        1.0,
    );

    if bond_flat_source(vertex_index) {
        let pixel_a_axis =
            bond_pixel_a_axis(
                ndc_a,
                ndc_b,
            );

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

        let half_width =
            line_width * 0.5;

        out.endpoint_a =
            endpoint_a;

        out.endpoint_axis =
            endpoint_b - endpoint_a;

        out.pixel_a_axis =
            pixel_a_axis;

        out.color_a =
            color_a;

        out.color_delta =
            color_b - color_a;

        out.motion_a_delta =
            vec4f(
                motion_a,
                motion_b - motion_a,
            );

        out.aux = vec4f(
            varied_roughness(
                bond.entity_id,
                representation.material.x,
            ),

            1.0 / max(
                dot(
                    pixel_a_axis.zw,
                    pixel_a_axis.zw,
                ),
                BOND_LINE_AXIS_EPSILON_SQ,
            ),

            half_width * half_width,

            material_payload(
                representation.material,
            ),
        );

        out.entity_id =
            bond.entity_id;
    }

    return out;
}

fn bond_line_miss() -> BondLineHit {
    return BondLineHit(
        vec3f(0.0),
        0.0,
        false,
    );
}

fn bond_line_hit(
    in: BondLineVsOut,
) -> BondLineHit {
    let pixel_a =
        in.pixel_a_axis.xy;

    let axis =
        in.pixel_a_axis.zw;

    let along = clamp(
        dot(
            in.position.xy - pixel_a,
            axis,
        ) * in.aux.y,
        0.0,
        1.0,
    );

    let delta =
        in.position.xy -
        (pixel_a + axis * along);

    if dot(delta, delta) > in.aux.z {
        return bond_line_miss();
    }

    let position =
        in.endpoint_a +
        in.endpoint_axis * along;

    if representation.clip_meta.x != 0u
        && !representation_visible(
            bond_world_position(position),
        ) {
        return bond_line_miss();
    }

    return BondLineHit(
        position,
        along,
        true,
    );
}

@fragment
fn fs_bond_line(
    in: BondLineVsOut,
) -> BondFsOut {
    let hit = bond_line_hit(in);

    if !hit.valid {
        discard;
    }

    let color =
        bond_color(
            in.color_a,
            in.color_delta,
            hit.along,
        );

    var out: BondFsOut;

    out.albedo_material =
        vec4f(
            color.rgb,
            in.aux.w,
        );

    out.normal_roughness =
        vec4f(
            encode_shading_frame(
                BOND_LINE_NORMAL,
                canonical_tangent(
                    BOND_LINE_NORMAL,
                ),
            ),
            in.aux.x,
        );

    out.entity_id =
        in.entity_id;

    out.structure_id =
        model.structure_id;

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
fn fs_bond_line_transparent(
    in: BondLineVsOut,
) -> OitOutput {
    let hit = bond_line_hit(in);

    if !hit.valid {
        discard;
    }

    let color =
        bond_color(
            in.color_a,
            in.color_delta,
            hit.along,
        );

    let depth =
        bond_view_depth(
            hit.position,
        );

    let lit =
        shade_molecule(
            color.rgb,
            BOND_LINE_NORMAL,
            in.aux.x,
            in.aux.w,
            hit.position,
            oit_occlusion(in.position),
        );

    return weighted_transparency(
        lit,
        color.a,
        depth,
    );
}
