use super::*;

/// A regular tetrahedron: four points, four faces.
fn tetrahedron() -> Vec<Vec3> {
    vec![
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(1.0, -1.0, -1.0),
        Vec3::new(-1.0, 1.0, -1.0),
        Vec3::new(-1.0, -1.0, 1.0),
    ]
}

/// A regular octahedron: six points, eight faces.
fn octahedron() -> Vec<Vec3> {
    vec![Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z]
}

#[test]
fn a_tetrahedral_shell_yields_four_faces() {
    let Some((vertices, indices)) = coordination_hull(&tetrahedron()) else {
        panic!("four non-coplanar points bound a volume")
    };
    assert_eq!(vertices.len(), 4);
    assert_eq!(indices.len() / 3, 4, "a tetrahedron has four faces");
}

#[test]
fn an_octahedral_shell_yields_eight_faces() {
    let Some((_, indices)) = coordination_hull(&octahedron()) else {
        panic!("six points bound an octahedron")
    };
    assert_eq!(indices.len() / 3, 8, "an octahedron has eight faces");
}

#[test]
fn every_face_winds_away_from_the_interior() {
    let Some((vertices, indices)) = coordination_hull(&octahedron()) else {
        panic!("six points bound an octahedron")
    };
    let centre = vertices.iter().fold(Vec3::ZERO, |sum, p| sum + *p) / 6.0;
    for face in indices.as_chunks::<3>().0 {
        let (Some(&a), Some(&b), Some(&c)) = (
            vertices.get(face[0] as usize),
            vertices.get(face[1] as usize),
            vertices.get(face[2] as usize),
        ) else {
            panic!("indices address real vertices")
        };
        let normal = (b - a).cross(c - a);
        assert!(
            normal.dot(a - centre) > 0.0,
            "the face normal points outward"
        );
    }
}

#[test]
fn a_coplanar_or_undersized_shell_builds_nothing() {
    // Four points in a plane bound no volume.
    let flat = vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(1.0, 1.0, 0.0)];
    assert!(coordination_hull(&flat).is_none());
    assert!(coordination_hull(&[Vec3::ZERO, Vec3::X, Vec3::Y]).is_none());
}

#[test]
fn duplicated_neighbours_do_not_create_degenerate_faces() {
    let mut shell = tetrahedron();
    let Some(first) = shell.first().copied() else {
        panic!("the fixture has points")
    };
    shell.push(first);
    let Some((vertices, indices)) = coordination_hull(&shell) else {
        panic!("the duplicate is merged, leaving a tetrahedron")
    };
    assert_eq!(vertices.len(), 4);
    assert_eq!(indices.len() / 3, 4);
}

#[test]
fn an_oversized_shell_is_refused() {
    let mut shell = Vec::with_capacity(MAX_SHELL + 1);
    let mut step = 0.0f32;
    for _ in 0..=MAX_SHELL {
        shell.push(Vec3::new(step.sin(), step.cos(), step * 0.1));
        step += 1.0;
    }
    assert!(coordination_hull(&shell).is_none());
}
