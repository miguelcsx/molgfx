// Atom, bond and ribbon shadow casters.
//
// A sphere's depth comes straight from its circular cross-section and a bond
// resolves only its ray parameter, so neither reconstructs a full hit point
// it would immediately discard. Ribbons use native indexed vertex fetching.

struct ShadowSphereVsOut {
    @builtin(position) position: vec4f,

    @location(0) @interpolate(linear) light_xy: vec2f,

    // xyz = light-space center, w = radius².
    @location(1) @interpolate(flat, either) center_radius_sq: vec4f,
    @location(2) @interpolate(flat, either) color: vec4f,
    @location(3) @interpolate(flat, either) entity_id: u32,
}

@vertex
fn vs_shadow_sphere(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> ShadowSphereVsOut {
    let atom =
        atoms[
            visible_atoms[instance]
        ];

    let center =
        shadow_view_position(
            atom_position(atom.entity_id)
        );

    let radius =
        abs(atom.radius);

    let light_xy =
        center.xy
        + quad_corner(vertex) * radius;

    return ShadowSphereVsOut(
        shadow_clip(
            vec3f(
                light_xy,
                center.z,
            )
        ),
        light_xy,
        vec4f(
            center,
            radius * radius,
        ),
        atom_color(atom.color),
        atom.entity_id,
    );
}

@fragment
fn fs_shadow_sphere(
    in: ShadowSphereVsOut,
) -> @builtin(frag_depth) f32 {
    let delta =
        in.light_xy
        - in.center_radius_sq.xy;

    let radial_sq =
        dot(delta, delta);

    let radius_sq =
        in.center_radius_sq.w;

    if radial_sq > radius_sq {
        discard;
    }

    let root =
        sqrt(
            max(
                radius_sq - radial_sq,
                0.0,
            )
        );

    let center_z =
        in.center_radius_sq.z;

    let front_z =
        center_z + root;

    let rear_z =
        center_z - root;

    // Ray starts at light-space Z=0 and travels toward -Z.
    let hit_z =
        select(
            rear_z,
            front_z,
            front_z < 0.0,
        );

    if hit_z >= 0.0 {
        discard;
    }

    if VISUAL_PROGRAM_ENABLED {
        let light_position = vec3f(in.light_xy, hit_z);
        let world_position = shadow_world_position(light_position);
        let light_normal = normalize(light_position - in.center_radius_sq.xyz);
        let world_normal = normalize(
            frame.shadow_inv_view[0].xyz * light_normal.x
                + frame.shadow_inv_view[1].xyz * light_normal.y
                + frame.shadow_inv_view[2].xyz * light_normal.z
        );
        let visual = visual_fragment(
            in.entity_id,
            in.color,
            visual_local_position(world_position),
            world_position,
            world_normal,
        );
        if !visual.visible || visual.color.a <= 0.0 {
            discard;
        }
    }

    return stable_entity_depth(
        shadow_depth(
            vec3f(
                in.light_xy,
                hit_z,
            )
        ),
        in.entity_id,
    );
}

// -----------------------------------------------------------------------------
// Bond capsules
// -----------------------------------------------------------------------------

struct ShadowBondVsOut {
    @builtin(position) position: vec4f,

    @location(0) @interpolate(linear) light_xy: vec2f,

    @location(1) @interpolate(flat, either) endpoint_a: vec3f,
    @location(2) @interpolate(flat, either) endpoint_b: vec3f,
    @location(3) @interpolate(flat, either) radius: f32,
    @location(4) @interpolate(flat, either) atom_entities: vec2u,
    @location(5) @interpolate(flat, either) color_a: vec4f,
    @location(6) @interpolate(flat, either) color_b: vec4f,
}

@vertex
fn vs_shadow_bond(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> ShadowBondVsOut {
    let bond =
        bonds[
            visible_bonds[instance]
        ];

    let endpoint_a =
        shadow_view_position(
            atom_position(
                atoms[bond.atom_a].entity_id
            )
        );

    let endpoint_b =
        shadow_view_position(
            atom_position(
                atoms[bond.atom_b].entity_id
            )
        );

    let radius =
        abs(bond.radius);

    let atom_a = atoms[bond.atom_a];
    let atom_b = atoms[bond.atom_b];

    // Construct the bounding rectangle directly in light-view units.
    // No project -> NDC -> inverse-project round trip is required.
    let low =
        min(
            endpoint_a.xy,
            endpoint_b.xy,
        ) - vec2f(radius);

    let high =
        max(
            endpoint_a.xy,
            endpoint_b.xy,
        ) + vec2f(radius);

    let light_xy =
        mix(
            low,
            high,
            quad_uv(vertex),
        );

    let proxy_z =
        (
            endpoint_a.z +
            endpoint_b.z
        ) * 0.5;

    return ShadowBondVsOut(
        shadow_clip(
            vec3f(
                light_xy,
                proxy_z,
            )
        ),
        light_xy,
        endpoint_a,
        endpoint_b,
        radius,
        vec2u(atom_a.entity_id, atom_b.entity_id),
        atom_color(atom_a.color),
        atom_color(atom_b.color),
    );
}

@fragment
fn fs_shadow_bond(
    in: ShadowBondVsOut,
) -> @builtin(frag_depth) f32 {
    let origin =
        vec3f(
            in.light_xy,
            0.0,
        );

    let t =
        ray_capsule(
            vec3f(0.0, 0.0, -1.0),
            in.endpoint_a - origin,
            in.endpoint_b - origin,
            in.radius,
        );

    if t <= 0.0 {
        discard;
    }

    if VISUAL_PROGRAM_ENABLED {
        let light_position = vec3f(in.light_xy, -t);
        let axis = in.endpoint_b - in.endpoint_a;
        let along = clamp(
            dot(light_position - in.endpoint_a, axis) / max(dot(axis, axis), 1.0e-8),
            0.0,
            1.0,
        );
        let nearest = in.endpoint_a + axis * along;
        let light_normal = normalize(light_position - nearest);
        let world_position = shadow_world_position(light_position);
        let world_normal = normalize(
            frame.shadow_inv_view[0].xyz * light_normal.x
                + frame.shadow_inv_view[1].xyz * light_normal.y
                + frame.shadow_inv_view[2].xyz * light_normal.z
        );
        let entity_id = select(in.atom_entities.x, in.atom_entities.y, along >= 0.5);
        let base_color = mix(in.color_a, in.color_b, along);
        let visual = visual_fragment(
            entity_id,
            base_color,
            visual_local_position(world_position),
            world_position,
            world_normal,
        );
        if !visual.visible || visual.color.a <= 0.0 {
            discard;
        }
    }

    // No hit reconstruction / matrix transform needed:
    // origin.z = 0 and direction.z = -1.
    return shadow_depth(
        vec3f(
            in.light_xy,
            -t,
        )
    );
}
