use super::*;
use molgfx_core::ClipPlane;

fn slice_presentation(ramp: &str, domain: [f32; 2]) -> VolumePresentation {
    VolumePresentation::Slice {
        point: [1.0, -2.0, 3.0],
        normal: [1.0, 2.0, 3.0],
        ramp: ramp.into(),
        domain,
    }
}

#[test]
fn slices_preserve_every_catalog_anchor_in_both_directions() {
    let mut scene = molgfx_core::Scene::new();
    let volume = scene.add_volume(
        molgfx_core::ScalarVolume::new(
            [2; 3],
            molgfx_math::Mat4::IDENTITY,
            std::sync::Arc::from([0.0; 8]),
        )
        .unwrap(),
    );
    for name in crate::color::ramp_names() {
        for ramp_name in [name.to_owned(), format!("{name}_r")] {
            for domain in [[0.0, 1.0], [-3.25, 7.75], [0.1, 0.3]] {
                let colors = crate::color::ramp_colors(&ramp_name).unwrap();
                let native = slice_presentation(&ramp_name, domain).native().unwrap();
                let handle = scene.represent(volume, native).unwrap();
                let ramp = scene
                    .representation(handle)
                    .unwrap()
                    .volume
                    .slice_ramp
                    .expect("exact slice ramp");
                assert_eq!(ramp.colors().len(), colors.len(), "{ramp_name}");
                assert_eq!(ramp.values()[0].to_bits(), domain[0].to_bits());
                assert_eq!(ramp.values().last().unwrap().to_bits(), domain[1].to_bits());
                for (value, color) in ramp.values().iter().zip(colors) {
                    assert_eq!(
                        ramp.sample(*value, molgfx_math::Rgba8::WHITE),
                        color.native(),
                        "ramp {ramp_name}, domain {domain:?}, value {value}"
                    );
                }
            }
        }
    }
}

#[test]
fn slices_preserve_domain_plane_and_catalog_error_precedence() {
    let mut presentation = slice_presentation("unknown-ramp", [1.0, 1.0]);
    let VolumePresentation::Slice { normal, .. } = &mut presentation else {
        panic!("expected slice");
    };
    *normal = [0.0; 3];
    assert_eq!(
        presentation.native().unwrap_err().to_string(),
        invalid("slice domain must be finite and increasing").to_string()
    );
    let VolumePresentation::Slice { domain, .. } = &mut presentation else {
        panic!("expected slice");
    };
    *domain = [0.0, 1.0];
    let plane_error = ClipPlane::from_point_normal(Vec3::ZERO, Vec3::ZERO).unwrap_err();
    assert_eq!(
        presentation.native().unwrap_err().to_string(),
        Error::from(plane_error).to_string()
    );
    assert!(
        slice_presentation("unknown-ramp", [0.0, 1.0])
            .native()
            .unwrap_err()
            .to_string()
            .contains("unknown color ramp")
    );
}

#[test]
fn slices_reject_non_finite_overflowing_and_collapsed_anchor_domains() {
    for domain in [
        [f32::NAN, 1.0],
        [0.0, f32::INFINITY],
        [1.0, 0.0],
        [-f32::MAX, f32::MAX],
    ] {
        for name in crate::color::ramp_names() {
            assert!(
                slice_presentation(name, domain).native().is_err(),
                "ramp {name}, domain {domain:?}"
            );
        }
    }
    let collapsed = [1.0, f32::from_bits(1.0_f32.to_bits() + 1)];
    for name in ["blue_white_red", "viridis"] {
        assert!(slice_presentation(name, collapsed).native().is_err());
    }
    assert!(slice_presentation("grayscale", collapsed).native().is_ok());
}
