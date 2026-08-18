// Per-light response and environment ambient terms.
//
// Isotropic and anisotropic light evaluation are separate entry points, so a
// scene of ordinary surfaces never pays for tangent-frame work. Cost is
// O(lights) per shaded fragment, with the anisotropic path adding a constant
// factor only where a surface actually asks for it.

/// Combines Fresnel, diffuse and specular terms.
///
/// The original diffusion backlight expression was mathematically dead:
/// evaluate_light rejected n·l <= 0 before evaluating dot(-n,l), making that
/// value always zero. This preserves the original output using only its
/// reachable ambient-floor contribution.
fn light_response(
    material: LightingMaterial,
    normal: vec3f,
    view_direction: vec3f,
    light_direction: vec3f,
    radiance: vec3f,
    visibility: f32,
    n_dot_l: f32,
    v_dot_h: f32,
    distribution: f32,
    geometric_visibility: f32,
) -> vec3f {
    let fresnel =
        fresnel_schlick(
            v_dot_h,
            material.f0,
        );

    let specular =
        distribution *
        geometric_visibility *
        fresnel;

    let diffuse =
        (
            vec3f(1.0) -
            fresnel
        ) *
        material.diffuse_albedo *
        INV_PI;

    let attenuation =
        n_dot_l *
        visibility;

    var result =
        (
            diffuse +
            specular
        ) *
        radiance *
        attenuation;

    if material.diffusion > 0.0 {
        result +=
            material.albedo *
            radiance *
            (
                material.diffusion *
                DIFFUSION_AMBIENT_FLOOR *
                visibility
            );
    }

    return result;
}

fn evaluate_light_isotropic(
    material: LightingMaterial,
    normal: vec3f,
    view_direction: vec3f,
    n_dot_v: f32,
    light_direction: vec3f,
    n_dot_l: f32,
    radiance: vec3f,
    alpha_squared: f32,
    visibility: f32,
) -> vec3f {
    if n_dot_l <= 0.0 {
        return vec3f(0.0);
    }

    let half_vector =
        normalize(
            light_direction +
            view_direction
        );

    let n_dot_h =
        max(
            dot(
                normal,
                half_vector,
            ),
            0.0,
        );

    let v_dot_h =
        max(
            dot(
                view_direction,
                half_vector,
            ),
            0.0,
        );

    return light_response(
        material,
        normal,
        view_direction,
        light_direction,
        radiance,
        visibility,
        n_dot_l,
        v_dot_h,

        distribution_ggx(
            n_dot_h,
            alpha_squared,
        ),

        visibility_smith_ggx_correlated(
            n_dot_v,
            n_dot_l,
            alpha_squared,
        ),
    );
}

fn evaluate_light_anisotropic(
    material: LightingMaterial,
    normal: vec3f,
    view_direction: vec3f,
    n_dot_v: f32,
    light_direction: vec3f,
    n_dot_l: f32,
    radiance: vec3f,
    alpha_squared: f32,
    state: AnisotropicState,
    visibility: f32,
) -> vec3f {
    if n_dot_l <= 0.0 {
        return vec3f(0.0);
    }

    let half_vector =
        normalize(
            light_direction +
            view_direction
        );

    let n_dot_h =
        max(
            dot(
                normal,
                half_vector,
            ),
            0.0,
        );

    let v_dot_h =
        max(
            dot(
                view_direction,
                half_vector,
            ),
            0.0,
        );

    let anisotropic_distribution =
        distribution_ggx_anisotropic(
            n_dot_h,
            dot(
                state.tangent,
                half_vector,
            ),
            dot(
                state.bitangent,
                half_vector,
            ),
            state,
        );

    let anisotropic_visibility =
        visibility_smith_ggx_anisotropic(
            n_dot_v,
            n_dot_l,

            dot(
                state.tangent,
                light_direction,
            ),

            dot(
                state.bitangent,
                light_direction,
            ),

            state,
        );

    var distribution =
        anisotropic_distribution;

    var geometric_visibility =
        anisotropic_visibility;

    // Full anisotropy does not need the isotropic reference response.
    if material.anisotropy < 1.0 {
        distribution =
            mix(
                distribution_ggx(
                    n_dot_h,
                    alpha_squared,
                ),
                anisotropic_distribution,
                material.anisotropy,
            );

        geometric_visibility =
            mix(
                visibility_smith_ggx_correlated(
                    n_dot_v,
                    n_dot_l,
                    alpha_squared,
                ),
                anisotropic_visibility,
                material.anisotropy,
            );
    }

    return light_response(
        material,
        normal,
        view_direction,
        light_direction,
        radiance,
        visibility,
        n_dot_l,
        v_dot_h,
        distribution,
        geometric_visibility,
    );
}

fn multi_bounce_visibility(
    visibility: f32,
    albedo: vec3f,
) -> vec3f {
    let a =
        2.0404 * albedo -
        0.3324;

    let b =
        -4.7951 * albedo +
        0.6417;

    let c =
        2.7552 * albedo +
        0.6903;

    let visibility3 =
        vec3f(visibility);

    let bounce =
        fma(
            fma(
                visibility3,
                a,
                b,
            ),
            visibility3,
            c,
        ) * visibility;

    return max(
        visibility3,
        bounce,
    );
}

fn environment_radiance(
    direction: vec3f,
) -> vec3f {
    let up =
        clamp(
            direction.y,
            -1.0,
            1.0,
        );

    // Avoid evaluating both hemispheres.
    if up >= 0.0 {
        return mix(
            frame.lighting[1].rgb,
            frame.lighting[0].rgb,
            smoothstep(
                0.0,
                0.55,
                up,
            ),
        );
    }

    return mix(
        frame.lighting[1].rgb,
        frame.lighting[2].rgb,
        smoothstep(
            0.0,
            0.6,
            -up,
        ),
    );
}

fn environment_irradiance(
    normal: vec3f,
) -> vec3f {
    return (
        environment_radiance(normal) *
            0.9 +
        frame.lighting[1].rgb *
            0.35
    ) * frame.lighting[0].w;
}
