// GGX distribution and visibility terms, isotropic and anisotropic.
//
// The two families stay separate functions so an isotropic surface never
// evaluates an anisotropic term or builds a tangent frame. Fixed integer
// powers are expanded into multiplications, because a generic pow() on the
// hottest path in the renderer costs far more than the multiply it replaces.

fn varied_roughness(
    entity_id: u32,
    roughness: f32,
) -> f32 {
    var hash =
        entity_id * 747796405u +
        2891336453u;

    hash =
        (
            (
                hash >>
                ((hash >> 28u) + 4u)
            ) ^
            hash
        ) * 277803737u;

    hash =
        (hash >> 22u) ^
        hash;

    let offset =
        f32(hash & 0xffffu) *
        HASH_UNIT_16 -
        0.5;

    return clamp(
        roughness +
            offset * 0.12,
        0.05,
        0.92,
    );
}

/// Schlick Fresnel without generic pow(x, 5).
fn fresnel_schlick(
    cosine: f32,
    f0: vec3f,
) -> vec3f {
    let x =
        1.0 -
        clamp(
            cosine,
            0.0,
            1.0,
        );

    let x2 =
        x * x;

    let x5 =
        x2 * x2 * x;

    return fma(
        vec3f(x5),
        vec3f(1.0) - f0,
        f0,
    );
}

/// Isotropic GGX distribution.
///
/// alpha_squared is alpha² where alpha = roughness².
fn distribution_ggx(
    n_dot_h: f32,
    alpha_squared: f32,
) -> f32 {
    let denominator =
        fma(
            n_dot_h * n_dot_h,
            alpha_squared - 1.0,
            1.0,
        );

    return alpha_squared /
        max(
            PI *
                denominator *
                denominator,
            BRDF_EPSILON,
        );
}

fn visibility_smith_ggx_correlated(
    n_dot_v: f32,
    n_dot_l: f32,
    alpha_squared: f32,
) -> f32 {
    let one_minus_alpha =
        1.0 -
        alpha_squared;

    let lambda_v =
        n_dot_l *
        sqrt(
            fma(
                n_dot_v * n_dot_v,
                one_minus_alpha,
                alpha_squared,
            )
        );

    let lambda_l =
        n_dot_v *
        sqrt(
            fma(
                n_dot_l * n_dot_l,
                one_minus_alpha,
                alpha_squared,
            )
        );

    return 0.5 /
        max(
            lambda_v + lambda_l,
            BRDF_EPSILON,
        );
}

/// Builds anisotropic state once per fragment instead of once per light.
fn anisotropic_state(
    normal: vec3f,
    input_tangent: vec3f,
    view_direction: vec3f,
    n_dot_v: f32,
    alpha: f32,
    anisotropy: f32,
) -> AnisotropicState {
    var tangent =
        input_tangent;

    if dot(tangent, tangent) <=
        TANGENT_EPSILON_SQ {
        tangent =
            canonical_tangent(normal);
    }

    let bitangent =
        normalize(
            cross(
                normal,
                tangent,
            )
        );

    let aspect =
        sqrt(
            max(
                1.0 -
                    anisotropy * 0.82,
                0.18,
            )
        );

    let alpha_t =
        max(
            alpha / aspect,
            0.02,
        );

    let alpha_b =
        max(
            alpha * aspect,
            0.02,
        );

    let inverse_alpha_t =
        1.0 / alpha_t;

    let inverse_alpha_b =
        1.0 / alpha_b;

    let view_stretch =
        length(
            vec3f(
                alpha_t *
                    dot(
                        tangent,
                        view_direction,
                    ),

                alpha_b *
                    dot(
                        bitangent,
                        view_direction,
                    ),

                n_dot_v,
            )
        );

    return AnisotropicState(
        tangent,
        bitangent,

        alpha_t,
        alpha_b,

        inverse_alpha_t *
            inverse_alpha_t,

        inverse_alpha_b *
            inverse_alpha_b,

        alpha_t * alpha_b,
        view_stretch,
    );
}

fn distribution_ggx_anisotropic(
    n_dot_h: f32,
    t_dot_h: f32,
    b_dot_h: f32,
    state: AnisotropicState,
) -> f32 {
    let denominator =
        t_dot_h *
            t_dot_h *
            state.inverse_alpha_t_sq +
        b_dot_h *
            b_dot_h *
            state.inverse_alpha_b_sq +
        n_dot_h *
            n_dot_h;

    return 1.0 /
        max(
            PI *
                state.alpha_product *
                denominator *
                denominator,
            BRDF_EPSILON,
        );
}

fn visibility_smith_ggx_anisotropic(
    n_dot_v: f32,
    n_dot_l: f32,
    t_dot_l: f32,
    b_dot_l: f32,
    state: AnisotropicState,
) -> f32 {
    let light_stretch =
        length(
            vec3f(
                state.alpha_t *
                    t_dot_l,

                state.alpha_b *
                    b_dot_l,

                n_dot_l,
            )
        );

    return 0.5 /
        max(
            n_dot_l *
                state.view_stretch +
            n_dot_v *
                light_stretch,
            BRDF_EPSILON,
        );
}
