// Shared unit-quaternion vector rotation.

fn rotate_vector(
    quaternion: vec4f,
    value: vec3f,
) -> vec3f {
    let twice_cross =
        2.0 * cross(quaternion.xyz, value);

    return value
        + quaternion.w * twice_cross
        + cross(quaternion.xyz, twice_cross);
}
