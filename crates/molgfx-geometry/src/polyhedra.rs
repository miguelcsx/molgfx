//! Coordination polyhedra: the convex hull of the neighbours around a centre.
//!
//! Crystallographers read coordination by its shape — a tetrahedron, an
//! octahedron, a trigonal bipyramid — long before they read distances. Drawing
//! the hull of the coordinating atoms shows that shape directly.
//!
//! The hull is computed by testing every candidate face against the remaining
//! points. That is `O(n⁴)`, which would be indefensible for a general mesh and
//! is the right choice here: a coordination shell holds a handful of atoms, the
//! constant factor is tiny, and the method has no degenerate configurations to
//! special-case. `MAX_SHELL` keeps a caller from handing it a whole protein.

use molgfx_math::Vec3;

#[cfg(test)]
#[path = "polyhedra_tests.rs"]
mod tests;

/// Largest coordination shell accepted. Beyond this the quartic search stops
/// being negligible, and a shell that large is not a coordination polyhedron.
pub const MAX_SHELL: usize = 24;

/// Points closer than this are treated as one, so a duplicated neighbour cannot
/// produce a degenerate face.
const MERGE_DISTANCE: f32 = 1.0e-3;
/// How far off a plane a point may sit and still count as lying on it.
const PLANE_TOLERANCE: f32 = 1.0e-4;

/// Builds the convex hull of `shell` as outward-facing triangles.
///
/// Returns the hull vertices and their triangle indices, or `None` when the
/// shell is too small, too large, or degenerate — fewer than four points that
/// are not all coplanar cannot bound a volume, and inventing one would draw a
/// coordination that is not there.
#[must_use]
pub fn coordination_hull(shell: &[Vec3]) -> Option<(Vec<Vec3>, Vec<u32>)> {
    if shell.len() < 4 || shell.len() > MAX_SHELL {
        return None;
    }
    let points = merge_duplicates(shell);
    if points.len() < 4 {
        return None;
    }
    if !spans_volume(&points) {
        // Every point on one plane: the triples would all pass the side test
        // and produce zero-volume faces. A flat shell is not a polyhedron.
        return None;
    }
    // Accumulate the count as a float alongside the sum: the shell is bounded
    // by MAX_SHELL, so this avoids an index-to-float cast entirely.
    let (sum, count) = points
        .iter()
        .fold((Vec3::ZERO, 0.0f32), |(sum, count), point| {
            (sum + *point, count + 1.0)
        });
    let centre = sum / count.max(1.0);

    let mut indices = Vec::new();
    for first in 0..points.len() {
        for second in (first + 1)..points.len() {
            for third in (second + 1)..points.len() {
                let Some(face) = hull_face(&points, [first, second, third], centre) else {
                    continue;
                };
                indices.extend_from_slice(&face);
            }
        }
    }
    if indices.is_empty() {
        return None;
    }
    Some((points, indices))
}

/// Accepts one triple as a hull face when every other point lies on a single
/// side of its plane, and orders it to face away from the centre.
fn hull_face(points: &[Vec3], triple: [usize; 3], centre: Vec3) -> Option<[u32; 3]> {
    let [first, second, third] = triple;
    let (a, b, c) = (
        *points.get(first)?,
        *points.get(second)?,
        *points.get(third)?,
    );
    let normal = (b - a).cross(c - a).try_normalize()?;
    let offset = normal.dot(a);

    let mut above = false;
    let mut below = false;
    for (index, point) in points.iter().enumerate() {
        if index == first || index == second || index == third {
            continue;
        }
        let distance = normal.dot(*point) - offset;
        if distance > PLANE_TOLERANCE {
            above = true;
        } else if distance < -PLANE_TOLERANCE {
            below = true;
        }
        if above && below {
            // Points on both sides: the plane cuts the body, so this is not a
            // hull face.
            return None;
        }
    }

    // Wind the triangle so its normal points away from the interior.
    let outward = if normal.dot(a - centre) >= 0.0 {
        [first, second, third]
    } else {
        [first, third, second]
    };
    Some([
        u32::try_from(outward[0]).ok()?,
        u32::try_from(outward[1]).ok()?,
        u32::try_from(outward[2]).ok()?,
    ])
}

/// True when the points are not all coplanar, i.e. some tetrahedron formed
/// from them has non-zero signed volume.
fn spans_volume(points: &[Vec3]) -> bool {
    let Some(&origin) = points.first() else {
        return false;
    };
    let mut first_axis = None;
    let mut plane_normal = None;
    for point in points.iter().skip(1) {
        let edge = *point - origin;
        let Some(edge) = edge.try_normalize() else {
            continue;
        };
        match (first_axis, plane_normal) {
            (None, _) => first_axis = Some(edge),
            (Some(axis), None) => {
                if let Some(normal) = axis.cross(edge).try_normalize() {
                    plane_normal = Some(normal);
                }
            }
            (Some(_), Some(normal)) => {
                if normal.dot(edge).abs() > PLANE_TOLERANCE {
                    return true;
                }
            }
        }
    }
    false
}

fn merge_duplicates(shell: &[Vec3]) -> Vec<Vec3> {
    let mut points: Vec<Vec3> = Vec::with_capacity(shell.len());
    for candidate in shell {
        if !candidate.is_finite() {
            continue;
        }
        if points
            .iter()
            .any(|kept| kept.distance_squared(*candidate) <= MERGE_DISTANCE * MERGE_DISTANCE)
        {
            continue;
        }
        points.push(*candidate);
    }
    points
}
