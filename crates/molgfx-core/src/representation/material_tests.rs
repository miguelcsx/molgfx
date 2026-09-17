use super::*;

#[test]
fn molecular_material_response_is_finite_and_bounded() {
    let malformed = Material {
        opacity: 1.0,
        roughness: f32::NAN,
        specular: f32::INFINITY,
        model: MaterialModel::Molecular,
    };
    assert!(
        (malformed.perceptual_roughness() - Material::default().roughness).abs() < f32::EPSILON
    );
    assert!((malformed.specular_strength() - Material::default().specular).abs() < f32::EPSILON);

    let extreme = Material {
        opacity: 1.0,
        roughness: -4.0,
        specular: 7.0,
        model: MaterialModel::Molecular,
    };
    assert!((extreme.perceptual_roughness() - 0.05).abs() < f32::EPSILON);
    assert!((extreme.specular_strength() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn principled_materials_clamp_metalness_without_changing_the_default() {
    for (material, expected) in [
        (Material::default(), [0.0, 0.0]),
        (Material::principled(0.75), [1.0, 0.75]),
        (Material::principled(f32::NAN), [1.0, 0.0]),
        (Material::principled(4.0), [1.0, 1.0]),
    ] {
        assert_eq!(
            material.model_lanes().map(f32::to_bits),
            expected.map(f32::to_bits)
        );
    }
}

#[test]
fn anisotropic_ribbon_materials_clamp_their_tangent_response() {
    for (strength, expected) in [(0.72, 0.72), (f32::NAN, 0.0), (4.0, 1.0)] {
        assert_eq!(
            Material::anisotropic_ribbon(strength)
                .model_lanes()
                .map(f32::to_bits),
            [2.0, expected].map(f32::to_bits)
        );
    }
}

#[test]
fn diffusion_materials_clamp_their_art_directed_response() {
    for (strength, expected) in [(0.72, 0.72), (f32::NAN, 0.0), (4.0, 1.0)] {
        assert_eq!(
            Material::diffusion(strength)
                .model_lanes()
                .map(f32::to_bits),
            [3.0, expected].map(f32::to_bits)
        );
    }
}
