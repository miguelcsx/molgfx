use crate::{RibbonMesh, RibbonParams};
use molgfx_core::SecondaryStructure;
use molgfx_math::Vec3;

fn profile_diameters(vertices: &[crate::RibbonVertex]) -> (f32, f32) {
    let width = Vec3::from(vertices[0].position).distance(Vec3::from(vertices[8].position));
    let depth = Vec3::from(vertices[4].position).distance(Vec3::from(vertices[12].position));
    (width.max(depth), width.min(depth))
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
    );
    let shoulder = mesh
        .vertices
        .iter()
        .filter(|vertex| (vertex.position[0] - 1.0).abs() < 1.0e-6)
        .map(|vertex| (Vec3::from(vertex.position) - Vec3::X).length())
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
    shallow.generate_styled(
        &guides,
        &[1, 2],
        &[SecondaryStructure::AlphaHelix; 2],
        RibbonParams {
            aspect_ratio: 10.0,
            ..RibbonParams::default()
        },
    );
    deep.generate_styled(
        &guides,
        &[1, 2],
        &[SecondaryStructure::AlphaHelix; 2],
        RibbonParams {
            aspect_ratio: 5.0,
            ..RibbonParams::default()
        },
    );
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
        );
        let (wide, thin) = profile_diameters(&mesh.vertices);
        assert!((wide / thin - 7.0).abs() < 1.0e-5);
    }
}
