use crate::{RibbonMesh, RibbonParams};
use molgfx_core::SecondaryStructure;
use molgfx_math::Vec3;

#[test]
fn smooth_cartoon_sweeps_sheet_faces_while_classic_cartoon_connects_discrete_cells_with_cords() {
    use crate::cartoon::profiles::sample_profile;
    for (profile, shape) in [
        (
            crate::SplineProfile::Cartoon,
            molgfx_core::CartoonProfile::Square,
        ),
        (
            crate::SplineProfile::Rocket,
            molgfx_core::CartoonProfile::Elliptical,
        ),
    ] {
        let params = RibbonParams {
            profile,
            ..RibbonParams::default()
        };
        assert_eq!(
            sample_profile(params, SecondaryStructure::Strand, None),
            shape
        );
        assert_eq!(
            sample_profile(params, SecondaryStructure::Coil, None),
            molgfx_core::CartoonProfile::Elliptical
        );
    }
}

fn profile_diameters(vertices: &[crate::RibbonVertex]) -> (f32, f32) {
    let width = Vec3::from(vertices[0].position).distance(Vec3::from(vertices[8].position));
    let depth = Vec3::from(vertices[4].position).distance(Vec3::from(vertices[12].position));
    (width.max(depth), width.min(depth))
}

#[test]
fn every_helix_has_helix_profiles_and_only_beta_ladders_have_sheet_bodies() {
    for state in SecondaryStructure::ALL {
        let styles = [state; 3];
        let expected = if state.is_helix() {
            SecondaryStructure::AlphaHelix
        } else if state.is_strand() {
            SecondaryStructure::Strand
        } else {
            SecondaryStructure::Coil
        };
        assert_eq!(
            crate::cartoon::profiles::profile_scale(
                state,
                0.5,
                &styles,
                1,
                RibbonParams::default().aspect_ratio,
                RibbonParams::default().arrow_factor
            ),
            crate::cartoon::profiles::profile_scale(
                expected,
                0.5,
                &styles,
                1,
                RibbonParams::default().aspect_ratio,
                RibbonParams::default().arrow_factor
            )
        );
    }
}

#[test]
fn an_isolated_beta_bridge_keeps_the_connecting_cord_without_an_arrow() {
    use crate::cartoon::profiles::{arrow_shoulders, profile_scale};
    let params = RibbonParams::default();
    let styles = [SecondaryStructure::BetaBridge; 3];
    let scale = |state| {
        profile_scale(
            state,
            0.5,
            &styles,
            1,
            params.aspect_ratio,
            params.arrow_factor,
        )
    };
    assert_eq!(
        scale(SecondaryStructure::BetaBridge),
        scale(SecondaryStructure::Coil)
    );
    assert_ne!(
        scale(SecondaryStructure::BetaBridge),
        scale(SecondaryStructure::Strand)
    );
    let sample = |segment: u32, parameter: f32| molgfx_math::CurveSample {
        position: Vec3::ZERO,
        tangent: Vec3::Z,
        segment,
        parameter,
    };
    let mut samples = vec![sample(0, 1.0), sample(1, 0.0)];
    let before = samples.len();
    arrow_shoulders(
        &[
            SecondaryStructure::Coil,
            SecondaryStructure::BetaBridge,
            SecondaryStructure::Coil,
        ],
        &mut samples,
    );
    assert_eq!(samples.len(), before);
}

#[test]
fn a_straight_strand_places_its_authored_arrow_shoulder_at_the_last_strand_guide() {
    let mut mesh = RibbonMesh::default();
    let guides = [Vec3::ZERO, Vec3::X, Vec3::X * 2.0];
    mesh.generate_styled(
        &guides,
        &[1, 2, 3],
        &[
            SecondaryStructure::Strand,
            SecondaryStructure::Strand,
            SecondaryStructure::Coil,
        ],
        RibbonParams {
            arrow_factor: 2.0,
            ..RibbonParams::default()
        },
    )
    .expect("ribbon indices fit");
    let shoulder = mesh
        .vertices
        .iter()
        .filter(|vertex| (vertex.position[0] - 1.0).abs() < 1.0e-6)
        .map(|vertex| vertex.position[1].abs().max(vertex.position[2].abs()))
        .fold(0.0, f32::max);
    assert!(
        (shoulder - 2.8).abs() < 1.0e-5,
        "the shoulder follows the authored factor at the exact CA position"
    );
}

#[test]
fn a_cartoon_aspect_ratio_changes_depth_without_changing_helix_width() {
    let mut shallow = RibbonMesh::default();
    let mut deep = RibbonMesh::default();
    let guides = [Vec3::ZERO, Vec3::X];
    shallow
        .generate_styled(
            &guides,
            &[1, 2],
            &[SecondaryStructure::AlphaHelix; 2],
            RibbonParams {
                aspect_ratio: 10.0,
                ..RibbonParams::default()
            },
        )
        .expect("ribbon indices fit");
    deep.generate_styled(
        &guides,
        &[1, 2],
        &[SecondaryStructure::AlphaHelix; 2],
        RibbonParams {
            aspect_ratio: 5.0,
            ..RibbonParams::default()
        },
    )
    .expect("ribbon indices fit");
    let a = profile_diameters(&shallow.vertices);
    let b = profile_diameters(&deep.vertices);
    assert!((a.0 - b.0).abs() < 1.0e-5);
    assert!((a.1 * 2.0 - b.1).abs() < 1.0e-5);
}

#[test]
fn changing_cartoon_width_preserves_the_authored_cross_section_aspect_ratio() {
    for width in [0.6, 1.2, 2.4] {
        let mut mesh = RibbonMesh::default();
        mesh.generate_styled(
            &[Vec3::ZERO, Vec3::X],
            &[1, 2],
            &[SecondaryStructure::AlphaHelix; 2],
            RibbonParams {
                width,
                aspect_ratio: 7.0,
                ..RibbonParams::default()
            },
        )
        .expect("ribbon indices fit");
        let (wide, thin) = profile_diameters(&mesh.vertices);
        assert!((wide / thin - 7.0).abs() < 1.0e-5);
    }
}

#[test]
fn a_uniform_ribbon_keeps_its_width_and_depth_across_all_secondary_states() {
    let mut reference = None;
    for state in SecondaryStructure::ALL {
        let mut mesh = RibbonMesh::default();
        mesh.generate_styled(
            &[Vec3::ZERO, Vec3::X, Vec3::X * 2.0],
            &[1, 2, 3],
            &[state; 3],
            RibbonParams {
                profile: crate::SplineProfile::NewRibbons,
                width: 2.0,
                thickness: 0.4,
                arrow_factor: 0.0,
                ..RibbonParams::default()
            },
        )
        .expect("ribbon fits");
        let (width, depth) = profile_diameters(&mesh.vertices);
        assert!((width - 2.0).abs() < 1.0e-5);
        assert!((depth - 0.4).abs() < 1.0e-5);
        if let Some(vertices) = &reference {
            assert_eq!(&mesh.vertices, vertices);
        } else {
            reference = Some(mesh.vertices);
        }
    }
}
