use super::cross_section;
use molgfx_core::CartoonProfile;

#[test]
fn rounded_profiles_join_flat_faces_to_true_semicircular_ends() {
    let width = 2.0;
    let depth = 0.5;
    for side in [3, 4, 5] {
        let (_, y, normal) = cross_section(CartoonProfile::Rounded, side, width, depth);
        assert!((y - 1.0).abs() < 1.0e-6);
        assert!(normal[0].abs() < 1.0e-6);
        assert!((normal[1] - 1.0).abs() < 1.0e-6);
    }
    for side in [0, 1, 2, 6, 7, 8, 9, 10, 14, 15] {
        let (x, y, _) = cross_section(CartoonProfile::Rounded, side, width, depth);
        let x = x * width;
        let y = y * depth;
        let centre = (width - depth) * x.signum();
        assert!(((x - centre).powi(2) + y.powi(2) - depth.powi(2)).abs() < 1.0e-6);
    }
}

#[test]
fn square_profiles_have_separate_planar_face_normals() {
    for (face, normal) in [[0.0, 1.0], [-1.0, 0.0], [0.0, -1.0], [1.0, 0.0]]
        .into_iter()
        .enumerate()
    {
        for side in face * 4..face * 4 + 4 {
            let (x, y, actual) = cross_section(CartoonProfile::Square, side, 2.0, 0.5);
            assert!((actual[0] - normal[0]).abs() < 1.0e-6);
            assert!((actual[1] - normal[1]).abs() < 1.0e-6);
            assert!((x * normal[0] + y * normal[1] - 1.0).abs() < 1.0e-6);
        }
    }
}

#[test]
fn every_profile_preserves_width_and_depth_in_either_major_axis() {
    for profile in [
        CartoonProfile::Elliptical,
        CartoonProfile::Rounded,
        CartoonProfile::Square,
    ] {
        for (width, depth) in [(2.0, 0.5), (0.5, 2.0)] {
            let mut extent = [0.0_f32; 2];
            for side in 0..16 {
                let (x, y, normal) = cross_section(profile, side, width, depth);
                extent[0] = extent[0].max(x.abs());
                extent[1] = extent[1].max(y.abs());
                assert!(x.is_finite() && y.is_finite());
                assert!(normal.into_iter().all(f32::is_finite));
            }
            assert!((extent[0] - 1.0).abs() < 1.0e-6);
            assert!((extent[1] - 1.0).abs() < 1.0e-6);
        }
    }
}

#[test]
fn swept_shell_triangles_face_the_same_way_as_their_outward_vertex_normals() {
    use crate::{RibbonMesh, RibbonParams, SplineProfile};
    use molgfx_math::Vec3;
    for (profile, shell_indices) in [(SplineProfile::Cartoon, 96), (SplineProfile::Twister, 24)] {
        let mut mesh = RibbonMesh::default();
        mesh.generate(
            &[Vec3::ZERO, Vec3::X],
            &[1, 2],
            RibbonParams {
                profile,
                ..RibbonParams::default()
            },
        )
        .expect("ribbon indices fit");
        for triangle in mesh.indices[..shell_indices].as_chunks::<3>().0 {
            let a = mesh.vertices[triangle[0] as usize];
            let b = mesh.vertices[triangle[1] as usize];
            let c = mesh.vertices[triangle[2] as usize];
            let normal = (Vec3::from(b.position) - Vec3::from(a.position))
                .cross(Vec3::from(c.position) - Vec3::from(a.position));
            let outward = Vec3::from(a.normal) + Vec3::from(b.normal) + Vec3::from(c.normal);
            assert!(
                normal.dot(outward) > 0.0,
                "shell face points inward for {profile:?}"
            );
        }
    }
}

#[test]
fn changing_the_helix_profile_changes_shape_without_moving_source_anchors() {
    use crate::{RibbonMesh, RibbonParams};
    use molgfx_core::SecondaryStructure;
    use molgfx_math::Vec3;
    let mut previous = None;
    for helix_profile in [
        CartoonProfile::Elliptical,
        CartoonProfile::Rounded,
        CartoonProfile::Square,
    ] {
        let mut mesh = RibbonMesh::default();
        mesh.generate_styled(
            &[Vec3::ZERO, Vec3::X],
            &[7, 8],
            &[SecondaryStructure::AlphaHelix; 2],
            RibbonParams {
                helix_profile,
                ..RibbonParams::default()
            },
        )
        .expect("ribbon indices fit");
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| vertex.entity_id == 7 && Vec3::from(vertex.normal).is_normalized())
        );
        let positions: Vec<_> = mesh.vertices.iter().map(|vertex| vertex.position).collect();
        if let Some(previous) = previous {
            assert_ne!(positions, previous);
        }
        previous = Some(positions);
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let a = mesh.vertices[triangle[0] as usize];
            let b = mesh.vertices[triangle[1] as usize];
            let c = mesh.vertices[triangle[2] as usize];
            let normal = (Vec3::from(b.position) - Vec3::from(a.position))
                .cross(Vec3::from(c.position) - Vec3::from(a.position));
            if normal.length_squared() > 1.0e-12 {
                assert!(
                    normal.dot(Vec3::from(a.normal) + Vec3::from(b.normal) + Vec3::from(c.normal))
                        > 0.0
                );
            }
        }
    }
}

#[test]
fn rounded_cross_sections_keep_their_winding_when_the_depth_is_larger_than_width() {
    for (width, depth) in [(2.0, 0.5), (0.5, 2.0)] {
        for side in 0..16 {
            let (ax, ay, _) = cross_section(CartoonProfile::Rounded, side, width, depth);
            let (bx, by, _) = cross_section(CartoonProfile::Rounded, (side + 1) % 16, width, depth);
            assert!(ax * by - ay * bx > 0.0);
        }
    }
}

#[test]
fn nucleic_profiles_use_sugar_guides_and_preserve_the_authored_aspect_ratio() {
    use crate::{RibbonMesh, RibbonParams};
    use molgfx_core::AtomSelection;
    use molgfx_math::Vec3;
    let source = b"ATOM      1  C4'  DC A   1       0.000   0.000   0.000  1.00 10.00           C\nATOM      2  C4'  DG A   2       4.000   0.000   0.000  1.00 10.00           C\nEND\n";
    let (structure, _) = molframe::read_bytes(
        source.to_vec(),
        Some("sugar.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source parses");
    let mut previous = None;
    for nucleic_profile in [
        CartoonProfile::Elliptical,
        CartoonProfile::Rounded,
        CartoonProfile::Square,
    ] {
        let mut mesh = RibbonMesh::default();
        mesh.generate_structure(
            &structure,
            &AtomSelection::All,
            &[],
            8.0,
            RibbonParams {
                nucleic_profile,
                width: 2.0,
                aspect_ratio: 4.0,
                ..RibbonParams::default()
            },
        )
        .expect("source guides bind");
        assert!(!mesh.vertices.is_empty());
        let positions: Vec<_> = mesh.vertices[..16]
            .iter()
            .map(|vertex| Vec3::from(vertex.position))
            .collect();
        let extent = positions
            .iter()
            .fold(Vec3::ZERO, |extent, position| extent.max(position.abs()));
        let (width, depth) = (extent.y.max(extent.z), extent.y.min(extent.z));
        assert!((width - 1.0).abs() < 1.0e-6);
        assert!((width / depth - 4.0).abs() < 1.0e-6);
        if let Some(previous) = previous {
            assert_ne!(positions, previous);
        }
        previous = Some(positions);
    }
}
