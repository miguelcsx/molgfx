// Analytic expansion of one compact pose/topology pair.

const LIGAND_POSE_SCALE_EPSILON: f32 = 1.0e-6;

fn ligand_pose_quaternion_mul(
    left: vec4f,
    right: vec4f,
) -> vec4f {
    return vec4f(
        left.w * right.xyz
            + right.w * left.xyz
            + cross(left.xyz, right.xyz),
        left.w * right.w - dot(left.xyz, right.xyz),
    );
}

fn ligand_pose_transform_point(
    matrix: mat4x4f,
    point: vec3f,
) -> vec3f {
    return matrix[0].xyz * point.x
        + matrix[1].xyz * point.y
        + matrix[2].xyz * point.z
        + matrix[3].xyz;
}

fn ligand_pose_transform_direction(
    matrix: mat4x4f,
    direction: vec3f,
) -> vec3f {
    return matrix[0].xyz * direction.x
        + matrix[1].xyz * direction.y
        + matrix[2].xyz * direction.z;
}

fn ligand_pose_from_z(axis: vec3f) -> vec4f {
    if axis.z < -0.999999 {
        return vec4f(1.0, 0.0, 0.0, 0.0);
    }
    return normalize(
        vec4f(
            -axis.y,
            axis.x,
            0.0,
            1.0 + axis.z,
        )
    );
}

fn ligand_pose_source_index(
    ordinal: u32,
    source: u32,
    selected: u32,
) -> u32 {
    if selected >= source {
        return ordinal;
    }
    if selected <= 1u {
        return source / 2u;
    }
    let source_span = source - 1u;
    let sample_span = selected - 1u;
    let quotient = source_span / sample_span;
    let remainder = source_span % sample_span;
    return ordinal * quotient
        + ordinal * remainder / sample_span;
}

fn ligand_pose_instance(
    vertex: u32,
    instance: u32,
) -> LigandPoseInstance {
    let batch =
        ligand_pose_batches[
            vertex / 6u
        ];
    let sphere =
        LIGAND_POSE_SHAPE ==
        LIGAND_POSE_SPHERE;
    let topology_count =
        select(
            batch.topology.w,
            batch.topology.y,
            sphere,
        );
    let topology_row = instance % topology_count;
    let translucent =
        LIGAND_POSE_TRANSLUCENT != 0u;
    let source_first =
        select(batch.poses.x, batch.poses.y, translucent);
    let source_count =
        select(
            batch.sampling.x,
            batch.sampling.z,
            translucent,
        );
    let selected_count =
        select(
            batch.sampling.y,
            batch.sampling.w,
            translucent,
        );
    let pose_row =
        source_first
        + ligand_pose_source_index(
            instance / topology_count,
            source_count,
            selected_count,
        );
    let pose =
        ligand_pose_transforms[pose_row];

    var local_center: vec3f;
    var local_orientation: vec4f;
    var local_size: vec3f;
    if sphere {
        local_center =
            ligand_pose_atoms[
                batch.topology.x +
                topology_row
            ].xyz;
        local_orientation =
            pose.orientation;
        local_size =
            vec3f(batch.radii.x);
    } else {
        let bond =
            ligand_pose_bonds[
                batch.topology.z +
                topology_row
            ];
        local_center =
            bond.center_length.xyz;
        local_orientation =
            ligand_pose_quaternion_mul(
                pose.orientation,
                bond.orientation,
            );
        local_size =
            vec3f(
                batch.radii.y,
                batch.radii.y,
                bond.center_length.w +
                    batch.radii.y,
            );
    }

    let oriented_center =
        pose.translation_opacity.xyz
        + rotate_vector(
            pose.orientation,
            local_center,
        );
    let local_x =
        rotate_vector(
            local_orientation,
            vec3f(1.0, 0.0, 0.0),
        );
    let local_y =
        rotate_vector(
            local_orientation,
            vec3f(0.0, 1.0, 0.0),
        );
    let local_z =
        rotate_vector(
            local_orientation,
            vec3f(0.0, 0.0, 1.0),
        );
    let world_x =
        ligand_pose_transform_direction(
            batch.model,
            local_x,
        );
    let world_y =
        ligand_pose_transform_direction(
            batch.model,
            local_y,
        );
    let world_z =
        ligand_pose_transform_direction(
            batch.model,
            local_z,
        );
    let scales =
        max(
            vec3f(
                length(world_x),
                length(world_y),
                length(world_z),
            ),
            vec3f(LIGAND_POSE_SCALE_EPSILON),
        );
    let size =
        local_size * scales;
    var color =
        unpack4x8unorm(
            ligand_pose_styles[pose_row]
        );
    color.a *=
        pose.translation_opacity.w;

    return LigandPoseInstance(
        vec4f(
            ligand_pose_transform_point(
                batch.model,
                oriented_center,
            ),
            length(size * 0.5),
        ),
        select(
            vec4f(
                ligand_pose_from_z(
                    world_z / scales.z,
                )
            ),
            vec4f(0.0, 0.0, 0.0, 1.0),
            sphere,
        ),
        size,
        color,
        vec4u(
            batch.poses.z,
            batch.poses.w,
            3u,
            LIGAND_POSE_SHAPE,
        ),
    );
}
