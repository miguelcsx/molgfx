// Packed material payload decoding.
//
// One float carries the shading model and its parameters, so a gbuffer
// fragment costs no extra attachment to describe its material. Decoding runs
// once per shaded fragment rather than once per light, which is what keeps
// the cost of a many-light scene proportional to lights and not to lights
// times unpacking work.

const PI: f32 = 3.14159265;
const INV_PI: f32 = 0.318309886;

const DIELECTRIC_F0_LOW: vec3f = vec3f(0.02);
const DIELECTRIC_F0_HIGH: vec3f = vec3f(0.08);

const PRINCIPLED_PAYLOAD_BASE: f32 = 2.0;
const ANISOTROPIC_PAYLOAD_BASE: f32 = 4.0;
const DIFFUSION_PAYLOAD_BASE: f32 = 6.0;

const DIFFUSION_AMBIENT_FLOOR: f32 = 0.12;

const BRDF_EPSILON: f32 = 1.0e-5;
const TANGENT_EPSILON_SQ: f32 = 1.0e-12;
const HASH_UNIT_16: f32 = 1.0 / 65535.0;

struct LightingMaterial {
    albedo: vec3f,
    diffuse_albedo: vec3f,
    f0: vec3f,
    anisotropy: f32,
    diffusion: f32,
}

struct AnisotropicState {
    tangent: vec3f,
    bitangent: vec3f,

    alpha_t: f32,
    alpha_b: f32,

    inverse_alpha_t_sq: f32,
    inverse_alpha_b_sq: f32,

    alpha_product: f32,
    view_stretch: f32,
}

fn material_payload(material: vec4f) -> f32 {
    if material.z >= 0.5 && material.z < 1.5 {
        return PRINCIPLED_PAYLOAD_BASE +
            clamp(material.w, 0.0, 1.0);
    }

    if material.z >= 2.5 && material.z < 3.5 {
        return DIFFUSION_PAYLOAD_BASE +
            clamp(material.w, 0.0, 1.0);
    }

    return clamp(material.y, 0.0, 1.0);
}

fn ribbon_material_payload(material: vec4f) -> f32 {
    if material.z >= 1.5 && material.z < 2.5 {
        return ANISOTROPIC_PAYLOAD_BASE +
            clamp(material.w, 0.0, 1.0);
    }

    return material_payload(material);
}

fn payload_metallic(payload: f32) -> f32 {
    if payload < PRINCIPLED_PAYLOAD_BASE ||
        payload >= ANISOTROPIC_PAYLOAD_BASE {
        return 0.0;
    }

    return clamp(
        payload - PRINCIPLED_PAYLOAD_BASE,
        0.0,
        1.0,
    );
}

fn payload_anisotropy(payload: f32) -> f32 {
    if payload < ANISOTROPIC_PAYLOAD_BASE ||
        payload >= DIFFUSION_PAYLOAD_BASE {
        return 0.0;
    }

    return clamp(
        payload - ANISOTROPIC_PAYLOAD_BASE,
        0.0,
        1.0,
    );
}

fn payload_diffusion(payload: f32) -> f32 {
    if payload < DIFFUSION_PAYLOAD_BASE {
        return 0.0;
    }

    return clamp(
        payload - DIFFUSION_PAYLOAD_BASE,
        0.0,
        1.0,
    );
}

fn dielectric_f0(specular_strength: f32) -> vec3f {
    return mix(
        DIELECTRIC_F0_LOW,
        DIELECTRIC_F0_HIGH,
        clamp(specular_strength, 0.0, 1.0),
    );
}

fn payload_f0(
    albedo: vec3f,
    payload: f32,
) -> vec3f {
    if payload < PRINCIPLED_PAYLOAD_BASE {
        return dielectric_f0(payload);
    }

    return mix(
        vec3f(0.04),
        albedo,
        payload_metallic(payload),
    );
}

/// Decodes the packed material payload once for the complete lighting pass.
fn lighting_material(
    albedo: vec3f,
    payload: f32,
) -> LightingMaterial {
    var metallic = 0.0;
    var anisotropy = 0.0;
    var diffusion = 0.0;
    var dielectric = vec3f(0.04);

    if payload >= DIFFUSION_PAYLOAD_BASE {
        diffusion = clamp(
            payload - DIFFUSION_PAYLOAD_BASE,
            0.0,
            1.0,
        );
    } else if payload >= ANISOTROPIC_PAYLOAD_BASE {
        anisotropy = clamp(
            payload - ANISOTROPIC_PAYLOAD_BASE,
            0.0,
            1.0,
        );
    } else if payload >= PRINCIPLED_PAYLOAD_BASE {
        metallic = clamp(
            payload - PRINCIPLED_PAYLOAD_BASE,
            0.0,
            1.0,
        );
    } else {
        dielectric =
            dielectric_f0(payload);
    }

    return LightingMaterial(
        albedo,
        albedo * (1.0 - metallic),
        mix(dielectric, albedo, metallic),
        anisotropy,
        diffusion,
    );
}
