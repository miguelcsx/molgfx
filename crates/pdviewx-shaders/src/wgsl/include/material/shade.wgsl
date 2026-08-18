// The shading entry points every representation converges on.
//
// Surfaces, molecules and ribbons share one lighting model here rather than
// each carrying its own, so the deferred and forward paths cannot drift
// apart in appearance.

fn shade_surface(
    albedo: vec3f,
    normal: vec3f,
    roughness: f32,
    material_payload_value: f32,
    input_tangent: vec3f,
    position: vec3f,
    occlusion: vec2f,
) -> vec3f {
    let material =
        lighting_material(
            albedo,
            material_payload_value,
        );

    let view_direction =
        normalize(-position);

    let raw_n_dot_v =
        max(
            dot(
                normal,
                view_direction,
            ),
            0.0,
        );

    let n_dot_v =
        max(
            raw_n_dot_v,
            1.0e-4,
        );

    let key_direction =
        frame.lighting[4].xyz;

    let fill_direction =
        frame.lighting[6].xyz;

    let key_n_dot_l =
        max(
            dot(
                normal,
                key_direction,
            ),
            0.0,
        );

    let fill_n_dot_l =
        max(
            dot(
                normal,
                fill_direction,
            ),
            0.0,
        );

    // Original code computed two sqrt() via length(), then immediately
    // squared the maximum again. Work directly in squared space.
    let normal_dx =
        dpdx(normal);

    let normal_dy =
        dpdy(normal);

    let derivative_squared =
        max(
            dot(normal_dx, normal_dx),
            dot(normal_dy, normal_dy),
        );

    let roughness_squared =
        roughness * roughness;

    let filtered_roughness =
        clamp(
            sqrt(
                roughness_squared +
                min(
                    derivative_squared,
                    0.18,
                )
            ),
            0.05,
            0.92,
        );

    let alpha =
        filtered_roughness *
        filtered_roughness;

    let alpha_squared =
        alpha * alpha;

    let direct_visibility =
        occlusion.g;

    let soft_visibility =
        mix(
            1.0,
            direct_visibility,
            smoothstep(
                0.15,
                0.72,
                key_n_dot_l,
            ),
        );

    let key_radiance =
        frame.lighting[5].rgb *
        frame.lighting[4].w;

    let fill_radiance =
        frame.lighting[7].rgb *
        frame.lighting[6].w;

    var direct: vec3f;

    // Common molecular surfaces stop here and never evaluate tangent,
    // bitangent or anisotropic GGX.
    if material.anisotropy <= 0.0 {
        direct =
            evaluate_light_isotropic(
                material,
                normal,
                view_direction,
                n_dot_v,
                key_direction,
                key_n_dot_l,
                key_radiance,
                alpha_squared,
                soft_visibility,
            ) +
            evaluate_light_isotropic(
                material,
                normal,
                view_direction,
                n_dot_v,
                fill_direction,
                fill_n_dot_l,
                fill_radiance,
                alpha_squared,
                1.0,
            );
    } else {
        let anisotropic =
            anisotropic_state(
                normal,
                input_tangent,
                view_direction,
                n_dot_v,
                alpha,
                material.anisotropy,
            );

        direct =
            evaluate_light_anisotropic(
                material,
                normal,
                view_direction,
                n_dot_v,
                key_direction,
                key_n_dot_l,
                key_radiance,
                alpha_squared,
                anisotropic,
                soft_visibility,
            ) +
            evaluate_light_anisotropic(
                material,
                normal,
                view_direction,
                n_dot_v,
                fill_direction,
                fill_n_dot_l,
                fill_radiance,
                alpha_squared,
                anisotropic,
                1.0,
            );
    }

    let ao =
        occlusion.r;

    let indirect_visibility =
        multi_bounce_visibility(
            ao,
            material.diffuse_albedo,
        );

    let indirect_diffuse =
        material.diffuse_albedo *
        environment_irradiance(normal) *
        indirect_visibility;

    let environment_fresnel =
        fresnel_schlick(
            raw_n_dot_v,
            material.f0,
        );

    let reflection =
        reflect(
            -view_direction,
            normal,
        );

    let indirect_specular =
        environment_radiance(reflection) *
        frame.lighting[1].w *
        environment_fresnel *
        (
            1.0 -
            filtered_roughness * 0.6
        ) *
        ao;

    // (1 - N·V)^6 without generic pow().
    let rim_base =
        1.0 -
        raw_n_dot_v;

    let rim_squared =
        rim_base *
        rim_base;

    let rim =
        rim_squared *
        rim_squared *
        rim_squared *
        ao *
        mix(
            0.5,
            1.0,
            direct_visibility,
        );

    return direct +
        indirect_diffuse +
        indirect_specular +
        rim *
            frame.lighting[3].rgb *
            frame.lighting[2].w;
}

fn shade_molecule(
    albedo: vec3f,
    normal: vec3f,
    roughness: f32,
    material: f32,
    position: vec3f,
    occlusion: vec2f,
) -> vec3f {
    // No canonical tangent is generated unless the payload actually requests
    // anisotropic shading.
    return shade_surface(
        albedo,
        normal,
        roughness,
        material,
        vec3f(0.0),
        position,
        occlusion,
    );
}

fn shade_ribbon(
    albedo: vec3f,
    normal: vec3f,
    tangent: vec3f,
    roughness: f32,
    material: f32,
    position: vec3f,
    occlusion: vec2f,
) -> vec3f {
    return shade_surface(
        albedo,
        normal,
        roughness,
        material,
        tangent,
        position,
        occlusion,
    );
}
