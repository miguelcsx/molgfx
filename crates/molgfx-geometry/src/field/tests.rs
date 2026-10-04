//! Analytic boundary, topology and identity checks.

use super::{FieldError, extract_isosurface, extract_label_surfaces};
use molgfx_core::{ScalarVolume, SegmentedVolume};
use molgfx_math::{Mat3, Mat4, Quat, Vec3};
use std::collections::BTreeMap;
use std::sync::Arc;

#[test]
fn an_affine_plane_has_shared_vertices_and_inverse_transpose_normals() {
    let transform = Mat4::from_cols_array(&[
        2.0, 0.0, 0.0, 0.0, 1.0, 3.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 10.0, -3.0, 5.0, 1.0,
    ]);
    let values = (0_u8..27)
        .map(|index| f32::from(index % 3))
        .collect::<Vec<_>>();
    let volume = ScalarVolume::new([3; 3], transform, Arc::from(values)).unwrap();
    let mesh = extract_isosurface(&volume, 0.5).unwrap();
    assert!(!mesh.triangles.is_empty());
    assert!(mesh.vertices.len() < mesh.triangles.len() * 3);
    let expected = (Mat3::from_mat4(transform).inverse().transpose() * -Vec3::X).normalize();
    for vertex in &mesh.vertices {
        let local = transform
            .inverse()
            .transform_point3(Vec3::from_array(vertex.position));
        assert!((local.x - 0.5).abs() < 1.0e-5);
        assert!(Vec3::from_array(vertex.normal).distance(expected) < 1.0e-5);
        assert_eq!(vertex.label, 0);
    }
    assert_eq!(mesh, extract_isosurface(&volume, 0.5).unwrap());
}

#[test]
fn constant_and_absent_scalar_crossings_are_valid_empty_geometry() {
    let volume = ScalarVolume::new([2; 3], Mat4::IDENTITY, Arc::from([1.0; 8])).unwrap();
    for level in [0.0, 1.0, 2.0] {
        assert!(
            extract_isosurface(&volume, level)
                .unwrap()
                .triangles
                .is_empty()
        );
    }
    assert_eq!(
        extract_isosurface(&volume, f32::NAN),
        Err(FieldError::InvalidLevel)
    );
}

#[test]
fn a_single_large_integer_label_has_a_closed_boundary_without_aliasing() {
    let label = (1 << 24) + 1;
    let mut labels = [0; 27];
    labels[13] = label;
    let volume = SegmentedVolume::new([3; 3], Mat4::IDENTITY, Arc::from(labels)).unwrap();
    let mesh = extract_label_surfaces(&volume).unwrap();
    assert!(!mesh.triangles.is_empty());
    assert!(mesh.vertices.iter().any(|vertex| vertex.label == label));
    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| vertex.label == 0 || vertex.label == label)
    );
    let mut edges = BTreeMap::new();
    for triangle in &mesh.triangles {
        for [a, b] in [
            [triangle[0], triangle[1]],
            [triangle[1], triangle[2]],
            [triangle[2], triangle[0]],
        ] {
            *edges.entry([a.min(b), a.max(b)]).or_insert(0) += 1;
        }
    }
    assert!(edges.values().all(|count| *count == 2));
    for vertex in &mesh.vertices {
        if vertex.label != label {
            continue;
        }
        let offset = Vec3::from_array(vertex.position) - Vec3::ONE;
        assert!(offset.dot(Vec3::from_array(vertex.normal)) > 0.0);
    }
}

#[test]
fn labels_at_source_edges_close_and_negative_scale_preserves_outward_winding() {
    let transform = Mat4::from_scale_rotation_translation(
        Vec3::new(-2.0, 3.0, 4.0),
        Quat::IDENTITY,
        Vec3::ZERO,
    );
    let mut labels = [0; 8];
    labels[0] = u32::MAX;
    let volume = SegmentedVolume::new([2; 3], transform, Arc::from(labels)).unwrap();
    let mesh = extract_label_surfaces(&volume).unwrap();
    assert!(mesh.vertices.iter().any(|vertex| vertex.label == u32::MAX));
    for triangle in mesh.triangles {
        if mesh.vertices[usize::try_from(triangle[0]).unwrap()].label != u32::MAX {
            continue;
        }
        let points = triangle
            .map(|index| Vec3::from_array(mesh.vertices[usize::try_from(index).unwrap()].position));
        let centre = (points[0] + points[1] + points[2]) / 3.0;
        let normal = (points[1] - points[0]).cross(points[2] - points[0]);
        assert!(normal.dot(centre) > 0.0);
    }
}

#[test]
fn categorical_reconstruction_preserves_every_nodal_membership() {
    let labels = (0..27)
        .map(|index| [0, 1, u32::MAX][index % 3])
        .collect::<Vec<_>>();
    let volume = SegmentedVolume::new([3; 3], Mat4::IDENTITY, Arc::from(labels)).unwrap();
    let grid = super::grid::Grid::labels(&volume).unwrap();
    for z in -1..4 {
        for y in -1..4 {
            for x in -1..4 {
                let coordinate = [x, y, z];
                let actual = grid.label(coordinate).unwrap();
                for label in [0, 1, u32::MAX] {
                    assert_eq!(
                        grid.value(coordinate, label).unwrap() >= 0.5,
                        actual == Some(label)
                    );
                }
            }
        }
    }
}
